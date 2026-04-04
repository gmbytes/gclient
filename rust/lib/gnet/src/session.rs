#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct Session {
    pub state: ConnectionState,
}

impl Session {
    pub fn new() -> Self {
        Self { state: ConnectionState::Disconnected }
    }

    pub fn reset(&mut self) {
        self.state = ConnectionState::Disconnected;
    }

    pub fn on_connecting(&mut self) {
        self.state = ConnectionState::Connecting;
    }

    pub fn on_connected(&mut self) {
        self.state = ConnectionState::Connected;
    }

    pub fn on_disconnected(&mut self) {
        self.state = ConnectionState::Disconnected;
    }
}
