extends NetHandlerBase

const _cmd = preload("res://src/game/pb/cmd.gd")
const pb_req = preload("res://src/game/pb/cmd_req.gd")
const pb_rsp = preload("res://src/game/pb/cmd_rsp.gd")
const pb_dsp = preload("res://src/game/pb/cmd_dsp.gd")
const pb_cmd = _cmd.EKey.T

func _on_connected(_msg) -> void:
	print("[Net/Session] Connected to server")
	var m = net_manager()
	if m:
		m.start_heartbeat(5.0)
	var req := pb_req.ReqLogin.new()
	var account: String = "test_user"
	if m != null and m.account_to_login.strip_edges() != "":
		account = m.account_to_login.strip_edges()
	req.set_account(account)
	req.set_token("")
	req.set_version("1.0.0")
	send_msg(pb_cmd.ReqLogin, req)

func _on_rsp_ping(_msg: pb_rsp.RspPing) -> void:
	print("[Net/Session] Pong received")


func _on_dsp_kick_role(msg: pb_dsp.DspKickRole) -> void:
	var ty: int = 0
	if msg:
		ty = int(msg.get_ty())
	print("[Net/Session] Kicked: type=", ty)
	var m = net_manager()
	if m:
		m.stop_heartbeat()


func _on_dsp_srv_maintain(msg: pb_dsp.DspSrvMaintain) -> void:
	var reboot: int = 0
	var shutdown: int = 0
	if msg:
		reboot = int(msg.get_reboot_time())
		shutdown = int(msg.get_shutdown_time())
	print("[Net/Session] Server maintain: reboot=%s shutdown=%s" % [str(reboot), str(shutdown)])


func _on_dsp_login(msg: pb_dsp.DspLogin) -> void:
	print("[Net/Session] Login data pushed from server: ", msg)


func _on_dsp_enter_zone(_msg: pb_dsp.DspEnterZone) -> void:
	print("[Net/Session] Enter zone notify")


func _on_rsp_enter_zone(_msg: pb_rsp.RspEnterZone) -> void:
	print("[Net/Session] Enter zone response")


func _on_dsp_move(msg: pb_dsp.DspMove) -> void:
	var rid: int = 0
	var x: int = 0
	var y: int = 0
	var z: int = 0
	if msg:
		rid = int(msg.get_role_id())
		var pos = msg.get_pos()
		if pos:
			x = int(pos.get_x())
			y = int(pos.get_y())
			z = int(pos.get_z())
	print("[Net/Session] Move sync: role=%d pos=(%d,%d,%d)" % [rid, x, y, z])


func _on_rsp_move(_msg: pb_rsp.RspMove) -> void:
	pass


func _on_rsp_login(msg: pb_rsp.RspLogin) -> void:
	if not msg:
		print("[Net/Session] Login response missing data")
		return
	var err: int = int(msg.get_err())
	if err != 0:
		print("[Net/Session] Login failed: err=", err)
		return

	var roles: Array = msg.get_roles()
	if roles.size() == 0:
		var ts: int = int(Time.get_unix_time_from_system()) % 1000000
		var rname: String = "gd_%d" % ts
		var req := pb_req.ReqCreateRole.new()
		req.set_cid(1)
		req.set_name(rname)
		send_msg(pb_cmd.ReqCreateRole, req)
		print("[Net/Session] No roles, creating: ", rname)
		return

	var first = roles[0]
	var role_id: int = 0
	if first:
		role_id = int(first.get_id())
	var req := pb_req.ReqLoginRole.new()
	req.set_role_id(role_id)
	send_msg(pb_cmd.ReqLoginRole, req)
	print("[Net/Session] Logging in role: ", role_id)


func _on_rsp_create_role(msg: pb_rsp.RspCreateRole) -> void:
	if not msg:
		print("[Net/Session] Create role response missing data")
		return
	var err: int = int(msg.get_err())
	if err != 0:
		print("[Net/Session] Create role failed: err=", err)
		return

	var role = msg.get_role()
	var role_id: int = 0
	if role:
		role_id = int(role.get_id())
	if role_id > 0:
		var req := pb_req.ReqLoginRole.new()
		req.set_role_id(role_id)
		send_msg(pb_cmd.ReqLoginRole, req)
		print("[Net/Session] Created role, logging in: ", role_id)


func _on_rsp_login_role(msg: pb_rsp.RspLoginRole) -> void:
	if not msg:
		print("[Net/Session] Login role response missing data")
		return
	var err: int = int(msg.get_err())
	if err != 0:
		print("[Net/Session] Login role failed: err=", err)
		return

	print("[Net/Session] Login role success")
