extends NetHandlerBase

## Framework-level events: connected / disconnected / error / raw.

func _on_disconnected(event: NetEventGd) -> void:
	var ex: Dictionary = event.get_extra()
	var reason: String = str(ex.get("reason", "unknown"))
	print("[Net] Disconnected: ", reason)


func _on_error(event: NetEventGd) -> void:
	var ex: Dictionary = event.get_extra()
	var msg: String = str(ex.get("message", "unknown"))
	print("[Net] Error: ", msg)


func _on_raw(event: NetEventGd) -> void:
	var ex: Dictionary = event.get_extra()
	var key: int = int(ex.get("key", event.key))
	var err: int = int(ex.get("err", event.err))
	var body = ex.get("body", PackedByteArray())
	var blen: int = body.size() if body is PackedByteArray else 0
	print("[Net] Raw message: key=%d err=%d body_len=%d" % [key, err, blen])
