# 需求文档：本地 MCP Streamable HTTP 与 JSON 导入

## 功能概述

工作区的本地 MCP 代理除现有 `stdio` 外，支持已由管理员明确配置的 Streamable HTTP 上游。远端客户端仍连接本应用的同一 MCP 地址，桌面端负责初始化、发现和转发已公开工具。管理员还可通过“添加”菜单快速创建或粘贴 JSON 导入配置。

## 术语定义

- **stdio 上游**：由桌面端启动、通过标准输入输出传递 JSON-RPC 的本机 MCP 进程。
- **Streamable HTTP 上游**：由管理员提供 URL、通过 HTTP POST 传递 MCP JSON-RPC 的服务；可返回 JSON 或 SSE。
- **公开工具**：经工具前缀命名并显示在本应用公网 MCP 工具目录中的上游工具。

## 范围边界

**In Scope**

- 在一个工作区中混合配置 `stdio` 和 `streamableHttp` 上游。
- 对 Streamable HTTP 完成 `initialize`、`notifications/initialized`、`tools/list`、`tools/call`、会话 ID 续传和关闭时本地状态释放。
- 在表单中选择传输类型；HTTP 类型编辑 URL 和可选请求头，stdio 类型编辑命令、参数和环境变量。
- 提供“添加 → 快速创建 / 从 JSON 导入”。
- 支持单个配置对象和 `{ "mcpServers": { "name": config } }` JSON 导入结构。

**Out of Scope**

- 不自动扫描、连接或代理未经管理员保存的 URL。
- 不新增 OAuth 登录流程、HTTP 轮询、SSE 长连接订阅或从 URL 下载配置。
- 不删除、不改写现有 stdio 配置与旧白名单语义。

## 需求列表

### FR-1: 配置 Streamable HTTP 上游

**优先级:** Must

作为工作区管理员，我希望选择 Streamable HTTP 并填写 URL 与请求头，以便将已授权的 HTTP MCP 一并公开。

1. WHEN 管理员选择 `streamableHttp` THEN 系统 SHALL 显示 URL 和可选请求头，不显示 stdio 专属字段。
2. WHEN 保存已启用的 Streamable HTTP 配置 THEN 系统 SHALL 仅接受 `http` 或 `https` URL、有效请求头、唯一 ID、名称与已启用前缀。
3. IF HTTP 上游初始化、工具发现或工具调用失败 THEN 系统 SHALL 返回带上游名称的错误，且不得启动不完整的公网 MCP 服务。
4. WHILE 上游会话有效 THEN 系统 SHALL 在后续请求中传递服务返回的 `Mcp-Session-Id`。

### FR-2: 统一公开与调用上游工具

**优先级:** Must

作为远端 MCP 客户端使用者，我希望无论上游传输类型如何，均通过同一公网 MCP 目录调用被公开的工具。

1. WHEN 工作区 MCP 启动 THEN 系统 SHALL 对每个已启用的 stdio 或 Streamable HTTP 上游执行初始化和工具发现。
2. WHEN 上游返回一个合法工具目录 THEN 系统 SHALL 沿用前缀、`disabled_tools` 和旧 `allowed_tools` 的公开策略。
3. WHEN 调用公开工具 THEN 系统 SHALL 将原始工具名和参数发送给其所属上游，并返回原始 MCP 工具结果。
4. WHEN 未配置上游或全部工具关闭 THEN 系统 SHALL 保持现有核心工具目录行为。

### FR-3: 通过 JSON 导入配置

**优先级:** Must

作为工作区管理员，我希望从 JSON 粘贴一个或多个 MCP 配置，以便避免手动输入长 URL、请求头或 stdio 参数。

1. WHEN 点击“添加” THEN 系统 SHALL 展示“快速创建”和“从 JSON 导入”。
2. WHEN 导入单一 MCP 配置对象 THEN 系统 SHALL 创建一份可编辑草稿。
3. WHEN 导入 `{ "mcpServers": { "名称": 配置 } }` THEN 系统 SHALL 为每个可识别项创建草稿，并以对象键作为未提供名称时的默认名称。
4. IF JSON 语法、传输类型、字段类型、URL、名称或前缀无效 THEN 系统 SHALL 显示错误且不得修改当前草稿。
5. WHEN 导入成功 THEN 系统 SHALL 在保存前让管理员查看、修改、启用及选择公开工具。

## 非功能需求

- **NFR-1（兼容性）:** 未含 `type` 的历史配置继续按 `stdio` 解析；旧白名单继续是限制性公开策略。
- **NFR-2（安全）:** 仅保存的管理员配置可成为上游；请求头、环境变量、命令和参数不得出现在公共工具目录、工具结果或普通请求日志。
- **NFR-3（可靠性）:** HTTP 请求使用现有初始化/调用超时；关闭 HTTP 上游不终止外部服务。

## 依赖关系

- 前端配置表单经 Tauri `discover_upstream_tools` 和 `update_workspace` 连接 Rust 配置模型。
- MCP 服务启动链路复用 `UpstreamMcpManager` 的公开工具目录和失败回滚。
- HTTP 客户端使用已有 `reqwest` 依赖。
