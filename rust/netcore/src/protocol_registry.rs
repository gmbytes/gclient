use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use bytes::Bytes;
use log::{info, warn};
use prost::Message as _;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MessageDescriptor, Value};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolMetaEntry {
    pub ekey: String,
    pub message: String,
    pub event_name: String,
}

pub struct ProtocolRegistry {
    pool: Option<DescriptorPool>,
    /// key_u16 -> meta entry
    meta: HashMap<u16, ProtocolMetaEntry>,
    /// message_name -> MessageDescriptor (cached after first lookup)
    descriptors: HashMap<String, MessageDescriptor>,
    /// Keys whose runtime descriptor fingerprint differs from the compiled one
    overrides: HashSet<u16>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            pool: None,
            meta: HashMap::new(),
            descriptors: HashMap::new(),
            overrides: HashSet::new(),
        }
    }

    /// Load protocol.desc and protocol_meta.json from a directory.
    /// Returns Ok(()) even if files are missing (graceful degradation).
    pub fn load_from_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), String> {
        let dir = dir.as_ref();
        let desc_path = dir.join("protocol.desc");
        let meta_path = dir.join("protocol_meta.json");

        if !desc_path.exists() {
            warn!("[protocol_registry] {} not found, generic channel disabled", desc_path.display());
            return Ok(());
        }

        let desc_bytes = fs::read(&desc_path)
            .map_err(|e| format!("read {}: {}", desc_path.display(), e))?;

        let pool = DescriptorPool::decode(desc_bytes.as_slice())
            .map_err(|e| format!("decode descriptor set: {}", e))?;
        self.pool = Some(pool);

        if meta_path.exists() {
            let meta_str = fs::read_to_string(&meta_path)
                .map_err(|e| format!("read {}: {}", meta_path.display(), e))?;
            let raw: HashMap<String, ProtocolMetaEntry> = serde_json::from_str(&meta_str)
                .map_err(|e| format!("parse meta json: {}", e))?;

            self.meta.clear();
            for (key_str, entry) in raw {
                if let Ok(key) = key_str.parse::<u16>() {
                    self.meta.insert(key, entry);
                }
            }
            info!("[protocol_registry] loaded {} meta entries from {}", self.meta.len(), meta_path.display());
        } else {
            warn!("[protocol_registry] {} not found, event names unavailable", meta_path.display());
        }

        self.warm_descriptors();
        self.compute_overrides();
        info!(
            "[protocol_registry] ready, {} descriptors cached, {} overrides",
            self.descriptors.len(),
            self.overrides.len()
        );
        Ok(())
    }

    fn warm_descriptors(&mut self) {
        let pool = match &self.pool {
            Some(p) => p,
            None => return,
        };
        for entry in self.meta.values() {
            let full_name = format!("pb.{}", entry.message);
            if let Some(desc) = pool.get_message_by_name(&full_name) {
                self.descriptors.insert(entry.message.clone(), desc);
            }
        }
    }

    pub fn get(&self, key: u16) -> Option<&ProtocolMetaEntry> {
        self.meta.get(&key)
    }

    pub fn get_event_name(&self, key: u16) -> Option<&str> {
        self.meta.get(&key).map(|e| e.event_name.as_str())
    }

    fn resolve_descriptor(&mut self, message_name: &str) -> Option<MessageDescriptor> {
        if let Some(desc) = self.descriptors.get(message_name) {
            return Some(desc.clone());
        }
        let pool = self.pool.as_ref()?;
        let full_name = format!("pb.{}", message_name);
        let desc = pool.get_message_by_name(&full_name)?;
        self.descriptors.insert(message_name.to_string(), desc.clone());
        Some(desc)
    }

    pub fn decode_generic(&mut self, message_name: &str, data: &[u8]) -> Result<DynamicMessage, String> {
        let desc = self.resolve_descriptor(message_name)
            .ok_or_else(|| format!("unknown message: {}", message_name))?;
        DynamicMessage::decode(desc, data)
            .map_err(|e| format!("decode {}: {}", message_name, e))
    }

    pub fn encode_generic(&mut self, message_name: &str, msg: &DynamicMessage) -> Result<Vec<u8>, String> {
        let _ = self.resolve_descriptor(message_name)
            .ok_or_else(|| format!("unknown message: {}", message_name))?;
        Ok(msg.encode_to_vec())
    }

    /// Get the key_u16 for a given event name (for send_generic).
    pub fn find_key_by_event_name(&self, event_name: &str) -> Option<u16> {
        self.meta.iter()
            .find(|(_, e)| e.event_name == event_name)
            .map(|(k, _)| *k)
    }

    pub fn is_loaded(&self) -> bool {
        self.pool.is_some()
    }

    pub fn should_override(&self, key: u16) -> bool {
        self.overrides.contains(&key)
    }

    fn compute_overrides(&mut self) {
        use crate::cmd_ext::COMPILED_FINGERPRINTS;
        let compiled: HashMap<u16, u64> = COMPILED_FINGERPRINTS.iter().copied().collect();
        let mut overrides = HashSet::new();
        for (&key, entry) in &self.meta {
            if let Some(&compiled_fp) = compiled.get(&key) {
                if let Some(desc) = self.descriptors.get(&entry.message) {
                    let runtime_fp = Self::fingerprint_message(desc);
                    if runtime_fp != compiled_fp {
                        overrides.insert(key);
                        info!(
                            "[protocol_registry] override key={} ({}): schema changed",
                            key, entry.event_name
                        );
                    }
                }
            }
        }
        self.overrides = overrides;
    }

    fn fingerprint_message(desc: &MessageDescriptor) -> u64 {
        let mut entries: Vec<(u32, String)> = desc
            .fields()
            .map(|f| {
                let type_str = Self::kind_to_fp_type(f.kind());
                let labeled = if f.is_list() {
                    format!("r:{}", type_str)
                } else {
                    type_str
                };
                (f.number(), format!("{}:{}", f.number(), labeled))
            })
            .collect();
        entries.sort_by_key(|(num, _)| *num);
        let combined: String = entries
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join(";");
        Self::fnv1a(combined.as_bytes())
    }

    fn kind_to_fp_type(kind: Kind) -> String {
        match kind {
            Kind::Double => "double".into(),
            Kind::Float => "float".into(),
            Kind::Int32 => "int32".into(),
            Kind::Int64 => "int64".into(),
            Kind::Uint32 => "uint32".into(),
            Kind::Uint64 => "uint64".into(),
            Kind::Sint32 => "sint32".into(),
            Kind::Sint64 => "sint64".into(),
            Kind::Fixed32 => "fixed32".into(),
            Kind::Fixed64 => "fixed64".into(),
            Kind::Sfixed32 => "sfixed32".into(),
            Kind::Sfixed64 => "sfixed64".into(),
            Kind::Bool => "bool".into(),
            Kind::String => "string".into(),
            Kind::Bytes => "bytes".into(),
            Kind::Enum(e) => {
                let name = e.full_name();
                let short = name.strip_prefix("pb.").unwrap_or(name);
                format!("e.{}", short)
            }
            Kind::Message(m) => {
                let name = m.full_name();
                let short = name.strip_prefix("pb.").unwrap_or(name);
                format!("m.{}", short)
            }
        }
    }

    fn fnv1a(data: &[u8]) -> u64 {
        const OFFSET: u64 = 14695981039346656037;
        const PRIME: u64 = 1099511628211;
        let mut h = OFFSET;
        for &b in data {
            h ^= b as u64;
            h = h.wrapping_mul(PRIME);
        }
        h
    }

    /// Create a new DynamicMessage for the given message name.
    pub fn new_dynamic_message(&mut self, message_name: &str) -> Option<DynamicMessage> {
        let desc = self.resolve_descriptor(message_name)?;
        Some(DynamicMessage::new(desc))
    }

    /// Encode a message from a JSON value (used by the generic send path).
    pub fn encode_from_json_value(&mut self, message_name: &str, json: &serde_json::Value) -> Result<Vec<u8>, String> {
        let desc = self.resolve_descriptor(message_name)
            .ok_or_else(|| format!("unknown message: {}", message_name))?;
        let msg = json_to_dynamic_msg(json, &desc)?;
        Ok(msg.encode_to_vec())
    }
}

// ── JSON → DynamicMessage helpers (used by encode_from_json_value) ──

fn json_to_dynamic_msg(json: &serde_json::Value, desc: &MessageDescriptor) -> Result<DynamicMessage, String> {
    let mut msg = DynamicMessage::new(desc.clone());
    let obj = match json.as_object() {
        Some(o) => o,
        None => return Ok(msg),
    };
    for (field_name, field_val) in obj {
        let field_desc = match desc.get_field_by_name(field_name) {
            Some(f) => f,
            None => continue,
        };
        let prost_val = if field_desc.is_list() {
            let arr = match field_val.as_array() {
                Some(a) => a,
                None => continue,
            };
            let items: Vec<Value> = arr
                .iter()
                .filter_map(|item| json_to_prost_scalar(item, &field_desc.kind()))
                .collect();
            Value::List(items)
        } else {
            match json_to_prost_scalar(field_val, &field_desc.kind()) {
                Some(v) => v,
                None => continue,
            }
        };
        msg.set_field(&field_desc, prost_val);
    }
    Ok(msg)
}

fn json_to_prost_scalar(json: &serde_json::Value, kind: &Kind) -> Option<Value> {
    match kind {
        Kind::Bool => json.as_bool().map(Value::Bool),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
            json.as_i64().map(|v| Value::I32(v as i32))
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => json.as_i64().map(Value::I64),
        Kind::Uint32 | Kind::Fixed32 => json.as_u64().map(|v| Value::U32(v as u32)),
        Kind::Uint64 | Kind::Fixed64 => json.as_u64().map(Value::U64),
        Kind::Float => json.as_f64().map(|v| Value::F32(v as f32)),
        Kind::Double => json.as_f64().map(Value::F64),
        Kind::String => json.as_str().map(|s| Value::String(s.to_string())),
        Kind::Bytes => json
            .as_str()
            .map(|s| Value::Bytes(Bytes::from(s.as_bytes().to_vec()))),
        Kind::Enum(_) => json.as_i64().map(|v| Value::EnumNumber(v as i32)),
        Kind::Message(msg_desc) => {
            json_to_dynamic_msg(json, msg_desc).ok().map(Value::Message)
        }
    }
}

