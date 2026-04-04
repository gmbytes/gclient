extends RefCounted
class_name NetHandlerBase

## 业务协议处理器基类：放在 handlers/ 下，实现 _on_<event_name>(event: NetEventGd)。
## 新协议只需新建或扩展现有 handler 脚本，无需修改 net_manager.gd。

var _manager: Node = null

func bind_manager(m: Node) -> void:
	_manager = m


func net_manager() -> Node:
	return _manager


func bridge() -> Node:
	if _manager and _manager.has_method("get_bridge"):
		return _manager.get_bridge()
	return null
