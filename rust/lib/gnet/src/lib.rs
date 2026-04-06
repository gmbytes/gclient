pub mod codec;
pub mod event;
pub mod session;
pub mod transport;
pub mod client;

pub use codec::PacketCodec;
pub use event::NetEvent;
pub use client::NetClient;

pub fn init_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();
}
