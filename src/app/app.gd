extends Node

var net_manager: Node

func _ready():
	var NetManagerScript = preload("res://src/core/net/net_manager.gd")
	net_manager = NetManagerScript.new()
	net_manager.name = "NetManager"
	add_child(net_manager)

	var main_menu = preload("res://src/ui/menu/main_menu.tscn").instantiate()
	add_child(main_menu)

	print("[App] Application started (Rust client)")

func _exit_tree():
	print("[App] Application shutdown")
