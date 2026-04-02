# gclient

`gclient` 是一个 Godot 客户端工程，采用 **GDScript + Rust GDExtension** 的组合开发：

- `src/`：GDScript 场景与业务脚本（按功能模块）
- `rust/`：Rust workspace（`lib/gnet` 网络核心、`lib/gxlsx` 配置核心、Godot 桥接）
- `addons/gdbridge/`：GDExtension 接入点
- `assets/`：音频、纹理、字体、着色器、主题
- `data/`：静态配置与数据表

## 目录规划

```text
gclient/
├── project.godot
├── build.bat
├── build.sh
├── src/
│   ├── app/
│   ├── autoload/
│   ├── core/
│   │   ├── config/
│   │   ├── net/
│   │   │   ├── net_manager.gd       # 事件分发（建议迁移到 NetEventGd + event_name）
│   │   │   └── handlers/            # 热更处理器模块目录
│   │   └── state/
│   ├── game/
│   ├── ui/
│   └── utils/
├── assets/
├── data/
│   ├── config/
│   └── tables/
├── addons/gdbridge/
│   ├── gdbridge.gdextension
│   └── bin/                         # 动态库输出（git ignore）
└── rust/                            # Rust workspace
    ├── Cargo.toml
    ├── .gdignore
    ├── lib/
    │   ├── gnet/                    # 网络核心（协议解码、连接管理）
    │   │   └── src/
    │   │       ├── client.rs
    │   │       ├── codec.rs
    │   │       ├── dispatcher.rs    # 编译通道 + 热更覆盖 + 动态 fallback
    │   │       ├── event.rs         # NetEvent（ProtocolEvent / HotfixEvent）
    │   │       ├── protocol_registry.rs  # protocol.desc + protocol_manifest.json
    │   │       ├── gen/
    │   │       │   ├── pb.rs            # prost 生成（由 genpb 写入）
    │   │       │   └── typed_protocol.rs # EKey / 消息枚举 / 编解码（由 genpb 写入）
    │   │       ├── session.rs
    │   │       └── transport.rs
    │   └── gxlsx/                   # 配置核心（manifest、按表加载）
    │       └── src/
    ├── gdbridge/                    # GDExtension 桥接层（cdylib）
    │   └── src/
    │       ├── lib.rs
    │       ├── net_bridge.rs        # poll_events → Array<Gd<NetEventGd>>
    │       ├── gen/
    │       │   └── godot_bridge_gen.rs  # GodotClass + mapper（由 genpb 写入）
    │       └── config_bridge.rs
    └── scripts/build.ps1
```

## 协议架构

协议链路为 **descriptor/manifest 单一真源 → Rust 强类型 → GodotClass 主通道**，热更仍通过 `prost-reflect` 走旁路。

| 层级 | 位置 | 职责 |
|------|------|------|
| 传输层 | `lib/gnet/src/codec.rs` / `client.rs` | WebSocket 收发、包头 `[key][err][len][body]` |
| 协议元数据 | `genpb` 产出 `protocol_manifest.json` + `protocol.desc` | manifest 索引；descriptor 供动态解码与指纹 |
| 编译协议 API | `gen/typed_protocol.rs`（`EKey`、`ServerMessage`、`ClientMessage`） | prost 编解码入口、`event_name()`、`COMPILED_FINGERPRINTS` |
| 分发层 | `dispatcher.rs` | 覆盖检测 → 编译通道 `ProtocolEvent`；否则动态通道 `HotfixEvent`；未知 `RawMessage` |
| Godot 桥 | `gdbridge` + `gen/godot_bridge_gen.rs` | `NetEventGd`、`get_data()` 强类型 payload、`get_hotfix_fields()` 兜底 |
| 业务层 | `net_manager.gd` / `handlers/` | 按 `event_name` 或类型分发 |

### 协议生成（genpb）

`comm/tools/genpb` 是唯一协议生成入口。**Rust 侧元数据来自 `FileDescriptorSet`，不再维护与 descriptor 并行的正则解析。**

推荐一次性生成 gnet + gdbridge 产物（需本机 `protoc` 与 `protoc-gen-prost`）：

```bash
cd comm/tools/genpb
go run -buildvcs=false . --lang rust --flag client \
  --rust_out  ../../gclient/rust/lib/gnet/src/gen \
  --godot_out ../../gclient/rust/gdbridge/src/gen
```

仅服务端 Go 时：

```bash
go run -buildvcs=false . --lang go --flag server --go_out <server/pb路径>
```

| 输出路径 | 文件 | 作用 |
|----------|------|------|
| `--rust_out` | `pb.rs` | prost 消息类型 |
| `--rust_out` | `typed_protocol.rs` | `EKey`、`ClientMessage` / `ServerMessage`、`encode_client_message` / `decode_server_message`、`COMPILED_FINGERPRINTS`（递归 schema 指纹） |
| `--rust_out` | `protocol_manifest.json` | 协议索引：ekey、方向、kind、`event_name`、字段 schema、fingerprint 等（**替代**旧 `protocol_meta.json`） |
| `--rust_out` | `protocol.desc` | `FileDescriptorSet`，热更 / `prost-reflect` 动态编解码 |
| `--godot_out`（可选） | `godot_bridge_gen.rs` | 嵌套/下行消息 `*Gd` GodotClass、`NetEventGd`、`server_message_to_event` / `hotfix_to_event` |

`protocol_manifest.json` 中 `messages[]` 每条大致包含：`ekey_value`、`ekey_name`、`message_name`、`direction`、`kind`、`event_name`、`field_schema`、`fingerprint` / `fingerprint_u64`、`hotfix_fallback` 等（详见 genpb `manifest.go`）。

### 分发优先级（dispatcher）

```
WebSocket Raw Bytes
    │
    ▼
PacketCodec::decode (key, err, body)
    │
    ├─► ProtocolRegistry.should_override(key)？
    │       是 ──► 递归指纹与编译时不一致 ──► NetEvent::HotfixEvent { DynamicMessage, … }
    │
    ├─► EKey::from_u16 命中 且 decode_server_message 成功？
    │       是 ──► NetEvent::ProtocolEvent { event_name, msg: ServerMessage, … }
    │
    ├─► manifest 有该 key 且 descriptor 可解码？
    │       是 ──► NetEvent::HotfixEvent
    │
    └─► NetEvent::RawMessage
```

- **编译通道**：`ProtocolEvent` 内为 `Box<ServerMessage>`，GD 侧经 `server_message_to_event` 得到带 `Gd<XxxGd>` 的 `NetEventGd`。
- **热更旁路**：`HotfixEvent`；`net_bridge` 将其展平为 `NetEventGd.hotfix_fields`（Dictionary），不作为主业务 API。
- **指纹**：运行时与 `typed_protocol.rs` 中 `COMPILED_FINGERPRINTS` 比较；算法为**递归**子 message schema 字符串再 FNV-1a（与 genpb `manifest.go` 一致），避免仅改嵌套类型却漏检的问题。

### ProtocolRegistry

`protocol_registry.rs` 加载 **`protocol.desc` + `protocol_manifest.json`**（不再使用 `protocol_meta.json`），提供：

- `decode_generic` / `encode_from_json_value` / `send_generic` 路径
- `get_event_name(key)`、`should_override(key)`
- 递归指纹计算用于 `should_override`

### NetEvent（Rust）

```rust
pub enum NetEvent {
    Connected,
    Disconnected { reason: String },
    ConnectError { message: String },
    /// 主通道：强类型 ServerMessage
    ProtocolEvent {
        event_name: String,
        key: u16,
        err: u16,
        msg: Box<ServerMessage>,
    },
    /// 热更 / 动态解码旁路
    HotfixEvent {
        event_name: String,
        key: u16,
        err: u16,
        fields: DynamicMessage,
    },
    RawMessage { key: u16, err: u16, body: Vec<u8> },
}
```

新增下行协议时：**无需**再手改 `dispatcher`、`NetEvent` 业务变体或旧版 `event_to_dict`；重新运行 genpb 更新 `typed_protocol.rs` 与 `godot_bridge_gen.rs` 即可。

### Godot：`poll_events` → `NetEventGd`

`net_bridge.rs` 的 `poll_events()` 返回 **`Array<Gd<NetEventGd>>`**（不再是 `Dictionary`）。

| 成员 / 方法 | 含义 |
|-------------|------|
| `event_name` | snake_case，如 `rsp_login`、`connected`、`dsp_move` |
| `key` / `err` | 线协议上的 EKey 与错误码 |
| `get_data()` | 成功编译解码时：`Gd<RspLoginGd>` 等；框架事件多为空 |
| `get_hotfix_fields()` | `HotfixEvent` 或框架附带信息时的 `Dictionary` |

示例（GDScript）：

```gdscript
for event in net_client.poll_events():
    match event.event_name:
        "connected":
            _on_connected()
        "rsp_login":
            var data = event.get_data() as RspLoginGd
            if data:
                _on_rsp_login(data)
        _:
            if event.get_hotfix_fields().size() > 0:
                _on_hotfix(event)
```

`send_generic(key, fields: Dictionary)` 仍保留，用于调试 / 热更发包。

### GDScript 命名约定与热更处理器

`net_manager.gd` 可继续用 `event_name` 做方法名映射（`_on_rsp_login` 等），入参从 `Dictionary` 逐步改为 **`NetEventGd` + `get_data()`**。  
`handlers/` 动态加载机制不变；热更包需包含更新后的 **`protocol.desc` + `protocol_manifest.json`**（以及可选 `.gd`）。

### 新增协议路径

**路径 A：热更（不重编 Rust DLL）**

1. 修改 proto / EKey  
2. `genpb --lang rust --flag client` 生成新的 `protocol.desc` + `protocol_manifest.json`  
3. 更新 handler；将 desc + manifest 打入 PCK  

**路径 B：正式版本（重编 Rust）**

1. 修改 proto  
2. 运行 genpb（含 `--rust_out` 与建议的 `--godot_out`）  
3. 重编 `rust` workspace；编译通道与 GodotClass 随生成物自动一致  

## 关键约定

- `gdbridge` 是唯一 cdylib；新 Rust 能力放在 `rust/lib/`，在 `gdbridge/src/` 增加桥接。  
- `comm/tools/genpb/proto/*.proto` 为协议真源；**客户端 Rust/Godot 由 genpb 统一生成**（Go 服务端仍用 `gen_go.go`，与 manifest 无关）。  
- **C# 客户端生成已从 genpb 移除**；若仍有 C# 工程，需自行维护或其它管线。  
- `rust/.gdignore` 避免 Godot 扫描构建目录。  

## 构建

### Rust 扩展

```powershell
cd rust
./scripts/build.ps1
```

发布：

```powershell
./scripts/build.ps1 -Profile release
```

产物复制到 `addons/gdbridge/bin/`。

### 从项目根目录构建

- Windows：`build.bat`  
- Linux/macOS：`build.sh`  

## 运行验证

打开 `project.godot` 或：

```powershell
<godot_bin> --headless --path . --quit
```

扩展加载成功时日志中可见 godot-rust 初始化及网络桥就绪信息；若加载了协议目录，registry 会打印 manifest 条目数量。
