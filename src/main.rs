#![warn(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use axum::{
    Router,
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, Utf8Bytes, WebSocket},
    },
    response::IntoResponse,
    routing::{any, get},
};
use battlesnakes_shared::{ClientMessage, ServerMessage};
use futures_util::{SinkExt as _, StreamExt as _};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::{
    sync::{mpsc, oneshot},
    time::interval,
};

enum ClientUpdate {
    Join(
        SocketAddr,
        String,
        oneshot::Sender<mpsc::UnboundedReceiver<ServerMessage>>,
    ),
    Left(SocketAddr, String),
}

async fn game_loop(
    mut new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    mut msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
) {
    struct ClientInfo {
        name: String,
        msg: mpsc::UnboundedSender<ServerMessage>,
        msg_count: usize,
    }

    let mut interval = interval(Duration::from_secs(1));
    let mut clients: HashMap<SocketAddr, ClientInfo> = HashMap::new();
    loop {
        let (addr, msg) = tokio::select! {
            _ = interval.tick() => {
                info!("tick");
                for cli in clients.values_mut() {
                    _ = cli.msg.send(ServerMessage::Tick);
                    cli.msg_count = 0;
                }
                continue;
            }
            msg = new_clients.recv() => {
                let Some(msg) = msg else { break };
                match msg {
                    ClientUpdate::Join(addr, name, pipe) => {
                        let (msg_send, msg_recv) = mpsc::unbounded_channel();
                        trace!("got new client: {addr} | {name}");
                        _ = pipe.send(msg_recv);
                        clients.insert(addr, ClientInfo {
                            name,
                            msg: msg_send,
                            msg_count: 0
                        });
                    },
                    ClientUpdate::Left(addr, reason) => {
                        info!("{addr}: left, {reason}");
                        clients.remove(&addr);
                    }
                }
                continue;
            }
            msg = msgs.recv() => {
                let Some((addr, msg)) = msg else { break };
                let Some(cl) = clients.get_mut(&addr) else { continue };
                cl.msg_count += 1;
                if cl.msg_count == 2 || cl.msg_count.is_multiple_of(10) {
                    warn!("{addr}: sent too many messages: {}", cl.msg_count);
                }
                if cl.msg_count != 1 {
                    continue;
                }
                (addr, msg)
            }
        };
        info!("{addr}: {msg:?}");
    }
}

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

#[derive(Clone)]
struct ServerState {
    client_update: mpsc::UnboundedSender<ClientUpdate>,
    msg_send: mpsc::UnboundedSender<(SocketAddr, ClientMessage)>,
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter("none,battlesnakes=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed set up tracing");

    let (client_update, client_update_recv) = mpsc::unbounded_channel();
    let (msg_send, msg_recv) = mpsc::unbounded_channel();
    let app = Router::new()
        .route("/", get(async || "Hello, World!"))
        .route("/ws", any(ws_handler))
        .with_state(ServerState {
            client_update,
            msg_send,
        });

    tokio::spawn(game_loop(client_update_recv, msg_recv));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000")
        .await
        .expect("failed to bind address");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server crash");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    // user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    ServerState {
        client_update,
        msg_send,
    }: ServerState,
) {
    let Some(ClientMessage::SetName(name)) = socket
        .recv()
        .await
        .and_then(|s| s.ok())
        .and_then(|s| match s {
            Message::Text(bytes) => Some(bytes),
            _ => None,
        })
        .and_then(|s| serde_json::from_slice::<ClientMessage>(s.as_bytes()).ok())
    else {
        error!("client {who} did not send a proper handshake");
        return;
    };
    let (pipe_send, pipe_recv) = oneshot::channel();
    if client_update
        .send(ClientUpdate::Join(who, name, pipe_send))
        .is_err()
    {
        return;
    }

    let mut pipe = pipe_recv.await.expect("failed getting pipe from server");

    let (mut sender, mut receiver) = socket.split();

    tokio::spawn(async move {
        while let Some(msg) = pipe.recv().await {
            let json = serde_json::to_string(&msg).expect("failed encoding message");
            if let Err(e) = sender.send(Message::Text(Utf8Bytes::from(json))).await {
                client_update
                    .send(ClientUpdate::Left(who, format!("{e}")))
                    .expect("game server is dead");
                break;
            }
        }
    });

    tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(bytes) => {
                    match serde_json::from_slice::<ClientMessage>(bytes.as_bytes()) {
                        Ok(msg) => {
                            if msg_send.send((who, msg)).is_err() {
                                break;
                            };
                        }
                        Err(e) => {
                            warn!("invalid message from {who} (len = {}): {e:?}", bytes.len())
                        }
                    }
                }
                Message::Close(_close_frame) => break,
                x => error!("unsupported message from {who}: {x:?}"),
            }
        }
        info!("{who}: closed recv loop");
    });
}
