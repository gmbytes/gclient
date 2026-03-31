# rclient 验收与联调清单

## 1) 构建与产物

- Windows: 在 `client/` 目录执行 `build.bat`
- Linux/macOS: 在 `client/` 目录执行 `./build.sh`
- 产物检查:
  - Windows: `client/addons/gdbridge/bin/gdbridge.dll`
  - Linux: `client/addons/gdbridge/bin/libgdbridge.so`
  - macOS: `client/addons/gdbridge/bin/libgdbridge.dylib`

## 2) 协议与编解码

- 在 `rclient/rust` 目录执行 `cargo test`
- 必须通过:
  - `codec` 单元测试
  - `codec_parity` 协议对齐测试（EKey/包头/Req-Rsp 映射）

## 3) 登录链路验收

Godot 运行 `client/project.godot` 后，按以下流程验证:

1. 输入 host/port/account 并点击 Play
2. 收到 `connected` 后自动发送 `ReqLogin`
3. 无角色时自动 `ReqCreateRole`
4. 有角色或建角成功后自动 `ReqLoginRole`
5. 登录成功后进入 In Game，并开始 5 秒心跳

成功标准:

- UI 状态从 Connecting -> Connected -> In Game
- 控制台可见登录、建角、选角、心跳日志

## 4) 重连验收

默认开启自动重连（3 秒间隔，最多 5 次）:

- 在网络断开/服务端关闭后，状态显示 `reconnecting...`
- 到达重试上限后，保持断开状态并等待手动重连

## 5) 日志验收

Rust 侧启用 `env_logger`，默认 `Info` 级别，重点观察:

- 连接建立/断开
- 收发数据长度
- 登录链路关键动作
- 错误与重连次数

## 6) 回归建议

- 每次修改 proto 后执行:
  - `cargo clean`（可选）
  - `cargo build`
  - `cargo test`
- 每次修改桥接层后，重新执行 `build.bat`/`build.sh` 并在 Godot 中回归登录链路
