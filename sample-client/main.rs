use async_tungstenite::{client_async, tokio::connect_async};
use futures::StreamExt;
use tungstenite::{Message, connect};

#[tokio::main]
async fn main() {
    let (socket, response) = match connect_async("ws://0.0.0.0:8000/ws").await {
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

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (header, _value) in response.headers() {
        println!("* {header}");
    }

    let (writer, mut reader) = socket.split();
    writer
        .send(Message::Text("Hello WebSocket".into()))
        .await
        .unwrap();
    loop {
        let msg = reader
            .next()
            .await
            .expect("Error reading message")
            .expect("Error reading message");
        println!("Received: {msg}");
    }
    // socket.close(None);
}
