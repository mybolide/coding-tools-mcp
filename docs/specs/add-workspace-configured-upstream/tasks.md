# 任务清单：工作区本地 MCP 上游

## 概述

本任务实现工作区级 stdio 上游 MCP 配置和受控工具公开。所有任务回链需求和设计，且不包含 HTTP 上游或自动公开全部工具。

## 交付物清单

- **预计新建文件数**: 2 个
- **预计修改文件数**: 9 个
- **预计新增或修改函数数**: 约 20 个
- **交付物逐项列举**:
  1. `src-tauri/src/mcp/upstream.rs`
  2. `src/lib/components/UpstreamMcpForm.svelte`
  3. `src-tauri/src/workspace/model.rs`
  4. `src-tauri/src/mcp/mod.rs`
  5. `src-tauri/src/mcp/server.rs`
  6. `src-tauri/src/mcp/listener.rs`
  7. `src-tauri/src/commands/workspace.rs`
  8. `src/lib/types.ts`
  9. `src/lib/api/workspace.ts`
  10. `src/routes/workspace/[id]/+page.svelte`
  11. `README.md`

## 任务列表

### 阶段 1：配置契约

- [ ] 1.1 在工作区模型中定义并验证 stdio 上游配置、工具前缀和白名单
  - **证据块**: `src-tauri/src/workspace/model.rs:52` 的 `RuntimeConfig` 是工作区 MCP 运行时设置的持久化结构；`src/lib/types.ts:20` 的 `RuntimeConfig` 是前端镜像。
  - **涉及文件**: `src-tauri/src/workspace/model.rs`（约 110 行修改）、`src/lib/types.ts`（约 35 行修改）。
  - _需求: FR-1、FR-5_ ｜ _设计: 上游配置模型_

- [ ] 1.2 在工作区保存路径中验证配置并重启正在运行的 MCP
  - **证据块**: `src-tauri/src/commands/runtime.rs:178` 的 `restart_mcp_by_id` 是当前安全重启入口；`src-tauri/src/commands/workspace.rs` 保存工作区。
  - **涉及文件**: `src-tauri/src/commands/workspace.rs`（约 80 行修改）。
  - _需求: FR-1、FR-3_ ｜ _设计: 生命周期与失败处理_

### 阶段 2：stdio 上游代理

- [ ] 2.1 实现独立的 stdio JSON-RPC 上游管理器并验证初始化、超时和关闭
  - **证据块**: `src-tauri/src/mcp/listener.rs:41` 同步创建 listener；`src-tauri/Cargo.toml` 已启用 Tokio 的 process、io-util 和 time 特性。
  - **涉及文件**: `src-tauri/src/mcp/upstream.rs`（新建，约 430 行，保持单一职责）、`src-tauri/src/mcp/mod.rs`（约 5 行修改）。
  - _需求: FR-3、FR-5、NFR-1、NFR-2、NFR-3_ ｜ _设计: stdio JSON-RPC 协议_

- [ ] 2.2 在 MCP listener 生命周期中创建、共享并销毁上游管理器
  - **证据块**: `src-tauri/src/mcp/listener.rs:91` 组装 `ListenerState`；`src-tauri/src/mcp/listener.rs:123` 在 oneshot shutdown 后结束 HTTP 服务。
  - **涉及文件**: `src-tauri/src/mcp/listener.rs`（约 90 行修改）、`src-tauri/src/runtime/supervisor.rs`（约 20 行修改）。
  - _需求: FR-3、FR-5_ ｜ _设计: 生命周期与失败处理_

- [ ] 2.3 将白名单上游目录合并进 MCP 工具目录并转发带前缀的调用
  - **证据块**: `src-tauri/src/mcp/server.rs:25` 仅从核心 registry 构建 `tools/list`；`src-tauri/src/mcp/server.rs:58` 仅接受核心工具调用。
  - **涉及文件**: `src-tauri/src/mcp/server.rs`（约 130 行修改）、`src-tauri/src/mcp/listener.rs`（约 20 行修改）。
  - _需求: FR-4、FR-5、NFR-4_ ｜ _设计: 工具公开与转发_

### 阶段 3：桌面端配置

- [ ] 3.1 创建可编辑的上游 MCP 表单
  - **证据块**: `src/lib/components/RuntimePolicyForm.svelte` 展示现有运行时配置表单模式；`src/routes/workspace/[id]/+page.svelte:603` 组合工作区配置组件。
  - **涉及文件**: `src/lib/components/UpstreamMcpForm.svelte`（新建，约 320 行）、`src/lib/types.ts`（约 35 行修改）。
  - _需求: FR-2、FR-5_ ｜ _设计: 模块职责；安全设计_

- [ ] 3.2 将上游表单接入工作区读取、保存和运行时重启提示
  - **证据块**: `src/routes/workspace/[id]/+page.svelte` 已管理 `profile` 草稿及保存；`src/lib/api/workspace.ts` 负责 Tauri IPC 的工作区调用。
  - **涉及文件**: `src/routes/workspace/[id]/+page.svelte`（约 100 行修改）、`src/lib/api/workspace.ts`（约 20 行修改）。
  - _需求: FR-1、FR-2、FR-3_ ｜ _设计: 模块职责；生命周期与失败处理_

### 阶段 4：文档与验证

- [ ] 4.1 补充可复制的受控配置示例和白名单说明
  - **证据块**: `README.md` 的“启动 MCP”与“Agent 可以做什么”章节描述对外接入方式。
  - **涉及文件**: `README.md`（约 70 行修改）。
  - _需求: FR-2、FR-5_ ｜ _设计: 安全设计_

- [ ] 4.2 添加上游配置、stdio 协议、工具路由和关闭行为的 Rust 回归测试
  - **证据块**: `src-tauri/src/mcp/server.rs`、`src-tauri/src/mcp/listener.rs` 和 `src-tauri/src/runtime/supervisor.rs` 已各自包含单元测试模块。
  - **涉及文件**: `src-tauri/src/mcp/upstream.rs`（约 170 行测试）、`src-tauri/src/mcp/server.rs`（约 120 行测试）、`src-tauri/src/workspace/model.rs`（约 100 行测试）。
  - _需求: FR-1、FR-3、FR-4、FR-5、NFR-4_ ｜ _设计: 测试策略_

## 检查点

- [ ] 阶段 1 完成后：无效配置、重复前缀和重复白名单在保存前被拒绝。
- [ ] 阶段 2 完成后：fixture MCP 的白名单工具可在 `/mcp` 公开和调用，未许可工具不可达。
- [ ] 阶段 3 完成后：通用上游配置字段可编辑，保存后重启 MCP 运行时。
- [ ] 阶段 4 完成后：Rust 测试、前端类型检查、构建和变更审查均通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | 上游配置模型 | 1.1、1.2、3.2、4.2 | 未开始 |
| FR-2 | 模块职责；安全设计 | 3.1、3.2、4.1 | 未开始 |
| FR-3 | stdio JSON-RPC 协议；生命周期与失败处理 | 1.2、2.1、2.2、3.2、4.2 | 未开始 |
| FR-4 | 工具公开与转发 | 2.3、4.2 | 未开始 |
| FR-5 | 安全设计 | 1.1、2.1、2.2、2.3、3.1、4.1、4.2 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `src-tauri/src/workspace/model.rs` | 修改 | 210 | 配置模型、验证与测试 |
| `src-tauri/src/mcp/upstream.rs` | 新建 | 600 | stdio 客户端、管理器与测试 |
| `src-tauri/src/mcp/mod.rs` | 修改 | 5 | 模块导出 |
| `src-tauri/src/mcp/server.rs` | 修改 | 250 | 工具合并、路由与测试 |
| `src-tauri/src/mcp/listener.rs` | 修改 | 110 | manager 生命周期 |
| `src-tauri/src/commands/workspace.rs` | 修改 | 80 | 保存验证与重启 |
| `src-tauri/src/runtime/supervisor.rs` | 修改 | 20 | 启动错误上下文 |
| `src/lib/types.ts` | 修改 | 70 | 前端模型 |
| `src/lib/api/workspace.ts` | 修改 | 20 | IPC 类型 |
| `src/lib/components/UpstreamMcpForm.svelte` | 新建 | 320 | 配置编辑器与预设 |
| `src/routes/workspace/[id]/+page.svelte` | 修改 | 100 | 表单接入 |
| `README.md` | 修改 | 70 | 配置与安全指南 |

## 检查清单

- [x] 交付物清单已锁定，路径和预计数量明确。
- [x] 每条任务包含具体动作、证据块、文件预算和需求回链。
- [x] 核心任务覆盖配置、协议、生命周期、UI、测试和文档。
- [x] 范围明确排除非 stdio 传输和自动公开全部工具。
