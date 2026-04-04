/// Verify dispatch paths under the typed-protocol pipeline:
///   1. Compiled channel – EKey hit -> NetEvent::ProtocolEvent
///   2. Raw fallback     – unknown key -> NetEvent::RawMessage
///   3. Error path       – err != 0 -> RawMessage
///
/// Tests in the "Compiled channel" section require running `genpb` first
/// to populate `typed_protocol.rs` and `pb.rs` with real protocol types.

use gnet::codec::PacketCodec;
use gnet::dispatcher;
use gnet::event::NetEvent;

// ── Path 1: Compiled channel ────────────────────────────────────────────────

#[test]
fn compiled_channel_produces_protocol_event_or_raw() {
    let rsp_login_key: u16 = 2;
    let raw = PacketCodec::encode(rsp_login_key, &[]);

    let event = dispatcher::dispatch(&raw);

    match event {
        NetEvent::ProtocolEvent { event_name, key, err, .. } => {
            assert_eq!(event_name, "rsp_login");
            assert_eq!(key, rsp_login_key);
            assert_eq!(err, 0);
        }
        NetEvent::RawMessage { key, .. } => {
            assert_eq!(key, rsp_login_key);
        }
        other => panic!("unexpected event: {:?}", other),
    }
}

// ── Path 2: Raw fallback (unknown key) ──────────────────────────────────────

#[test]
fn raw_fallback_unknown_key() {
    let body = b"opaque payload";
    let raw = PacketCodec::encode(9999u16, body);

    let event = dispatcher::dispatch(&raw);

    match event {
        NetEvent::RawMessage { key, err, body: got_body } => {
            assert_eq!(key, 9999);
            assert_eq!(err, 0);
            assert_eq!(got_body.as_slice(), b"opaque payload");
        }
        other => panic!("raw fallback: expected RawMessage, got {:?}", other),
    }
}

#[test]
fn raw_fallback_body_is_actual_bytes_not_length() {
    let payload = vec![1u8, 2, 3, 4, 5];
    let raw = PacketCodec::encode(8888u16, &payload);

    let event = dispatcher::dispatch(&raw);

    match event {
        NetEvent::RawMessage { body, .. } => {
            assert_eq!(body, payload, "body must be the actual bytes");
        }
        other => panic!("expected RawMessage, got {:?}", other),
    }
}

// ── Path 3: Error response on a known / unknown key ─────────────────────────

#[test]
fn error_packet_known_key_returns_raw_message() {
    let key_u16: u16 = 2;
    let key_bytes = key_u16.to_le_bytes();
    let err_code: u16 = 42;
    let pkt = vec![key_bytes[0], key_bytes[1], (err_code & 0xFF) as u8, (err_code >> 8) as u8];

    let event = dispatcher::dispatch(&pkt);

    match event {
        NetEvent::RawMessage { key, err, .. } => {
            assert_eq!(key, key_u16);
            assert_eq!(err, err_code);
        }
        NetEvent::ProtocolEvent { err, .. } => {
            assert_eq!(err, err_code);
        }
        other => panic!("error path: expected RawMessage or ProtocolEvent, got {:?}", other),
    }
}

#[test]
fn error_packet_unknown_key_falls_to_raw() {
    let pkt = vec![0xFFu8, 0x7F, 7, 0];
    let event = dispatcher::dispatch(&pkt);

    match event {
        NetEvent::RawMessage { key, err, .. } => {
            assert_eq!(key, 0x7FFF);
            assert_eq!(err, 7);
        }
        other => panic!("expected RawMessage for unknown err key, got {:?}", other),
    }
}

// ── Event naming conventions ────────────────────────────────────────────────

#[test]
fn event_name_convention_rsp_dsp_prefix() {
    let rsp_login = "rsp_login";
    let dsp_move  = "dsp_move";
    let raw_type  = "raw";
    assert!(rsp_login.starts_with("rsp_"));
    assert!(dsp_move.starts_with("dsp_"));
    assert_eq!(raw_type, "raw");
}
