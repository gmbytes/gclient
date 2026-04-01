use crate::cmd_ext::{self, EKey, ServerMessage};
use crate::codec::PacketCodec;
use crate::event::{NetEvent, RoleInfo};
use crate::protocol_registry::ProtocolRegistry;

/// Dual-channel dispatch: compiled channel first, generic fallback second.
pub fn dispatch(raw: &[u8], registry: &mut ProtocolRegistry) -> NetEvent {
    let (key_u16, err_u16, body) = match PacketCodec::decode(raw) {
        Ok(v) => v,
        Err(e) => {
            return NetEvent::RawMessage {
                key: 0,
                err: 0,
                body: format!("codec error: {}", e).into_bytes(),
            };
        }
    };

    // --- Compiled channel: try strong-typed decode first ---
    if let Some(key) = EKey::from_u16(key_u16) {
        if err_u16 != 0 {
            return convert_error_response(key, err_u16);
        }
        if let Ok(msg) = cmd_ext::decode_server_message(key, body) {
            return convert_server_message(msg);
        }
    }

    // --- Generic channel: fall back to prost-reflect dynamic decode ---
    if let Some(meta) = registry.get(key_u16).cloned() {
        if err_u16 != 0 {
            return NetEvent::RawMessage {
                key: key_u16,
                err: err_u16,
                body: vec![],
            };
        }
        match registry.decode_generic(&meta.message, body) {
            Ok(dynamic_msg) => NetEvent::GenericMessage {
                event_name: meta.event_name,
                key: key_u16,
                err: err_u16,
                fields: dynamic_msg,
            },
            Err(_) => NetEvent::RawMessage {
                key: key_u16,
                err: err_u16,
                body: body.to_vec(),
            },
        }
    } else {
        NetEvent::RawMessage {
            key: key_u16,
            err: err_u16,
            body: body.to_vec(),
        }
    }
}

fn convert_error_response(key: EKey, err: u16) -> NetEvent {
    let err_i32 = err as i32;
    match key {
        EKey::RspLogin => NetEvent::LoginResponse {
            err: err_i32,
            fast: false,
            roles: vec![],
            account: String::new(),
            server_time: 0,
        },
        EKey::RspCreateRole => NetEvent::CreateRoleResponse {
            err: err_i32,
            role: None,
        },
        EKey::RspLoginRole => NetEvent::LoginRoleResponse {
            err: err_i32,
            regain: false,
        },
        EKey::RspPing => NetEvent::Pong,
        EKey::RspPingZzz => NetEvent::PongZzz,
        _ => NetEvent::RawMessage {
            key: key.as_u16(),
            err,
            body: vec![],
        },
    }
}

fn convert_server_message(msg: ServerMessage) -> NetEvent {
    match msg {
        ServerMessage::RspLogin(rsp) => {
            let roles = rsp
                .roles
                .iter()
                .map(|r| RoleInfo {
                    id: r.id,
                    cid: r.cid,
                    lv: r.lv,
                    name: r.name.clone(),
                    icon: r.icon,
                })
                .collect();
            NetEvent::LoginResponse {
                err: rsp.err,
                fast: rsp.fast,
                roles,
                account: rsp.account,
                server_time: rsp.server_time,
            }
        }
        ServerMessage::RspCreateRole(rsp) => {
            let role = rsp.role.map(|r| RoleInfo {
                id: r.id,
                cid: r.cid,
                lv: r.lv,
                name: r.name,
                icon: r.icon,
            });
            NetEvent::CreateRoleResponse {
                err: rsp.err,
                role,
            }
        }
        ServerMessage::RspLoginRole(rsp) => {
            let regain = rsp.data.as_ref().map_or(false, |d| d.regain);
            NetEvent::LoginRoleResponse {
                err: rsp.err,
                regain,
            }
        }
        ServerMessage::RspPing(_) => NetEvent::Pong,
        ServerMessage::RspPingZzz(_) => NetEvent::PongZzz,
        ServerMessage::RspEnterZone(_) => NetEvent::EnterZoneResponse,
        ServerMessage::RspMove(rsp) => {
            let (x, y, z) = rsp
                .pos
                .as_ref()
                .map_or((0, 0, 0), |p| (p.x, p.y, p.z));
            NetEvent::MoveResponse {
                role_id: rsp.role_id,
                x,
                y,
                z,
            }
        }
        ServerMessage::DspLogin(dsp) => {
            let regain = dsp.data.as_ref().map_or(false, |d| d.regain);
            NetEvent::LoginData { regain }
        }
        ServerMessage::DspSrvMaintain(dsp) => NetEvent::ServerMaintain {
            reboot_time: dsp.reboot_time,
            shutdown_time: dsp.shutdown_time,
        },
        ServerMessage::DspKickRole(dsp) => NetEvent::KickRole {
            kick_type: dsp.ty,
        },
        ServerMessage::DspEnterZone(_) => NetEvent::EnterZoneNotify,
        ServerMessage::DspMove(dsp) => {
            let (x, y, z) = dsp
                .pos
                .as_ref()
                .map_or((0, 0, 0), |p| (p.x, p.y, p.z));
            NetEvent::MoveSync {
                role_id: dsp.role_id,
                x,
                y,
                z,
            }
        }
    }
}
