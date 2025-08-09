use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnDirection {
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
            (Down, Clockwise) => Right,
            (Down, CounterClockwise) => Left,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    SetName(String),
    Turn(TurnDirection),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Tick { map: Map, map_size: (usize, usize) },
}

pub type Map = Vec<MapPiece>;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum MapPiece {
    Snake,
    SnakeHead,
    Apple,
    Empty,
}
