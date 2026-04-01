extends Node

var _bridge: Node  # NetClientBridge (Rust GDExtension)
var account_to_login: String = "test_user"
var auto_reconnect_enabled: bool = true
var auto_reconnect_interval_secs: float = 3.0
var auto_reconnect_max_retries: int = 5

var _status_text: String = "Disconnected"

# Dynamic handler registry: method_name -> handler object
# Populated by _load_handlers() from res://src/core/net/handlers/*.gd
var _handlers: Dictionary = {}

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
	_load_handlers()

func _process(_delta):
	if _bridge == null:
		return
	var events = _bridge.poll_events()
	for event in events:
		_handle_event(event)

# ── Public API ──

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

# ── Handler loading ──

## Scan handlers/ directory and register all _on_xxx methods from each script.
## Called once at startup; call again after loading a hot-patch PCK to pick up new handlers.
func _load_handlers():
	var dir = DirAccess.open("res://src/core/net/handlers/")
	if dir == null:
		return
	dir.list_dir_begin()
	var fname = dir.get_next()
	while fname != "":
		if fname.ends_with(".gd"):
			var script = ResourceLoader.load("res://src/core/net/handlers/" + fname)
			if script:
				var handler = script.new()
				if handler.has_method("bind_manager"):
					handler.bind_manager(self)
				for method_info in handler.get_method_list():
					var mname: String = method_info["name"]
					if mname.begins_with("_on_"):
						_handlers[mname] = handler
						print("[Net] registered handler %s from %s" % [mname, fname])
		fname = dir.get_next()
	dir.list_dir_end()

# ── Central dispatch (stable entry point – do not add protocol logic here) ──

func _handle_event(event: Dictionary):
	var etype: String = event.get("type", "")
	if etype.is_empty():
		return
	var method_name: String = "_on_" + etype
	if _handlers.has(method_name):
		_handlers[method_name].call(method_name, event)
	elif has_method(method_name):
		call(method_name, event)
	else:
		_on_unknown_event(event)

# ── Framework methods (connection lifecycle) ──

func _on_connected(_event: Dictionary):
	print("[Net] Connected to server")
	_status_text = "Connected – logging in..."
	_bridge.send_login(account_to_login, "")

func _on_disconnected(event: Dictionary):
	var reason: String = event.get("reason", "unknown")
	print("[Net] Disconnected: ", reason)
	if auto_reconnect_enabled:
		_status_text = "Disconnected (%s), reconnecting..." % reason
	else:
		_status_text = "Disconnected: %s" % reason

func _on_error(event: Dictionary):
	var msg: String = event.get("message", "unknown")
	print("[Net] Error: ", msg)
	if auto_reconnect_enabled:
		_status_text = "Error (%s), reconnecting..." % msg
	else:
		_status_text = "Error: %s" % msg

func _on_raw(event: Dictionary):
	var key: int = event.get("key", 0)
	var err: int = event.get("err", 0)
	var body = event.get("body", PackedByteArray())
	print("[Net] Raw message: key=%d err=%d body_len=%d" % [key, err, body.size()])

func _on_unknown_event(event: Dictionary):
	var etype: String = event.get("type", "?")
	print("[Net] Unhandled event: ", etype)

# ── Business protocol handlers (auto-dispatched via naming convention) ──

func _on_pong(_event: Dictionary):
	pass

func _on_kick_role(event: Dictionary):
	var kt: int = event.get("kick_type", 0)
	print("[Net] Kicked: type=", kt)
	_status_text = "Kicked (type=%d)" % kt

func _on_server_maintain(event: Dictionary):
	var reboot: int = event.get("reboot_time", 0)
	var shutdown: int = event.get("shutdown_time", 0)
	print("[Net] Server maintain: reboot=%d shutdown=%d" % [reboot, shutdown])
	_status_text = "Server maintaining..."

func _on_dsp_login(_event: Dictionary):
	print("[Net] Login data pushed from server")

func _on_dsp_enter_zone(_event: Dictionary):
	print("[Net] Enter zone notify")

func _on_rsp_enter_zone(_event: Dictionary):
	print("[Net] Enter zone response")

func _on_dsp_move(event: Dictionary):
	var rid: int = event.get("role_id", 0)
	var x: int = event.get("x", 0)
	var y: int = event.get("y", 0)
	var z: int = event.get("z", 0)
	print("[Net] Move sync: role=%d pos=(%d,%d,%d)" % [rid, x, y, z])

func _on_rsp_move(_event: Dictionary):
	pass

func _on_rsp_login(event: Dictionary):
	var err: int = event.get("err", -1)
	if err != 0:
		print("[Net] Login failed: err=", err)
		_status_text = "Login failed (err=%d)" % err
		return

	var roles: Array = event.get("roles", [])
	if roles.size() == 0:
		var ts: int = int(Time.get_unix_time_from_system()) % 1000000
		var rname: String = "rust_%d" % ts
		_bridge.send_create_role(1, rname)
		print("[Net] No roles, creating: ", rname)
		_status_text = "Creating role..."
		return

	var role_id: int = roles[0].get("id", 0)
	_bridge.send_login_role(role_id)
	print("[Net] Logging in role: ", role_id)
	_status_text = "Logging in role %d..." % role_id

func _on_rsp_create_role(event: Dictionary):
	var err: int = event.get("err", -1)
	if err != 0:
		print("[Net] Create role failed: err=", err)
		_status_text = "Create role failed (err=%d)" % err
		return

	var role: Dictionary = event.get("role", {})
	var role_id: int = role.get("id", 0)
	if role_id > 0:
		_bridge.send_login_role(role_id)
		print("[Net] Created role, logging in: ", role_id)
		_status_text = "Logging in role %d..." % role_id

func _on_rsp_login_role(event: Dictionary):
	var err: int = event.get("err", -1)
	if err != 0:
		print("[Net] Login role failed: err=", err)
		_status_text = "Login role failed (err=%d)" % err
		return

	print("[Net] Login role success – starting heartbeat")
	_status_text = "In Game"
	_bridge.start_heartbeat(5.0)
