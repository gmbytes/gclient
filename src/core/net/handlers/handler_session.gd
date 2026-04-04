extends NetHandlerBase

## Login / role / move session logic.
## Strong-typed first param: RspXxxGd / DspXxxGd, auto-passed via get_data().

func _on_connected(_event: NetEventGd) -> void:
	print("[Net/Session] Connected to server")
	var b = bridge()
	if b:
		b.send_login("test_user", "", "1.0.0")


func _on_rsp_ping() -> void:
	pass


func _on_dsp_kick_role(msg: DspKickRoleGd, _event: NetEventGd) -> void:
	var ty: int = 0
	if msg:
		ty = int(msg.ty)
	print("[Net/Session] Kicked: type=", ty)
	var b = bridge()
	if b:
		b.stop_heartbeat()


func _on_dsp_srv_maintain(msg: DspSrvMaintainGd, _event: NetEventGd) -> void:
	var reboot: int = 0
	var shutdown: int = 0
	if msg:
		reboot = int(msg.reboot_time)
		shutdown = int(msg.shutdown_time)
	print("[Net/Session] Server maintain: reboot=%s shutdown=%s" % [str(reboot), str(shutdown)])


func _on_dsp_login(_event: NetEventGd) -> void:
	print("[Net/Session] Login data pushed from server")


func _on_dsp_enter_zone(_event: NetEventGd) -> void:
	print("[Net/Session] Enter zone notify")


func _on_rsp_enter_zone(_event: NetEventGd) -> void:
	print("[Net/Session] Enter zone response")


func _on_dsp_move(msg: DspMoveGd, _event: NetEventGd) -> void:
	var rid: int = 0
	var x: int = 0
	var y: int = 0
	var z: int = 0
	if msg:
		rid = int(msg.role_id)
		var pos = msg.pos
		if pos:
			x = int(pos.x)
			y = int(pos.y)
			z = int(pos.z)
	print("[Net/Session] Move sync: role=%d pos=(%d,%d,%d)" % [rid, x, y, z])


func _on_rsp_move() -> void:
	pass


func _on_rsp_login(msg: RspLoginGd, _event: NetEventGd) -> void:
	if not msg:
		print("[Net/Session] Login response missing data")
		return
	var err: int = int(msg.err)
	if err != 0:
		print("[Net/Session] Login failed: err=", err)
		return

	var roles: Array = msg.roles
	if roles.size() == 0:
		var ts: int = int(Time.get_unix_time_from_system()) % 1000000
		var rname: String = "rust_%d" % ts
		var b = bridge()
		if b:
			b.send_create_role(1, rname)
		print("[Net/Session] No roles, creating: ", rname)
		return

	var first = roles[0]
	var role_id: int = 0
	if first is Object:
		role_id = int(first.get("id"))
	var b2 = bridge()
	if b2:
		b2.send_login_role(role_id)
	print("[Net/Session] Logging in role: ", role_id)


func _on_rsp_create_role(msg: RspCreateRoleGd, _event: NetEventGd) -> void:
	if not msg:
		print("[Net/Session] Create role response missing data")
		return
	var err: int = int(msg.err)
	if err != 0:
		print("[Net/Session] Create role failed: err=", err)
		return

	var role = msg.role
	var role_id: int = 0
	if role != null and role is Object:
		role_id = int(role.get("id"))
	if role_id > 0:
		var b = bridge()
		if b:
			b.send_login_role(role_id)
		print("[Net/Session] Created role, logging in: ", role_id)


func _on_rsp_login_role(msg: RspLoginRoleGd, _event: NetEventGd) -> void:
	if not msg:
		print("[Net/Session] Login role response missing data")
		return
	var err: int = int(msg.err)
	if err != 0:
		print("[Net/Session] Login role failed: err=", err)
		return

	print("[Net/Session] Login role success – starting heartbeat")
	var b = bridge()
	if b:
		b.start_heartbeat(5.0)
