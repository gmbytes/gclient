extends NetHandlerBase


func _on_disconnected(_msg) -> void:
	var m = net_manager()
	if m:
		m.stop_heartbeat()
