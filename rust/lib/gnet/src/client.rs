use std::time::{Duration, Instant};

use log::{debug, info, warn};

use crate::codec::PacketCodec;
use crate::event::NetEvent;
use crate::session::{ConnectionState, Session};
use crate::transport::{RawNetEvent, WsTransport};

pub struct NetClient {
    transport: Option<WsTransport>,
    session: Session,
    pending_packets: Vec<Vec<u8>>,
    // reconnect state
    last_url: String,
    reconnect_enabled: bool,
    reconnect_interval: Duration,
    reconnect_max_retries: u32,
    reconnect_attempt: u32,
    last_disconnect: Option<Instant>,
}

impl NetClient {
    pub fn new() -> Self {
        Self {
            transport: None,
            session: Session::new(),
            pending_packets: Vec::new(),
            last_url: String::new(),
            reconnect_enabled: false,
            reconnect_interval: Duration::from_secs(3),
            reconnect_max_retries: 5,
            reconnect_attempt: 0,
            last_disconnect: None,
        }
    }

    pub fn connect(&mut self, host: &str, port: u16, path: &str) {
        if self.transport.is_some() {
            self.disconnect();
        }
        self.session.reset();
        self.session.on_connecting();
        self.reconnect_attempt = 0;

        let url = format!("ws://{}:{}{}", host, port, path);
        self.last_url = url.clone();
        info!("[net] connecting to {}", url);
        self.transport = Some(WsTransport::connect(&url));
    }

    pub fn disconnect(&mut self) {
        self.reconnect_enabled = false;
        if let Some(ref mut t) = self.transport {
            t.close();
        }
        self.transport = None;
        self.session.on_disconnected();
        self.pending_packets.clear();
        info!("[net] disconnected");
    }

    pub fn set_reconnect(&mut self, enabled: bool, interval_secs: f64, max_retries: u32) {
        self.reconnect_enabled = enabled;
        self.reconnect_interval = Duration::from_secs_f64(interval_secs.max(1.0));
        self.reconnect_max_retries = max_retries;
    }

    pub fn connection_state(&self) -> ConnectionState {
        self.session.state
    }

    /// Send a raw pre-encoded frame (GDScript handles framing via cmd_ext.gd).
    pub fn send_raw(&mut self, data: Vec<u8>) {
        if self.session.state == ConnectionState::Disconnected
            || self.session.state == ConnectionState::Connecting
        {
            debug!("[net] queued raw packet ({} bytes)", data.len());
            self.pending_packets.push(data);
            return;
        }

        if let Some(ref transport) = self.transport {
            if !transport.send(data.clone()) {
                self.pending_packets.push(data);
            }
        } else {
            self.pending_packets.push(data);
        }
    }

    /// Poll network events. Called from GDScript _process every frame.
    pub fn poll_events(&mut self) -> Vec<NetEvent> {
        let mut events = Vec::new();

        // Auto-reconnect
        if self.reconnect_enabled
            && self.transport.is_none()
            && self.reconnect_attempt < self.reconnect_max_retries
            && !self.last_url.is_empty()
        {
            if let Some(last) = self.last_disconnect {
                if last.elapsed() >= self.reconnect_interval {
                    self.reconnect_attempt += 1;
                    warn!(
                        "[net] reconnecting attempt {}/{} to {}",
                        self.reconnect_attempt, self.reconnect_max_retries, self.last_url
                    );
                    self.session.on_connecting();
                    self.transport = Some(WsTransport::connect(&self.last_url));
                    self.last_disconnect = None;
                }
            }
        }

        let mut raw_events = Vec::new();
        if let Some(ref transport) = self.transport {
            while let Some(raw) = transport.try_recv() {
                raw_events.push(raw);
            }
        }

        for raw in raw_events {
            match raw {
                RawNetEvent::Connected => {
                    self.session.on_connected();
                    self.reconnect_attempt = 0;
                    self.flush_pending();
                    info!("[net] connected");
                    events.push(NetEvent::Connected);
                }
                RawNetEvent::Disconnected(reason) => {
                    self.session.on_disconnected();
                    self.transport = None;
                    self.last_disconnect = Some(Instant::now());
                    info!("[net] disconnected: {}", reason);
                    events.push(NetEvent::Disconnected { reason });
                }
                RawNetEvent::Error(message) => {
                    self.session.on_disconnected();
                    self.transport = None;
                    self.last_disconnect = Some(Instant::now());
                    warn!("[net] error: {}", message);
                    events.push(NetEvent::Disconnected { reason: message });
                }
                RawNetEvent::Message(data) => {
                    match PacketCodec::decode(&data) {
                        Ok((key, err, body)) => {
                            if err != 0 {
                                events.push(NetEvent::Error { key, err_code: err });
                            } else {
                                events.push(NetEvent::Message { key, body: body.to_vec() });
                            }
                        }
                        Err(e) => {
                            warn!("[net] codec decode error: {}", e);
                        }
                    }
                }
            }
        }

        events
    }

    fn flush_pending(&mut self) {
        if self.pending_packets.is_empty() {
            return;
        }
        let count = self.pending_packets.len();
        let packets: Vec<Vec<u8>> = self.pending_packets.drain(..).collect();
        info!("[net] flushing {} pending packets", count);
        for pkt in packets {
            if let Some(ref transport) = self.transport {
                if !transport.send(pkt.clone()) {
                    self.pending_packets.push(pkt);
                    break;
                }
            }
        }
    }
}
