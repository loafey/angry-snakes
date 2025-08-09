use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::Duration,
};

use anyhow::Context;
use battlesnakes_shared::{ClientMessage, Direction, Map, MapPiece, ServerMessage};
use tokio::{
    sync::mpsc,
    time::{Instant, Interval, interval, interval_at},
};

use crate::ClientUpdate;

struct ClientInfo {
    name: String,
    msg: mpsc::UnboundedSender<ServerMessage>,
    msg_count: usize,
    position: (usize, usize),
    tail: VecDeque<(usize, usize)>,
    tail_len: usize,
    direction: Direction,
}

pub struct Game {
    new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
    clients: HashMap<SocketAddr, ClientInfo>,
    interval: Interval,

    map: Map,
    map_size: (usize, usize),
    tick: usize,
}
impl Game {
    pub fn new(
        msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
        new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    ) -> Self {
        let map_size = (20, 9);
        Self {
            new_clients,
            msgs,
            clients: HashMap::new(),
            interval: interval(Duration::from_secs(1)),
            map: vec![MapPiece::Empty; map_size.0 * map_size.1],
            map_size,
            tick: 0,
        }
    }

    #[allow(clippy::print_stdout)]
    async fn handle_tick(&mut self) -> anyhow::Result<()> {
        self.tick += 1;
        self.map = vec![MapPiece::Empty; self.map_size.0 * self.map_size.1];
        for c in self.clients.values_mut() {
            c.tail.push_front(c.position);
            if c.tail.len() > c.tail_len {
                c.tail.pop_back();
            }
            c.tail_len += rand::random_bool(0.1) as usize;
            match c.direction {
                Direction::Left => {
                    if c.position.0 == 0 {
                        c.position.0 = self.map_size.0 - 1;
                    } else {
                        c.position.0 -= 1;
                    }
                }
                Direction::Right => {
                    if c.position.0 == self.map_size.0 - 1 {
                        c.position.0 = 0;
                    } else {
                        c.position.0 += 1;
                    }
                }
                Direction::Up => {
                    if c.position.1 == 0 {
                        c.position.1 = self.map_size.1 - 1;
                    } else {
                        c.position.1 -= 1;
                    }
                }
                Direction::Down => {
                    if c.position.1 == self.map_size.1 - 1 {
                        c.position.1 = 0;
                    } else {
                        c.position.1 += 1;
                    }
                }
            }

            let index = c.position.0 + (c.position.1 * self.map_size.0);
            self.map[index] = MapPiece::SnakeHead;
            for tail in &c.tail {
                let index = tail.0 + (tail.1 * self.map_size.0);
                if self.map[index] == MapPiece::Empty {
                    self.map[index] = MapPiece::Snake;
                }
            }
        }

        for (i, r) in self.map.iter().enumerate() {
            let c = match r {
                MapPiece::Snake => "🟩",
                MapPiece::SnakeHead => "🐍",
                MapPiece::Apple => "🍎",
                MapPiece::Empty => "░░",
            };

            if i.is_multiple_of(self.map_size.0) {
                println!()
            }
            print!("{c}");
        }
        println!("\nTick: {}", self.tick);
        Ok(())
    }
    async fn handle_message(&mut self, who: SocketAddr, msg: ClientMessage) -> anyhow::Result<()> {
        let Some(cli) = self.clients.get_mut(&who) else {
            error!("got message for non-existent client: {who}");
            return Ok(());
        };
        match msg {
            ClientMessage::Turn(turn_direction) => cli.direction += turn_direction,
            ClientMessage::SetName(_) => {}
        }
        Ok(())
    }

    fn speedup(&mut self) {
        let dur = self.interval.period();
        let m = Duration::from_secs_f32(0.01);
        if dur > m && dur > Duration::from_secs_f32(0.1) {
            let new = dur - m;
            self.interval = interval_at(Instant::now() + new, new);
        }
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        let (addr, msg) = tokio::select! {
            _ = self.interval.tick() => {
                self.speedup();
                for cli in self.clients.values_mut() {
                    _ = cli.msg.send(ServerMessage::Tick{
                        map: self.map.clone(),
                        map_size: self.map_size
                    });
                    cli.msg_count = 0;
                }
                return self.handle_tick().await;
            }
            msg = self.new_clients.recv() => {
                let msg = msg.context("new client pipe is dead")?;
                match msg {
                    ClientUpdate::Join(addr, name, pipe) => {
                        let (msg_send, msg_recv) = mpsc::unbounded_channel();
                        trace!("got new client: {addr} | {name}");
                        _ = pipe.send(msg_recv);
                        let position = 'outer: loop {
                            let x = rand::random_range(0..self.map_size.0);
                            let y = rand::random_range(0..self.map_size.1);
                            for c in self.clients.values() {
                                if c.position == (x,y) {
                                    continue 'outer;
                                }
                            }
                            break (x,y);
                        };
                        self.clients.insert(addr, ClientInfo {
                            name,
                            msg: msg_send,
                            msg_count: 0,
                            position,
                            direction: Direction::from(rand::random_range(0..4)),
                            tail: VecDeque::new(),
                            tail_len: 2,
                        });
                    },
                    ClientUpdate::Left(addr, reason) => {
                        info!("{addr}: left, {reason}");
                        self.clients.remove(&addr);
                    }
                }
                return Ok(());
            }
            msg = self.msgs.recv() => {
                let (addr, msg) = msg.context("msg pipe is dead")?;
                let cl = self.clients.get_mut(&addr).context(format!("missing client: {addr}"))?;
                cl.msg_count += 1;
                if cl.msg_count == 2 || cl.msg_count.is_multiple_of(10) {
                    warn!("{addr}: sent too many messages: {}", cl.msg_count);
                }
                if cl.msg_count != 1 {
                    return Ok(());
                }
                (addr, msg)
            }
        };
        self.handle_message(addr, msg).await
    }
}
