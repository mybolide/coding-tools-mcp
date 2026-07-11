# 技术栈

> 本文档描述 Coding Tools MCP Rust 的技术栈信息。

## 基本信息

| 属性 | 值 |
|------|-----|
| 项目名称 | Coding Tools MCP Rust |
| 版本 | 0.0.0 |
| 语言 | Rust 1.77+ / TypeScript |
| 框架 | Tauri 2 |

## 核心技术

| 类别 | 技术 | 用途 |
|------|------|------|
| 桌面壳 | Tauri 2 | 跨平台桌面应用框架 |
| 后端语言 | Rust | MCP 核心、进程管理、状态机 |
| 前端 | Svelte + TypeScript | UI 界面 |
| 异步运行时 | tokio | 异步 I/O、进程监督 |
| HTTP 服务 | axum | 内嵌 MCP Streamable HTTP |
| MCP SDK | rmcp | MCP 协议实现 |
| Git 操作 | git2 | git_status / git_diff 等工具 |
| 密钥存储 | keyring | 系统钥匙串（Windows Credential Manager 等） |
| 序列化 | serde + serde_json | 配置持久化 |

## 开发工具

| 类别 | 工具 | 用途 |
|------|------|------|
| 包管理 | cargo / pnpm | Rust / 前端依赖 |
| 构建 | tauri-cli | 桌面应用构建 |
| 测试 | cargo test | Rust 单元/集成测试 |
| 代码检查 | clippy / rustfmt | Rust lint 与格式化 |
| 前端检查 | eslint / prettier | TypeScript 检查 |

## 参考实现技术栈（old/）

| 类别 | 技术 | 说明 |
|------|------|------|
| MCP 核心 | Python 3.11+ | `coding_tools_mcp/server.py` |
| 桌面客户端 | PySide6 | `apps/desktop-client/` |
| Actions 网关 | FastAPI + uvicorn | `coding_tools_actions/` |
| 测试 | pytest + unittest | `tests/compliance/` |

## 主要依赖（计划）

### Rust (src-tauri/Cargo.toml)

- `tauri` — 桌面壳
- `tokio` — 异步运行时
- `axum` — HTTP server
- `rmcp` — MCP 协议
- `git2` — Git 操作
- `keyring` — 系统密钥存储
- `serde` / `serde_json` — 序列化
- `tracing` — 结构化日志

### 前端 (package.json)

- `svelte` — UI 框架
- `@tauri-apps/api` — Tauri 前端 API
- `tailwindcss` — 样式

---
*返回索引: [../project-context.md](../project-context.md)*
