use godot::prelude::*;
use prost_reflect::{DynamicMessage, MapKey, ReflectMessage, Value as ProstValue};

use gnet::event::NetEvent;
use gnet::session::ConnectionState;
use gnet::NetClient;

use crate::godot_bridge_gen::{
    self, hotfix_to_event, make_framework_event, server_message_to_event, NetEventGd,
};

#[derive(GodotClass)]
#[class(base = Node)]
pub struct NetClientBridge {
    base: Base<Node>,
    client: NetClient,
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

    /// Load `protocol.desc` + `protocol_manifest.json` from a directory
    /// to enable the hotfix fallback channel.
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

    /// Generic / hotfix send: encode `fields` Dictionary via the protocol registry.
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
            ConnectionState::Connecting   => "connecting".into(),
            ConnectionState::Connected    => "connected".into(),
            ConnectionState::LoggingIn    => "logging_in".into(),
            ConnectionState::InGame       => "in_game".into(),
        }
    }

    /// Primary event poll API.
    ///
    /// Returns `Array<Gd<NetEventGd>>`.  In GDScript:
    ///   ```gdscript
    ///   for event in net_client.poll_events():
    ///       match event.event_name:
    ///           "connected":   _on_connected()
    ///           "rsp_login":   _on_login(event.get_data() as RspLoginGd)
    ///           "dsp_move":    _on_move(event.get_data() as DspMoveGd)
    ///           _:             pass
    ///   ```
    #[func]
    fn poll_events(&mut self) -> Array<Gd<NetEventGd>> {
        let raw_events = self.client.poll_events();
        let mut arr: Array<Gd<NetEventGd>> = Array::new();
        for event in raw_events {
            arr.push(&net_event_to_godot(event));
        }
        arr
    }
}

// ── NetEvent → Gd<NetEventGd> ────────────────────────────────────────────────

fn net_event_to_godot(event: NetEvent) -> Gd<NetEventGd> {
    match event {
        NetEvent::Connected => make_framework_event("connected"),

        NetEvent::Disconnected { reason } => {
            let ev = make_framework_event("disconnected");
            // reason is accessible via hotfix_fields if needed; here we embed it
            // in a Dictionary so GDScript can read it.
            let mut fields = Dictionary::new();
            fields.set("reason", GString::from(reason));
            // Wrap using hotfix_to_event to carry the reason string to GDScript.
            hotfix_to_event("disconnected", 0, 0, fields)
        }

        NetEvent::ConnectError { message } => {
            let mut fields = Dictionary::new();
            fields.set("message", GString::from(message));
            hotfix_to_event("error", 0, 0, fields)
        }

        // ── Primary typed channel ──
        NetEvent::ProtocolEvent { event_name: _, key, err, msg } => {
            server_message_to_event(key, err, &msg)
        }

        // ── Hotfix / fallback channel ──
        NetEvent::HotfixEvent { event_name, key, err, fields } => {
            let dict = dynamic_msg_to_dict(&fields);
            hotfix_to_event(&event_name, key, err, dict)
        }

        // ── Last-resort raw fallback ──
        NetEvent::RawMessage { key, err, body } => {
            let mut fields = Dictionary::new();
            fields.set("key",  key as i64);
            fields.set("err",  err as i64);
            let mut byte_arr = PackedByteArray::new();
            for b in body {
                byte_arr.push(b);
            }
            fields.set("body", byte_arr);
            hotfix_to_event("raw", key, err, fields)
        }
    }
}

// ── DynamicMessage → Godot Dictionary helpers ─────────────────────────────────

fn prost_value_to_variant(value: &ProstValue) -> Variant {
    match value {
        ProstValue::Bool(b)         => b.to_variant(),
        ProstValue::I32(v)          => (*v as i64).to_variant(),
        ProstValue::I64(v)          => v.to_variant(),
        ProstValue::U32(v)          => (*v as i64).to_variant(),
        ProstValue::U64(v)          => (*v as i64).to_variant(),
        ProstValue::F32(v)          => (*v as f64).to_variant(),
        ProstValue::F64(v)          => v.to_variant(),
        ProstValue::String(s)       => GString::from(s.as_str()).to_variant(),
        ProstValue::Bytes(b)        => {
            let mut arr = PackedByteArray::new();
            for byte in b.iter() { arr.push(*byte); }
            arr.to_variant()
        }
        ProstValue::EnumNumber(n)   => (*n as i64).to_variant(),
        ProstValue::Message(sub)    => dynamic_msg_to_dict(sub).to_variant(),
        ProstValue::List(items)     => {
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
                    MapKey::Bool(b)   => b.to_variant(),
                    MapKey::I32(n)    => (*n as i64).to_variant(),
                    MapKey::I64(n)    => n.to_variant(),
                    MapKey::U32(n)    => (*n as i64).to_variant(),
                    MapKey::U64(n)    => (*n as i64).to_variant(),
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

// ── Godot Dictionary → serde_json::Value (used by send_generic) ──────────────

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
        VariantType::NIL  => serde_json::Value::Null,
        VariantType::BOOL => v.try_to::<bool>().map(serde_json::Value::Bool).unwrap_or(serde_json::Value::Null),
        VariantType::INT  => v.try_to::<i64>().map(|i| serde_json::json!(i)).unwrap_or(serde_json::Value::Null),
        VariantType::FLOAT => v.try_to::<f64>()
            .ok()
            .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
            .unwrap_or(serde_json::Value::Null),
        VariantType::STRING => v.try_to::<GString>()
            .map(|s| serde_json::Value::String(s.to_string()))
            .unwrap_or(serde_json::Value::Null),
        VariantType::DICTIONARY => v.try_to::<Dictionary>()
            .map(|d| dict_to_json_value(&d))
            .unwrap_or(serde_json::Value::Null),
        VariantType::ARRAY => v.try_to::<Array<Variant>>()
            .map(|arr| {
                let items: Vec<serde_json::Value> = arr.iter_shared()
                    .map(|item| variant_to_json_value(&item))
                    .collect();
                serde_json::Value::Array(items)
            })
            .unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::Null,
    }
}
