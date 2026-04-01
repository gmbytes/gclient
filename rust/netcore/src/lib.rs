pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/pb.rs"));
}

#[allow(unused_imports)]
pub mod cmd_ext {
    include!(concat!(env!("OUT_DIR"), "/cmd_ext.rs"));
}

pub mod codec;
pub mod event;
pub mod session;
pub mod transport;
pub mod dispatcher;
pub mod protocol_registry;
pub mod client;

pub use cmd_ext::{
    ClientMessage, EKey, ServerMessage, decode_server_message, encode_client_message,
};
pub use codec::PacketCodec;
pub use event::NetEvent;
pub use protocol_registry::ProtocolRegistry;
pub use client::NetClient;

pub fn init_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();
}
