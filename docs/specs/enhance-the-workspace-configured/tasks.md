# 任务清单：本地 MCP 自动工具发现与启动可靠性

## 交付物清单

- **预计新建文件数**: 3 个规格文件。
- **预计修改文件数**: 8 个生产文件与相关测试。
- **预计新增/修改函数数**: 约 18 个。
- **交付物**:
  1. `src-tauri/src/workspace/model.rs`
  2. `src-tauri/src/mcp/upstream.rs`
  3. `src-tauri/src/mcp/listener.rs`
  4. `src-tauri/src/runtime/supervisor.rs`
  5. `src-tauri/src/commands/runtime.rs`
  6. `src-tauri/src/commands/workspace.rs`
  7. `src-tauri/src/lib.rs`
  8. `src/lib/types.ts`
  9. `src/lib/components/UpstreamMcpForm.svelte`

## 任务列表

### 阶段 1：模型与协议

- [ ] 1.1 增加关闭项模型并保留旧白名单兼容选择规则。
  - **证据块**: `RuntimeConfig.upstream_mcps` 是工作区持久化入口；`UpstreamMcpConfig.allowed_tools` 为当前白名单字段。
  - **涉及文件**: `src-tauri/src/workspace/model.rs`，约 90 行；`src/lib/types.ts`，约 10 行。
  - _需求: FR-1、FR-3_ ｜ _设计: 数据模型、兼容与迁移_

- [ ] 1.2 让上游 manager 按可见性规则自动发现并支持临时探测。
  - **证据块**: `UpstreamMcpManager::start` 已在启动时发送 `initialize` 和 `tools/list`；当前过滤直接依赖 `allowed_tools`。
  - **涉及文件**: `src-tauri/src/mcp/upstream.rs`，约 170 行。
  - _需求: FR-1、FR-2、FR-5_ ｜ _设计: 架构设计、失败与可观测性_

### 阶段 2：启动可靠性

- [ ] 2.1 将上游初始化移出同步 RuntimeSupervisor 启动路径，避免嵌套阻塞异步运行时。
  - **证据块**: `mcp::listener::spawn_listener` 当前通过 `tauri::async_runtime::block_on` 初始化 manager；`commands::runtime::start_mcp_service` 已是 async。
  - **涉及文件**: `src-tauri/src/mcp/listener.rs`、`src-tauri/src/runtime/supervisor.rs`、`src-tauri/src/commands/runtime.rs`，各不超过 180 行改动。
  - _需求: FR-4_ ｜ _设计: 异步生命周期_

- [ ] 2.2 暴露安全的临时工具发现 Tauri command，并确保失败时关闭子进程。
  - **证据块**: `commands::workspace` 管理配置读写；`lib.rs` 注册 Tauri command。
  - **涉及文件**: `src-tauri/src/commands/workspace.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`，约 80 行。
  - _需求: FR-2、FR-4_ ｜ _设计: 公共契约_

### 阶段 3：界面与回归

- [ ] 3.1 将手写白名单替换为发现工具列表和逐项开关。
  - **证据块**: `UpstreamMcpForm.svelte` 当前把 `allowed_tools` 以每行一个名称编辑。
  - **涉及文件**: `src/lib/components/UpstreamMcpForm.svelte`，约 180 行；`src/lib/types.ts`。
  - _需求: FR-2、FR-3_ ｜ _设计: 模块职责_

- [ ] 3.2 添加模型、上游协议和异步启动的回归测试，并运行前后端检查。
  - **证据块**: `upstream.rs` 已有 Node stdio fixture；`workspace/model.rs` 已有配置测试。
  - **涉及文件**: `src-tauri/src/mcp/upstream.rs`、`src-tauri/src/workspace/model.rs` 和必要的 runtime 测试，约 180 行。
  - _需求: FR-1、FR-3、FR-4、FR-5_ ｜ _设计: 测试策略_

## 检查点

- [ ] 默认公开与逐项关闭可在 fixture 工具目录中验证。
- [ ] 非空旧白名单仍只公开旧名称。
- [ ] 上游初始化失败在 10 秒内返回 error，且 28766 不会无限显示 starting。
- [ ] 前端检查、Rust 测试、构建和审查均通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | 数据模型、架构设计 | 1.1、1.2、3.2 | 未开始 |
| FR-2 | 公共契约、模块职责 | 1.2、2.2、3.1 | 未开始 |
| FR-3 | 兼容与迁移 | 1.1、3.1、3.2 | 未开始 |
| FR-4 | 异步生命周期 | 2.1、2.2、3.2 | 未开始 |
| FR-5 | 失败与可观测性 | 1.2、3.2 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `workspace/model.rs` | 修改 | 90 | 关闭项与兼容选择规则 |
| `mcp/upstream.rs` | 修改 | 170 | 自动公开、临时发现、测试 |
| `mcp/listener.rs` | 修改 | 70 | 接受异步准备好的 manager |
| `runtime/supervisor.rs` | 修改 | 130 | 已准备 listener 启动 |
| `commands/runtime.rs` | 修改 | 90 | 异步预检与启动错误传播 |
| `commands/workspace.rs` | 修改 | 70 | 临时发现 command |
| `commands/mod.rs` | 修改 | 10 | 导出 command |
| `lib.rs` | 修改 | 5 | 注册 command |
| `lib/types.ts` | 修改 | 10 | 新字段和探测结果类型 |
| `components/UpstreamMcpForm.svelte` | 修改 | 180 | 获取工具与逐项开关 |
