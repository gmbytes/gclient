use prost_reflect::DynamicMessage;

/// All network events produced by the dispatch pipeline.
///
/// Design:
///   - Framework variants (Connected / Disconnected / ConnectError) carry only metadata.
///   - ProtocolEvent is the primary channel: carries a compiled, strongly-typed ServerMessage.
///   - HotfixEvent is the secondary / fallback channel: carries a dynamically-decoded
///     DynamicMessage when the runtime descriptor differs from the compiled fingerprint.
///   - RawMessage is the last-resort fallback for completely unknown keys or decode failures.
///
/// GDScript consumers: convert ProtocolEvent via `server_message_to_event` (godot_bridge_gen)
///   to get a `NetEventGd` with a typed `Gd<XxxGd>` payload.
#[derive(Clone, Debug)]
pub enum NetEvent {
    /// TCP/WebSocket connection established.
    Connected,

    /// Connection closed (graceful or transport-level).
    Disconnected { reason: String },

    /// Connection attempt failed.
    ConnectError { message: String },

    /// Primary channel: a compiled, strongly-typed server message.
    /// `event_name` is the snake_case name from the manifest (e.g. "rsp_login").
    ProtocolEvent {
        event_name: String,
        key: u16,
        err: u16,
        msg: Box<crate::typed_protocol::ServerMessage>,
    },

    /// Fallback channel: runtime descriptor differs from compiled fingerprint.
    /// Consumed by gdbridge as a Dictionary-based fallback; not the primary API.
    HotfixEvent {
        event_name: String,
        key: u16,
        err: u16,
        fields: DynamicMessage,
    },

    /// Last-resort fallback: completely unknown key or decode failure.
    RawMessage {
        key: u16,
        err: u16,
        body: Vec<u8>,
    },
}
