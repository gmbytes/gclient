use godot::prelude::*;

use netcore::event::{NetEvent, RoleInfo};
use netcore::session::ConnectionState;
use netcore::NetClient;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct NetClientBridge {
    base: Base<Node>,
    client: NetClient,
}

#[godot_api]
impl INode for NetClientBridge {
    fn init(base: Base<Node>) -> Self {
        netcore::init_logging();
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
    fn send_login(&mut self, account: GString, token: GString) {
        self.client
            .send_login(&account.to_string(), &token.to_string(), "1.0.0");
    }

    #[func]
    fn send_create_role(&mut self, cid: i64, name: GString) {
        self.client.send_create_role(cid, &name.to_string());
    }

    #[func]
    fn send_login_role(&mut self, role_id: i64) {
        self.client.send_login_role(role_id);
    }

    #[func]
    fn send_ping(&mut self) {
        self.client.send_ping();
    }

    #[func]
    fn send_enter_zone(&mut self) {
        self.client.send_enter_zone();
    }

    #[func]
    fn send_move(&mut self, x: i64, y: i64, z: i64) {
        self.client.send_move(x, y, z);
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
            ConnectionState::Connecting => "connecting".into(),
            ConnectionState::Connected => "connected".into(),
            ConnectionState::LoggingIn => "logging_in".into(),
            ConnectionState::InGame => "in_game".into(),
        }
    }

    #[func]
    fn poll_events(&mut self) -> Array<Dictionary> {
        let events = self.client.poll_events();
        let mut arr = Array::new();
        for event in events {
            arr.push(&event_to_dict(&event));
        }
        arr
    }
}

fn event_to_dict(event: &NetEvent) -> Dictionary {
    let mut d = Dictionary::new();
    match event {
        NetEvent::Connected => {
            d.set("type", "connected");
        }
        NetEvent::Disconnected { reason } => {
            d.set("type", "disconnected");
            d.set("reason", GString::from(reason.as_str()));
        }
        NetEvent::ConnectError { message } => {
            d.set("type", "error");
            d.set("message", GString::from(message.as_str()));
        }
        NetEvent::LoginResponse {
            err,
            fast,
            roles,
            account,
            server_time,
        } => {
            d.set("type", "rsp_login");
            d.set("err", *err);
            d.set("fast", *fast);
            d.set("account", GString::from(account.as_str()));
            d.set("server_time", *server_time);
            d.set("roles", roles_to_array(roles));
        }
        NetEvent::CreateRoleResponse { err, role } => {
            d.set("type", "rsp_create_role");
            d.set("err", *err);
            if let Some(r) = role {
                d.set("role", role_to_dict(r));
            }
        }
        NetEvent::LoginRoleResponse { err, regain } => {
            d.set("type", "rsp_login_role");
            d.set("err", *err);
            d.set("regain", *regain);
        }
        NetEvent::Pong | NetEvent::PongZzz => {
            d.set("type", "pong");
        }
        NetEvent::LoginData { regain } => {
            d.set("type", "dsp_login");
            d.set("regain", *regain);
        }
        NetEvent::KickRole { kick_type } => {
            d.set("type", "kick_role");
            d.set("kick_type", *kick_type);
        }
        NetEvent::ServerMaintain {
            reboot_time,
            shutdown_time,
        } => {
            d.set("type", "server_maintain");
            d.set("reboot_time", *reboot_time);
            d.set("shutdown_time", *shutdown_time);
        }
        NetEvent::EnterZoneResponse => {
            d.set("type", "rsp_enter_zone");
        }
        NetEvent::MoveResponse {
            role_id,
            x,
            y,
            z,
        } => {
            d.set("type", "rsp_move");
            d.set("role_id", *role_id);
            d.set("x", *x);
            d.set("y", *y);
            d.set("z", *z);
        }
        NetEvent::MoveSync {
            role_id,
            x,
            y,
            z,
        } => {
            d.set("type", "dsp_move");
            d.set("role_id", *role_id);
            d.set("x", *x);
            d.set("y", *y);
            d.set("z", *z);
        }
        NetEvent::EnterZoneNotify => {
            d.set("type", "dsp_enter_zone");
        }
        NetEvent::RawMessage { key, err, body } => {
            d.set("type", "raw");
            d.set("key", *key as i64);
            d.set("err", *err as i64);
            d.set("body_len", body.len() as i64);
        }
    }
    d
}

fn roles_to_array(roles: &[RoleInfo]) -> Array<Dictionary> {
    let mut arr = Array::new();
    for r in roles {
        arr.push(&role_to_dict(r));
    }
    arr
}

fn role_to_dict(r: &RoleInfo) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("id", r.id);
    d.set("cid", r.cid);
    d.set("lv", r.lv);
    d.set("name", GString::from(r.name.as_str()));
    d.set("icon", r.icon);
    d
}
