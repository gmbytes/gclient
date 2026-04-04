## 协议处理器模板（本文件无 _on_*，不会被注册）
##
## 步骤：
##   1. 复制为 handler_你的模块.gd
##   2. extends NetHandlerBase，实现 _on_<event_name>(event: NetEventGd)
##   3. 无需再改 net_manager.gd
##
## event_name 与 genpb manifest 一致（snake_case），如 rsp_shop_list、dsp_world_msg。
##
## 强类型业务（函数名仍为 _on_<event_name>，由 NetManager 按首参类型自动传参）：
##   仅编译通道：
##     func _on_rsp_shop_list(msg: RspShopListGd) -> void:
##         _use(msg.items)
##   需要热更 / get_data() 可能为空时，加第二参：
##     func _on_rsp_shop_list(msg: RspShopListGd, event: NetEventGd) -> void:
##         if msg:
##             _use(msg.items)
##         else:
##             var hf := event.get_hotfix_fields()
##             ...
##
## 通用字段读取：NetEventUtils.field(event, "err", 0)、NetEventUtils.msg_err(event)
## 发协议：bridge() 或 net_manager().get_bridge() 上调用 send_xxx / send_generic

extends NetHandlerBase
