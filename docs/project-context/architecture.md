# 架构设计

> 本文档描述 Coding Tools MCP Rust 的架构和项目结构。

## 项目结构（目标）

```
coding-tools-mcp-rust/
├── src-tauri/              # Tauri Rust 后端
│   ├── src/
│   │   ├── main.rs         # 入口
│   │   ├── lib.rs          # 库入口
│   │   ├── commands/       # Tauri IPC 命令
│   │   ├── workspace/      # Workspace 配置存储
│   │   ├── runtime/        # Runtime 状态机
│   │   ├── mcp/            # 内嵌 MCP 协议 + 工具
│   │   ├── tunnel/         # FRP / Cloudflare 隧道管理
│   │   ├── auth/           # OAuth / Bearer 认证
│   │   └── health/         # 健康检查
│   └── Cargo.toml
├── src/                    # Svelte 前端
│   ├── lib/
│   │   ├── components/     # UI 组件
│   │   └── stores/         # 状态管理
│   └── routes/             # 页面路由
├── docs/                   # 项目文档
│   ├── specs/              # 功能规格
│   ├── project-context/    # 项目上下文
│   └── graph-insights/     # 代码图谱
├── tests/                  # 合规测试（从 old/ 移植）
├── old/                    # 旧版 Python 参考实现
└── AGENTS.md               # Agent 入口
```

## 当前状态

仓库处于重构初期，根目录仅有 `old/` 参考实现和 `docs/` 文档。Tauri 工程骨架待创建。

## 架构模式

### 分层设计

```
┌─────────────────────────────────────────┐
│  UI Layer (Svelte)                      │
│  Workspace 卡片 / 配置 / 日志 / 健康检查  │
├─────────────────────────────────────────┤
│  Tauri Commands (IPC)                   │
│  前端 ↔ Rust 后端通信                    │
├─────────────────────────────────────────┤
│  App Orchestrator (Rust)                │
│  Workspace Store / Runtime State Machine│
├─────────────────────────────────────────┤
│  MCP Core (内嵌, Rust)                  │
│  axum HTTP /mcp + 工具实现               │
├─────────────────────────────────────────┤
│  Tunnel Supervisor (Rust)               │
│  管理 cloudflared / frp 外部进程         │
└─────────────────────────────────────────┘
```

### 与旧版 Python 客户端的关键差异

| 维度 | 旧版 (PySide6) | 新版 (Tauri) |
|------|---------------|-------------|
| MCP 运行时 | 外部 Python 子进程 | **内嵌 Rust** |
| 进程管理 | psutil 启发式猜 PID | 状态机 + 自有子进程 |
| UI | Qt 样式表 | Web 设计系统 |
| 密钥 | 明文 secrets.json | 系统钥匙串 |
| 分发 | 需要 Python 环境 | 单二进制 |

## 核心模块

### workspace/
- **职责**: Workspace 配置的 CRUD、持久化、密钥分离存储
- **参考**: `old/apps/desktop-client/mcp_desktop_client/models.py`, `storage.py`

### runtime/
- **职责**: MCP 运行时生命周期状态机（Stopped → Starting → Running → Stopping → Error）
- **参考**: `old/apps/desktop-client/mcp_desktop_client/runtime.py`

### mcp/
- **职责**: MCP 协议实现、17 个工具、HTTP transport
- **参考**: `old/coding_tools_mcp/server.py`, `old/docs/profile-v0.1.md`

### tunnel/
- **职责**: FRP 配置生成、Cloudflare 隧道进程监督
- **参考**: `old/apps/desktop-client/mcp_desktop_client/runtime.py` 中的隧道逻辑

## 入口文件

- **Tauri 入口**: `src-tauri/src/main.rs`（待创建）
- **前端入口**: `src/routes/+page.svelte`（待创建）
- **Agent 入口**: `AGENTS.md`

---
*返回索引: [../project-context.md](../project-context.md)*
