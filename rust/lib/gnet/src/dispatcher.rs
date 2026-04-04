use crate::codec::PacketCodec;
use crate::event::NetEvent;
use crate::typed_protocol::{self, EKey};

/// Decode a raw wire packet into a `NetEvent`.
///
/// Priority:
///   1. Compiled channel: strongly-typed decode via `typed_protocol`.
///   2. Raw fallback: unknown key or decode failure.
pub fn dispatch(raw: &[u8]) -> NetEvent {
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

    if let Some(key) = EKey::from_u16(key_u16) {
        if err_u16 != 0 {
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

    NetEvent::RawMessage {
        key: key_u16,
        err: err_u16,
        body: body.to_vec(),
    }
}
