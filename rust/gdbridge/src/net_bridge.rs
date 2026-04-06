use godot::prelude::*;

use gnet::event::NetEvent;
use gnet::session::ConnectionState;
use gnet::NetClient;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct NetClientBridge {
    base: Base<Node>,
    pub(crate) client: NetClient,
}

#[godot_api]
impl INode for NetClientBridge {
    fn init(base: Base<Node>) -> Self {
        gnet::init_logging();
        Self {
            base,
            client: NetClient::new(),
        }
    }
}

#[godot_api]
impl NetClientBridge {
    #[signal]
    fn net_connected();

    #[signal]
    fn net_disconnected(reason: GString);

    #[signal]
    fn net_message(key: i64, body: PackedByteArray);

    #[signal]
    fn net_error(key: i64, err_code: i64);

    #[func]
    fn connect_to_server(&mut self, host: GString, port: i64, path: GString) {
        self.client
            .connect(&host.to_string(), port as u16, &path.to_string());
    }

    #[func]
    fn disconnect_from_server(&mut self) {
        self.client.disconnect();
    }

    #[func]
    fn set_reconnect(&mut self, enabled: bool, interval_secs: f64, max_retries: i64) {
        self.client
            .set_reconnect(enabled, interval_secs, max_retries as u32);
    }

    #[func]
    fn get_connection_state(&self) -> GString {
        match self.client.connection_state() {
            ConnectionState::Disconnected => "disconnected".into(),
            ConnectionState::Connecting   => "connecting".into(),
            ConnectionState::Connected    => "connected".into(),
        }
    }

    #[func]
    fn is_connected(&self) -> bool {
        self.client.connection_state() == ConnectionState::Connected
    }

    #[func]
    fn send_raw(&mut self, data: PackedByteArray) {
        self.client.send_raw(data.to_vec());
    }

    /// Called every frame from GDScript _process.
    /// Polls transport events and emits typed signals instead of returning Dictionary.
    #[func]
    fn process_network(&mut self) {
        let events = self.client.poll_events();
        for event in events {
            match event {
                NetEvent::Connected => {
                    self.base_mut()
                        .emit_signal("net_connected", &[]);
                }
                NetEvent::Disconnected { reason } => {
                    self.base_mut()
                        .emit_signal("net_disconnected", &[
                            GString::from(reason).to_variant(),
                        ]);
                }
                NetEvent::Message { key, body } => {
                    let mut byte_arr = PackedByteArray::new();
                    byte_arr.extend(body);
                    self.base_mut()
                        .emit_signal("net_message", &[
                            (key as i64).to_variant(),
                            byte_arr.to_variant(),
                        ]);
                }
                NetEvent::Error { key, err_code } => {
                    self.base_mut()
                        .emit_signal("net_error", &[
                            (key as i64).to_variant(),
                            (err_code as i64).to_variant(),
                        ]);
                }
            }
        }
    }
}
