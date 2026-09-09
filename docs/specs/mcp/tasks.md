# 任务清单：本地 MCP 配置入口与自动挂载

## 交付物清单

- **预计新建文件数:** 3 个规格文档。
- **预计修改文件数:** 2 个生产文件。
- **预计新增或修改函数数:** 0 个；删除 1 个仅由按钮使用的预设函数。
- **交付物:** `requirements.md`、`design.md`、本文件、`src/lib/components/UpstreamMcpForm.svelte` 与 `README.md`。

## 任务列表

### 阶段 1：确认现状

- [ ] 1.1 核对表单预设与运行时挂载顺序，限定改动范围。
  - **证据块:** 机器特定预设仅生成预填配置；`runtime.rs:138-158` 在监听器和隧道前创建上游管理器。
  - **涉及文件:** `src/lib/components/UpstreamMcpForm.svelte`，预计删除 19 行。
  - _需求: FR-1, FR-2_ ｜ _设计: 架构设计、决策 1、决策 2_

### 阶段 2：核心实现

- [ ] 2.1 删除机器特定预设函数和入口按钮，保留新增 stdio MCP 按钮。
  - **证据块:** 表单标题区域同时调用通用新增和机器特定预设；后者没有其他引用。
  - **涉及文件:** `src/lib/components/UpstreamMcpForm.svelte`，预计删除 20 行。
  - _需求: FR-1_ ｜ _设计: 决策 1_

- [ ] 2.2 将使用说明改为通用 stdio MCP 配置流程，不保留机器特定路径。
  - **证据块:** README 现有本地 MCP 章节要求选择预填示例，已与目标 UI 不一致。
  - **涉及文件:** `README.md`，预计修改 15 行。
  - _需求: FR-1_ ｜ _设计: 决策 1_

### 阶段 3：集成验证

- [ ] 3.1 执行前端检查、Rust 库测试，并确认服务启动链路先初始化上游再创建公网隧道。
  - **证据块:** `start_mcp_service` 在 `UpstreamMcpManager::start` 成功后调用 `start_prepared_mcp`，随后才调用 `maybe_start_for_runtime`。
  - **涉及文件:** `src/lib/components/UpstreamMcpForm.svelte`、`src-tauri/src/commands/runtime.rs`。
  - _需求: FR-2_ ｜ _设计: 测试策略_

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 决策 1 | 1.1、2.1、2.2 | 未开始 |
| FR-2 | 决策 2、测试策略 | 1.1、3.1 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src/lib/components/UpstreamMcpForm.svelte` | 修改 | -20 | 删除机器特定预设和按钮。 |
| `README.md` | 修改 | 15 | 替换为通用 stdio 配置说明。 |
| `docs/specs/mcp/requirements.md` | 新建 | 50 | 验收需求。 |
| `docs/specs/mcp/design.md` | 新建 | 50 | 设计与验证策略。 |
| `docs/specs/mcp/tasks.md` | 新建 | 55 | 实施清单。 |
