extends Node

var config_manager: Node
var net_manager: Node

func _ready():
	var ConfigManagerScript = preload("res://src/game/config/config_manager.gd")
	config_manager = ConfigManagerScript.new()
	config_manager.name = "ConfigManager"
	add_child(config_manager)

	var NetManagerScript = preload("res://src/core/net/net_manager.gd")
	net_manager = NetManagerScript.new()
	net_manager.name = "NetManager"
	add_child(net_manager)

	var main_menu = preload("res://src/game/ui/menu/main_menu.tscn").instantiate()
	add_child(main_menu)

	print("[App] Application started (Rust client)")

func _exit_tree():
	print("[App] Application shutdown")
