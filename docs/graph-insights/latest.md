# Graph Insights

更新时间：2026-07-10

## 仓库状态

当前仓库处于重构初期：

- 根目录仅有 `old/`（旧版 Python 完整实现）和 `docs/`（新文档）
- 尚无 Rust 源码或 Tauri 工程
- 新功能 `rust-desktop-client` 规格待落盘

## 旧版核心路径（参考实现）

### MCP 核心

- `old/coding_tools_mcp/server.py` — ~5400 行单体，17 个 MCP 工具 + HTTP/stdio transport + OAuth
- `old/coding_tools_mcp/landlock_exec.py` — Linux Landlock 沙箱辅助

### 桌面客户端

- `old/apps/desktop-client/mcp_desktop_client/app.py` — ~1228 行 PySide6 UI
- `old/apps/desktop-client/mcp_desktop_client/runtime.py` — ~508 行 MCP 进程管理
- `old/apps/desktop-client/mcp_desktop_client/actions_runtime.py` — ~537 行 Actions 进程管理
- `old/apps/desktop-client/mcp_desktop_client/models.py` — Workspace 数据模型
- `old/apps/desktop-client/mcp_desktop_client/storage.py` — 配置持久化
- `old/apps/desktop-client/mcp_desktop_client/health.py` — 健康检查

### Actions 网关

- `old/coding_tools_actions/app.py` — FastAPI 网关，~149 行

### 测试与契约

- `old/tests/compliance/` — 71 项合规测试（全 PASS）
- `old/docs/profile-v0.1.md` — MCP 协议规范（1330 行）

## 结构结论

1. `server.py` 是高耦合核心，Rust 重构需拆分为独立 crate
2. 桌面客户端的 bug 主要来自外部进程编排（psutil 启发式），内嵌 MCP 可消除
3. `runtime.py` 与 `actions_runtime.py` 大量重复，Rust 版应统一为 Runtime Supervisor
4. 合规测试是重构的行为契约，必须优先移植

## 对 rust-desktop-client 的启发

- Workspace 是第一主对象
- MCP 核心内嵌到 Tauri Rust 后端，不再管理外部 Python 进程
- 隧道（FRP/Cloudflare）仍管理外部进程，但用 tokio::process 正规监督
- 密钥走系统钥匙串，不走明文 JSON
- UI 用 Svelte + Web 设计系统，替代 PySide6

## 模块依赖（目标架构）

```
UI (Svelte)
  ↓ Tauri IPC
Commands Layer
  ↓
Workspace Store ←→ Secret Store (keyring)
  ↓
Runtime State Machine
  ├── MCP Server (内嵌 axum)
  └── Tunnel Supervisor (cloudflared/frp)
```

---
*生成时间: 2026-07-10*
*来源: code_insight + 人工分析 old/ 参考实现*
