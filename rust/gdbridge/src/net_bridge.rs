use godot::prelude::*;
use prost_reflect::{DynamicMessage, MapKey, ReflectMessage, Value as ProstValue};

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

    /// Load protocol.desc + protocol_meta.json from a directory for the generic decode channel.
    #[func]
    fn load_protocol_registry(&mut self, dir: GString) {
        self.client.load_protocol_registry(&dir.to_string());
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

    /// Generic send: encode `fields` Dictionary using the protocol descriptor and send as key.
    #[func]
    fn send_generic(&mut self, key: i64, fields: Dictionary) {
        let json_val = dict_to_json_value(&fields);
        if let Err(e) = self.client.send_generic(key as u16, &json_val) {
            log::warn!("[net_bridge] send_generic key={}: {}", key, e);
        }
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

// ── Event → Dictionary (Rust → GDScript bridge) ──

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
        // Generic channel: auto-flatten DynamicMessage fields into the dict.
        // type = event_name (e.g. "rsp_shop_list"), all proto fields are top-level keys.
        NetEvent::GenericMessage {
            event_name,
            key: _,
            err,
            fields,
        } => {
            d.set("type", GString::from(event_name.as_str()));
            d.set("err", *err as i64);
            for field_desc in fields.descriptor().fields() {
                let value = fields.get_field(&field_desc);
                d.set(field_desc.name(), prost_value_to_variant(&*value));
            }
        }
        // Raw fallback: pass actual body bytes as PackedByteArray.
        NetEvent::RawMessage { key, err, body } => {
            d.set("type", "raw");
            d.set("key", *key as i64);
            d.set("err", *err as i64);
            let mut byte_arr = PackedByteArray::new();
            for b in body {
                byte_arr.push(*b);
            }
            d.set("body", byte_arr);
        }
    }
    d
}

// ── DynamicMessage → Godot Variant helpers ──

fn prost_value_to_variant(value: &ProstValue) -> Variant {
    match value {
        ProstValue::Bool(b) => b.to_variant(),
        ProstValue::I32(v) => (*v as i64).to_variant(),
        ProstValue::I64(v) => v.to_variant(),
        ProstValue::U32(v) => (*v as i64).to_variant(),
        ProstValue::U64(v) => (*v as i64).to_variant(),
        ProstValue::F32(v) => (*v as f64).to_variant(),
        ProstValue::F64(v) => v.to_variant(),
        ProstValue::String(s) => GString::from(s.as_str()).to_variant(),
        ProstValue::Bytes(b) => {
            let mut arr = PackedByteArray::new();
            for byte in b.iter() {
                arr.push(*byte);
            }
            arr.to_variant()
        }
        ProstValue::EnumNumber(n) => (*n as i64).to_variant(),
        ProstValue::Message(sub_msg) => dynamic_msg_to_dict(sub_msg).to_variant(),
        ProstValue::List(items) => {
            let mut arr: Array<Variant> = Array::new();
            for item in items {
                let v = prost_value_to_variant(item);
                arr.push(&v);
            }
            arr.to_variant()
        }
        ProstValue::Map(map) => {
            let mut sub_dict = Dictionary::new();
            for (k, v) in map {
                let key_var: Variant = match k {
                    MapKey::Bool(b) => b.to_variant(),
                    MapKey::I32(n) => (*n as i64).to_variant(),
                    MapKey::I64(n) => n.to_variant(),
                    MapKey::U32(n) => (*n as i64).to_variant(),
                    MapKey::U64(n) => (*n as i64).to_variant(),
                    MapKey::String(s) => GString::from(s.as_str()).to_variant(),
                };
                sub_dict.set(key_var, prost_value_to_variant(v));
            }
            sub_dict.to_variant()
        }
    }
}

fn dynamic_msg_to_dict(msg: &DynamicMessage) -> Dictionary {
    let mut d = Dictionary::new();
    for field_desc in msg.descriptor().fields() {
        let value = msg.get_field(&field_desc);
        d.set(field_desc.name(), prost_value_to_variant(&*value));
    }
    d
}

// ── Godot Dictionary → serde_json::Value helpers (used by send_generic) ──

fn dict_to_json_value(d: &Dictionary) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key_var, val_var) in d.iter_shared() {
        if let Ok(key_str) = key_var.try_to::<GString>() {
            map.insert(key_str.to_string(), variant_to_json_value(&val_var));
        }
    }
    serde_json::Value::Object(map)
}

fn variant_to_json_value(v: &Variant) -> serde_json::Value {
    use godot::builtin::VariantType;
    match v.get_type() {
        VariantType::NIL => serde_json::Value::Null,
        VariantType::BOOL => match v.try_to::<bool>() {
            Ok(b) => serde_json::Value::Bool(b),
            Err(_) => serde_json::Value::Null,
        },
        VariantType::INT => match v.try_to::<i64>() {
            Ok(i) => serde_json::json!(i),
            Err(_) => serde_json::Value::Null,
        },
        VariantType::FLOAT => match v.try_to::<f64>() {
            Ok(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Err(_) => serde_json::Value::Null,
        },
        VariantType::STRING => match v.try_to::<GString>() {
            Ok(s) => serde_json::Value::String(s.to_string()),
            Err(_) => serde_json::Value::Null,
        },
        VariantType::DICTIONARY => match v.try_to::<Dictionary>() {
            Ok(d) => dict_to_json_value(&d),
            Err(_) => serde_json::Value::Null,
        },
        VariantType::ARRAY => match v.try_to::<Array<Variant>>() {
            Ok(arr) => {
                let json_arr: Vec<serde_json::Value> = arr
                    .iter_shared()
                    .map(|item| variant_to_json_value(&item))
                    .collect();
                serde_json::Value::Array(json_arr)
            }
            Err(_) => serde_json::Value::Null,
        },
        _ => serde_json::Value::Null,
    }
}

// ── Compiled channel helpers ──

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
