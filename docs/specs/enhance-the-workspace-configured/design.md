# 设计文档：本地 MCP 自动工具发现与启动可靠性

## 概述

本设计将本地 stdio 上游的“手写白名单”替换为“自动发现 + 默认可见 + 持久化关闭项”。它还将上游初始化从同步运行时路径移到异步启动流程，避免 `block_on` 嵌套导致的 listener 启动停滞。

**对应需求:** FR-1、FR-2、FR-3、FR-4、FR-5、NFR-1、NFR-2、NFR-3。

## 技术方案

采用工作区拥有的 stdio 上游管理器，启动时异步读取工具目录；工具可见性由持久化关闭项计算。临时探测和常规启动共用同一 JSON-RPC 握手实现，避免 UI 与实际暴露目录不一致。HTTP listener 只接收已准备好的 manager，从而不在同步状态机内等待异步子进程响应。

## 架构设计

```text
工作区配置页
  ├─ 临时发现命令 ── UpstreamMcpManager::inspect
  └─ disabled_tools 保存
          │
          ▼
异步 MCP 启动路径
  Runtime command ── await spawn_listener ── UpstreamMcpManager::start
                                              ├─ initialize
                                              ├─ tools/list
                                              └─ 路由 = 已发现 - 已关闭
                                                           │
ChatGPT / 云端 Codex ── 认证 ── /mcp router ── 核心目录 + 上游目录
```

## 数据模型

`UpstreamMcpConfig` 增加 `disabled_tools: Vec<String>`，默认空。`allowed_tools` 保留为仅反序列化的旧版兼容字段。

| 字段 | 类型 | 语义 |
|---|---|---|
| `allowed_tools` | string array | 旧字段；非空且 `disabled_tools` 为空时采用仅允许这些工具的旧行为 |
| `disabled_tools` | string array | 新字段；列出的已发现工具不公开，未列出的工具默认公开 |

保存新版开关时，前端发送 `disabled_tools`，并把 `allowed_tools` 置空，表示用户已迁移到默认公开模型。

## 异步生命周期

1. `start_mcp_service` 在进入 RuntimeSupervisor 前异步创建 `UpstreamMcpManager`。
2. manager 完成所有 enabled upstream 的初始化和目录发现后，作为已准备依赖传入 listener。
3. RuntimeSupervisor 只执行已准备 listener 的同步登记和端口绑定，立即进入 Running；任一异步预检失败会直接返回 `AppError`，UI 进入错误状态。
4. 临时工具发现使用相同的 manager/protocol，但完成后关闭所有子进程，不修改运行中的 manager。

## 模块职责

| 模块 | 职责 |
|---|---|
| `workspace::model` | `disabled_tools` 兼容模型、选择规则和校验 |
| `mcp::upstream` | 启动、发现、可见性过滤、临时探测、关闭 |
| `mcp::listener` | 接收已准备 manager，不在同步路径中等待异步协议 |
| `runtime::supervisor` | 只管理 listener 的状态登记、端口和停止 |
| `commands::runtime` | 预检上游、失败时返回状态错误、启动 listener |
| `commands::workspace` | 暴露临时发现命令并保持保存时重启/回滚 |
| `UpstreamMcpForm.svelte` | 获取工具、逐项开关、转换旧白名单和安全提示 |

## 文件结构

```text
src-tauri/src/
├── workspace/model.rs                 修改：disabled_tools 与旧白名单兼容规则
├── mcp/upstream.rs                    修改：发现、过滤、临时探测与测试
├── mcp/listener.rs                    修改：使用异步预备的 manager
├── runtime/supervisor.rs              修改：接受已准备的 listener 依赖
├── commands/runtime.rs                修改：异步上游预检与错误状态
├── commands/workspace.rs              修改：临时发现 Tauri command
├── commands/mod.rs                    修改：导出临时发现 command
└── lib.rs                             修改：注册 command
src/lib/
├── types.ts                           修改：配置和发现结果类型
└── components/UpstreamMcpForm.svelte  修改：发现目录与逐项开关
```

## 公共契约

| 方法 | 输入 | 输出 |
|---|---|---|
| `discover_upstream_tools` | 一项未保存的上游配置 | 已发现工具的安全目录 |
| `/mcp tools/list` | 标准 MCP 请求 | 核心工具与可见的上游工具 |
| `/mcp tools/call` | 命名空间公开工具名 | 上游 MCP 结果 |

## 兼容与迁移

- 没有 `disabled_tools` 且 `allowed_tools` 非空：保留白名单。
- 没有 `disabled_tools` 且 `allowed_tools` 空：默认公开全部。
- 新版保存：永远写入 `disabled_tools`（可为空），清空 `allowed_tools`。
- 旧配置因 Serde default 不需要磁盘迁移。

## 失败与可观测性

- 所有 `initialize` 和 `tools/list` 调用受 10 秒上限约束。
- 预检失败通过 Tauri command 返回错误；不留下端口监听器或子进程。
- 日志记录上游显示名、安全错误类别与持续时间；不记录 command、args、env、stderr 和工具参数。

## 测试策略

- 模型测试：默认公开、关闭项、旧版非空白名单兼容和校验。
- 上游测试：临时发现、默认公开、关闭项过滤、旧版白名单过滤和 fixture 转发。
- 运行时测试：异步预检失败可返回错误且不创建 listener。
- Svelte 检查：工具探测和开关类型无错误。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 上游启动慢 | 中 | 超时、异步等待和清晰失败提示 |
| 新工具自动公开 | 高 | 用户明确要求默认公开；保留逐项关闭和旧非空白名单兼容 |
| 旧白名单被忽略 | 高 | 仅空白名单采用新默认；非空白名单继续限制 |
| 子进程遗留 | 中 | 所有临时和启动失败路径调用 shutdown |
