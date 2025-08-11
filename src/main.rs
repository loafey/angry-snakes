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

use crate::game::Game;
mod game;

enum ClientUpdate {
    Join(
        SocketAddr,
        String,
        oneshot::Sender<mpsc::UnboundedReceiver<ServerMessage>>,
    ),
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
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter("none,battlesnakes=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed set up tracing");

    let (client_update, client_update_recv) = mpsc::unbounded_channel();
    let (msg_send, msg_recv) = mpsc::unbounded_channel();
    let app = Router::new()
        .route("/", get(async || "Hello, World!"))
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

async fn game_ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        let mut socket = socket;
        let who = addr;
        let ServerState {
            client_update,
            msg_send,
        } = state;
        async move {
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
                                    warn!(
                                        "invalid message from {who} (len = {}): {e:?}",
                                        bytes.len()
                                    )
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
    })
}
