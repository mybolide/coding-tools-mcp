# Coding Tools MCP Rust - 项目上下文

> 本文档是项目上下文的索引文件，提供项目概览和文档导航。

## 项目概览

| 属性 | 值 |
|------|-----|
| 项目名称 | Coding Tools MCP Rust |
| 版本 | 0.0.0（重构中） |
| 语言 | Rust + TypeScript |
| 框架 | Tauri 2 + Svelte |
| 类型 | 桌面客户端 + 内嵌 MCP 运行时 |
| 描述 | 用 Rust/Tauri 重构 Coding Tools MCP 桌面客户端，内嵌 MCP 核心，单二进制分发 |

## 文档导航

### [技术栈](./project-context/tech-stack.md)
项目使用的语言、框架、工具

### [架构设计](./project-context/architecture.md)
项目结构、目录说明、设计模式

### [如何开发](./project-context/how-to-develop.md)
开发新功能的基本步骤

### [如何编写测试](./project-context/how-to-test.md)
测试框架和测试编写规范

### [代码图谱洞察](./graph-insights/latest.md)
模块依赖、调用链和影响面摘要

### [设计系统](./design-system.md)
2026 开发者工具 UI 审美、色彩、字体、交互规范

## 参考实现

旧版 Python 实现完整归档在 `old/` 目录：

- `old/coding_tools_mcp/server.py` — MCP 核心（~5400 行）
- `old/apps/desktop-client/` — PySide6 桌面客户端
- `old/docs/profile-v0.1.md` — MCP 协议契约
- `old/tests/compliance/` — 71 项合规测试

## 快速开始

1. 阅读 [技术栈](./project-context/tech-stack.md) 了解项目使用的技术
2. 阅读 [架构设计](./project-context/architecture.md) 了解项目结构
3. 阅读 [代码图谱洞察](./graph-insights/latest.md) 理解模块边界
4. 查看 `docs/specs/rust-desktop-client/` 了解当前功能规格

## 开发时查看对应文档

### 新功能开发
- 先调用 `start_feature` MCP 工具
- 规格文档：`docs/specs/<feature>/`
- 通过 `check_spec` 后再写实现代码

### 理解 MCP 协议行为
- `old/docs/profile-v0.1.md` — 协议规范
- `old/docs/tools-and-schemas.md` — 工具 schema

### 编写测试
- [how-to-test.md](./project-context/how-to-test.md)
- `old/tests/compliance/` — 行为参考

---
*生成时间: 2026-07-10*
*生成工具: MCP Probe Kit - init_project_context*
