use crate::protocol::codec::RakCodec;
use crate::protocol::packets::incompatible_protocol::IncompatibleProtocol;
use crate::protocol::packets::open_connection_reply_1::OpenConnectionReply1;
use crate::protocol::packets::open_connection_reply_2::OpenConnectionReply2;
use crate::protocol::packets::open_connection_request_1::OpenConnectionRequest1;
use crate::protocol::packets::open_connection_request_2::OpenConnectionRequest2;
use crate::protocol::packets::unconnected_ping::UnconnectedPing;
use crate::protocol::packets::unconnected_pong::UnconnectedPong;
use crate::server::config::RakServerConfig;
use crate::session::RakSession;
use crate::session::event::RakSessionEvent;
use crate::util::constants::{PROTOCOL, UDP_HEADER_SIZE};
use crate::util::flags::VALID;
use crate::util::packet_id;
use crate::util::socket_addr::get_overhead;
use dashmap::DashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::debug;

pub struct RakServerInner {
    addr: SocketAddr,
    pub config: RakServerConfig,

    sessions: Arc<DashMap<SocketAddr, RakSession>>,

    pub out_tx: UnboundedSender<(Vec<u8>, SocketAddr)>,
}

impl RakServerInner {
    pub fn new(config: RakServerConfig, addr: SocketAddr, out_tx: UnboundedSender<(Vec<u8>, SocketAddr)>) -> Self {
        Self {
            config,
            addr,
            sessions: Arc::new(DashMap::new()),
            out_tx,
        }
    }

    pub async fn handle(&self, buf: &[u8], addr: SocketAddr) {
        if let Some(&header) = buf.first() {
            match header & VALID {
                0 => self.handle_offline(buf, addr).await,
                _ => {
                    if let Some(s) = self.sessions.get(&addr) {
                        _ = s.inbound(buf.to_vec());
                    }
                }
            }
        }
    }

    async fn handle_offline(&self, buf: &[u8], addr: SocketAddr) {
        if let Some(&id) = buf.first() {
            let mut cursor = Cursor::new(buf);
            match id {
                packet_id::UNCONNECTED_PING => self.handle_unconnected_ping(&mut cursor, addr).await,
                packet_id::OPEN_CONNECTION_REQUEST_1 => self.handle_open_connection_request_1(&mut cursor, addr).await,
                packet_id::OPEN_CONNECTION_REQUEST_2 => self.handle_open_connection_request_2(&mut cursor, addr).await,

                _ => debug!("received unknown offline packet from {}, id: {:#04X}", addr, id),
            }
        }
    }

    async fn handle_unconnected_ping(&self, cursor: &mut Cursor<&[u8]>, addr: SocketAddr) {
        let Ok(ping) = UnconnectedPing::deserialize(cursor) else {
            return debug!("failed to deserialize UnconnectedPing from {}", addr);
        };

        let pong = UnconnectedPong {
            timestamp: ping.timestamp,
            guid: self.config.guid,
            message: self.config.message.clone(),
        };

        let mut buf = Vec::with_capacity(UnconnectedPong::size_hint(&pong));
        UnconnectedPong::serialize(&pong, &mut buf).unwrap();

        self.send((buf, addr));

        debug!("ponged {} with {:?}", addr, pong)
    }

    async fn handle_open_connection_request_1(&self, cursor: &mut Cursor<&[u8]>, addr: SocketAddr) {
        let Ok(request) = OpenConnectionRequest1::deserialize(cursor) else {
            return debug!("failed to deserialize OpenConnectionRequest1 from {}", addr);
        };

        let req_protocol = request.protocol;
        if req_protocol != PROTOCOL {
            let incompatible = IncompatibleProtocol {
                protocol: PROTOCOL,
                guid: self.config.guid,
            };

            debug!("refusing connection from {} due to incompatible protocol {}, expected {}", addr, req_protocol, PROTOCOL);

            let mut buf = Vec::with_capacity(IncompatibleProtocol::size_hint(&incompatible));
            IncompatibleProtocol::serialize(&incompatible, &mut buf).unwrap();

            self.send((buf, addr));

            return;
        }

        let reply = OpenConnectionReply1 {
            guid: self.config.guid,
            cookie: None,
            mtu: (request.mtu + UDP_HEADER_SIZE + get_overhead(&addr)).clamp(self.config.min_mtu_size, self.config.max_mtu_size),
        };

        let mut buf = Vec::with_capacity(OpenConnectionReply1::size_hint(&reply));
        OpenConnectionReply1::serialize(&reply, &mut buf).unwrap();

        self.send((buf, addr));
    }

    async fn handle_open_connection_request_2(&self, cursor: &mut Cursor<&[u8]>, addr: SocketAddr) {
        let Ok(request) = OpenConnectionRequest2::deserialize(cursor) else {
            return debug!("failed to deserialize OpenConnectionRequest2 from {}", addr);
        };

        if request.addr != self.addr {
            return debug!("refusing connection from {} due to address mismatch", addr);
        }

        let mtu = request.mtu;

        if !(self.config.min_mtu_size..=self.config.max_mtu_size).contains(&mtu) {
            return debug!("refusing connection from {} due to invalid mtu size", addr);
        }

        if self.sessions.contains_key(&addr) {
            return debug!("refusing connection from {} due to existing connection", addr);
        }

        debug!("establishing connection from {} with mtu size of {}", addr, mtu);

        let reply = OpenConnectionReply2::new(self.config.guid, addr, mtu, false);

        let mut buf = Vec::with_capacity(OpenConnectionReply2::size_hint(&reply));
        OpenConnectionReply2::serialize(&reply, &mut buf).unwrap();

        self.send((buf, addr));

        let (event_tx, mut event_rx) = unbounded_channel();

        self.sessions.insert(addr, RakSession::new(event_tx, addr, request.client, request.mtu, |_| ()));

        tokio::spawn({
            let sessions = self.sessions.clone();
            async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        RakSessionEvent::Connected(_) => {}
                        RakSessionEvent::Inbound(buf, addr) => {
                            if let Some(&b) = buf.first()
                                && let Some(session) = sessions.get(&addr)
                            {
                                let mut cursor = Cursor::new(buf.as_slice());
                                match b {
                                    packet_id::CONNECTION_REQUEST => session.handle_connection_request(&mut cursor).await,
                                    packet_id::NEW_INCOMING_CONNECTION => session.handle_new_incoming_connection(&mut cursor).await,
                                    _ => debug!("packet from {}, id: {:#04X}", addr, b),
                                }
                            }
                        }
                        RakSessionEvent::Outbound(_, _) => {}
                        RakSessionEvent::Disconnected(addr) => {
                            sessions.remove(&addr);
                        }
                    }
                }
            }
        });
    }

    fn send(&self, packet: (Vec<u8>, SocketAddr)) {
        _ = self.out_tx.send(packet);
    }
}
