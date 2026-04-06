extends Node

var _bridge: Node
var _loaded: bool = false

func _ready():
	print("[Config] ConfigManager._ready() start")
	_bridge = ClassDB.instantiate("ConfigBridge")
	if _bridge == null:
		push_error("[Config] ConfigBridge class not found – is gdbridge library loaded?")
		return
	_bridge.name = "ConfigBridge"
	add_child(_bridge)

	var config_root := _resolve_config_root()
	print("[Config] loading from: %s" % config_root)
	_loaded = _bridge.load_all(config_root)
	if _loaded:
		var ver = _bridge.get_manifest_version()
		var names = _bridge.get_table_names()
		print("[Config] loaded version=%s, tables=%d (%s)" % [ver, names.size(), ", ".join(names)])
	else:
		push_warning("[Config] config load failed from: %s" % config_root)

func _resolve_config_root() -> String:
	var user_config = OS.get_user_data_dir() + "/config"
	if FileAccess.file_exists(user_config + "/manifest.json"):
		return user_config

	var res_config = "res://data/config"
	if FileAccess.file_exists(res_config + "/manifest.json"):
		return ProjectSettings.globalize_path(res_config)
	if FileAccess.file_exists(res_config + "/all.json"):
		return ProjectSettings.globalize_path(res_config)

	return ProjectSettings.globalize_path(res_config)

func is_loaded() -> bool:
	return _loaded

func get_table(table_name: String) -> Array:
	if _bridge == null or not _loaded:
		return []
	return _bridge.get_table(table_name)

func get_row(table_name: String, id: int) -> Dictionary:
	if _bridge == null or not _loaded:
		return {}
	return _bridge.get_row(table_name, id)

func reload_table(table_name: String) -> bool:
	if _bridge == null:
		return false
	var ok = _bridge.reload_table(table_name)
	if ok:
		print("[Config] reloaded table: %s" % table_name)
	return ok

func get_version() -> String:
	if _bridge == null:
		return ""
	return _bridge.get_manifest_version()

func get_table_names() -> PackedStringArray:
	if _bridge == null:
		return PackedStringArray()
	return _bridge.get_table_names()
