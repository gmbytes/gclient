#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    LoggingIn,
    InGame,
}

pub struct Session {
    pub state: ConnectionState,
    pub account: String,
    pub role_id: i64,
    pub server_time: i64,
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            account: String::new(),
            role_id: 0,
            server_time: 0,
        }
    }

    pub fn reset(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.account.clear();
        self.role_id = 0;
        self.server_time = 0;
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

    pub fn on_login_sent(&mut self, account: &str) {
        self.state = ConnectionState::LoggingIn;
        self.account = account.to_string();
    }

    pub fn on_login_role_ok(&mut self, role_id: i64) {
        self.state = ConnectionState::InGame;
        self.role_id = role_id;
    }
}
