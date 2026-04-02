use std::fmt;

const HEADER_SIZE: usize = 8;

#[derive(Debug)]
pub enum CodecError {
    PacketTooShort { got: usize },
    BodyOverflow { declared: u32, available: usize },
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::PacketTooShort { got } => {
                write!(f, "packet too short: {} < {}", got, HEADER_SIZE)
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

/// Binary packet codec matching the mserver wire format:
/// Normal:  `[2B key LE] [2B err LE] [4B body_len LE] [body (protobuf)]`
/// Error:   `[2B key LE] [2B err LE]`  (only 4 bytes when err != 0)
pub struct PacketCodec;

impl PacketCodec {
    pub fn encode(key: u16, body: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(HEADER_SIZE + body.len());
        packet.extend_from_slice(&key.to_le_bytes());
        packet.extend_from_slice(&0u16.to_le_bytes());
        packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
        packet.extend_from_slice(body);
        packet
    }

    pub fn decode(data: &[u8]) -> Result<(u16, u16, &[u8]), CodecError> {
        if data.len() < 4 {
            return Err(CodecError::PacketTooShort { got: data.len() });
        }
        let key = u16::from_le_bytes([data[0], data[1]]);
        let err = u16::from_le_bytes([data[2], data[3]]);
        if err != 0 || data.len() == 4 {
            return Ok((key, err, &[]));
        }
        if data.len() < HEADER_SIZE {
            return Err(CodecError::PacketTooShort { got: data.len() });
        }
        let body_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let available = data.len() - HEADER_SIZE;
        if (body_len as usize) > available {
            return Err(CodecError::BodyOverflow {
                declared: body_len,
                available,
            });
        }
        Ok((key, err, &data[HEADER_SIZE..HEADER_SIZE + body_len as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let body = b"hello";
        let encoded = PacketCodec::encode(42, body);
        let (key, err, decoded_body) = PacketCodec::decode(&encoded).unwrap();
        assert_eq!(key, 42);
        assert_eq!(err, 0);
        assert_eq!(decoded_body, body);
    }

    #[test]
    fn empty_body() {
        let encoded = PacketCodec::encode(1, &[]);
        let (key, err, body) = PacketCodec::decode(&encoded).unwrap();
        assert_eq!(key, 1);
        assert_eq!(err, 0);
        assert!(body.is_empty());
    }

    #[test]
    fn too_short() {
        assert!(PacketCodec::decode(&[0u8; 3]).is_err());
    }

    #[test]
    fn error_packet_4bytes() {
        let mut pkt = vec![0u8; 4];
        pkt[0] = 101; // key = RspPing
        pkt[1] = 0;
        pkt[2] = 1; // err != 0
        pkt[3] = 0;
        let (key, err, body) = PacketCodec::decode(&pkt).unwrap();
        assert_eq!(key, 101);
        assert_eq!(err, 1);
        assert!(body.is_empty());
    }

    #[test]
    fn body_overflow() {
        let mut pkt = PacketCodec::encode(1, b"hi");
        // Corrupt body_len to a huge value
        pkt[4] = 0xFF;
        pkt[5] = 0xFF;
        assert!(PacketCodec::decode(&pkt).is_err());
    }
}
