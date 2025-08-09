use async_tungstenite::{WebSocketSender, tokio::TokioAdapter};
use battlesnakes_shared::ClientMessage;
use tokio::net::TcpStream;
use tungstenite::{Message, Utf8Bytes};

pub trait ClientExt {
    async fn msg(&self, message: ClientMessage) -> anyhow::Result<()>;
}
impl ClientExt for WebSocketSender<TokioAdapter<TcpStream>> {
    async fn msg(&self, message: ClientMessage) -> anyhow::Result<()> {
        let json = serde_json::to_string(&message)?;
        self.send(Message::Text(Utf8Bytes::from(json))).await?;
        Ok(())
    }
}
