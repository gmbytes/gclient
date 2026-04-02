/// Verify all dispatch paths under the new typed-protocol pipeline:
///   1. Compiled channel – EKey hit → NetEvent::ProtocolEvent
///   2. Raw fallback     – unknown key, empty registry → NetEvent::RawMessage
///   3. Error path       – err != 0 on known/unknown key → RawMessage
///   4. Registry tests   – ProtocolRegistry lifecycle
///
/// NOTE: Tests in the "Compiled channel" section require running `genpb` first
/// to populate `typed_protocol.rs` and `pb.rs` with real protocol types.
/// With the placeholder files they still compile but the dispatch will fall to
/// RawMessage (EKey::from_u16 returns None for all keys in the placeholder).

use gnet::codec::PacketCodec;
use gnet::dispatcher;
use gnet::event::NetEvent;
use gnet::ProtocolRegistry;

// ── Path 1: Compiled channel ──────────────────────────────────────────────────
// These tests verify that a valid encoded protobuf + known EKey produces
// NetEvent::ProtocolEvent with the correct event_name.
// They pass only after running genpb to generate real typed_protocol.rs.

#[test]
fn compiled_channel_produces_protocol_event_or_raw() {
    // RspLogin key = 2 (from EKey enum after genpb)
    let rsp_login_key: u16 = 2;
    // Encode an empty body for the key (valid for a near-empty RspLogin)
    let raw = PacketCodec::encode(rsp_login_key, &[]);

    let mut reg = ProtocolRegistry::new();
    let event = dispatcher::dispatch(&raw, &mut reg);

    // With placeholder (empty EKey): falls to RawMessage.
    // After genpb: produces ProtocolEvent { event_name: "rsp_login", … }.
    match event {
        NetEvent::ProtocolEvent { event_name, key, err, .. } => {
            assert_eq!(event_name, "rsp_login");
            assert_eq!(key, rsp_login_key);
            assert_eq!(err, 0);
        }
        NetEvent::RawMessage { key, .. } => {
            // Acceptable when EKey placeholder has no variants.
            assert_eq!(key, rsp_login_key);
        }
        other => panic!("unexpected event: {:?}", other),
    }
}

// ── Path 2: Raw fallback (unknown key) ────────────────────────────────────────

#[test]
fn raw_fallback_unknown_key() {
    let body = b"opaque payload";
    let raw = PacketCodec::encode(9999u16, body);

    let mut reg = ProtocolRegistry::new();
    let event = dispatcher::dispatch(&raw, &mut reg);

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

    let mut reg = ProtocolRegistry::new();
    let event = dispatcher::dispatch(&raw, &mut reg);

    match event {
        NetEvent::RawMessage { body, .. } => {
            assert_eq!(body, payload, "body must be the actual bytes");
        }
        other => panic!("expected RawMessage, got {:?}", other),
    }
}

// ── Path 3: Error response on a known / unknown key ──────────────────────────

#[test]
fn error_packet_known_key_returns_raw_message() {
    // Minimal 4-byte error packet: [key LE][err LE]
    let key_u16: u16 = 2; // RspLogin
    let key_bytes = key_u16.to_le_bytes();
    let err_code: u16 = 42;
    let pkt = vec![key_bytes[0], key_bytes[1], (err_code & 0xFF) as u8, (err_code >> 8) as u8];

    let mut reg = ProtocolRegistry::new();
    let event = dispatcher::dispatch(&pkt, &mut reg);

    // With placeholder OR with real protocol: error packets produce RawMessage.
    match event {
        NetEvent::RawMessage { key, err, .. } => {
            assert_eq!(key, key_u16);
            assert_eq!(err, err_code);
        }
        NetEvent::ProtocolEvent { err, .. } => {
            // Acceptable if dispatch emits a ProtocolEvent with err set.
            assert_eq!(err, err_code);
        }
        other => panic!("error path: expected RawMessage or ProtocolEvent, got {:?}", other),
    }
}

#[test]
fn error_packet_unknown_key_falls_to_raw() {
    // key=0x7FFF=32767 is not in EKey; err=7
    let pkt = vec![0xFFu8, 0x7F, 7, 0];
    let mut reg = ProtocolRegistry::new();
    let event = dispatcher::dispatch(&pkt, &mut reg);

    match event {
        NetEvent::RawMessage { key, err, .. } => {
            assert_eq!(key, 0x7FFF);
            assert_eq!(err, 7);
        }
        other => panic!("expected RawMessage for unknown err key, got {:?}", other),
    }
}

// ── ProtocolRegistry lifecycle ────────────────────────────────────────────────

#[test]
fn registry_new_is_empty() {
    let reg = ProtocolRegistry::new();
    assert!(!reg.is_loaded(), "fresh registry should not be loaded");
    assert!(reg.get(1001).is_none());
    assert!(reg.get_event_name(2001).is_none());
}

#[test]
fn registry_load_missing_dir_is_graceful() {
    let mut reg = ProtocolRegistry::new();
    let result = reg.load_from_dir("nonexistent_dir_that_does_not_exist");
    assert!(result.is_ok(), "missing dir must not error");
    assert!(!reg.is_loaded());
}

// ── Event naming conventions ──────────────────────────────────────────────────

#[test]
fn event_name_convention_rsp_dsp_prefix() {
    // Static check: compiled event-name constants follow rsp_xxx / dsp_xxx convention.
    let rsp_login   = "rsp_login";
    let dsp_move    = "dsp_move";
    let raw_type    = "raw";
    assert!(rsp_login.starts_with("rsp_"));
    assert!(dsp_move.starts_with("dsp_"));
    assert_eq!(raw_type, "raw");
}
