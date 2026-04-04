extends Node

const _HANDLER_MODE_ARGS0 := 0
const _HANDLER_MODE_NET_EVENT := 1
const _HANDLER_MODE_TYPED_PAYLOAD := 2
const _HANDLER_MODE_TYPED_AND_EVENT := 3

var _bridge: Node

# method_name -> { "handler": Object, "mode": int }
var _handlers: Dictionary = {}

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
	_load_handlers()

func _process(_delta):
	if _bridge == null:
		return
	var events = _bridge.poll_events()
	for event: NetEventGd in events:
		_handle_event(event)

# ── Public API ──

func connect_to_server(host: String = "127.0.0.1", port: int = 8080):
	if _bridge == null:
		return
	_bridge.connect_to_server(host, port, "/ws")

func disconnect_from_server():
	if _bridge == null:
		return
	_bridge.disconnect_from_server()

func get_bridge() -> Node:
	return _bridge

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
			var mode: int = _infer_handler_dispatch_mode(method_info)
			_handlers[mname] = {"handler": handler, "mode": mode}


func _infer_handler_dispatch_mode(method_info: Dictionary) -> int:
	var args: Array = method_info.get("args", [])
	var n: int = args.size()
	if n == 0:
		return _HANDLER_MODE_ARGS0
	var c0: String = _arg_hint_class(args[0])
	if n >= 2:
		var c1: String = _arg_hint_class(args[1])
		if _is_typed_payload_class(c0) and c1 == "NetEventGd":
			return _HANDLER_MODE_TYPED_AND_EVENT
	if c0 == "" or c0 == "NetEventGd":
		return _HANDLER_MODE_NET_EVENT
	if _is_typed_payload_class(c0):
		return _HANDLER_MODE_TYPED_PAYLOAD
	return _HANDLER_MODE_NET_EVENT


func _is_typed_payload_class(class_name_str: String) -> bool:
	if class_name_str.is_empty() or class_name_str == "NetEventGd":
		return false
	return true


func _arg_hint_class(arg: Variant) -> String:
	if typeof(arg) != TYPE_DICTIONARY:
		return ""
	var d: Dictionary = arg
	if d.has("class_name"):
		var cn = d["class_name"]
		if cn != null and str(cn) != "":
			return str(cn)
	return ""

# ── Central dispatch ──

func _handle_event(event: NetEventGd) -> void:
	var ename: String = str(event.event_name)
	if ename.is_empty():
		return
	var method_name: String = "_on_" + ename
	if _handlers.has(method_name):
		_invoke_registered_handler(method_name, event)
	else:
		_on_unknown_event(event)


func _invoke_registered_handler(method_name: String, event: NetEventGd) -> void:
	var rec: Dictionary = _handlers[method_name]
	var handler: Object = rec["handler"]
	var mode: int = int(rec["mode"])
	var data: Variant = event.get_data()
	match mode:
		_HANDLER_MODE_ARGS0:
			handler.call(method_name)
		_HANDLER_MODE_NET_EVENT:
			handler.call(method_name, event)
		_HANDLER_MODE_TYPED_PAYLOAD:
			handler.call(method_name, data)
		_HANDLER_MODE_TYPED_AND_EVENT:
			handler.call(method_name, data, event)
		_:
			handler.call(method_name, event)


func _on_unknown_event(event: NetEventGd) -> void:
	print("[Net] Unhandled event: ", str(event.event_name))
