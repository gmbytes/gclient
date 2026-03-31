# rclient

`rclient` 是一个 Godot 客户端工程，采用 **GDScript + Rust GDExtension** 的组合开发：

- `client/`: 业务客户端（场景、脚本、资源、Rust 扩展）
- `engine/`: Godot 引擎源码（按需修改引擎层）

## 目录规划

```text
rclient/
├── client/
│   ├── project.godot
│   ├── build.bat
│   ├── build.sh
│   ├── src/                    # GDScript + 场景（按功能模块）
│   │   ├── app/
│   │   ├── core/
│   │   │   └── net/
│   │   ├── ui/
│   │   │   └── menu/
│   │   ├── autoload/
│   │   ├── game/
│   │   └── utils/
│   ├── assets/                 # 资源（audio/textures/fonts/shaders/themes）
│   ├── data/                   # 静态配置与数据表
│   ├── addons/gdbridge/
│   │   ├── gdbridge.gdextension
│   │   └── bin/                # 动态库输出目录（git ignore）
│   └── rust/                   # Rust workspace
│       ├── gdbridge/           # 通用 Godot 桥接层（cdylib，按模块扩展）
│       │   └── src/
│       │       ├── lib.rs
│       │       └── net_bridge.rs
│       ├── netcore/            # 网络核心库
│       └── scripts/build.ps1
└── engine/
```

## 关键约定

- `gdbridge` 是唯一的 cdylib 桥接 crate，内部按模块文件（`net_bridge.rs`、将来的 `storage_bridge.rs` 等）扩展功能，避免多 .gdextension 的复杂度。
- 新增 Rust 功能时，在 `rust/` 下添加独立核心库 crate，然后在 `gdbridge/src/` 中增加对应桥接文件即可。
- Rust 已内聚到 `client/rust/`，与客户端业务同仓维护。
- `engine/` 独立保存上游 Godot 源码，避免与业务代码混杂。
- `client/rust/.gdignore` 用于避免 Godot 扫描 Rust 构建目录。

## 构建方式

### 方式 1：在 Rust 子目录构建

```powershell
cd client/rust
./scripts/build.ps1
```

发布构建：

```powershell
./scripts/build.ps1 -Profile release
```

### 方式 2：在 `client/` 目录构建

- Windows: `client/build.bat`
- Linux/macOS: `client/build.sh`

以上脚本都会编译 `gdbridge` 并复制产物到 `client/addons/gdbridge/bin/`。

## 运行验证

使用 Godot 打开 `client/project.godot`，或命令行执行：

```powershell
engine/bin/godot.windows.editor.x86_64.exe --headless --path client --quit
```

若扩展加载成功，日志应包含：

- `Initialize godot-rust ...`
- `[Net] NetClientBridge ready`
