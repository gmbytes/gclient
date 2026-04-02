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
│   │   │   ├── net_manager.gd       # 命名约定自动分发 + 动态加载
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
    ├── lib/                         # 可复用核心库（gdbridge 依赖）
    │   ├── gnet/                    # 网络核心（协议解码、连接管理）
    │   │   ├── build.rs
    │   │   └── src/
    │   │       ├── client.rs
    │   │       ├── codec.rs
    │   │       ├── dispatcher.rs        # 双通道分发
    │   │       ├── event.rs             # NetEvent（含 GenericMessage）
    │   │       ├── protocol_registry.rs # descriptor 动态解码
    │   │       ├── session.rs
    │   │       └── transport.rs
    │   └── gxlsx/                   # 配置核心（manifest、按表加载）
    │       └── src/
    ├── gdbridge/                    # GDExtension 桥接层（cdylib）
    │   └── src/
    │       ├── lib.rs
    │       ├── net_bridge.rs        # 网络桥接 + send_generic
    │       └── config_bridge.rs
    └── scripts/build.ps1
```

## 协议架构

协议链路采用 **双通道解码 + 命名约定自动分发** 的分层架构，各层职责清晰：

| 层级 | 位置 | 职责 |
|------|------|------|
| 传输层 | `lib/gnet/src/codec.rs` / `client.rs` | WebSocket 收发、数据包拆包 |
| 协议层 | `genpb` 产物（`pb.rs` / `cmd_ext.rs` / `protocol.desc` / `protocol_meta.json`） | 协议定义与元数据 |
| 适配层 | `lib/gnet/src/dispatcher.rs` / `gdbridge/.../net_bridge.rs` | 双通道解码、事件字典输出 |
| 业务层 | `net_manager.gd` / `handlers/` | 命名约定分发、热更处理器 |

### 协议生成（genpb）

`comm/tools/genpb` 是唯一的协议生成入口，支持多目标产出：

```bash
genpb --lang rust --flag client --out gclient/rust/lib/gnet/src/gen
```

Rust 模式产出四类文件：

| 文件 | 作用 |
|------|------|
| `pb.rs` | Rust protobuf 绑定（prost 生成） |
| `cmd_ext.rs` | `EKey`、`ClientMessage`、`ServerMessage`、编解码入口、事件名规则 |
| `protocol.desc` | 二进制 `FileDescriptorSet`，用于通用通道动态解码 |
| `protocol_meta.json` | EKey → message 名 + 事件名映射表（热更核心） |

`protocol_meta.json` 示例：

```json
{
  "2001": { "ekey": "RspLogin",  "message": "RspLogin",  "event_name": "rsp_login" },
  "33005": { "ekey": "DspMove",  "message": "DspMove",   "event_name": "dsp_move" }
}
```

### 双通道解码

`dispatcher.rs` 按优先级依次尝试四条路径，无需任何配置项：

```
WebSocket Raw Bytes
    │
    ▼
PacketCodec::decode (key, err, body)
    │
    ├─► ProtocolRegistry.should_override(key)？
    │       是 ──► 指纹不匹配，prost-reflect 动态解码 ──► NetEvent::GenericMessage
    │
    ├─► EKey::from_u16 命中？
    │       是 ──► 强类型 decode（cmd_ext match）──► NetEvent::XxxMessage
    │
    ├─► ProtocolRegistry 有该 key 的 descriptor？
    │       是 ──► prost-reflect 动态解码 ──► NetEvent::GenericMessage
    │
    └─► 兜底 ──► NetEvent::RawMessage（传 body 字节）
```

- **指纹覆盖**：已编入 Rust 的协议如果字段发生变更（热更了新的 `protocol.desc`），运行时指纹比对检测到差异后自动跳过编译通道，走通用通道解码新字段。
- **编译通道**：已编入 Rust 且指纹一致的协议走强类型快速路径，性能最优。
- **通用通道**：未编入 Rust 但在 `protocol.desc` + `protocol_meta.json` 中存在的协议，由 `ProtocolRegistry` 用 `prost-reflect` 动态解码，无需重编 DLL 即可消费。
- **兜底路径**：完全未知的 key 输出 `RawMessage`，携带原始 body 字节，GDScript 侧通过 `_on_raw` 接收。

#### ProtocolRegistry

`lib/gnet/src/protocol_registry.rs` 启动时加载 `protocol.desc` + `protocol_meta.json`，持有 `prost_reflect::DescriptorPool`，提供：

- `decode_generic(msg_name, bytes) -> DynamicMessage`
- `encode_generic(msg_name, fields) -> Vec<u8>`
- `get_event_name(key_u16) -> Option<String>`
- `should_override(key_u16) -> bool`

#### 指纹覆盖机制

`genpb` 生成 `cmd_ext.rs` 时，为每个服务端消息的字段布局计算 FNV-1a 指纹，写入 `COMPILED_FINGERPRINTS` 常量。`ProtocolRegistry` 加载 `protocol.desc` 时，从 descriptor 中计算同一算法的运行时指纹，两者不一致则将该 key 加入覆盖集合。`dispatcher.rs` 在编译通道之前检查覆盖集合，命中则直接走通用通道解码。

指纹算法：对 message 内所有 field 按 `field_number` 排序，生成 `"number:type[;...]"` 规范字符串后取 FNV-1a 哈希。类型标记规则：标量用原始 proto 类型名，enum 加 `e.` 前缀，message 加 `m.` 前缀，repeated 字段加 `r:` 前缀。

> **已知限制**：指纹仅覆盖顶层 command message 自身字段；若只修改了嵌套数据类型（如 `RoleSummaryData`）的字段而 command message 本身未变，指纹不会触发覆盖。此时需同步在 command message 上做变更（如添加保留字段），或执行完整的 Rust 重编。

#### NetEvent

```rust
pub enum NetEvent {
    // 编译通道 variant（强类型，随协议增量扩展）
    RspLogin { ... },
    DspMove  { ... },
    // ...

    // 通用通道：descriptor 动态解码
    GenericMessage {
        event_name: String,
        key: u16,
        err: u16,
        fields: DynamicMessage,
    },

    // 兜底：完全未知协议
    RawMessage {
        key: u16,
        err: u16,
        body: Vec<u8>,
    },

    // 框架事件
    Connected,
    Disconnected,
    Error { message: String },
}
```

### Rust → GDScript 事件字典约定

`net_bridge.rs` 的 `event_to_dict` 保证编译通道与通用通道输出格式一致：

| 字段 | 规则 |
|------|------|
| `type` | snake_case 事件名（如 `rsp_login`、`dsp_move`） |
| 业务字段 | 统一 snake_case，与 proto 字段名一致 |
| 嵌套 message | 递归展为子 `Dictionary` |
| repeated 字段 | 映射为 `Array` |
| `RawMessage` | `{ "type": "raw", "key": u16, "err": u16, "body": PackedByteArray }` |

新增 `send_generic(key: int, fields: Dictionary)` 方法，通过 descriptor 动态编码发送，支持通用通道的发送侧。

### GDScript 命名约定自动分发

`net_manager.gd` 的 `_handle_event` 是稳定入口，不再随协议增加而修改：

```gdscript
func _handle_event(event: Dictionary):
    var method_name = "_on_" + event.get("type", "")
    if _handlers.has(method_name):
        _handlers[method_name].call(method_name, event)
    elif has_method(method_name):
        call(method_name, event)
    else:
        _on_unknown_event(event)
```

框架级保留方法：`_on_connected`、`_on_disconnected`、`_on_error`、`_on_raw`、`_on_unknown_event`。

业务协议全部走命名约定：`rsp_login` → `_on_rsp_login(event)`，`dsp_move` → `_on_dsp_move(event)`。

#### 热更处理器动态加载

业务处理器放在 `src/core/net/handlers/` 下，启动时由 `net_manager.gd` 自动扫描加载：

```gdscript
var _handlers: Dictionary = {}

func _load_handlers():
    var dir = DirAccess.open("res://src/core/net/handlers/")
    if dir == null:
        return
    dir.list_dir_begin()
    var fname = dir.get_next()
    while fname != "":
        if fname.ends_with(".gd"):
            var script = ResourceLoader.load("res://src/core/net/handlers/" + fname)
            if script:
                var handler = script.new()
                handler.bind(self)
                for method in handler.get_method_list():
                    if method.name.begins_with("_on_"):
                        _handlers[method.name] = handler
        fname = dir.get_next()
```

热更时将新 `.gd` 打入 PCK，客户端重新扫描即可生效，无需修改分发逻辑。

### 新增协议的两条路径

**路径 A：热更（不重编 Rust DLL）**

适用于新增协议和已有协议字段变更两种场景：

1. 在 proto 中添加新 message / EKey，或修改已有 message 的字段
2. 运行 `genpb --lang rust --flag client`，产出新 `protocol.desc` + `protocol_meta.json`
3. 编写或更新 `handler_xxx.gd` 处理协议
4. 将 `.desc` + `.json` + `.gd` 打入 PCK 热更包
5. 客户端下载 PCK → ProtocolRegistry 加载后自动检测指纹差异 → 变更协议走通用通道解码 → GDScript handler 消费

**路径 B：正式版本（重编 Rust）**

1. proto 中添加新 message 和 EKey
2. 运行 `genpb`，产出 Rust 编译通道代码 + desc + meta
3. 补充 `NetEvent` variant + `convert_server_message` + `event_to_dict`
4. 重编 Rust → 协议自然升级到编译通道，性能更优
5. 通用通道的回退能力仍然保留

两条路径不冲突：协议先通过热更上线验证，稳定后下个版本编入 Rust 获得强类型与最优性能。

## 关键约定

- `gdbridge` 是唯一的 cdylib 桥接 crate，内部按模块文件扩展，避免多 `.gdextension` 的复杂度。
- 新增 Rust 功能时，在 `rust/lib/` 下添加独立核心库 crate，在 `gdbridge/src/` 中增加对应桥接文件。
- `protocol_meta.json` 不区分通道——编译通道由 Rust 二进制内编译了哪些协议自动决定，meta 只提供 key-message-event_name 的映射。
- `rust/.gdignore` 避免 Godot 扫描 Rust 构建目录。
- `comm/tools/genpb/proto/*.proto` 是唯一协议真相源，Go / C# / Rust 生成均由 `genpb` 统一产出。

## 构建

### Rust 扩展

```powershell
cd rust
./scripts/build.ps1
```

发布构建：

```powershell
./scripts/build.ps1 -Profile release
```

产物自动复制到 `addons/gdbridge/bin/`。

### 从项目根目录构建

- Windows：`build.bat`
- Linux/macOS：`build.sh`

## 运行验证

使用 Godot 打开 `project.godot`，或命令行执行：

```powershell
<godot_bin> --headless --path . --quit
```

若扩展加载成功，日志应包含：

- `Initialize godot-rust ...`
- `[Net] NetClientBridge ready`
- `[Net] ProtocolRegistry loaded: N messages`
