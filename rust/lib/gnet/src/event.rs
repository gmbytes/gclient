#[derive(Clone, Debug)]
pub enum NetEvent {
    Connected,
    Disconnected { reason: String },
    /// Success packet: key + raw protobuf body bytes (no deserialization).
    Message { key: u16, body: Vec<u8> },
    /// Error packet: key + err_code (no body).
    Error { key: u16, err_code: u16 },
}
