# 任务清单：harness-high-availability

## 概述

按“观察 → 事务 → 恢复 → Session → 验证”的顺序实现客户端级 Harness 高可用能力。所有任务都回链到需求和设计章节。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 0 个；优先复用现有模块，只有测试需要时才新增文件。
- **预计修改文件数**: 8 个核心 Rust 文件，可能增加对应测试文件。
- **预计新增/修改函数数**: 约 25 个公开/内部函数。
- **交付物逐项列举**:
  1. `harness_status` 与能力矩阵。
  2. 统一 Harness 错误信封和恢复动作。
  3. `patch_check`、原子 `apply_patch`、`undo_last_patch`。
  4. `workspace_snapshot`、`workspace_rollback`。
  5. Session 终止原因、耗时、退出码和事件记录。
  6. Change Intelligence 聚合结果。
  7. 单元/集成回归测试和验证报告。

## 任务列表

### 阶段 1: 状态观察和错误契约

- [x] 1.1 扩展 Harness 状态模型并实现 `harness_status`，返回任务、基线、能力矩阵、standalone 模式和恢复动作
  - **证据块**: `src-tauri/src/harness/state.rs:44-56` 当前 `current_task` 只返回可写任务；`src-tauri/src/tools/dispatch.rs:31-45` 当前门禁仅返回 `TASK_STATE_REQUIRED` 文本。
  - **涉及文件**: `src-tauri/src/harness/model.rs`（新增状态结构，约 100 行）；`src-tauri/src/harness/state.rs`（状态计算，约 160 行）；`src-tauri/src/harness/tools.rs`（工具注册，约 80 行）；`src-tauri/src/tools/dispatch.rs`（错误注入，约 40 行）。
  - _需求: FR-1, FR-2_ ｜ _设计: 状态与能力模型_

- [x] 1.2 将能力门禁拆分为 Read/Write/Exec/Git/Network，并统一返回 recoverable/suggestion/next_actions；无 Task 不阻断开发操作
  - **证据块**: `src-tauri/src/tools/dispatch.rs:31-48` 当前只区分写入工具和其他工具；`src-tauri/src/harness/tools.rs:140-158` 当前错误只映射 code/message/category/retryable。
  - **涉及文件**: `src-tauri/src/tools/dispatch.rs`（错误信封和能力注入，约 100 行）；`src-tauri/src/harness/tools.rs`（错误转换，约 60 行）；相关测试模块（约 100 行）。
  - _需求: FR-1, FR-2_ ｜ _设计: 状态与能力模型_

### 阶段 2: Patch 事务和恢复

- [x] 2.1 抽取 patch 预检结果并实现 `patch_check`，确保预检阶段不触碰工作区
  - **证据块**: `src-tauri/src/tools/patch.rs:9-72` 当前 `apply_patch` 已先解析全部文件并调用 `commit_staged`，但没有独立预检工具和稳定预检结果。
  - **涉及文件**: `src-tauri/src/tools/patch.rs`（预检与结果，约 150 行）；`src-tauri/src/tools/dispatch.rs`（工具分发，约 10 行）；`src-tauri/src/tools/registry.rs`（注册 schema，约 30 行）。
  - _需求: FR-3_ ｜ _设计: Patch 事务_

- [x] 2.2 将 patch 提交改为可检测失败并自动恢复的原子事务，记录 ChangeSet 和 affected_files
  - **证据块**: `src-tauri/src/tools/patch.rs:244-293` 当前 `commit_staged` 采用备份后逐文件写入，需补齐替换失败时的恢复结果和持久化变更记录。
  - **涉及文件**: `src-tauri/src/tools/patch.rs`（事务实现，约 180 行）；`src-tauri/src/harness/model.rs`（ChangeSet 扩展，约 30 行）；`src-tauri/src/harness/store.rs`（变更记录，约 100 行）；`src-tauri/src/harness/state.rs`（记录入口，约 80 行）。
  - _需求: FR-3, FR-6, NFR-1, NFR-2_ ｜ _设计: Patch 事务_

- [x] 2.3 实现 `undo_last_patch`，以 after hash 检测外部修改并拒绝危险覆盖
  - **证据块**: `src-tauri/src/harness/model.rs:80-94` 已有 `latest_change_id` 和 `checkpoint_ids` 字段，但当前 `change_summary` 明确返回 `rollback_capability=not_available_in_foundation`。
  - **涉及文件**: `src-tauri/src/harness/tools.rs`（工具入口，约 50 行）；`src-tauri/src/tools/patch.rs`（undo 复用事务，约 80 行）；`src-tauri/src/harness/store.rs`（读取变更，约 60 行）。
  - _需求: FR-3_ ｜ _设计: Patch 事务_

### 阶段 3: Workspace Snapshot 和 Rollback

- [x] 3.1 实现快照 manifest、内容存储和 `workspace_snapshot`
  - **证据块**: `src-tauri/src/harness/state.rs:184-257` 当前 `project_state` 只计算 hash 和文件状态，没有内容快照或恢复记录。
  - **涉及文件**: `src-tauri/src/harness/model.rs`（快照结构，约 50 行）；`src-tauri/src/harness/store.rs`（manifest/内容原子持久化，约 150 行）；`src-tauri/src/harness/state.rs`（快照入口，约 120 行）；`src-tauri/src/harness/tools.rs`（工具入口，约 50 行）。
  - _需求: FR-4_ ｜ _设计: Snapshot/rollback_

- [x] 3.2 实现冲突检测、显式确认和 `workspace_rollback`，失败自动恢复
  - **证据块**: `src-tauri/src/harness/state.rs:127-151` 当前基线失效时只返回错误，没有生成冲突清单或安全恢复动作。
  - **涉及文件**: `src-tauri/src/harness/state.rs`（冲突/回滚，约 160 行）；`src-tauri/src/harness/tools.rs`（参数和输出，约 70 行）；`src-tauri/src/tools/workspace.rs`（路径安全复用，约 20 行）。
  - _需求: FR-4, NFR-5_ ｜ _设计: Snapshot/rollback_

### 阶段 4: Session 与 Change Intelligence

- [x] 4.1 为 ExecSession 增加终止原因并统一 exec 返回结构
  - **证据块**: `src-tauri/src/tools/session.rs:56-75` 当前只保存 exit_code 和 started_at；`src-tauri/src/tools/exec.rs:122-131` 超时只返回裸 `TIMEOUT` 错误。
  - **涉及文件**: `src-tauri/src/tools/session.rs`（状态模型和 snapshot，约 120 行）；`src-tauri/src/tools/exec.rs`（spawn/timeout 结果，约 80 行）。
  - _需求: FR-5_ ｜ _设计: Session 结构化状态_

- [x] 4.2 记录 exec/patch/快照/回滚的结构化事件并完善 `change_summary`
  - **证据块**: `src-tauri/src/tools/dispatch.rs:100-108` 当前只记录 arguments_present 和 ok，没有 duration、退出码、原因或 affected_files。
  - **涉及文件**: `src-tauri/src/tools/dispatch.rs`（事件字段，约 100 行）；`src-tauri/src/harness/state.rs`（聚合，约 100 行）；`src-tauri/src/harness/tools.rs`（summary 输出，约 60 行）。
  - _需求: FR-5, FR-6_ ｜ _设计: Change Intelligence_

### 阶段 5: 测试和验收

- [x] 5.1 补齐状态、门禁、事务、快照、session 的异常路径测试并对照 FR 验收
  - **证据块**: 现有 Rust 测试已覆盖 Foundation 基线和任务状态，但尚未覆盖 `harness_status`、patch_check、undo、snapshot、rollback 及终止原因契约。
  - **涉及文件**: 对应模块 `#[cfg(test)]` 与现有集成测试文件（预计约 300 行）；不改现有用户测试数据。
  - _需求: FR-1 至 FR-6, NFR-1 至 NFR-5_ ｜ _设计: 测试策略_

- [x] 5.2 运行全量验证并检查变更范围
  - **证据块**: 当前基线验证记录显示 `cargo check`、`cargo test` 和 `npm run check` 已通过；新增实现必须保持这些门禁通过。
  - **涉及文件**: 无新增业务文件；产出命令验证结果。
  - _需求: NFR-1 至 NFR-5_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1 完成后：无任务时 Read/Git/Status/Write/Exec 可用；仅已有任务基线冲突时拒绝，并包含 task_id、reason、recoverable、suggestion、next_actions。
- [x] 阶段 2 完成后：多文件 patch 预检失败不改文件；提交失败可恢复；undo 能检测外部修改。
- [x] 阶段 3 完成后：快照可跨进程重启读取；冲突 rollback 不覆盖外部修改。
- [x] 阶段 4 完成后：timeout、kill、spawn_failed、crashed 与 exited 可区分，事件包含操作证据。
- [x] 阶段 5 完成后：Rust 和前端检查全部通过，检测变更范围只包含本功能及用户已有改动。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 状态与能力模型 | 1.1, 1.2 | 已完成 |
| FR-2 | 状态与能力模型 | 1.2, 4.2 | 已完成 |
| FR-3 | Patch 事务 | 2.1, 2.2, 2.3 | 已完成 |
| FR-4 | Snapshot/rollback | 3.1, 3.2 | 已完成 |
| FR-5 | Session 结构化状态、Change Intelligence | 4.1, 4.2 | 已完成 |
| FR-6 | Change Intelligence | 4.2, 5.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/harness/model.rs` | 修改 | 150 | 状态、能力、快照和变更模型 |
| `src-tauri/src/harness/state.rs` | 修改 | 500 | 状态计算、事务记录、快照/回滚入口；超 500 行时拆出 `snapshot.rs` |
| `src-tauri/src/harness/store.rs` | 修改 | 350 | 原子持久化、快照和变更内容 |
| `src-tauri/src/harness/tools.rs` | 修改 | 300 | 新增工具与统一输出 |
| `src-tauri/src/tools/dispatch.rs` | 修改 | 180 | 状态注入、能力门禁、事件证据 |
| `src-tauri/src/tools/patch.rs` | 修改 | 450 | 预检、原子提交和撤销；超 500 行时拆出事务辅助模块 |
| `src-tauri/src/tools/session.rs` | 修改 | 450 | Session 终止原因和输出契约 |
| `src-tauri/src/tools/exec.rs` | 修改 | 220 | 执行生命周期和事件数据 |

## 检查清单

- [x] 交付物清单已填并锁定主要产出。
- [x] 每条任务标题为具体动词+对象+约束。
- [x] 每条任务含现状证据、文件和行数预算。
- [x] 每条任务回链到 FR 与设计章节。
- [x] 需求覆盖矩阵无遗漏。
- [x] 阶段 5 包含按验收标准核验。
- [x] 全文无模板占位符、`TODO` 或省略号占位。
