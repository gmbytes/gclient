use gnet::codec::PacketCodec;

#[test]
fn codec_encode_produces_6byte_header() {
    let body = b"test_body";
    let pkt = PacketCodec::encode(0x1234u16, body);

    assert_eq!(pkt.len(), 6 + body.len());

    let key_decoded = u16::from_le_bytes([pkt[0], pkt[1]]);
    let body_len_decoded = u32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]);

    assert_eq!(key_decoded, 0x1234);
    assert_eq!(body_len_decoded as usize, body.len());
    assert_eq!(&pkt[6..], body);
}

#[test]
fn codec_encode_empty_body() {
    let pkt = PacketCodec::encode(1, &[]);
    assert_eq!(pkt.len(), 6);
    let body_len = u32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]);
    assert_eq!(body_len, 0);
}

#[test]
fn codec_decode_server_success_8byte_header() {
    let body = b"hello world";
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&42u16.to_le_bytes());
    pkt.extend_from_slice(&0u16.to_le_bytes());
    pkt.extend_from_slice(&(body.len() as u32).to_le_bytes());
    pkt.extend_from_slice(body);

    let (key, err, decoded_body) = PacketCodec::decode(&pkt).unwrap();
    assert_eq!(key, 42);
    assert_eq!(err, 0);
    assert_eq!(decoded_body, body);
}

#[test]
fn codec_reject_corrupt_packet() {
    let short = [0u8; 3];
    assert!(PacketCodec::decode(&short).is_err());

    let mut pkt = Vec::new();
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.extend_from_slice(&0u16.to_le_bytes());
    pkt.extend_from_slice(&0xFFFFu32.to_le_bytes());
    pkt.extend_from_slice(b"hello");
    assert!(PacketCodec::decode(&pkt).is_err());
}

#[test]
fn codec_4byte_error_packet() {
    let pkt = vec![101u8, 0, 1, 0];
    let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
    assert_eq!(key, 101);
    assert_eq!(err, 1);
    assert!(body.is_empty());
}
