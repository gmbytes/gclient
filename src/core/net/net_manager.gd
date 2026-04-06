extends Node

const CmdExt = preload("res://src/game/pb/cmd_ext.gd")
const Cmd    = preload("res://src/game/pb/cmd.gd")
const CmdReq = preload("res://src/game/pb/cmd_req.gd")

var _bridge: Node
var _cmd_ext: RefCounted

# method_name -> { "handler": Object }
var _handlers: Dictionary = {}

var _heartbeat_active: bool = false
var _heartbeat_interval: float = 5.0
var _heartbeat_elapsed: float = 0.0

## Filled by UI before connect; session handler reads this for ReqLogin.
var account_to_login: String = "test_user"

## Emitted when server returns a 4-byte error packet; UI connects to show toast.
signal server_error(key: int, key_name: String, err_code: int)

func _ready():
	var ext_path = "res://addons/gdbridge/gdbridge.gdextension"
	var load_result = GDExtensionManager.load_extension(ext_path)
	if load_result != OK and load_result != ERR_ALREADY_EXISTS:
		push_warning("[Net] load_extension returned code=%d" % load_result)

	_bridge = ClassDB.instantiate("NetClientBridge")
	if _bridge == null:
		push_error("[Net] NetClientBridge class not found")
		return
	_bridge.name = "Bridge"
	add_child(_bridge)

	_bridge.net_connected.connect(_on_net_connected)
	_bridge.net_disconnected.connect(_on_net_disconnected)
	_bridge.net_message.connect(_on_net_message)
	_bridge.net_error.connect(_on_net_error)

	_cmd_ext = CmdExt.new()
	_load_handlers()


func _process(delta):
	if _bridge == null:
		return
	if _heartbeat_active:
		_heartbeat_elapsed += delta
		if _heartbeat_elapsed >= _heartbeat_interval:
			_heartbeat_elapsed = 0.0
			_send_heartbeat()
	_bridge.process_network()

# ── Public API ──

func connect_to_server(host: String = "127.0.0.1", port: int = 8080):
	if _bridge == null:
		return
	_bridge.connect_to_server(host, port, "/ws")


func disconnect_from_server():
	if _bridge == null:
		return
	stop_heartbeat()
	_bridge.disconnect_from_server()


func get_bridge() -> Node:
	return _bridge


func get_status_text() -> String:
	if _bridge == null:
		return "offline (no bridge)"
	var st: String = str(_bridge.get_connection_state())
	match st:
		"disconnected":
			return "Disconnected"
		"connecting":
			return "Connecting…"
		"connected":
			return "Connected"
	return st


func send_msg(key: int, msg) -> void:
	if _bridge == null:
		return
	var data: PackedByteArray = CmdExt.marshal_request(key, msg)
	_bridge.send_raw(data)


func start_heartbeat(interval: float = 5.0) -> void:
	_heartbeat_interval = maxf(interval, 1.0)
	_heartbeat_active = true
	_heartbeat_elapsed = 0.0
	print("[Net] Heartbeat started interval=%.1fs" % _heartbeat_interval)


func stop_heartbeat() -> void:
	if _heartbeat_active:
		_heartbeat_active = false
		print("[Net] Heartbeat stopped")

# ── Handler loading ──

func _load_handlers():
	_handlers.clear()
	var dir = DirAccess.open("res://src/core/net/handlers/")
	if dir == null:
		return
	var fnames: Array[String] = []
	dir.list_dir_begin()
	var fname = dir.get_next()
	while fname != "":
		if fname.ends_with(".gd"):
			fnames.append(fname)
		fname = dir.get_next()
	dir.list_dir_end()
	fnames.sort()

	for f in fnames:
		var script: Script = ResourceLoader.load("res://src/core/net/handlers/" + f) as Script
		if script == null:
			continue
		var handler = script.new()
		if handler.has_method("bind_manager"):
			handler.bind_manager(self)
		if not script.has_method("get_script_method_list"):
			continue
		for method_info in script.get_script_method_list():
			var mname: String = str(method_info.get("name", ""))
			if not mname.begins_with("_on_"):
				continue
			if _handlers.has(mname):
				push_warning("[Net] skip duplicate handler %s from %s" % [mname, f])
				continue
			_handlers[mname] = {"handler": handler}

# ── Signal callbacks ──

func _on_net_connected() -> void:
	_dispatch("_on_connected")


func _on_net_disconnected(reason: String) -> void:
	print("[Net] Disconnected: %s" % reason)
	_dispatch("_on_disconnected")


func _on_net_message(key: int, body: PackedByteArray) -> void:
	var msg = _cmd_ext.unmarshal(key, body)
	var kname: String = _cmd_ext.key_name(key)
	if kname.is_empty():
		print("[Net] Unknown key: %d" % key)
		return
	_dispatch("_on_" + kname, msg)


func _on_net_error(key: int, err_code: int) -> void:
	var kname: String = _cmd_ext.key_name(key)
	if kname.is_empty():
		kname = str(key)
	print("[Net] Server error: %s err_code=%d" % [kname, err_code])
	server_error.emit(key, kname, err_code)


func _dispatch(method_name: String, msg: Variant = null) -> void:
	if not _handlers.has(method_name):
		return
	var handler: Object = _handlers[method_name]["handler"]
	handler.call(method_name, msg)


func _send_heartbeat() -> void:
	var ping = CmdReq.ReqPing.new()
	send_msg(Cmd.EKey.T.ReqPing, ping)
