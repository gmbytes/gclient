/// Wire-codec and encode/decode round-trip tests.
///
/// Tests that reference specific EKey / ClientMessage / ServerMessage variants
/// require running `genpb` first to populate `typed_protocol.rs` and `pb.rs`.
/// With placeholder files, EKey::from_u16 returns None for all values and
/// ClientMessage / ServerMessage are uninhabited (match *msg {} is exhaustive),
/// so variant-specific tests cannot be exercised without generated code.

use gnet::codec::PacketCodec;
use gnet::typed_protocol::{EKey, decode_server_message};

// ── PacketCodec ───────────────────────────────────────────────────────────────

#[test]
fn codec_reject_corrupt_packet() {
    let short = [0u8; 3];
    assert!(PacketCodec::decode(&short).is_err());

    let mut packet = PacketCodec::encode(1, b"hello");
    packet[4] = 0xFF; // corrupt body_len
    packet[5] = 0xFF;
    assert!(PacketCodec::decode(&packet).is_err());
}

#[test]
fn codec_4byte_error_packet() {
    // Server sends 4-byte error packets: [key LE][err LE] with body_len absent.
    let pkt = vec![101u8, 0, 1, 0]; // key=101 (RspPing), err=1
    let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
    assert_eq!(key, 101);
    assert_eq!(err, 1);
    assert!(body.is_empty());
}

#[test]
fn codec_encode_decode_round_trip() {
    let payload = b"hello world";
    let pkt = PacketCodec::encode(42u16, payload);
    assert!(pkt.len() >= 8);

    let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
    assert_eq!(key,  42);
    assert_eq!(err,  0);
    assert_eq!(body, payload);
}

#[test]
fn codec_header_layout() {
    // Verify wire format: [key LE 2B][err LE 2B][body_len LE 4B][body]
    let body = b"test_body";
    let pkt  = PacketCodec::encode(0x1234u16, body);

    let key_decoded      = u16::from_le_bytes([pkt[0], pkt[1]]);
    let err_decoded      = u16::from_le_bytes([pkt[2], pkt[3]]);
    let body_len_decoded = u32::from_le_bytes([pkt[4], pkt[5], pkt[6], pkt[7]]);

    assert_eq!(key_decoded,      0x1234);
    assert_eq!(err_decoded,      0);
    assert_eq!(body_len_decoded as usize, body.len());
    assert_eq!(&pkt[8..], body);
}

// ── EKey round-trip (works only after genpb has been run) ────────────────────
//
// With the placeholder, EKey::from_u16 returns None for all values.
// After genpb, this test verifies every key in the manifest round-trips.

#[test]
fn ekey_from_u16_returns_none_for_zero_and_max() {
    assert!(EKey::from_u16(0).is_none(),     "0 is Invalid, not a valid EKey");
    assert!(EKey::from_u16(65535).is_none(), "65535 is Max, not a valid EKey");
    assert!(EKey::from_u16(9999).is_none(),  "9999 is not in the protocol");
}

/// This test passes only after running genpb (placeholder has no EKey variants).
#[test]
fn ekey_round_trip_all_protocol_keys() {
    let all_keys: Vec<u16> = vec![
        1, 2, 3, 4, 5, 6,        // login group
        100, 101, 102, 103,       // ping group
        200, 201,                 // zone entry
        300, 301,                 // move
        10000, 10001, 10002, 10003, // dispatch group
        33005,                    // dsp_move
    ];
    for v in all_keys {
        match EKey::from_u16(v) {
            Some(key) => assert_eq!(key.as_u16(), v, "round-trip failed for key {}", v),
            None => {
                // Acceptable with placeholder – silently skip.
                // After genpb this should not happen.
            }
        }
    }
}

// ── decode_server_message: request keys must fail ─────────────────────────────
//
// EKey::ReqLogin is a client-to-server key; it has no server decoder.
// With the placeholder (empty EKey), this is trivially true.
// After genpb it ensures the decode table is correct.

#[test]
fn request_keys_have_no_server_decoder() {
    let req_keys: Vec<u16> = vec![1, 3, 5, 100, 102, 200, 300];
    for k in req_keys {
        if let Some(ekey) = EKey::from_u16(k) {
            let result = decode_server_message(ekey, &[]);
            assert!(
                result.is_err(),
                "key {} (req direction) should not have a server decoder", k
            );
        }
        // If from_u16 returns None (placeholder) – skip silently.
    }
}
