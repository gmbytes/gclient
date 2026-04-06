use std::fmt;

const REQ_HEADER_SIZE: usize = 6;
const RSP_HEADER_SIZE: usize = 8;

#[derive(Debug)]
pub enum CodecError {
    PacketTooShort { got: usize, expected: usize },
    BodyOverflow { declared: u32, available: usize },
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::PacketTooShort { got, expected } => {
                write!(f, "packet too short: {} < {}", got, expected)
            }
            CodecError::BodyOverflow {
                declared,
                available,
            } => write!(
                f,
                "body overflow: declared {} but only {} available",
                declared, available
            ),
        }
    }
}

impl std::error::Error for CodecError {}

/// Binary packet codec for the asymmetric wire protocol:
///
/// Client request (encode): `[2B key LE] [4B body_len LE] [body]`  (6-byte header)
/// Server response (decode): `[2B key LE] [2B err LE] [4B body_len LE] [body]`  (8-byte header, err=0)
/// Server error   (decode): `[2B key LE] [2B err LE]`  (4 bytes, err!=0)
pub struct PacketCodec;

impl PacketCodec {
    /// Encode a client request: `[2B key LE] [4B body_len LE] [body]`
    pub fn encode(key: u16, body: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(REQ_HEADER_SIZE + body.len());
        packet.extend_from_slice(&key.to_le_bytes());
        packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
        packet.extend_from_slice(body);
        packet
    }

    /// Decode a server response: returns `(key, err, body)`.
    /// Handles both 8-byte success packets and 4-byte error packets.
    pub fn decode(data: &[u8]) -> Result<(u16, u16, &[u8]), CodecError> {
        if data.len() < 4 {
            return Err(CodecError::PacketTooShort { got: data.len(), expected: 4 });
        }
        let key = u16::from_le_bytes([data[0], data[1]]);
        let err = u16::from_le_bytes([data[2], data[3]]);
        if err != 0 || data.len() == 4 {
            return Ok((key, err, &[]));
        }
        if data.len() < RSP_HEADER_SIZE {
            return Err(CodecError::PacketTooShort { got: data.len(), expected: RSP_HEADER_SIZE });
        }
        let body_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let available = data.len() - RSP_HEADER_SIZE;
        if (body_len as usize) > available {
            return Err(CodecError::BodyOverflow {
                declared: body_len,
                available,
            });
        }
        Ok((key, err, &data[RSP_HEADER_SIZE..RSP_HEADER_SIZE + body_len as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_6byte_header() {
        let body = b"hello";
        let encoded = PacketCodec::encode(42, body);
        assert_eq!(encoded.len(), 6 + body.len());
        let key = u16::from_le_bytes([encoded[0], encoded[1]]);
        let body_len = u32::from_le_bytes([encoded[2], encoded[3], encoded[4], encoded[5]]);
        assert_eq!(key, 42);
        assert_eq!(body_len as usize, body.len());
        assert_eq!(&encoded[6..], body);
    }

    #[test]
    fn encode_empty_body() {
        let encoded = PacketCodec::encode(1, &[]);
        assert_eq!(encoded.len(), 6);
        let key = u16::from_le_bytes([encoded[0], encoded[1]]);
        let body_len = u32::from_le_bytes([encoded[2], encoded[3], encoded[4], encoded[5]]);
        assert_eq!(key, 1);
        assert_eq!(body_len, 0);
    }

    #[test]
    fn decode_server_success() {
        let body = b"hello";
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
    fn decode_too_short() {
        assert!(PacketCodec::decode(&[0u8; 3]).is_err());
    }

    #[test]
    fn decode_error_packet_4bytes() {
        let mut pkt = vec![0u8; 4];
        pkt[0] = 101;
        pkt[1] = 0;
        pkt[2] = 1; // err != 0
        pkt[3] = 0;
        let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
        assert_eq!(key, 101);
        assert_eq!(err, 1);
        assert!(body.is_empty());
    }

    #[test]
    fn decode_body_overflow() {
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&1u16.to_le_bytes());
        pkt.extend_from_slice(&0u16.to_le_bytes());
        pkt.extend_from_slice(&0xFFFFu32.to_le_bytes());
        pkt.extend_from_slice(b"hi");
        assert!(PacketCodec::decode(&pkt).is_err());
    }
}
