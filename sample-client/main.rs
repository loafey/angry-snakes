use crate::client_ext::ClientExt;
use async_tungstenite::tokio::connect_async;
use battlesnakes_shared::{ClientMessage, Direction, MapPiece, ServerMessage, TurnDirection};
use futures::StreamExt;
use pathfinding::directed::dijkstra::dijkstra;
use rand::{Rng, distr::Alphanumeric};
use tungstenite::Message;

mod client_ext;

fn get_map(map: Vec<MapPiece>, map_size: (usize, usize)) -> Vec<Vec<MapPiece>> {
    let mut mv = Vec::new();
    for (i, _) in map.iter().enumerate() {
        if i.is_multiple_of(map_size.0) {
            mv.push(Vec::new());
        }
        mv.last_mut().unwrap().push(map[i]);
    }
    mv
}
fn path_to_apple(
    map: &[Vec<MapPiece>],
    (m_x, m_y): (usize, usize),
    (p_x, p_y): (usize, usize),
    your_direction: Direction,
) -> Option<TurnDirection> {
    let apples = map
        .iter()
        .enumerate()
        .flat_map(|(y, r)| r.iter().enumerate().map(move |(x, v)| (x, y, v)))
        .filter_map(|(x, y, v)| match *v == MapPiece::Apple {
            true => Some((x, y)),
            false => None,
        })
        .collect::<Vec<_>>();
    let mut shortest_path = (Vec::new(), 10000);
    let position = match your_direction {
        Direction::Left => (p_x.wrapping_sub(1) % m_x, p_y),
        Direction::Right => (p_x.wrapping_add(1) % m_x, p_y),
        Direction::Up => (p_x, p_y.wrapping_sub(1) % m_y),
        Direction::Down => (p_x, p_y.wrapping_add(1) % m_y),
    };
    for apple in apples {
        let dir = dijkstra(
            &(your_direction, position.0, position.1),
            |p| {
                let (dir, p_x, p_y) = *p;

                let neigh_pos = match dir {
                    Direction::Left => [
                        (dir, p_x.wrapping_sub(1) % m_x, p_y),
                        (
                            dir + TurnDirection::Clockwise,
                            p_x,
                            p_y.wrapping_sub(1) % m_y,
                        ),
                        (
                            dir + TurnDirection::CounterClockwise,
                            p_x,
                            p_y.wrapping_add(1) % m_y,
                        ),
                    ],
                    Direction::Right => [
                        (dir, p_x.wrapping_add(1) % m_x, p_y),
                        (
                            dir + TurnDirection::Clockwise,
                            p_x,
                            p_y.wrapping_add(1) % m_y,
                        ),
                        (
                            dir + TurnDirection::CounterClockwise,
                            p_x,
                            p_y.wrapping_sub(1) % m_y,
                        ),
                    ],
                    Direction::Up => [
                        (dir, p_x, p_y.wrapping_sub(1) % m_y),
                        (
                            dir + TurnDirection::Clockwise,
                            p_x.wrapping_add(1) % m_x,
                            p_y,
                        ),
                        (
                            dir + TurnDirection::CounterClockwise,
                            p_x.wrapping_sub(1) % m_x,
                            p_y,
                        ),
                    ],
                    Direction::Down => [
                        (dir, p_x, p_y.wrapping_add(1) % m_y),
                        (
                            dir + TurnDirection::Clockwise,
                            p_x.wrapping_sub(1) % m_x,
                            p_y,
                        ),
                        (
                            dir + TurnDirection::CounterClockwise,
                            p_x.wrapping_add(1) % m_x,
                            p_y,
                        ),
                    ],
                };
                neigh_pos
                    .into_iter()
                    .filter(|(_, p_x, p_y)| {
                        matches!(map[*p_y][*p_x], MapPiece::Apple | MapPiece::Empty)
                    })
                    .map(|m| (m, 1))
            },
            |(_, p_x, p_y)| (*p_x, *p_y) == apple,
        );
        if let Some((dir, cost)) = dir
            && cost < shortest_path.1
        {
            shortest_path = (dir, cost)
        }
    }
    if let Some((d, _, _)) = shortest_path.0.get(1) {
        use Direction::*;
        use TurnDirection::*;

        // println!("{your_direction:?} -> {d:?} = {res:?}");
        match (your_direction, d) {
            (Left, Up) => Some(Clockwise),
            (Left, Down) => Some(CounterClockwise),
            (Right, Up) => Some(CounterClockwise),
            (Right, Down) => Some(Clockwise),
            (Up, Left) => Some(CounterClockwise),
            (Up, Right) => Some(Clockwise),
            (Down, Left) => Some(Clockwise),
            (Down, Right) => Some(CounterClockwise),
            _ => None,
        }
    } else if rand::random_bool(0.1) {
        match rand::random::<bool>() {
            true => Some(TurnDirection::Clockwise),
            false => Some(TurnDirection::CounterClockwise),
        }
    } else {
        None
    }
}

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
            ServerMessage::Tick {
                map,
                map_size,
                your_position,
                your_direction,
            } => {
                let map = get_map(map, map_size);
                // println!();
                // for r in &map {
                //     for c in r {
                //         print!("{c}")
                //     }
                //     println!();
                // }
                let path = path_to_apple(&map, map_size, your_position, your_direction);
                if let Some(dir) = path {
                    writer.msg(ClientMessage::Turn(dir)).await?;
                }

                // if rand::random::<bool>() {
                //     let dir = match rand::random::<bool>() {
                //         true => TurnDirection::Clockwise,
                //         false => TurnDirection::CounterClockwise,
                //     };
                //     writer.msg(ClientMessage::Turn(dir)).await?;
                // }
            }
        }
    }
    writer.close(None).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[allow(clippy::reversed_empty_ranges)]
    for _ in 0..10 {
        tokio::spawn(game_client());
    }
    game_client().await?;
    Ok(())
}
