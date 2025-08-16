use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::Duration,
};

use anyhow::Context;
use snakes_shared::{
    ClientMessage, Direction, Map, MapPiece, PlayerData, ServerMessage, WatchUpdate,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{Instant, Interval, interval, interval_at},
};

use crate::{ClientUpdate, tick_buffer::TickBuffer};

struct ClientInfo {
    name: String,
    msg: mpsc::UnboundedSender<ServerMessage>,
    msg_count: usize,
    position: (usize, usize),
    tail: VecDeque<(usize, usize)>,
    tail_len: usize,
    death: usize,
    direction: Direction,
    id: usize,
}

pub struct Game {
    id: usize,
    tb: TickBuffer<50>,
    id_counter: usize,
    new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
    clients: HashMap<SocketAddr, ClientInfo>,
    interval: Interval,
    watchers: HashMap<SocketAddr, mpsc::UnboundedSender<WatchUpdate>>,

    map: Map,
    map_size: (usize, usize),
    tick: usize,
    apples: Vec<(usize, usize)>,
}
impl Game {
    pub fn new(
        id: usize,
    ) -> (
        Self,
        mpsc::UnboundedSender<(SocketAddr, ClientMessage)>,
        mpsc::UnboundedSender<ClientUpdate>,
    ) {
        info!("lobby {id}: started game");
        let map_size = (20, 14);
        let (msgs_send, msgs) = mpsc::unbounded_channel();
        let (new_clients_send, new_clients) = mpsc::unbounded_channel();
        let apples = vec![(
            rand::random_range(0..map_size.0),
            rand::random_range(0..map_size.1),
        )];
        (
            Self {
                tb: TickBuffer::new(),
                id,
                id_counter: 0,
                new_clients,
                msgs,
                clients: HashMap::new(),
                interval: interval(Duration::from_secs(1)),
                map: vec![MapPiece::Empty; map_size.0 * map_size.1],
                map_size,
                tick: 0,
                apples,
                watchers: HashMap::new(),
            },
            msgs_send,
            new_clients_send,
        )
    }

    fn spawn_apple(&mut self, count: usize) {
        self.apples = Vec::new();
        for _ in 0..count {
            'outer: for _ in 0..100 {
                let position = (
                    rand::random_range(0..self.map_size.0),
                    rand::random_range(0..self.map_size.1),
                );
                for apple in &self.apples {
                    if *apple == position {
                        continue 'outer;
                    }
                }
                for c in self.clients.values() {
                    if c.position == position {
                        continue 'outer;
                    }
                    for t in &c.tail {
                        if *t == position {
                            continue 'outer;
                        }
                    }
                }
                self.apples.push(position);
                break;
            }
        }
    }

    #[allow(clippy::print_stdout)]
    async fn handle_tick(&mut self) -> anyhow::Result<()> {
        self.tick += 1;
        self.map = vec![MapPiece::Empty; self.map_size.0 * self.map_size.1];
        for (x, y) in &self.apples {
            let index = x + (y * self.map_size.0);
            self.map[index] = MapPiece::Apple;
        }
        let mut needs_new_apples = false;
        for c in self.clients.values_mut() {
            c.tail.push_front(c.position);
            if c.tail.len() > c.tail_len {
                c.tail.pop_back();
            }
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
            if self.map[index] == MapPiece::Apple {
                c.tail_len += 1;
                needs_new_apples = true;
            }
            self.map[index] = MapPiece::SnakeHead(c.id);
            for tail in &c.tail {
                let index = tail.0 + (tail.1 * self.map_size.0);
                if self.map[index] == MapPiece::Empty {
                    self.map[index] = MapPiece::Snake(c.id);
                }
            }
        }
        if needs_new_apples {
            self.spawn_apple(1);
        }

        let mut dead_snakes = Vec::new();
        'outer: for (a1, c1) in &self.clients {
            for (a2, c2) in &self.clients {
                if c1.position == c2.position && a1 != a2 {
                    dead_snakes.push(*a1);
                    dead_snakes.push(*a2);
                    continue 'outer;
                }
                for t in &c2.tail {
                    if *t == c1.position {
                        dead_snakes.push(*a1);
                        continue 'outer;
                    }
                }
            }
        }
        let map_size = self.map_size;
        for snake in dead_snakes {
            let Some(data) = self.clients.get_mut(&snake) else {
                unreachable!()
            };
            data.position = (
                rand::random_range(0..map_size.0),
                rand::random_range(0..map_size.1),
            );
            data.tail_len = 2;
            data.tail.clear();
            data.death += 1;
        }

        let mut dead_clients = Vec::new();
        let data = WatchUpdate {
            map: self.map.clone(),
            map_size: self.map_size,
            clients: self
                .clients
                .values()
                .map(|s| PlayerData {
                    name: s.name.clone(),
                    position: s.position,
                    tail_len: s.tail_len,
                    death: s.death,
                    id: s.id,
                })
                .collect(),
        };
        for (client, send) in &self.watchers {
            if send.send(data.clone()).is_err() {
                dead_clients.push(*client);
            }
        }
        for client in dead_clients {
            self.watchers.remove(&client);
        }
        Ok(())
    }
    async fn handle_message(&mut self, who: SocketAddr, msg: ClientMessage) -> anyhow::Result<()> {
        let Some(cli) = self.clients.get_mut(&who) else {
            error!(
                "lobby {}: got message for non-existent client: {who}",
                self.id
            );
            return Ok(());
        };
        match msg {
            ClientMessage::Turn(tick_id, turn_direction) => {
                cli.direction += turn_direction;
            }
            ClientMessage::SetName(_) => {}
        }
        Ok(())
    }

    fn speedup(&mut self) {
        let dur = self.interval.period();
        let m = Duration::from_secs_f32(0.01);
        if dur > m && dur > Duration::from_secs_f32(0.2) {
            let new = dur - m;
            self.interval = interval_at(Instant::now() + new, new);
        }
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        let (empty_send, empty_recv) =
            oneshot::channel::<Result<ClientUpdate, (SocketAddr, ClientMessage)>>();
        if self.clients.is_empty() && self.watchers.is_empty() {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    return Err(anyhow::Error::msg("no clients"));
                }
                msg = self.new_clients.recv() => {
                    let msg = msg.context("new client pipe is dead")?;
                    _ = empty_send.send(Ok(msg));
                }
                msg = self.msgs.recv() => {
                    let msg = msg.context("msg pipe is dead")?;
                    _ = empty_send.send(Err(msg));
                }
            }
        }
        let msg = tokio::select! {
            msg = empty_recv => msg?,
            _ = self.interval.tick() => {
                println!("{}", self.tb.next());
                self.speedup();
                let mut to_remove = Vec::new();
                for (addr, cli) in &mut self.clients {
                    let e = cli.msg.send(ServerMessage::Tick {
                        tick_id: 0,
                        map: self.map.clone(),
                        map_size: self.map_size,
                        your_direction: cli.direction,
                        your_position: cli.position,
                    });
                    if e.is_err() {
                        to_remove.push(*addr)
                    }
                    cli.msg_count = 0;
                }
                for cli in to_remove {
                    info!("lobby {}: {cli} left",self.id);
                    self.clients.remove(&cli);
                }
                return self.handle_tick().await;
            }
            msg = self.new_clients.recv() => {
                let msg = msg.context("new client pipe is dead")?;
                Ok(msg)
            }
            msg = self.msgs.recv() => {
                let (addr, msg) = msg.context("msg pipe is dead")?;
                Err((addr,msg))
            }
        };
        match msg {
            Ok(msg) => match msg {
                ClientUpdate::Join(addr, name, pipe) => {
                    let (msg_send, msg_recv) = mpsc::unbounded_channel();
                    trace!("lobby {}: got new client: {addr} | {name}", self.id);
                    _ = pipe.send(msg_recv);
                    let position = 'outer: loop {
                        let x = rand::random_range(0..self.map_size.0);
                        let y = rand::random_range(0..self.map_size.1);
                        for c in self.clients.values() {
                            if c.position == (x, y) {
                                continue 'outer;
                            }
                        }
                        break (x, y);
                    };
                    self.clients.insert(
                        addr,
                        ClientInfo {
                            id: self.id_counter,
                            name,
                            msg: msg_send,
                            msg_count: 0,
                            position,
                            direction: Direction::from(rand::random_range(0..4)),
                            tail: VecDeque::new(),
                            tail_len: 2,
                            death: 0,
                        },
                    );
                    self.id_counter += 1;
                }
                ClientUpdate::Watcher(addr, send) => {
                    info!("lobby {}: watcher joined at {addr}", self.id);
                    self.watchers.insert(addr, send);
                }
            },
            Err((addr, msg)) => {
                let cl = self
                    .clients
                    .get_mut(&addr)
                    .context(format!("missing client: {addr}"))?;
                cl.msg_count += 1;
                if cl.msg_count == 2 || cl.msg_count.is_multiple_of(10) {
                    warn!(
                        "lobby {}: {addr}/{} sent too many messages: {}",
                        self.id, cl.name, cl.msg_count
                    );
                }
                if cl.msg_count != 1 {
                    return Ok(());
                }
                self.handle_message(addr, msg).await?;
            }
        }
        Ok(())
    }
}
