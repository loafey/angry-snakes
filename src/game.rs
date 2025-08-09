use std::{collections::HashMap, net::SocketAddr, time::Duration};

use anyhow::Context;
use battlesnakes_shared::{ClientMessage, ServerMessage};
use tokio::{
    sync::mpsc,
    time::{Interval, interval},
};

use crate::ClientUpdate;

struct ClientInfo {
    name: String,
    msg: mpsc::UnboundedSender<ServerMessage>,
    msg_count: usize,
}

pub struct Game {
    new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
    clients: HashMap<SocketAddr, ClientInfo>,
    interval: Interval,
}
impl Game {
    pub fn new(
        msgs: mpsc::UnboundedReceiver<(SocketAddr, ClientMessage)>,
        new_clients: mpsc::UnboundedReceiver<ClientUpdate>,
    ) -> Self {
        Self {
            new_clients,
            msgs,
            clients: HashMap::new(),
            interval: interval(Duration::from_secs(1)),
        }
    }
    async fn handle_message(&mut self, who: SocketAddr, msg: ClientMessage) -> anyhow::Result<()> {
        info!("{who}: {msg:?}");
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        let (addr, msg) = tokio::select! {
            _ = self.interval.tick() => {
                for cli in self.clients.values_mut() {
                    _ = cli.msg.send(ServerMessage::Tick);
                    cli.msg_count = 0;
                }
                return Ok(());
            }
            msg = self.new_clients.recv() => {
                let msg = msg.context("new client pipe is dead")?;
                match msg {
                    ClientUpdate::Join(addr, name, pipe) => {
                        let (msg_send, msg_recv) = mpsc::unbounded_channel();
                        trace!("got new client: {addr} | {name}");
                        _ = pipe.send(msg_recv);
                        self.clients.insert(addr, ClientInfo {
                            name,
                            msg: msg_send,
                            msg_count: 0
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
