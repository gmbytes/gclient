use std::time::{Duration, Instant};

use log::{debug, info, warn};

use crate::typed_protocol::{self, ClientMessage};
use crate::codec::PacketCodec;
use crate::dispatcher;
use crate::event::NetEvent;
use crate::pb;
use crate::protocol_registry::ProtocolRegistry;
use crate::session::{ConnectionState, Session};
use crate::transport::{RawNetEvent, WsTransport};

pub struct NetClient {
    transport: Option<WsTransport>,
    session: Session,
    registry: ProtocolRegistry,
    pending_packets: Vec<Vec<u8>>,
    heartbeat_active: bool,
    heartbeat_interval: Duration,
    last_heartbeat: Instant,
    last_login_role_id: i64,
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
            registry: ProtocolRegistry::new(),
            pending_packets: Vec::new(),
            heartbeat_active: false,
            heartbeat_interval: Duration::from_secs(5),
            last_heartbeat: Instant::now(),
            last_login_role_id: 0,
            last_url: String::new(),
            reconnect_enabled: false,
            reconnect_interval: Duration::from_secs(3),
            reconnect_max_retries: 5,
            reconnect_attempt: 0,
            last_disconnect: None,
        }
    }

    /// Load protocol.desc + protocol_manifest.json from a directory for the hotfix channel.
    pub fn load_protocol_registry(&mut self, dir: &str) {
        match self.registry.load_from_dir(dir) {
            Ok(()) => info!("[net] protocol registry loaded from {}", dir),
            Err(e) => warn!("[net] protocol registry load failed: {}", e),
        }
    }

    /// Send a message via the generic channel, encoding from a JSON value using the registry.
    pub fn send_generic(&mut self, key: u16, json: &serde_json::Value) -> Result<(), String> {
        let meta = self.registry.get(key)
            .ok_or_else(|| format!("no meta entry for key {}", key))?
            .clone();
        let body = self.registry.encode_from_json_value(&meta.message, json)?;
        self.send_packet(key, &body);
        Ok(())
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
        self.stop_heartbeat();
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

    // ── Convenience send methods ──

    pub fn send_login(&mut self, account: &str, token: &str, version: &str) {
        self.session.on_login_sent(account);
        info!("[net] send ReqLogin account={}", account);
        self.send_message(&ClientMessage::ReqLogin(pb::ReqLogin {
            account: account.to_string(),
            token: token.to_string(),
            version: version.to_string(),
            ..Default::default()
        }));
    }

    pub fn send_create_role(&mut self, cid: i64, name: &str) {
        info!("[net] send ReqCreateRole cid={} name={}", cid, name);
        self.send_message(&ClientMessage::ReqCreateRole(pb::ReqCreateRole {
            cid,
            name: name.to_string(),
        }));
    }

    pub fn send_login_role(&mut self, role_id: i64) {
        self.last_login_role_id = role_id;
        info!("[net] send ReqLoginRole role_id={}", role_id);
        self.send_message(&ClientMessage::ReqLoginRole(pb::ReqLoginRole { role_id }));
    }

    pub fn send_ping(&mut self) {
        self.send_message(&ClientMessage::ReqPing(pb::ReqPing {}));
    }

    pub fn send_enter_zone(&mut self) {
        info!("[net] send ReqEnterZone");
        self.send_message(&ClientMessage::ReqEnterZone(pb::ReqEnterZone {}));
    }

    pub fn send_move(&mut self, x: i64, y: i64, z: i64) {
        self.send_message(&ClientMessage::ReqMove(pb::ReqMove {
            pos: Some(pb::Vector { x, y, z }),
        }));
    }

    // ── Heartbeat ──

    pub fn start_heartbeat(&mut self, interval_secs: f64) {
        self.heartbeat_interval =
            Duration::from_secs_f64(interval_secs.max(1.0));
        self.heartbeat_active = true;
        self.last_heartbeat = Instant::now();
        info!("[net] heartbeat started interval={:.1}s", interval_secs);
    }

    pub fn stop_heartbeat(&mut self) {
        if self.heartbeat_active {
            self.heartbeat_active = false;
            debug!("[net] heartbeat stopped");
        }
    }

    // ── Core poll loop (called from GDScript _process) ──

    pub fn poll_events(&mut self) -> Vec<NetEvent> {
        let mut events = Vec::new();

        // Auto-heartbeat
        if self.heartbeat_active
            && self.session.state == ConnectionState::InGame
            && self.last_heartbeat.elapsed() >= self.heartbeat_interval
        {
            self.send_ping();
            self.last_heartbeat = Instant::now();
        }

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
                    self.stop_heartbeat();
                    self.transport = None;
                    self.last_disconnect = Some(Instant::now());
                    info!("[net] disconnected: {}", reason);
                    events.push(NetEvent::Disconnected { reason });
                }
                RawNetEvent::Error(message) => {
                    self.session.on_disconnected();
                    self.stop_heartbeat();
                    self.transport = None;
                    self.last_disconnect = Some(Instant::now());
                    warn!("[net] error: {}", message);
                    events.push(NetEvent::ConnectError { message });
                }
                RawNetEvent::Message(data) => {
                    let event = dispatcher::dispatch(&data, &mut self.registry);
                    self.update_session_from_event(&event);
                    events.push(event);
                }
            }
        }

        events
    }

    // ── Internals ──

    fn send_message(&mut self, msg: &ClientMessage) {
        let (key, body) = typed_protocol::encode_client_message(msg);
        self.send_packet(key.as_u16(), &body);
    }

    /// Low-level send: encode into a wire packet and queue or send immediately.
    fn send_packet(&mut self, key: u16, body: &[u8]) {
        let packet = PacketCodec::encode(key, body);

        if self.session.state == ConnectionState::Disconnected
            || self.session.state == ConnectionState::Connecting
        {
            debug!("[net] queued packet key={} ({} bytes)", key, packet.len());
            self.pending_packets.push(packet);
            return;
        }

        if let Some(ref transport) = self.transport {
            if !transport.send(packet.clone()) {
                self.pending_packets.push(packet);
            }
        } else {
            self.pending_packets.push(packet);
        }
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

    fn update_session_from_event(&mut self, event: &NetEvent) {
        match event {
            NetEvent::ProtocolEvent { event_name, err, .. } if *err == 0 => {
                match event_name.as_str() {
                    "rsp_login_role" => {
                        self.session.on_login_role_ok(self.last_login_role_id);
                        info!("[net] login role success, role_id={}", self.last_login_role_id);
                    }
                    "dsp_kick_role" => {
                        self.stop_heartbeat();
                        warn!("[net] kicked by server");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
