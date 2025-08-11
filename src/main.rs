#![warn(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]
#![feature(try_blocks)]

use axum::{
    Router,
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, Utf8Bytes},
    },
    response::IntoResponse,
    routing::{any, get},
};
use futures_util::{SinkExt as _, StreamExt as _};
use schemars::schema_for;
use snakes_shared::{ClientMessage, ServerMessage, WatchUpdate};
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot};

use crate::{frontend::index, game::Game};
mod frontend;
mod game;

enum ClientUpdate {
    Join(
        SocketAddr,
        String,
        oneshot::Sender<mpsc::UnboundedReceiver<ServerMessage>>,
    ),
    Watcher(SocketAddr, mpsc::UnboundedSender<WatchUpdate>),
    Left(SocketAddr, String),
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
    #[allow(clippy::print_stdout, clippy::unwrap_used)]
    {
        println!(
            "Client JSON schema:\n{}",
            serde_json::to_string_pretty(&schema_for!(ServerMessage)).unwrap()
        );
    }

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter("none,battlesnakes=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed set up tracing");

    let (client_update, client_update_recv) = mpsc::unbounded_channel();
    let (msg_send, msg_recv) = mpsc::unbounded_channel();
    let app = Router::new()
        .route("/", get(index))
        .route("/watch", any(watch_ws_handler))
        .route("/ws", any(game_ws_handler))
        .with_state(ServerState {
            client_update,
            msg_send,
        });

    tokio::spawn(async move {
        let mut game = Game::new(msg_recv, client_update_recv);
        loop {
            let Err(e) = game.tick().await else {
                continue;
            };
            error!("game loop: {e}");
        }
    });

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

async fn watch_ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(async move |socket| {
        let socket = socket;
        let who = addr;
        let ServerState { client_update, .. } = state;
        let (pipe_send, mut pipe) = mpsc::unbounded_channel();

        client_update
            .send(ClientUpdate::Watcher(who, pipe_send))
            .expect("game server dead");
        let (mut sender, mut receiver) = socket.split();
        tokio::spawn(async move {
            loop {
                let msg = tokio::select! {
                    _ = receiver.next() => {
                        break;
                    }
                    msg = pipe.recv() => {
                        let Some(msg) = msg else { break };
                        msg
                    }
                };
                let data = serde_json::to_string(&msg).expect("failed encoding");
                sender
                    .send(Message::Text(Utf8Bytes::from(data)))
                    .await
                    .expect("failed to send to client");
            }
        });
    })
}

async fn game_ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(async move |socket| {
        let mut socket = socket;
        let who = addr;
        let ServerState {
            client_update,
            msg_send,
            ..
        } = state;

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
                let e: anyhow::Result<()> = try {
                    let json = serde_json::to_string(&msg)?;
                    sender.send(Message::Text(Utf8Bytes::from(json))).await?
                };
                if let Err(e) = e {
                    error!("{who} send error: {e}");
                    break;
                }
            }
            info!("{who}: closed send loop");
        });
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                let e: anyhow::Result<()> = try {
                    match msg {
                        Message::Text(bytes) => {
                            let msg = serde_json::from_slice::<ClientMessage>(bytes.as_bytes())?;
                            msg_send.send((who, msg))?;
                        }
                        Message::Close(_close_frame) => break,
                        x => Err(anyhow::Error::msg(format!("{x:?}",)))?,
                    }
                };
                if let Err(e) = e {
                    error!("{who} recv error: {e}");
                    break;
                }
            }
            info!("{who}: closed recv loop");
        });
    })
}
