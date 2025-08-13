use std::ops::{Add, AddAssign};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnDirection {
    Clockwise,
    CounterClockwise,
}

#[derive(
    JsonSchema, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
impl From<usize> for Direction {
    fn from(value: usize) -> Self {
        use Direction::*;
        match value % 4 {
            0 => Left,
            1 => Right,
            2 => Up,
            3 => Down,
            _ => unreachable!(),
        }
    }
}
impl AddAssign<TurnDirection> for Direction {
    fn add_assign(&mut self, rhs: TurnDirection) {
        *self = *self + rhs;
    }
}
impl Add<TurnDirection> for Direction {
    type Output = Direction;

    fn add(self, rhs: TurnDirection) -> Self::Output {
        use Direction::*;
        use TurnDirection::*;
        match (self, rhs) {
            (Left, Clockwise) => Up,
            (Left, CounterClockwise) => Down,
            (Right, Clockwise) => Down,
            (Right, CounterClockwise) => Up,
            (Up, Clockwise) => Right,
            (Up, CounterClockwise) => Left,
            (Down, Clockwise) => Left,
            (Down, CounterClockwise) => Right,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    SetName(String),
    Turn(TurnDirection),
}

#[derive(JsonSchema, Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Tick {
        map: Map,
        map_size: (usize, usize),
        your_position: (usize, usize),
        your_direction: Direction,
    },
}

pub type Map = Vec<MapPiece>;

#[derive(JsonSchema, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum MapPiece {
    Snake(usize),
    SnakeHead(usize),
    Apple,
    Empty,
}

impl std::fmt::Display for MapPiece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapPiece::Snake(_) => write!(f, "🟩"),
            MapPiece::SnakeHead(_) => write!(f, "🐍"),
            MapPiece::Apple => write!(f, "🍎"),
            MapPiece::Empty => write!(f, "░░"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct WatchUpdate {
    pub map: Map,
    pub map_size: (usize, usize),
    pub clients: Vec<PlayerData>,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct PlayerData {
    pub name: String,
    pub position: (usize, usize),
    pub tail_len: usize,
    pub death: usize,
    pub id: usize,
}
