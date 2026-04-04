# gclient

`gclient` 是 Godot **4.7** 客户端工程（`project.godot` 中应用名为 `slgame-rust`），采用 **GDScript + Rust GDExtension**：

- **GDScript**：场景与业务脚本（`src/app`、`src/core`、`src/ui`）
- **`rust/`**：workspace（`lib/gnet` 网络、`lib/gxlsx` 配置、`gdbridge` 桥接 cdylib）
- **`addons/gdbridge/`**：GDExtension 描述文件；编译产物在 `addons/gdbridge/bin/`（已 `.gitignore`）
- **`data/`**：表配置导出目录（Windows 下 `build.bat` 会调用 `genxls` 写入 `data/config/` 等）
- **`assets/`**：音频、纹理、字体等资源

与 `comm/`（`genpb`、`genxls`、Excel 等）位于同一上级目录 `game/` 下时，下文相对路径以该布局为准。

## 目录结构

```text
gclient/
├── project.godot              # 主场景 res://src/app/app.tscn；扩展 gdbridge
├── build.bat                  # Windows：可选 genxls + cargo + 复制 DLL
├── build.sh                   # Unix：cargo + 复制 .so/.dylib
├── icon.svg
├── src/
│   ├── app/                   # app.tscn / app.gd（组装 ConfigManager、NetManager、主菜单）
│   ├── core/
│   │   ├── config/            # config_manager.gd
│   │   └── net/
│   │       ├── net_manager.gd # 轮询 poll_events + 按 event_name 路由到 handler
│   │       ├── net_handler_base.gd
│   │       ├── net_event_utils.gd
│   │       └── handlers/
│   │           ├── handler_framework.gd  # 框架事件：connected/disconnected/error/raw
│   │           ├── handler_session.gd    # 会话流程：登录/创角/进角色/心跳
│   │           └── handler_example.gd    # 模板说明
│   └── ui/menu/               # main_menu 场景与脚本
├── data/                      # 配置导出目标（见构建说明）
├── assets/                    # 可选资源目录
├── addons/gdbridge/
│   └── gdbridge.gdextension   # 指向 bin/ 下动态库
└── rust/
    ├── Cargo.toml             # workspace: gnet, gxlsx, gdbridge
    ├── .gdignore
    ├── scripts/build.ps1      # 仅构建 gdbridge 并复制到 addons/gdbridge/bin/
    ├── lib/
    │   ├── gnet/              # WebSocket 传输、编解码、dispatcher、事件产出
    │   │   ├── src/gen/       # genpb 生成：pb.rs、typed_protocol.rs
    │   │   └── tests/         # codec_parity、dispatch_channels
    │   └── gxlsx/             # manifest、按表加载；config.gen.rs 由 genxls 生成
    └── gdbridge/
        └── src/gen/           # genpb 生成 godot_bridge_gen.rs
```

## 网络架构

### 数据流

```
proto 定义 → genpb 生成 → gnet (WebSocket + codec + dispatch)
                           → gdbridge (NetEvent → NetEventGd)
                             → NetManager (poll + route)
                               → handlers/*.gd (业务消费)
```

协议链路为 **单向流动**，各层职责清晰：

| 层 | 位置 | 职责 | 不做什么 |
|----|------|------|----------|
| `genpb` | `comm/tools/genpb` | 生成 `pb.rs`、`typed_protocol.rs`、`godot_bridge_gen.rs` | — |
| `gnet` | `rust/lib/gnet/` | WebSocket 传输、PacketCodec 编解码、dispatcher 把 bytes 转 `NetEvent`、心跳、自动重连 | 不含游戏业务逻辑、不写 per-message send 方法 |
| `gdbridge` | `rust/gdbridge/` | 暴露 Godot API；`NetEvent` → `NetEventGd`；`send_*` 方法 | 不做业务判断 |
| `NetManager` | `src/core/net/net_manager.gd` | 轮询 `poll_events()`、按 `_on_<event_name>` 路由到 handler | 不含业务状态、不处理事件内容 |
| `handlers/*.gd` | `src/core/net/handlers/` | 唯一业务消费层 | — |

### NetEvent（Rust 侧，5 个变体）

```rust
pub enum NetEvent {
    Connected,
    Disconnected { reason: String },
    ConnectError { message: String },
    ProtocolEvent { event_name: String, key: u16, err: u16, msg: Box<ServerMessage> },
    RawMessage { key: u16, err: u16, body: Vec<u8> },
}
```

### NetEventGd（Godot 侧）

| 成员 / 方法 | 含义 |
|-------------|------|
| `event_name` | snake_case，如 `rsp_login`、`connected`、`dsp_move` |
| `key` / `err` | 线协议上的 EKey 与错误码 |
| `get_data()` | 强类型下行消息：`Gd<RspLoginGd>` 等；框架事件为空 |
| `get_extra()` | 框架事件附带信息的 `Dictionary`（如 `reason`、`message`） |

### Session 状态（仅传输层三态）

```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}
```

游戏级状态（"登录中"、"已进入游戏"等）由 GDScript handler 自行维护。

### 分发优先级（dispatcher）

```
WebSocket Raw Bytes
    │
    ▼
PacketCodec::decode → (key, err, body)
    │
    ├─► EKey::from_u16 命中 且 decode_server_message 成功
    │       → NetEvent::ProtocolEvent { event_name, msg: ServerMessage }
    │
    └─► 未命中或解码失败
            → NetEvent::RawMessage { key, err, body }
```

新增下行消息后 **dispatcher 无需修改**——`EKey` 和 `decode_server_message` 来自 genpb 生成的 `typed_protocol.rs`。

### NetClientBridge API（GDScript 可调用）

| 类别 | 方法 |
|------|------|
| 连接 | `connect_to_server(host, port, path)`、`disconnect_from_server()` |
| 重连 | `set_reconnect(enabled, interval_secs, max_retries)` |
| 心跳 | `start_heartbeat(interval_secs)`、`stop_heartbeat()` |
| 状态 | `get_connection_state()` → `"disconnected"` / `"connecting"` / `"connected"` |
| 轮询 | `poll_events()` → `Array[NetEventGd]` |
| 发送 | `send_login(account, token, version)`、`send_create_role(cid, name)`、`send_login_role(role_id)`、`send_ping()`、`send_enter_zone()`、`send_move(x, y, z)` |

发送方法未来由 genpb 自动生成到 `net_bridge.rs`；`client.rs` 只保留底层 `send_packet(key, body)` 和 `send_message(msg: &ClientMessage)`。

### Handler 参数模式

| 模式 | 条件 | 调用方式 |
|------|------|----------|
| 无参 | 方法无参数 | `handler._on_xxx()` |
| 完整事件 | 首参为 `NetEventGd` | `handler._on_xxx(event)` |
| 强类型负载 | 首参为某 `*Gd` 类型 | `handler._on_xxx(event.get_data())` |
| 强类型 + 事件 | 首参 `*Gd`、第二参 `NetEventGd` | `handler._on_xxx(data, event)` |

### 框架级事件名

| event_name | 含义 | `get_extra()` 字段 |
|------------|------|--------------------|
| `connected` | 连接成功 | — |
| `disconnected` | 断线 | `reason: String` |
| `error` | 连接失败 | `message: String` |
| `raw` | 未知 key 或解码失败 | `key`, `err`, `body: PackedByteArray` |

由 `handler_framework.gd` 处理。

## 协议生成（genpb）

`comm/tools/genpb` 是唯一协议生成入口：

```bash
cd comm/tools/genpb
go run -buildvcs=false . --lang rust --flag client \
  --rust_out  ../../gclient/rust/lib/gnet/src/gen \
  --godot_out ../../gclient/rust/gdbridge/src/gen
```

| 输出路径 | 文件 | 作用 |
|----------|------|------|
| `--rust_out` | `pb.rs` | prost 消息类型 |
| `--rust_out` | `typed_protocol.rs` | `EKey`、`ClientMessage` / `ServerMessage`、`encode_client_message` / `decode_server_message` |
| `--godot_out` | `godot_bridge_gen.rs` | 下行消息 `*Gd` GodotClass、`NetEventGd`、`server_message_to_event` |

### 扩展新协议的流程

1. 在 `comm/tools/genpb/proto/` 中修改 `.proto`，添加新消息与 EKey
2. 运行 genpb，生成 `pb.rs`、`typed_protocol.rs`、`godot_bridge_gen.rs`
3. 重编 Rust workspace（`cargo build`）
4. 在 `handlers/` 中实现 `_on_<event_name>` 方法

**零手改验收标准**：
- 新增下行消息：跑 genpb → 写 handler → 完毕。Rust 手写代码零修改。
- 新增上行消息：跑 genpb → GDScript 调用生成的 `send_*` → 完毕。Rust 手写代码零修改。
- `dispatcher.rs` 不需要改。
- `net_bridge.rs` 手写部分不需要改。
- `net_manager.gd` 不需要改。

## 配置表（genxls）

Windows 下 **`build.bat`** 会在存在 `../comm/tools/genxls/genxls.exe` 与 `../comm/excel` 时：

1. 导出到 `data/config/`（`manifest.json`、`tables/`、`--split-json` 等）
2. 将 `data/config/config.gen.rs` 复制到 `rust/lib/gxlsx/src/config.gen.rs`

若未先构建 genxls 或无 Excel 目录，脚本跳过该步继续编译 Rust。Linux/macOS 的 `build.sh` 不调用 genxls。

## 关键约定

- `gdbridge` 是唯一 cdylib；新 Rust 能力放在 `rust/lib/`，在 `gdbridge/src/` 增加桥接。
- `comm/tools/genpb/proto/*.proto` 为协议真源；客户端 Rust/Godot 由 genpb 统一生成。
- `gnet` 不含游戏业务逻辑（Session 只有连接三态）。
- `net_bridge.rs` 手写部分只包含连接/断开/轮询/心跳/重连/状态查询（约 80 行），`send_*` 方法来自生成。
- `NetManager` 不含业务状态和业务处理函数，只做轮询 + 路由。
- `rust/.gdignore` 减轻 Godot 对 Rust 构建树的扫描。

## 构建

### Windows：`build.bat`（项目根）

顺序：可选 **genxls → 配置落盘与 `config.gen.rs` 同步** → **`cargo build`（整个 workspace）** → **复制 `target/debug/gdbridge.dll` 到 `addons/gdbridge/bin/`**。

### Linux / macOS：`build.sh`

**仅** `cargo build` 与按平台复制 `libgdbridge.so` / `libgdbridge.dylib` 到 `addons/gdbridge/bin/`。

### 仅 Rust 扩展：`rust/scripts/build.ps1`

```powershell
cd rust
./scripts/build.ps1              # debug
./scripts/build.ps1 -Profile release  # release
```

只构建 `gdbridge` crate 并复制产物到 `addons/gdbridge/bin/`。

## 运行与验证

打开 `gclient/project.godot`，或使用命令行：

```powershell
<godot_bin> --headless --path . --quit
```

扩展加载成功时日志中可见 godot-rust 初始化及网络桥就绪信息。

## Rust 测试

```powershell
cd rust
cargo test -p gnet          # gnet 单元 + 集成测试
cargo test --workspace       # 全 workspace
```
