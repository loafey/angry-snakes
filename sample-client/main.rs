use std::time::Duration;

use async_tungstenite::tokio::connect_async;
use futures::StreamExt;
use tokio::time::interval;
use tungstenite::Message;

#[tokio::main]
async fn main() {
    let (socket, _) = match connect_async("ws://0.0.0.0:8000/ws").await {
        Ok(o) => o,
        Err(e) => match e {
            tungstenite::Error::Http(res) => {
                let status = res.status();
                let msg = res
                    .into_body()
                    .map(|a| String::from_utf8_lossy(&a).to_string());
                panic!("http error ({status}): {msg:?}")
            }
            _ => panic!("{e}"),
        },
    };

    let (writer, mut reader) = socket.split();
    let mut interval = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                writer
                    .send(Message::Text("Hello WebSocket".into()))
                    .await
                    .unwrap();
                println!("tick!")
            }
            msg = reader.next() => {
                let msg = msg
                    .expect("Error reading message")
                    .expect("Error reading message");
                println!("Received: {msg}");
            }
        }
    }
    // socket.close(None);
}
