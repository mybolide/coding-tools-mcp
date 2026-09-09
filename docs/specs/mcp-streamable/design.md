# 设计文档：本地 MCP Streamable HTTP 与 JSON 导入

## 概述

本设计覆盖 FR-1 至 FR-3、NFR-1 至 NFR-3。`UpstreamMcpConfig` 保持扁平且向后兼容：`type` 默认为 `stdio`，HTTP 配置新增 `url` 和 `headers`，stdio 保持 `command`、`args`、`env`。

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| HTTP 客户端 | 已有 reqwest | 已支持 JSON、Rustls 和超时；不新增运行时依赖 | FR-1、FR-2 |
| 运行时客户机 | `UpstreamMcp` 枚举 | 将 stdio 与 HTTP 的传输细节隔离，保留统一初始化、发现、路由和工具公开逻辑 | FR-1、FR-2 |
| 导入体验 | Svelte 菜单 + 粘贴对话框 | 满足快速创建与常见配置 JSON 导入，导入内容在保存前仍可审阅 | FR-3 |

### 架构设计

```text
UpstreamMcpForm
  ├─ 添加 → 快速创建
  └─ 添加 → 从 JSON 导入
        └─ WorkspaceProfile.runtime.upstream_mcps
               │
               ▼
UpstreamMcpManager::start
  ├─ StdioClient: spawn → initialize → tools/list
  └─ StreamableHttpClient: POST initialize → session id → POST tools/list
               │
               ▼
公开目录（prefix + visibility） → 本应用 MCP listener → 公网隧道
```

## 数据模型

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `type` | `stdio | streamableHttp` | 缺省 `stdio` | 上游传输 |
| `url` | string | HTTP 类型必须是 `http` 或 `https` URL | Streamable HTTP 端点 |
| `headers` | string map | 合法 HTTP header 名和值 | 仅 HTTP 请求注入 |
| `command/args/env` | existing | 仅 stdio 使用 | 保持历史配置兼容 |

## API 设计

| 方法 | 签名 | 入参 | 出参 | 关联需求 |
|------|------|------|------|----------|
| 上游启动 | `UpstreamMcp::start(config)` | stdio 或 HTTP 配置 | 初始化客户机 | FR-1、FR-2 |
| HTTP 请求 | `StreamableHttpMcp::request` | JSON-RPC 方法、参数、超时 | MCP `result` | FR-1、FR-2 |
| 工具预检 | `discover_upstream_tools` | 未保存配置 | 工具摘要 | FR-1、FR-2 |

## 文件结构

```text
src-tauri/src/workspace/model.rs          # 类型、URL 与 header 校验
src-tauri/src/mcp/upstream.rs             # stdio / HTTP 上游客户机与测试
src/lib/types.ts                          # 传输联合类型
src/lib/components/UpstreamMcpForm.svelte # 传输选择、添加菜单、JSON 导入
README.md                                 # 配置与安全说明
docs/specs/mcp-streamable/                # 本功能规格
```

## 设计决策

### 决策 1: 显式配置的 HTTP 端点，而非通用请求代理

**关联需求:** FR-1、NFR-2

只在应用启动时连接保存的 URL；公开工具路由只持有已经发现的工具名。此举避免远端工具调用携带任意 URL，维持现有工作区所有者授权边界。

### 决策 2: 兼容 JSON 和单事件 SSE 响应

**关联需求:** FR-1、FR-2

HTTP 请求发送 JSON-RPC POST，同时接受 JSON 或 `text/event-stream` 响应。响应中的 `Mcp-Session-Id` 被保存并随请求传递。通知可接受空的成功响应，外部 HTTP 服务在关闭时不会被终止。

### 决策 3: 导入仅生成草稿，不立即持久化或启用

**关联需求:** FR-3、NFR-2

导入函数只接受结构化配置，生成新 ID、默认关闭，并先通过前端形状校验；Rust 保存校验仍是最终防线。请求头被保留以支持服务鉴权，但不会展示于工具目录或日志。

## 测试策略

- Rust 模型测试：`stdio` 兼容、HTTP URL 和 header 约束。
- Rust HTTP 集成测试：本地 Axum fixture 检查 initialize、session header、tools/list、tools/call、disabled 工具及关闭。
- 前端检查：类型与 Svelte 模板。
- 手工/自动文本断言：添加菜单和两种 JSON 导入结构。
- 完整 Rust 库回归与 Tauri Windows 打包。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| HTTP 服务协议差异 | 中 | 限定 JSON/SSE 的标准 JSON-RPC POST 响应，明确错误。 |
| 导入恶意或错误 JSON | 中 | 只作为可审阅草稿；严格类型、URL、header 与保存校验。 |
| 历史 stdio 配置失效 | 高 | `type` 默认 stdio，保留字段与现有集成测试。 |
