extends Control

@onready var host_input: LineEdit = $CenterContainer/VBoxContainer/HostInput
@onready var port_input: LineEdit = $CenterContainer/VBoxContainer/PortInput
@onready var account_input: LineEdit = $CenterContainer/VBoxContainer/AccountInput
@onready var play_button: Button = $CenterContainer/VBoxContainer/PlayButton
@onready var quit_button: Button = $CenterContainer/VBoxContainer/QuitButton
@onready var status_label: Label = $CenterContainer/VBoxContainer/StatusLabel

func _ready():
	play_button.pressed.connect(_on_play_pressed)
	quit_button.pressed.connect(_on_quit_pressed)

func _on_play_pressed():
	print("[MainMenu] Connect & Play button pressed")
	var net = _get_net_manager()
	if net == null:
		print("[MainMenu] ERROR: NetManager not found, parent=%s" % get_parent())
		status_label.text = "Status: NetManager not found"
		return
	print("[MainMenu] NetManager found: %s" % net)

	var host = host_input.text.strip_edges()
	if host.is_empty():
		host = "127.0.0.1"
	var port_text = port_input.text.strip_edges()
	var port = 8080
	if not port_text.is_empty():
		port = int(port_text)
	var account = account_input.text.strip_edges()
	if account.is_empty():
		account = "test_user"

	print("[MainMenu] Connecting to ws://%s:%d/ws  account=%s" % [host, port, account])
	status_label.text = "Status: Connecting to %s:%d ..." % [host, port]
	net.connect_to_server(host, port)
	net.account_to_login = account
	print("[MainMenu] connect_to_server() called")

func _on_quit_pressed():
	get_tree().quit()

func _process(_delta):
	var net = _get_net_manager()
	if net:
		status_label.text = "Status: %s" % net.get_status_text()

func _get_net_manager() -> Node:
	var app = get_parent()
	if app and app.has_node("NetManager"):
		return app.get_node("NetManager")
	return null
