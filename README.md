# gclient

`gclient` 是 Godot **4.7** 客户端工程（`project.godot` 中应用名为 **`slgame`**），采用 **GDScript + Rust GDExtension**：

- **GDScript**：场景与业务脚本（`src/app`、`src/core`、`src/ui`）；**协议编解码、分发、心跳**在 GDScript（`cmd_ext.gd` + godobuf 生成的 `src/game/pb/*.gd`）。
- **`rust/`**：workspace（`lib/gnet` 传输层、`lib/gxlsx` 配置、`gdbridge` 桥接 cdylib）
- **`addons/gdbridge/`**：GDExtension 描述文件；编译产物在 `addons/gdbridge/bin/`（已 `.gitignore`）
- **`data/config/`**：运行时读取的 **`all.json`**（由 **`comm/gen_client.bat`** 或本目录 **`build.bat`** 从 Excel 导出）
- **`data/generated/`**：导表生成的 **`.res`**、`tables/*.res` 及中间 **`gd/`** 脚本
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
│   ├── game/
│   │   └── pb/                # genpb --gd_out：godobuf *.gd + cmd_ext.gd
│   ├── app/                   # app.tscn / app.gd
│   ├── core/
│   │   ├── config/            # config_manager.gd
│   │   └── net/
│   │       ├── net_manager.gd # poll_events(Dictionary) + cmd_ext 反序列化 + 路由 + 心跳
│   │       ├── net_handler_base.gd
│   │       ├── net_event_utils.gd
│   │       └── handlers/
│   │           ├── handler_framework.gd  # disconnected 等
│   │           ├── handler_session.gd    # 登录/创角/进角色/业务消息
│   │           └── handler_example.gd    # 模板说明
│   └── ui/menu/
├── data/
├── assets/
├── addons/gdbridge/
│   └── gdbridge.gdextension
└── rust/
    ├── Cargo.toml             # workspace: gnet, gxlsx, gdbridge
    ├── scripts/build.ps1
    ├── lib/
    │   ├── gnet/              # WebSocket、PacketCodec（帧边界）、Session、重连
    │   │   └── tests/         # codec_parity 等
    │   └── gxlsx/
    └── gdbridge/
        └── net_bridge.rs      # NetClientBridge：connect / send_raw / poll_events → Dictionary
```

## 网络架构

### 数据流

```
proto → genpb (--go_out + --gd_out)
         ├── Go 服务端 *.pb.go / cmd.ext.go
         └── gclient: godobuf *.gd + cmd_ext.gd

运行时:
  handlers / NetManager (GDScript)
       │ send_msg → CmdExt.marshal_request → send_raw
       ▼
  gdbridge: WebSocket 发二进制帧
       ▼
  gnet: 收帧 → PacketCodec::decode → NetEvent { Message|Error|... }
       ▼
  NetManager: unmarshal(key, body) → _on_<snake_case>
```

各层职责：

| 层 | 位置 | 职责 | 不做什么 |
|----|------|------|----------|
| **genpb** | `comm/tools/genpb` | Go：`*.pb.go`、`cmd.ext.go`、`data.pb.vector.go`；可选 GD：`*.gd`、`cmd_ext.gd` | 不生成 Rust protobuf |
| **gnet** | `rust/lib/gnet/` | WebSocket、`PacketCodec`（拆出 key/err/body）、连接三态、待发队列、自动重连 | 不反序列化 protobuf、不心跳 |
| **gdbridge** | `rust/gdbridge/` | `NetClientBridge`：`connect_to_server`、`send_raw`、`poll_events` → `Array[Dictionary]`、`set_reconnect` | 不解析业务消息 |
| **cmd_ext.gd** | `src/game/pb/` | `unmarshal` / `marshal_request` / `key_name` | — |
| **NetManager** | `net_manager.gd` | 轮询、心跳定时发 `ReqPing`、`send_msg`、按 key 路由 `_on_*` | 具体业务在 handler |
| **handlers** | `handlers/*.gd` | 消费 godobuf 消息类型（`get_err()` 等） | — |

### NetEvent（Rust `gnet`，传输层）

```rust
pub enum NetEvent {
    Connected,
    Disconnected { reason: String },
    Message { key: u16, body: Vec<u8> },   // err==0 的成功下行，body 为纯 protobuf
    Error { key: u16, err_code: u16 },     // 4 字节错误帧
}
```

连接失败、握手失败等仍通过 **`Disconnected { reason }`** 上报（与原先 `ConnectError` 合并为同一类事件在 Godot 侧统一按 `disconnected` 处理）。

### `poll_events()` 返回的 `Dictionary`

| `type` | 字段 | 含义 |
|--------|------|------|
| `connected` | — | WebSocket 已连通 |
| `disconnected` | `reason: String` | 断线或连接失败原因 |
| `message` | `key: int`, `body: PackedByteArray` | 成功下行，待 `cmd_ext.unmarshal` |
| `error` | `key: int`, `err_code: int` | 服务端错误帧，无 body |

### Session 状态（传输层三态）

```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}
```

### 二进制帧格式

与 `comm/doc/消息设计.md`、`gnet` 中 `codec.rs` 一致：

- 上行：`[2B key][4B len][body]`（GDScript 用 `CmdExt.marshal_request`）
- 下行成功：`[2B key][2B err=0][4B len][body]`
- 下行错误：`[2B key][2B err!=0]`

### NetClientBridge API（GDScript）

| 类别 | 方法 |
|------|------|
| 连接 | `connect_to_server(host, port, path)`、`disconnect_from_server()` |
| 重连 | `set_reconnect(enabled, interval_secs, max_retries)` |
| 状态 | `get_connection_state()`、`is_connected()` |
| 轮询 | `poll_events()` → `Array[Dictionary]` |
| 发送 | `send_raw(PackedByteArray)` — 完整一帧（通常由 `NetManager.send_msg` 封装） |

### NetManager 与 Handler 约定

- 路由方法名：**`_on_` + `cmd_ext.key_name(key)`**（由 genpb 生成的 snake_case，如 `rsp_login`、`dsp_move`）。
- 框架：`_on_connected(ev)`、`_on_disconnected(ev)`；业务：`_on_rsp_login(msg, ev)` 等（首参为 godobuf 消息对象或省略）。
- **心跳**：`NetManager.start_heartbeat` / `stop_heartbeat`，定时 `ReqPing`；业务可在 `_on_rsp_ping` 中更新 RTT 等。

新增下行消息时：**改 proto → 运行 genpb（含 `--gd_out`）→ 在 handler 增加 `_on_<name>`**；**无需**改 Rust 分发表。

## 协议生成（推荐入口）

**推荐**：在仓库 **`comm/`** 运行 **`gen_client.bat`** —— 使用 **`gclient`** 内 **godobuf**（`addons/godobuf`）生成 **`src/game/pb/*.gd`**，再调用 **`genpb -gd_cmd_ext_only`** 写入 **`cmd_ext.gd`**。需本机 **`godot`** 在 PATH，或放置 **`comm/tools/protoc-gen-gd.exe`**。

**备选**：在 **`comm/tools/genpb`** 执行 **`gen.bat`** / `go run`，同时传 **`--go_out`** 与 **`--gd_out`**，由 **genpb** 内置流程一次生成 Go + GD（依赖 **`comm/tools/protoc-gen-gd.exe`**）。例如：

```bash
go run -buildvcs=false . --flag client \
  --go_out ../../../server/server/internal/pb \
  --gd_out ../../../gclient/src/game/pb \
  --tools_dir ..
```

`gen.bat` 默认 **`gd_out`** 已指向本工程 **`src/game/pb`**。

仅更新 **服务端 Go** 时，使用 **`comm/gen_server.bat`**，无需跑客户端 godobuf。

客户端 **不再** 使用 `pb.rs`、`typed_protocol.rs`、`godot_bridge_gen.rs`；协议类以 **`src/game/pb/*.gd`** 为准。

### 扩展新协议

1. 编辑 `comm/tools/genpb/proto/` 下 `.proto` 与 `EKey`。
2. 运行 genpb（Go + `--gd_out`）。
3. 在 `handlers/` 中实现 `_on_<snake_name>`；上行在业务里 `send_msg(Cmd.EKey.T.ReqXxx, req)`。

## 配置表（genxls）

有两种常见方式，与 **`comm/`** 约定一致：

1. **`comm/gen_client.bat`**：`--split-json` 导出到 **`comm/client_gen/`**，生成 **`gclient/data/generated/gd/c_*.gd`** 与 **`res_importer.gd`**，复制 **`all.json`** 到 **`data/config/`**，再用 Godot 无头导入 **`data/generated/*.res`**。适合与协议生成一条命令跑完。
2. **本目录 `build.bat`**：若存在 **`../comm/tools/genxls.exe`**（由 **`comm/build.bat`** 产出）与 **`../comm/excel`**，则 **`genxls`** 直接 **`--out data/config`**、**`--gd-out data/generated/gd`**，并传 **`--gclient`** 触发无头导入 **`.res`**（见脚本内注释）。

若未构建 genxls 或无 Excel 目录，**`build.bat`** 会跳过导表继续编译 Rust。Linux/macOS 的 **`build.sh`** 不调用 genxls。

> 服务端表结构 Go 代码由 **`comm/gen_server.bat`** 写入 **`server/data/xls/go.gen.go`**，与客户端导出相互独立。

## 关键约定

- `gdbridge` 是唯一 cdylib；新 Rust 能力放在 `rust/lib/`，在 `gdbridge/src/` 增加桥接。
- `comm/tools/genpb/proto/*.proto` 为协议真源；服务端 Go 与客户端 GDScript 均由 genpb（含可选 `--gd_out`）生成。
- `gnet` 只做传输与帧边界，**不含** protobuf 与游戏业务。
- `NetManager` 不做具体业务逻辑，只做轮询、编解码调度与心跳定时器。
- 更细的网络说明见仓库 **`docs/gclient-network.md`**（若与本文不一致，以代码与 **`comm/doc/消息设计.md`** 为准）。

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

## 运行与验证

打开 `gclient/project.godot`，或使用命令行：

```powershell
<godot_bin> --headless --path . --quit
```

扩展加载成功时日志中可见 godot-rust 初始化及网络桥就绪信息。

## Rust 测试

```powershell
cd rust
cargo test -p gnet          # gnet 单元测试（含 codec）
cargo test --workspace       # 全 workspace
```
