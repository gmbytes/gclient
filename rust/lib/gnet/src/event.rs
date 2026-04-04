/// All network events produced by the dispatch pipeline.
#[derive(Clone, Debug)]
pub enum NetEvent {
    Connected,
    Disconnected { reason: String },
    ConnectError { message: String },

    /// Primary channel: a compiled, strongly-typed server message.
    ProtocolEvent {
        event_name: String,
        key: u16,
        err: u16,
        msg: Box<crate::typed_protocol::ServerMessage>,
    },

    /// Unknown key or decode failure.
    RawMessage {
        key: u16,
        err: u16,
        body: Vec<u8>,
    },
}
