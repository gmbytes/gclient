use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use bytes::Bytes;
use log::{info, warn};
use prost::Message as _;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MessageDescriptor, Value};
use serde::Deserialize;

/// Per-entry metadata loaded from protocol_manifest.json.
/// Mirrors the `ManifestEntry` struct in genpb/manifest.go.
#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolMetaEntry {
    #[serde(default)]
    pub ekey_name:   String,
    #[serde(rename = "message_name")]
    pub message:     String,
    pub event_name:  String,
    #[serde(default)]
    pub hotfix_fallback: bool,
}

pub struct ProtocolRegistry {
    pool:        Option<DescriptorPool>,
    /// key_u16 → meta entry
    meta:        HashMap<u16, ProtocolMetaEntry>,
    /// message_name → MessageDescriptor (cached)
    descriptors: HashMap<String, MessageDescriptor>,
    /// Keys whose runtime schema fingerprint differs from the compiled one
    overrides:   HashSet<u16>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            pool:        None,
            meta:        HashMap::new(),
            descriptors: HashMap::new(),
            overrides:   HashSet::new(),
        }
    }

    /// Load `protocol.desc` and `protocol_manifest.json` from `dir`.
    /// Gracefully returns `Ok(())` when files are missing.
    pub fn load_from_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), String> {
        let dir = dir.as_ref();
        let desc_path     = dir.join("protocol.desc");
        let manifest_path = dir.join("protocol_manifest.json");

        if !desc_path.exists() {
            warn!("[protocol_registry] {} not found – hotfix channel disabled", desc_path.display());
            return Ok(());
        }

        let desc_bytes = fs::read(&desc_path)
            .map_err(|e| format!("read {}: {}", desc_path.display(), e))?;
        let pool = DescriptorPool::decode(desc_bytes.as_slice())
            .map_err(|e| format!("decode descriptor set: {}", e))?;
        self.pool = Some(pool);

        if manifest_path.exists() {
            let json = fs::read_to_string(&manifest_path)
                .map_err(|e| format!("read {}: {}", manifest_path.display(), e))?;
            self.load_manifest_json(&json)?;
        } else {
            warn!("[protocol_registry] {} not found – event names unavailable", manifest_path.display());
        }

        self.warm_descriptors();
        self.compute_overrides();
        info!(
            "[protocol_registry] ready – {} descriptors, {} overrides",
            self.descriptors.len(), self.overrides.len()
        );
        Ok(())
    }

    fn load_manifest_json(&mut self, json: &str) -> Result<(), String> {
        // protocol_manifest.json format:
        // { "version": 1, "proto_pkg": "pb", "messages": [ { "ekey_value": 2, ... }, … ] }
        let root: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| format!("parse manifest json: {}", e))?;

        self.meta.clear();
        if let Some(messages) = root.get("messages").and_then(|v| v.as_array()) {
            for msg in messages {
                let key_val = msg.get("ekey_value").and_then(|v| v.as_u64());
                let key = match key_val {
                    Some(k) if k > 0 && k <= 65534 => k as u16,
                    _ => continue,
                };
                let entry: ProtocolMetaEntry = serde_json::from_value(msg.clone())
                    .map_err(|e| format!("parse manifest entry key={}: {}", key, e))?;
                self.meta.insert(key, entry);
            }
        }
        info!("[protocol_registry] loaded {} manifest entries", self.meta.len());
        Ok(())
    }

    fn warm_descriptors(&mut self) {
        let pool = match &self.pool {
            Some(p) => p,
            None    => return,
        };
        let entries: Vec<_> = self.meta.values()
            .map(|e| (e.message.clone(), format!("pb.{}", e.message)))
            .collect();
        for (short, full) in entries {
            if let Some(desc) = pool.get_message_by_name(&full) {
                self.descriptors.insert(short, desc);
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
        if let Some(d) = self.descriptors.get(message_name) {
            return Some(d.clone());
        }
        let pool = self.pool.as_ref()?;
        let full = format!("pb.{}", message_name);
        let d = pool.get_message_by_name(&full)?;
        self.descriptors.insert(message_name.to_string(), d.clone());
        Some(d)
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
        use crate::typed_protocol::COMPILED_FINGERPRINTS;
        let compiled: HashMap<u16, u64> = COMPILED_FINGERPRINTS.iter().copied().collect();
        let mut overrides = HashSet::new();
        for (&key, entry) in &self.meta {
            if !entry.hotfix_fallback {
                continue;
            }
            if let Some(&compiled_fp) = compiled.get(&key) {
                if compiled_fp == 0 {
                    // Zero fingerprint means "genpb not yet run" – skip override detection.
                    continue;
                }
                if let Some(desc) = self.descriptors.get(&entry.message) {
                    let runtime_fp = Self::recursive_fingerprint(desc);
                    if runtime_fp != compiled_fp {
                        overrides.insert(key);
                        info!(
                            "[protocol_registry] override key={} ({}): schema changed \
                             compiled=0x{:016x} runtime=0x{:016x}",
                            key, entry.event_name, compiled_fp, runtime_fp
                        );
                    }
                }
            }
        }
        self.overrides = overrides;
    }

    /// Recursive FNV-1a fingerprint of a MessageDescriptor, matching manifest.go `recursiveFingerprintStr`.
    fn recursive_fingerprint(desc: &MessageDescriptor) -> u64 {
        let fp_str = Self::recursive_fp_str(desc, &mut HashSet::new());
        Self::fnv1a(fp_str.as_bytes())
    }

    fn recursive_fp_str(desc: &MessageDescriptor, visited: &mut HashSet<String>) -> String {
        let full = desc.full_name().to_string();
        if visited.contains(&full) {
            return "?".to_string(); // cycle guard
        }
        visited.insert(full.clone());

        let mut parts: Vec<String> = desc
            .fields()
            .map(|f| {
                let repeated = f.is_list();
                let type_str = match f.kind() {
                    Kind::Message(sub) => {
                        let inner = Self::recursive_fp_str(&sub, visited);
                        let short = sub.full_name().strip_prefix("pb.").unwrap_or(sub.full_name());
                        if repeated {
                            format!("{}:r:m:{}{{{}}}",  f.number(), short, inner)
                        } else {
                            format!("{}:m:{}{{{}}}",    f.number(), short, inner)
                        }
                    }
                    kind => {
                        let ks = Self::kind_str(kind);
                        if repeated {
                            format!("{}:r:{}", f.number(), ks)
                        } else {
                            format!("{}:{}", f.number(), ks)
                        }
                    }
                };
                type_str
            })
            .collect();
        parts.sort();
        visited.remove(&full);
        parts.join(";")
    }

    fn kind_str(kind: Kind) -> String {
        match kind {
            Kind::Double   => "double".into(),
            Kind::Float    => "float".into(),
            Kind::Int32    => "int32".into(),
            Kind::Int64    => "int64".into(),
            Kind::Uint32   => "uint32".into(),
            Kind::Uint64   => "uint64".into(),
            Kind::Sint32   => "sint32".into(),
            Kind::Sint64   => "sint64".into(),
            Kind::Fixed32  => "fixed32".into(),
            Kind::Fixed64  => "fixed64".into(),
            Kind::Sfixed32 => "sfixed32".into(),
            Kind::Sfixed64 => "sfixed64".into(),
            Kind::Bool     => "bool".into(),
            Kind::String   => "string".into(),
            Kind::Bytes    => "bytes".into(),
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
        const PRIME:  u64 = 1099511628211;
        let mut h = OFFSET;
        for &b in data {
            h ^= b as u64;
            h = h.wrapping_mul(PRIME);
        }
        h
    }

    pub fn new_dynamic_message(&mut self, message_name: &str) -> Option<DynamicMessage> {
        let desc = self.resolve_descriptor(message_name)?;
        Some(DynamicMessage::new(desc))
    }

    pub fn encode_from_json_value(&mut self, message_name: &str, json: &serde_json::Value) -> Result<Vec<u8>, String> {
        let desc = self.resolve_descriptor(message_name)
            .ok_or_else(|| format!("unknown message: {}", message_name))?;
        let msg = json_to_dynamic_msg(json, &desc)?;
        Ok(msg.encode_to_vec())
    }
}

// ── JSON → DynamicMessage helpers ────────────────────────────────────────────

fn json_to_dynamic_msg(json: &serde_json::Value, desc: &MessageDescriptor) -> Result<DynamicMessage, String> {
    let mut msg = DynamicMessage::new(desc.clone());
    let obj = match json.as_object() {
        Some(o) => o,
        None    => return Ok(msg),
    };
    for (field_name, field_val) in obj {
        let field_desc = match desc.get_field_by_name(field_name) {
            Some(f) => f,
            None    => continue,
        };
        let prost_val = if field_desc.is_list() {
            let arr = match field_val.as_array() {
                Some(a) => a,
                None    => continue,
            };
            let items: Vec<Value> = arr.iter()
                .filter_map(|item| json_to_prost_scalar(item, &field_desc.kind()))
                .collect();
            Value::List(items)
        } else {
            match json_to_prost_scalar(field_val, &field_desc.kind()) {
                Some(v) => v,
                None    => continue,
            }
        };
        msg.set_field(&field_desc, prost_val);
    }
    Ok(msg)
}

fn json_to_prost_scalar(json: &serde_json::Value, kind: &Kind) -> Option<Value> {
    match kind {
        Kind::Bool               => json.as_bool().map(Value::Bool),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => json.as_i64().map(|v| Value::I32(v as i32)),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => json.as_i64().map(Value::I64),
        Kind::Uint32 | Kind::Fixed32                => json.as_u64().map(|v| Value::U32(v as u32)),
        Kind::Uint64 | Kind::Fixed64                => json.as_u64().map(Value::U64),
        Kind::Float              => json.as_f64().map(|v| Value::F32(v as f32)),
        Kind::Double             => json.as_f64().map(Value::F64),
        Kind::String             => json.as_str().map(|s| Value::String(s.to_string())),
        Kind::Bytes              => json.as_str().map(|s| Value::Bytes(Bytes::from(s.as_bytes().to_vec()))),
        Kind::Enum(_)            => json.as_i64().map(|v| Value::EnumNumber(v as i32)),
        Kind::Message(msg_desc)  => json_to_dynamic_msg(json, msg_desc).ok().map(Value::Message),
    }
}
