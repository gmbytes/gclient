#[derive(Clone, Debug)]
pub struct RoleInfo {
    pub id: i64,
    pub cid: i64,
    pub lv: i64,
    pub name: String,
    pub icon: i64,
}

#[derive(Clone, Debug)]
pub enum NetEvent {
    Connected,
    Disconnected {
        reason: String,
    },
    ConnectError {
        message: String,
    },
    LoginResponse {
        err: i32,
        fast: bool,
        roles: Vec<RoleInfo>,
        account: String,
        server_time: i64,
    },
    CreateRoleResponse {
        err: i32,
        role: Option<RoleInfo>,
    },
    LoginRoleResponse {
        err: i32,
        regain: bool,
    },
    Pong,
    PongZzz,
    LoginData {
        regain: bool,
    },
    KickRole {
        kick_type: i32,
    },
    ServerMaintain {
        reboot_time: i64,
        shutdown_time: i64,
    },
    EnterZoneResponse,
    MoveResponse {
        role_id: i64,
        x: i64,
        y: i64,
        z: i64,
    },
    MoveSync {
        role_id: i64,
        x: i64,
        y: i64,
        z: i64,
    },
    EnterZoneNotify,
    RawMessage {
        key: u16,
        err: u16,
        body: Vec<u8>,
    },
}
