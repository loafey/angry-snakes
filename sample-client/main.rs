use crate::client_ext::ClientExt;
use async_tungstenite::tokio::connect_async;
use battlesnakes_shared::{ClientMessage, ServerMessage, TurnDirection};
use futures::StreamExt;
use rand::{Rng, distr::Alphanumeric};
use tungstenite::Message;

mod client_ext;

async fn game_client() -> anyhow::Result<()> {
    let name = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect::<String>();
    println!("My name is: {name}");
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
    writer.msg(ClientMessage::SetName(name.clone())).await?;

    while let Some(Ok(Message::Text(msg))) = reader.next().await {
        let msg = serde_json::from_slice::<ServerMessage>(msg.as_bytes())?;
        match msg {
            ServerMessage::Tick { map, map_size } => {
                if rand::random::<bool>() {
                    let dir = match rand::random::<bool>() {
                        true => TurnDirection::Clockwise,
                        false => TurnDirection::CounterClockwise,
                    };
                    writer.msg(ClientMessage::Turn(dir)).await?;
                }
            }
        }
    }
    writer.close(None).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    for _ in 0..1 {
        tokio::spawn(game_client());
    }
    game_client().await?;
    Ok(())
}
