extends Node

var _bridge: Node  # NetClientBridge (Rust GDExtension)
var account_to_login: String = "test_user"
var auto_reconnect_enabled: bool = true
var auto_reconnect_interval_secs: float = 3.0
var auto_reconnect_max_retries: int = 5

var _status_text: String = "Disconnected"

func _ready():
	print("[Net] NetManager._ready() start")
	var ext_path = "res://addons/gdbridge/gdbridge.gdextension"
	print("[Net] gdextension exists=%s" % FileAccess.file_exists(ext_path))
	var load_result = GDExtensionManager.load_extension(ext_path)
	print("[Net] load_extension result=%d (OK=0, ERR_ALREADY_EXISTS=32)" % load_result)
	if load_result != OK and load_result != ERR_ALREADY_EXISTS:
		push_warning("[Net] load_extension returned code=%d, will still try instantiate NetClientBridge (it may already be auto-loaded)" % load_result)

	_bridge = ClassDB.instantiate("NetClientBridge")
	if _bridge == null:
		push_error("[Net] NetClientBridge class not found – is gdbridge library loaded?")
		print("[Net] ERROR: ClassDB.instantiate('NetClientBridge') returned null")
		_status_text = "ERROR: gdbridge not loaded"
		return
	_bridge.name = "Bridge"
	add_child(_bridge)
	_bridge.set_reconnect(
		auto_reconnect_enabled,
		auto_reconnect_interval_secs,
		auto_reconnect_max_retries
	)
	print("[Net] NetClientBridge ready, _bridge=%s" % _bridge)

func _process(_delta):
	if _bridge == null:
		return
	var events = _bridge.poll_events()
	for event in events:
		_handle_event(event)

func connect_to_server(host: String = "127.0.0.1", port: int = 8080):
	print("[Net] connect_to_server(%s, %d) _bridge=%s" % [host, port, _bridge])
	if _bridge == null:
		print("[Net] ERROR: _bridge is null, cannot connect!")
		return
	_status_text = "Connecting..."
	_bridge.connect_to_server(host, port, "/ws")
	print("[Net] _bridge.connect_to_server() called -> ws://%s:%d/ws" % [host, port])

func disconnect_from_server():
	if _bridge == null:
		return
	_bridge.disconnect_from_server()
	_status_text = "Disconnected"

func get_status_text() -> String:
	return _status_text

func _handle_event(event: Dictionary):
	var event_type = event.get("type", "")
	match event_type:
		"connected":
			print("[Net] Connected to server")
			_status_text = "Connected – logging in..."
			_bridge.send_login(account_to_login, "")
		"disconnected":
			var reason = event.get("reason", "unknown")
			print("[Net] Disconnected: ", reason)
			if auto_reconnect_enabled:
				_status_text = "Disconnected (%s), reconnecting..." % reason
			else:
				_status_text = "Disconnected: %s" % reason
		"error":
			var msg = event.get("message", "unknown")
			print("[Net] Error: ", msg)
			if auto_reconnect_enabled:
				_status_text = "Error (%s), reconnecting..." % msg
			else:
				_status_text = "Error: %s" % msg
		"rsp_login":
			_on_login_response(event)
		"rsp_create_role":
			_on_create_role_response(event)
		"rsp_login_role":
			_on_login_role_response(event)
		"pong":
			pass
		"kick_role":
			var kt = event.get("kick_type", 0)
			print("[Net] Kicked: type=", kt)
			_status_text = "Kicked (type=%d)" % kt
		"server_maintain":
			var reboot = event.get("reboot_time", 0)
			var shutdown = event.get("shutdown_time", 0)
			print("[Net] Server maintain: reboot=%d shutdown=%d" % [reboot, shutdown])
			_status_text = "Server maintaining..."
		"dsp_login":
			print("[Net] Login data pushed from server")
		"dsp_enter_zone":
			print("[Net] Enter zone notify")
		"rsp_enter_zone":
			print("[Net] Enter zone response")
		"dsp_move":
			var rid = event.get("role_id", 0)
			var x = event.get("x", 0)
			var y = event.get("y", 0)
			var z = event.get("z", 0)
			print("[Net] Move sync: role=%d pos=(%d,%d,%d)" % [rid, x, y, z])
		"rsp_move":
			pass
		"raw":
			var key = event.get("key", 0)
			print("[Net] Unhandled raw message: key=%d" % key)
		_:
			print("[Net] Unknown event: ", event_type)

func _on_login_response(event: Dictionary):
	var err = event.get("err", -1)
	if err != 0:
		print("[Net] Login failed: err=", err)
		_status_text = "Login failed (err=%d)" % err
		return

	var roles = event.get("roles", [])
	if roles.size() == 0:
		var ts = int(Time.get_unix_time_from_system()) % 1000000
		var rname = "rust_%d" % ts
		_bridge.send_create_role(1, rname)
		print("[Net] No roles, creating: ", rname)
		_status_text = "Creating role..."
		return

	var role_id = roles[0].get("id", 0)
	_bridge.send_login_role(role_id)
	print("[Net] Logging in role: ", role_id)
	_status_text = "Logging in role %d..." % role_id

func _on_create_role_response(event: Dictionary):
	var err = event.get("err", -1)
	if err != 0:
		print("[Net] Create role failed: err=", err)
		_status_text = "Create role failed (err=%d)" % err
		return

	var role = event.get("role", {})
	var role_id = role.get("id", 0)
	if role_id > 0:
		_bridge.send_login_role(role_id)
		print("[Net] Created role, logging in: ", role_id)
		_status_text = "Logging in role %d..." % role_id

func _on_login_role_response(event: Dictionary):
	var err = event.get("err", -1)
	if err != 0:
		print("[Net] Login role failed: err=", err)
		_status_text = "Login role failed (err=%d)" % err
		return

	print("[Net] Login role success – starting heartbeat")
	_status_text = "In Game"
	_bridge.start_heartbeat(5.0)
