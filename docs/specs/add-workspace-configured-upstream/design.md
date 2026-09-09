# 设计文档：工作区本地 MCP 上游

## 概述

本设计实现受控的 stdio MCP 聚合能力。Coding Tools MCP 保持唯一的对外 Streamable HTTP 服务器、认证入口和隧道终点；它在本机为每个启用的工作区配置启动一个上游 MCP 客户端，并只转发允许的工具。

**对应需求:** FR-1、FR-2、FR-3、FR-4、FR-5、NFR-1、NFR-2、NFR-3、NFR-4。

## 技术方案

### 架构设计

```text
ChatGPT / 云端 Codex
        │  OAuth 或 Bearer
        ▼
现有 axum /mcp listener
        │
        ▼
MCP request router ───── 核心 tools registry
        │
        ▼
UpstreamMcpManager
        │  每个已启用配置一条独立 JSON-RPC stdio 通道
        ├── example ── command → upstream-mcp
        └── 其他用户配置的 stdio MCP
```

### 模块职责

| 模块 | 职责 | 关联需求 |
|---|---|---|
| `workspace::model` | 声明并验证可序列化的上游配置模型 | FR-1、FR-5 |
| `mcp::upstream` | 启动和停止子进程、实现逐行 JSON-RPC、初始化、目录缓存和调用转发 | FR-3、FR-4、FR-5 |
| `mcp::server` | 将允许的上游目录与核心目录合并，路由并规范化工具调用结果 | FR-4 |
| `mcp::listener` | 在创建 HTTP listener 前建立上游 manager，在优雅关闭时释放它 | FR-3、FR-5 |
| `runtime::supervisor` | 将配置变更后的重启与现有 MCP 生命周期串联 | FR-1、FR-3 |
| 工作区 Svelte 页面与 API | 编辑配置、显示安全提示并在保存后重启正在运行的 MCP | FR-2 |

### 上游配置模型

配置存储在 `WorkspaceProfile.runtime.upstream_mcps`，避免创建全局服务或将其混入隧道设置。

| 字段 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | string | 唯一、稳定、URL 安全 | 内部路由与日志 ID |
| `name` | string | 工作区内唯一、非空 | UI 显示名 |
| `enabled` | boolean | 默认 `false` | 是否随 MCP 运行时启动 |
| `transport` | string | 仅允许 `stdio` | 为未来 HTTP 预留显式类型 |
| `command` | string | 非空绝对或系统可解析命令 | 子进程可执行文件 |
| `args` | string array | 每项为单独参数 | 不经 shell 拼接 |
| `env` | string map | 非空键、可选空值 | 仅注入子进程环境 |
| `tool_prefix` | string | 小写字母、数字、短横线；工作区内唯一 | 公开工具名的前缀 |
| `allowed_tools` | string array | 无重复；默认空 | 可公开的上游原始工具名 |

通过 `Command` 及其 `args` API 启动，不使用 shell，因此配置字符串不会被解释为命令连接、重定向或额外执行。

### stdio JSON-RPC 协议

`UpstreamMcp` 拥有子进程、异步 stdin writer、stdout reader 和原子递增请求 ID。启动顺序为：启动子进程、发送 `initialize`、验证 `result`、发送 `notifications/initialized`、发送 `tools/list`、过滤和缓存工具。所有请求以一行 UTF-8 JSON 写入 stdin，并从 stdout 匹配相同 ID 的一行 JSON-RPC 响应。

首版每个上游请求串行执行，避免不支持并发 stdout 交错的 stdio 服务产生响应错配。单个请求和初始化均有超时；超时、EOF、无效 JSON、错误 ID 或进程退出都标记该上游为不健康。

### 工具公开与转发

`tools/list` 从核心工具 registry 获取原有目录，再追加上游缓存的工具。公开名称规则为：`<tool_prefix>__<original_name>`。启动前校验公开名称在所有启用上游和核心工具中唯一。

`tools/call` 先判断该名称是否对应上游路由，再从公开工具名还原原始名称。只有缓存目录中且被 `allowed_tools` 包含的名称能转发。调用结果保留 MCP `content`、`structuredContent` 和 `isError` 结构；JSON-RPC 错误转为安全的本服务错误结果。

### 生命周期与失败处理

`spawn_listener` 在绑定 HTTP 端口前构建 `UpstreamMcpManager`。任一已启用上游初始化失败即拒绝这次本地 MCP 启动，防止 UI 显示“已启动”但配置声称启用的工具消失。管理器存入 listener state；shutdown 时先停止 HTTP 接入，再关闭上游 stdin，等待短暂退出并在必要时终止子进程。

保存、启用、停用或删除上游配置时，若工作区 MCP 正在运行，沿用现有 `restart_mcp_by_id` 重启路径。因此单次重启同时重新加载配置和工具目录。

### 安全设计

- 外层 HTTP 认证、工作区路径策略和隧道不变；上游调用只发生在认证完成之后。
- 默认白名单为空，发现到的工具不等于被公开的工具。
- 不返回环境变量、完整命令行、参数或上游 stdout/stderr 正文；日志只记录安全的状态和时长。
- `env` 配置为工作区私有数据；前端编辑回显现有键和值给本机所有者，但远端 MCP schema 和响应中从不包含它。
- 只允许来自持久化配置的 `stdio` 上游，不提供远端输入指定 command、args、环境或上游 URL 的工具。

## API 设计

| 方法 | 签名 | 作用 | 关联需求 |
|---|---|---|---|
| `save_workspace` | `WorkspaceProfile -> AppResult<WorkspaceProfile>` | 通过现有工作区保存路径持久化上游配置 | FR-1 |
| `UpstreamMcpManager::start` | `&[UpstreamMcpConfig] -> Result<Self, String>` | 启动并发现所有启用上游 | FR-3 |
| `UpstreamMcpManager::list_tools` | `-> Vec<Value>` | 返回验证后的公开工具目录 | FR-4 |
| `UpstreamMcpManager::call_tool` | `&str, Value -> Result<Value, UpstreamError>` | 调用已路由的上游工具 | FR-4 |
| `handle_request` | `SharedState, JSON-RPC request -> JSON-RPC response` | 合并或转发工具请求 | FR-4 |

## 文件结构

```text
src-tauri/src/
├── workspace/model.rs                 修改：上游配置模型和验证
├── mcp/upstream.rs                    新建：stdio JSON-RPC 上游管理器
├── mcp/mod.rs                         修改：导出上游模块
├── mcp/server.rs                      修改：目录合并和调用路由
├── mcp/listener.rs                    修改：启动与关闭管理器
├── commands/workspace.rs              修改：保存时验证和运行时重启
└── runtime/supervisor.rs              修改：传递上游生命周期错误
src/
├── lib/types.ts                       修改：前端数据类型
├── lib/api/workspace.ts               修改：序列化配置
├── lib/components/UpstreamMcpForm.svelte 新建：配置表单
└── routes/workspace/[id]/+page.svelte 修改：加载和保存表单
```

## 设计决策

### 决策 1：由本应用代理而非让 ChatGPT 直接运行本机 stdio MCP

**问题**：某些本机 MCP 仅提供 stdio 服务，而云端客户端无法启动本机命令。

**选项**：

1. 为每个本机 stdio MCP 单独部署 HTTP 桥接和隧道。
2. 在 Coding Tools MCP 内实现受控的 stdio 上游代理。

**决策**：选择选项 2。

**理由**：复用已有的公网入口、OAuth、日志、工作区和隧道生命周期，用户只维护一个 MCP URL。

### 决策 2：前缀与显式白名单

**问题**：上游工具可能与核心或其他上游重名，并可能拥有高权限。

**选项**：

1. 原样公开全部工具。
2. 以稳定前缀公开并要求显式白名单。

**决策**：选择选项 2。

**理由**：名称稳定、故障可定位，且远端不会因新增上游工具而自动获得额外本机能力。

### 决策 3：启动失败即拒绝启动 MCP 运行时

**问题**：已启用上游初始化失败时，是否仍提供核心 MCP。

**选项**：

1. 继续启动核心工具并静默略过该上游。
2. 返回启动失败，要求修复或停用该上游。

**决策**：选择选项 2。

**理由**：桌面端状态和用户保存的“已启用”意图保持一致，远端工具目录不会悄然变化。

## 测试策略

- Rust 单元测试覆盖配置验证、公开名称生成、白名单过滤、碰撞拒绝和错误映射。
- Rust 集成测试启动一个最小 stdio MCP fixture，验证初始化、`tools/list`、`tools/call`、shutdown 和超时失败。
- MCP server 测试验证核心目录在没有上游配置时完全不变，并验证公开工具可调用、未公开工具被拒绝。
- Svelte 类型检查覆盖表单和序列化；人工验证通用上游配置可以编辑且不会泄露环境变量。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 上游 stdout 输出非协议日志 | 高 | 严格 JSON-RPC 解析、失败即停止本次启动、将安全诊断写入本地日志 |
| 上游工具隐含写入设备或构建副作用 | 高 | 默认空白名单、命名空间、保留外层 OAuth 和现有审批策略 |
| Windows 含空格路径 | 中 | 使用 `Command` 的分离 command/args API，单测覆盖路径参数 |
| 子进程遗留 | 中 | listener shutdown 拥有 manager，超时后显式终止，并测试 stop/restart |
| 上游工具 schema 不可信或冲突 | 中 | 校验工具对象、过滤名单、拒绝重复公开名称 |
