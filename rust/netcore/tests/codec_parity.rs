use netcore::codec::PacketCodec;
use netcore::cmd_ext::{EKey, ClientMessage, encode_client_message, decode_server_message};
use netcore::pb;

#[test]
fn encode_decode_req_login() {
    let msg = ClientMessage::ReqLogin(pb::ReqLogin {
        account: "test_user".into(),
        username: "TestPlayer".into(),
        version: "1.0.0".into(),
        ..Default::default()
    });
    let (key, body) = encode_client_message(&msg);
    assert_eq!(key, EKey::ReqLogin);
    assert_eq!(key.as_u16(), 1);

    let packet = PacketCodec::encode(key.as_u16(), &body);
    assert!(packet.len() >= 8);

    // Verify header matches mserver wire format: [key LE] [err LE] [bodyLen LE] [body]
    let decoded_key = u16::from_le_bytes([packet[0], packet[1]]);
    let decoded_err = u16::from_le_bytes([packet[2], packet[3]]);
    let decoded_body_len = u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]);
    assert_eq!(decoded_key, 1);
    assert_eq!(decoded_err, 0);
    assert_eq!(decoded_body_len as usize, body.len());
    assert_eq!(&packet[8..], &body);
}

#[test]
fn round_trip_all_req_keys() {
    let cases: Vec<(ClientMessage, EKey)> = vec![
        (ClientMessage::ReqLogin(pb::ReqLogin::default()), EKey::ReqLogin),
        (ClientMessage::ReqCreateRole(pb::ReqCreateRole::default()), EKey::ReqCreateRole),
        (ClientMessage::ReqLoginRole(pb::ReqLoginRole::default()), EKey::ReqLoginRole),
        (ClientMessage::ReqPing(pb::ReqPing::default()), EKey::ReqPing),
        (ClientMessage::ReqEnterZone(pb::ReqEnterZone::default()), EKey::ReqEnterZone),
        (ClientMessage::ReqMove(pb::ReqMove::default()), EKey::ReqMove),
    ];
    for (msg, expected_key) in cases {
        let (key, body) = encode_client_message(&msg);
        assert_eq!(key, expected_key, "key mismatch for {:?}", msg);
        let packet = PacketCodec::encode(key.as_u16(), &body);
        let (dk, derr, dbody) = PacketCodec::decode(&packet).unwrap();
        assert_eq!(dk, expected_key.as_u16());
        assert_eq!(derr, 0);
        assert_eq!(dbody, &body);
    }
}

#[test]
fn decode_rsp_login_empty_roles() {
    use prost::Message;
    let rsp = pb::RspLogin {
        err: 0,
        fast: false,
        roles: vec![],
        account: "test".into(),
        server_time: 1234567890,
        ..Default::default()
    };
    let body = rsp.encode_to_vec();
    let result = decode_server_message(EKey::RspLogin, &body);
    assert!(result.is_ok());
    match result.unwrap() {
        netcore::ServerMessage::RspLogin(decoded) => {
            assert_eq!(decoded.err, 0);
            assert_eq!(decoded.account, "test");
            assert_eq!(decoded.server_time, 1234567890);
            assert!(decoded.roles.is_empty());
        }
        other => panic!("unexpected variant: {:?}", other),
    }
}

#[test]
fn decode_rsp_login_with_roles() {
    use prost::Message;
    let rsp = pb::RspLogin {
        err: 0,
        roles: vec![
            pb::RoleSummaryData {
                id: 42,
                cid: 1,
                lv: 10,
                name: "hero".into(),
                icon: 100,
                ..Default::default()
            },
        ],
        account: "player1".into(),
        ..Default::default()
    };
    let body = rsp.encode_to_vec();
    let result = decode_server_message(EKey::RspLogin, &body).unwrap();
    match result {
        netcore::ServerMessage::RspLogin(decoded) => {
            assert_eq!(decoded.roles.len(), 1);
            assert_eq!(decoded.roles[0].id, 42);
            assert_eq!(decoded.roles[0].name, "hero");
        }
        other => panic!("unexpected variant: {:?}", other),
    }
}

#[test]
fn unknown_key_returns_error() {
    let result = decode_server_message(EKey::ReqLogin, &[]);
    assert!(result.is_err(), "Req keys should not have a server decoder");
}

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
    // Server sends 4-byte error packets: [key LE][err LE] with err != 0
    let pkt = vec![101u8, 0, 1, 0]; // key=101 (RspPing), err=1
    let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
    assert_eq!(key, 101);
    assert_eq!(err, 1);
    assert!(body.is_empty());
}

#[test]
fn ekey_round_trip_all_values() {
    let all_keys: Vec<u16> = vec![
        1, 2, 3, 4, 5, 6, 100, 101, 102, 103, 200, 201, 300, 301,
        10000, 10001, 10002, 10003, 33005,
    ];
    for v in all_keys {
        let key = EKey::from_u16(v).unwrap_or_else(|| panic!("EKey::from_u16({}) returned None", v));
        assert_eq!(key.as_u16(), v);
    }
    assert!(EKey::from_u16(0).is_none());
    assert!(EKey::from_u16(9999).is_none());
    assert!(EKey::from_u16(65535).is_none());
}
