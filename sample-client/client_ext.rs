use async_tungstenite::{WebSocketSender, stream::Stream, tokio::TokioAdapter};
use snakes_shared::ClientMessage;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;
use tungstenite::{Message, Utf8Bytes};

pub trait ClientExt {
    async fn msg(&self, message: ClientMessage) -> anyhow::Result<()>;
}
impl ClientExt
    for WebSocketSender<Stream<TokioAdapter<TcpStream>, TokioAdapter<TlsStream<TcpStream>>>>
{
    async fn msg(&self, message: ClientMessage) -> anyhow::Result<()> {
        let json = serde_json::to_string(&message)?;
        self.send(Message::Text(Utf8Bytes::from(json))).await?;
        Ok(())
    }
}
