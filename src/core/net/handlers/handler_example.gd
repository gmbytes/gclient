## 协议处理器模板（本文件无 _on_*，不会被注册）
##
## 步骤：
##   1. 复制为 handler_你的模块.gd
##   2. extends NetHandlerBase，实现 _on_<key_name>(msg: CmdRsp.RspXxx)
##   3. 无需再改 net_manager.gd
##
## key_name 与 cmd_ext.gd 中 key_name() 一致（snake_case），
## 如 rsp_login、dsp_move、rsp_enter_zone。
##
## 所有 handler 统一签名（只有 msg，不含 err_code）：
##
##   func _on_rsp_login(msg: CmdRsp.RspLogin) -> void:
##       # msg: protobuf 消息对象，若为 null 表示解析失败
##       if not msg:
##           return
##       # 使用 msg.get_xxx() ...
##
## 服务端 4 字节错误包不会分发到 handler，
## 由 net_manager 统一通过 server_error 信号通知 UI 层处理。
##
## 连接/断开事件（msg 为 null）：
##   func _on_connected(_msg) -> void: ...
##   func _on_disconnected(_msg) -> void: ...
##
## 发协议：send_msg(Cmd.EKey.T.ReqXxx, req)

extends NetHandlerBase
