## 协议处理器示例模板
##
## 用法：
##   1. 复制此文件为 handler_xxx.gd（如 handler_shop.gd）
##   2. 在其中定义 _on_<event_type>(event: Dictionary) 方法
##   3. NetManager 启动时自动扫描并注册
##   4. 热更时随 PCK 包下发，调用 net_manager._load_handlers() 重新扫描
##
## 命名规则：
##   event type "rsp_shop_list"  →  方法名 "_on_rsp_shop_list"
##   event type "dsp_world_msg"  →  方法名 "_on_dsp_world_msg"
##
## 注意：此文件仅作示例，不会被注册（没有 _on_ 方法）

extends RefCounted

## 可选：由 NetManager 在加载时自动调用，让处理器持有管理器引用
var _manager: Node = null

func bind_manager(m: Node) -> void:
	_manager = m

# 示例：处理商店列表响应（event type = "rsp_shop_list"）
# func _on_rsp_shop_list(event: Dictionary) -> void:
# 	var items: Array = event.get("items", [])
# 	print("[Shop] received %d items" % items.size())
# 	# 如需发送消息，通过 _manager._bridge.send_xxx(...)

# 示例：处理广播消息（event type = "dsp_world_msg"）
# func _on_dsp_world_msg(event: Dictionary) -> void:
# 	var msg: String = event.get("content", "")
# 	print("[World] broadcast: ", msg)
