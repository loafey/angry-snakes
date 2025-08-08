#![warn(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::{net::SocketAddr, ops::ControlFlow, time::Duration};

use axum::{
    Router,
    extract::{
        ConnectInfo, WebSocketUpgrade,
        ws::{CloseFrame, Message, Utf8Bytes, WebSocket},
    },
    response::IntoResponse,
    routing::{any, get},
};
use axum_extra::{TypedHeader, headers};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::time::interval;

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter("none,battlesnakes=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed set up tracing");

    let app = Router::new()
        .route("/", get(async || "Hello, World!"))
        .route("/ws", any(ws_handler));

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
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    trace!("`{user_agent}` at {addr} connected.");
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, who: SocketAddr) {
    let (mut sender, mut receiver) = socket.split();

    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    sender
                        .send(Message::Text(Utf8Bytes::from_static("yo")))
                        .await.expect("failed sending message");
                    info!("tick!")
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(msg, who).is_break() {
                break;
            }
        }
    });

    info!("Websocket context {who} destroyed");
}

fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            info!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            info!(">>> {who} sent {} bytes: {d:?}", d.len());
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                info!(
                    ">>> {who} sent close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                info!(">>> {who} somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            info!(">>> {who} sent pong with {v:?}");
        }
        Message::Ping(v) => {
            info!(">>> {who} sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(())
}
