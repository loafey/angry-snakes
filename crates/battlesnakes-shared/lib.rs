use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnDirection {
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    SetName(String),
    Turn(TurnDirection),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Tick,
}
