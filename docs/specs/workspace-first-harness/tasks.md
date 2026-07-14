# 任务清单：workspace-first-harness

## 概述

本版本只做减耦和可靠性增强，不删除已有 Task API，不引入新的任务生命周期。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 0 个。
- **预计修改文件数**: 9 个 Rust 文件，2 个集成测试文件。
- **预计新增/修改函数数**: 约 20 个。
- **交付物逐项列举**:
  1. Workspace 级 Operation Log 与 `operation_log` 工具。
  2. Patch/Undo 及明确写入或高风险 Exec 的变更前 Snapshot 引用。
  3. 关键文件删除和危险命令确认保护。
  4. standalone 模式与 Task 兼容测试。

## 任务列表

### 阶段 1：Workspace Operation Log

- [x] 1.1 在 `HarnessStore` 增加 Workspace 级 JSONL Operation Log，支持分页读取且不依赖 Task
  - **证据块**: `src-tauri/src/harness/store.rs` 当前事件路径按 task_id 保存；`src-tauri/src/tools/dispatch.rs` 只有关联 Task 时记录 operation_started/finished。
  - **涉及文件**: `src-tauri/src/harness/model.rs`（OperationRecord，约 50 行）；`src-tauri/src/harness/store.rs`（追加/分页，约 80 行）；`src-tauri/src/harness/state.rs`（入口，约 50 行）；`src-tauri/src/harness/tools.rs`（operation_log，约 50 行）。
  - _需求: FR-2_ ｜ _设计: 数据模型、API 设计_

- [x] 1.2 在统一 dispatch 中记录 standalone 和 Task 两种操作，并返回 operation_id
  - **证据块**: `src-tauri/src/tools/dispatch.rs` 当前 `task_id` 为空时不记录操作。
  - **涉及文件**: `src-tauri/src/tools/dispatch.rs`（约 80 行）；`src-tauri/tests/harness_tool_contract.rs`（约 40 行）。
  - _需求: FR-1, FR-2_ ｜ _设计: 执行流程_

### 阶段 2：自动 Snapshot 和安全保护

- [x] 2.1 在 Patch/Undo 及明确写入或高风险 Exec 前创建尽力而为的变更前 Snapshot，并将引用写入结果和 Operation Log
  - **证据块**: `src-tauri/src/harness/state.rs` 已有 `create_snapshot`；`src-tauri/src/tools/dispatch.rs` 当前只在任务开始时自动创建 Snapshot。
  - **涉及文件**: `src-tauri/src/tools/dispatch.rs`（约 60 行）；`src-tauri/src/harness/state.rs`（约 20 行）；`src-tauri/src/tools/patch.rs`（约 20 行）。
  - _需求: FR-3_ ｜ _设计: 执行流程_

- [x] 2.2 增加关键文件删除和危险命令的显式确认保护，普通修改保持可用
  - **证据块**: `src-tauri/src/tools/workspace.rs` 已有 workspace_root、路径穿越和 symlink 保护；`src-tauri/src/tools/policy.rs` 已有命令 allowlist，但未覆盖高风险参数组合。
  - **涉及文件**: `src-tauri/src/tools/workspace.rs`（约 40 行）；`src-tauri/src/tools/patch.rs`（约 30 行）；`src-tauri/src/tools/policy.rs`（约 60 行）；`src-tauri/src/tools/registry.rs`（schema 约 20 行）。
  - _需求: FR-4_ ｜ _设计: 关键保护_

### 阶段 3：验证和兼容

- [x] 3.1 补充无 Task 操作、日志持久化、自动快照引用和安全保护测试
  - **证据块**: `src-tauri/tests/harness_tool_contract.rs` 已覆盖 standalone Patch/Exec；需扩展 Operation Log、Snapshot 和危险操作场景。
  - **涉及文件**: `src-tauri/tests/harness_tool_contract.rs`、`src-tauri/tests/call_tool_security.rs`、对应模块单测。
  - _需求: FR-1 至 FR-4_ ｜ _设计: 测试策略_

- [x] 3.2 运行全量验证和 GitNexus 变更检查
  - **证据块**: 当前基线已通过 54 项 Rust 测试、Clippy 和 `npm run check`。
  - **涉及文件**: 无业务新增文件；产出验证结果。
  - _需求: NFR-1 至 NFR-4_ ｜ _设计: 向后兼容、测试策略_

## 检查点

- [x] 阶段 1：无 Task 的 Patch/Exec 有 operation_id，Operation Log 可查询。
- [x] 阶段 2：Patch/Undo 及明确写入或高风险 Exec 有 snapshot_id；普通查询 Exec 不被快照阻塞；关键删除和危险命令需要确认。
- [x] 阶段 3：56 项 Rust 测试、Clippy、前端检查通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、向后兼容 | 1.2, 3.1 | 已完成 |
| FR-2 | 数据模型、API 设计 | 1.1, 1.2, 3.1 | 已完成 |
| FR-3 | 执行流程 | 2.1, 3.1 | 已完成 |
| FR-4 | 关键保护 | 2.2, 3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/harness/model.rs` | 修改 | 180 | OperationRecord |
| `src-tauri/src/harness/store.rs` | 修改 | 240 | Operation Log 持久化 |
| `src-tauri/src/harness/state.rs` | 修改 | 650 | Workspace 操作入口；超 500 行时拆出 `operations.rs` |
| `src-tauri/src/harness/tools.rs` | 修改 | 380 | operation_log 工具 |
| `src-tauri/src/tools/dispatch.rs` | 修改 | 240 | 操作记录和自动快照 |
| `src-tauri/src/tools/policy.rs` | 修改 | 260 | 危险命令保护 |
| `src-tauri/src/tools/workspace.rs` | 修改 | 420 | 关键路径判断 |
| `src-tauri/src/tools/patch.rs` | 修改 | 520 | 关键删除确认；超 500 行时拆出事务辅助 |
| `src-tauri/src/tools/registry.rs` | 修改 | 650 | 新工具和参数 schema |
| `src-tauri/tests/harness_tool_contract.rs` | 修改 | 180 | Workspace-first 契约 |
| `src-tauri/tests/call_tool_security.rs` | 修改 | 140 | 危险操作安全测试 |

## 检查清单

- [x] 交付物和文件范围已锁定。
- [x] 任务均包含证据、文件预算和需求回链。
- [x] 需求覆盖矩阵已填写。
- [x] 未引入新的 Task 生命周期。
