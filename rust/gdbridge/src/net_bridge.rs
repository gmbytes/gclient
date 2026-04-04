use godot::prelude::*;

use gnet::event::NetEvent;
use gnet::session::ConnectionState;
use gnet::NetClient;

use crate::godot_bridge_gen::{
    make_framework_event, make_framework_event_with_extra, server_message_to_event, NetEventGd,
    make_event,
};

#[derive(GodotClass)]
#[class(base = Node)]
pub struct NetClientBridge {
    base: Base<Node>,
    pub(crate) client: NetClient,
}

#[godot_api]
impl INode for NetClientBridge {
    fn init(base: Base<Node>) -> Self {
        gnet::init_logging();
        Self {
            base,
            client: NetClient::new(),
        }
    }
}

#[godot_api]
impl NetClientBridge {
    #[func]
    fn connect_to_server(&mut self, host: GString, port: i64, path: GString) {
        self.client
            .connect(&host.to_string(), port as u16, &path.to_string());
    }

    #[func]
    fn disconnect_from_server(&mut self) {
        self.client.disconnect();
    }

    #[func]
    fn set_reconnect(&mut self, enabled: bool, interval_secs: f64, max_retries: i64) {
        self.client
            .set_reconnect(enabled, interval_secs, max_retries as u32);
    }

    #[func]
    fn start_heartbeat(&mut self, interval_secs: f64) {
        self.client.start_heartbeat(interval_secs);
    }

    #[func]
    fn stop_heartbeat(&mut self) {
        self.client.stop_heartbeat();
    }

    #[func]
    fn get_connection_state(&self) -> GString {
        match self.client.connection_state() {
            ConnectionState::Disconnected => "disconnected".into(),
            ConnectionState::Connecting   => "connecting".into(),
            ConnectionState::Connected    => "connected".into(),
        }
    }

    #[func]
    fn poll_events(&mut self) -> Array<Gd<NetEventGd>> {
        let raw_events = self.client.poll_events();
        let mut arr: Array<Gd<NetEventGd>> = Array::new();
        for event in raw_events {
            arr.push(&net_event_to_godot(event));
        }
        arr
    }

    // ── Generated send_* (TODO: genpb should generate these) ──────────────

    #[func]
    fn send_login(&mut self, account: GString, token: GString, version: GString) {
        use prost::Message;
        let body = gnet::pb::ReqLogin {
            account: account.to_string(),
            token: token.to_string(),
            version: version.to_string(),
            ..Default::default()
        }.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqLogin.as_u16(), &body);
    }

    #[func]
    fn send_create_role(&mut self, cid: i64, name: GString) {
        use prost::Message;
        let body = gnet::pb::ReqCreateRole {
            cid, name: name.to_string(),
        }.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqCreateRole.as_u16(), &body);
    }

    #[func]
    fn send_login_role(&mut self, role_id: i64) {
        use prost::Message;
        let body = gnet::pb::ReqLoginRole { role_id }.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqLoginRole.as_u16(), &body);
    }

    #[func]
    fn send_ping(&mut self) {
        use prost::Message;
        let body = gnet::pb::ReqPing {}.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqPing.as_u16(), &body);
    }

    #[func]
    fn send_enter_zone(&mut self) {
        use prost::Message;
        let body = gnet::pb::ReqEnterZone {}.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqEnterZone.as_u16(), &body);
    }

    #[func]
    fn send_move(&mut self, x: i64, y: i64, z: i64) {
        use prost::Message;
        let body = gnet::pb::ReqMove {
            pos: Some(gnet::pb::Vector { x, y, z }),
        }.encode_to_vec();
        self.client.send_packet(gnet::EKey::ReqMove.as_u16(), &body);
    }
}

// ── NetEvent -> Gd<NetEventGd> ──────────────────────────────────────────────

fn net_event_to_godot(event: NetEvent) -> Gd<NetEventGd> {
    match event {
        NetEvent::Connected => make_framework_event("connected"),

        NetEvent::Disconnected { reason } => {
            let mut extra = Dictionary::new();
            extra.set("reason", GString::from(reason));
            make_framework_event_with_extra("disconnected", extra)
        }

        NetEvent::ConnectError { message } => {
            let mut extra = Dictionary::new();
            extra.set("message", GString::from(message));
            make_framework_event_with_extra("error", extra)
        }

        NetEvent::ProtocolEvent { key, err, msg, .. } => {
            server_message_to_event(key, err, &msg)
        }

        NetEvent::RawMessage { key, err, body } => {
            let mut extra = Dictionary::new();
            extra.set("key", key as i64);
            extra.set("err", err as i64);
            let mut byte_arr = PackedByteArray::new();
            for b in body { byte_arr.push(b); }
            extra.set("body", byte_arr);
            make_event("raw", key, err, None, extra)
        }
    }
}
