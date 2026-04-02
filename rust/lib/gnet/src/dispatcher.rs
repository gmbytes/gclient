use crate::codec::PacketCodec;
use crate::event::NetEvent;
use crate::protocol_registry::ProtocolRegistry;
use crate::typed_protocol::{self, EKey};

/// Dispatch a raw wire packet to a `NetEvent`.
///
/// Priority:
///   1. Override check: if runtime descriptor fingerprint differs from compiled one,
///      route to the hotfix/fallback channel (DynamicMessage).
///   2. Compiled channel: decode via strongly-typed `typed_protocol::decode_server_message`.
///   3. Generic channel: dynamic decode via ProtocolRegistry (for keys not in EKey).
///   4. Raw fallback: return the bytes as-is.
pub fn dispatch(raw: &[u8], registry: &mut ProtocolRegistry) -> NetEvent {
    let (key_u16, err_u16, body) = match PacketCodec::decode(raw) {
        Ok(v) => v,
        Err(e) => {
            return NetEvent::RawMessage {
                key: 0,
                err: 0,
                body: format!("codec error: {}", e).into_bytes(),
            };
        }
    };

    // ── 1. Hotfix override: runtime descriptor is newer than compiled struct ──
    if registry.should_override(key_u16) {
        if let Some(meta) = registry.get(key_u16).cloned() {
            if err_u16 != 0 {
                return NetEvent::RawMessage { key: key_u16, err: err_u16, body: vec![] };
            }
            match registry.decode_generic(&meta.message, body) {
                Ok(dynamic_msg) => {
                    return NetEvent::HotfixEvent {
                        event_name: meta.event_name,
                        key: key_u16,
                        err: err_u16,
                        fields: dynamic_msg,
                    };
                }
                Err(_) => {}
            }
        }
    }

    // ── 2. Compiled channel: typed decode ──
    if let Some(key) = EKey::from_u16(key_u16) {
        if err_u16 != 0 {
            // Error packet for a known key: produce ProtocolEvent with err set.
            // We need a dummy ServerMessage or just return RawMessage for non-server keys.
            if typed_protocol::decode_server_message(key, &[]).is_ok() {
                // Key has a server decoder – produce a degenerate ProtocolEvent.
                // Since body is empty on error packets the decode would give defaults.
                // Better: just return RawMessage with metadata for error handling in GDScript.
                return NetEvent::RawMessage { key: key_u16, err: err_u16, body: vec![] };
            }
            return NetEvent::RawMessage { key: key_u16, err: err_u16, body: vec![] };
        }
        if let Ok(msg) = typed_protocol::decode_server_message(key, body) {
            let event_name = msg.event_name().to_string();
            return NetEvent::ProtocolEvent {
                event_name,
                key: key_u16,
                err: err_u16,
                msg: Box::new(msg),
            };
        }
    }

    // ── 3. Generic channel: dynamic decode via registry ──
    if let Some(meta) = registry.get(key_u16).cloned() {
        if err_u16 != 0 {
            return NetEvent::RawMessage { key: key_u16, err: err_u16, body: vec![] };
        }
        match registry.decode_generic(&meta.message, body) {
            Ok(dynamic_msg) => {
                return NetEvent::HotfixEvent {
                    event_name: meta.event_name,
                    key: key_u16,
                    err: err_u16,
                    fields: dynamic_msg,
                };
            }
            Err(_) => {}
        }
    }

    // ── 4. Raw fallback ──
    NetEvent::RawMessage {
        key: key_u16,
        err: err_u16,
        body: body.to_vec(),
    }
}
