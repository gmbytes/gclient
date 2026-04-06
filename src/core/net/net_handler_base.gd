extends RefCounted
class_name NetHandlerBase

var _manager: Node = null

func bind_manager(m: Node) -> void:
	_manager = m


func net_manager() -> Node:
	return _manager


func send_msg(key: int, msg) -> void:
	if _manager and _manager.has_method("send_msg"):
		_manager.send_msg(key, msg)
