extends Node

const GEN_DIR = "res://data/generated/"

var _config: CAllConfig
var _loaded: bool = false

func _ready():
	print("[Config] ConfigManager._ready() start")
	var path = GEN_DIR + "game_config.res"
	if not ResourceLoader.exists(path):
		push_warning("[Config] %s not found, config not loaded" % path)
		return
	_config = load(path) as CAllConfig
	if _config == null:
		push_error("[Config] failed to load %s" % path)
		return
	_loaded = true
	print("[Config] loaded game_config.res")

func is_loaded() -> bool:
	return _loaded

func get_config() -> CAllConfig:
	return _config

func load_table(table_name: String) -> Resource:
	var path = GEN_DIR + table_name + ".res"
	if not ResourceLoader.exists(path):
		push_warning("[Config] table %s not found at %s" % [table_name, path])
		return null
	return load(path)
