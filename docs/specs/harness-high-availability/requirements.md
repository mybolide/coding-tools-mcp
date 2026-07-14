# 需求文档：harness-high-availability

## 功能概述

为 ChatGPT 提供一个接近 Codex 客户端可靠性的本地 Coding Harness。Harness 必须让 Agent 随时知道当前任务、工作区和能力状态；在写入、执行或进程异常时给出可判断、可重试、可恢复的结果；对代码修改提供事务保证、撤销和工作区快照，避免一次失败把工作区留在不确定状态。

## 历史经验与坑（来自记忆库）

- **可复用经验**：系统健康状态应返回组件级状态、降级原因和下一步动作，而不是只有一个聚合状态；本功能将该模式用于 Read/Write/Exec/Git/Network 能力矩阵。
- **必须规避的坑**：当前实现只做了写入门禁，缺少原因、任务 ID 和恢复建议；不能把 `TASK_STATE_REQUIRED` 当成“整个工作区不可用”。Task 应是可选的持久化上下文，无 Task 时也必须允许正常开发操作。
- **必须规避的坑**：失败必须保留结构化 stderr、exit code、duration 和 recoverable 信息，禁止把不同终止原因统一成 `Session terminated`。

## 术语定义

- **Harness 状态**：任务会话、工作区基线、能力矩阵、最近操作和恢复动作的综合状态。
- **原子修改**：所有文件预检通过后统一提交；任一步失败时不留下部分写入。
- **快照**：工作区指定范围内文件内容、元数据和 Git 基线的可恢复副本。
- **能力**：Read、Write、Exec、Git、Network 五类独立的可用性状态。

## 范围边界

**In Scope（本次要做）**

- 新增 `harness_status`，返回任务、TTL/更新时间、可写性、原因、能力矩阵和 `next_actions`。
- 所有 Harness 门禁和运行时错误返回稳定错误信封，包含 `code`、`category`、`recoverable`、`suggestion`、当前 Harness 状态摘要。
- 保持 `read_file`、`list_dir`、文件搜索、`git_*`、`project_state`、`dry_run`、Patch 和命令执行在没有活动任务时可用；Task 只提供额外的基线、事件、快照和撤销上下文。
- 新增 patch preflight；apply patch 使用全量预检、staging、原子替换和失败回滚。
- 新增 `undo_last_patch`，记录可撤销的最近一次成功修改。
- 新增 `workspace_snapshot` 和 `workspace_rollback`，支持任务内多文件修改后的安全恢复。
- 为 exec/session 返回结束原因、退出码、耗时、stdout/stderr、是否可恢复和建议动作；记录执行事件。
- 补充单元测试、集成测试和崩溃/超时/外部修改等异常场景测试。

**Out of Scope（本次不做）**

- 不实现 AST 级编辑器；先保证现有 unified diff 的事务可靠性。
- 不实现跨机器分布式高可用；本功能的高可用指单机客户端进程内和持久化工作区级可恢复。
- 不自动执行具有破坏性的 rollback；rollback 必须由 Agent 显式调用并返回影响文件清单。
- 不改变现有权限策略的安全边界，不因“高可用”放开网络或任意命令。

## 需求列表

### FR-1: Harness 状态可见且可恢复

**优先级:** Must
**用户故事:** 作为 Agent，我想查询当前 Harness 状态，以便在上下文丢失或任务异常后继续工作，而不是猜测发生了什么。

#### 验收标准（EARS）

1. WHEN 调用 `harness_status` THEN 系统 SHALL 返回 workspace_id、task_id、task state、更新时间、writable、reason、recoverable、capabilities 和 next_actions。
2. WHEN 没有活动任务 THEN 系统 SHALL 将工作区标记为 standalone/无任务模式，允许开发操作，并将 `start_task` 作为可选的长期追踪建议；当任务暂停、过期或基线失效时 SHALL 返回明确恢复动作。
3. WHEN Harness 状态存储可读但工作区发生外部修改 THEN 系统 SHALL 保留 Read/Git/Status 能力为可用，并只阻止可能覆盖外部修改的 Write/Exec 能力。

### FR-2: 能力隔离和统一错误契约

**优先级:** Must
**用户故事:** 作为 Agent，我想区分 Read、Write、Exec、Git、Network 的失败原因，以便选择安全的下一步。

#### 验收标准（EARS）

1. WHEN 任意工具返回错误 THEN 系统 SHALL 返回稳定的 status、summary、error.code、error.category、error.recoverable、error.suggestion 和 harness 状态摘要。
2. WHEN 已有任务的基线校验拒绝 Write/Exec THEN 系统 SHALL 在错误中包含 task_id、当前状态、拒绝原因和恢复动作；没有活动任务不得返回 `TASK_STATE_REQUIRED`。
3. WHEN Read 或 Git 工具调用 THEN 系统 SHALL 不因缺少活动任务而返回 `TASK_STATE_REQUIRED`。

### FR-3: 原子 Patch 和撤销

**优先级:** Must
**用户故事:** 作为 Agent，我想先检查 patch，再一次性提交修改，并能撤销最近一次 patch，以便失败时不损坏工作区。

#### 验收标准（EARS）

1. WHEN 调用 `patch_check` THEN 系统 SHALL 只执行解析、路径安全、基线匹配和 hunk 应用预览，不修改工作区，并返回所有文件的预检结果。
2. WHEN 任一文件预检失败 THEN `apply_patch` SHALL 不修改任何文件，并返回失败文件、失败原因和修复建议；该预检不要求活动 Task。
3. WHEN 所有预检通过 THEN `apply_patch` SHALL 通过临时文件 staging 和原子替换提交全部文件；替换失败时 SHALL 自动恢复已替换文件。
4. WHEN patch 成功 THEN 系统 SHALL 保存变更记录和足以执行 `undo_last_patch` 的恢复数据。
5. WHEN 调用 `undo_last_patch` THEN 系统 SHALL 先检查当前文件是否仍匹配该 patch 的 after hash；不匹配时拒绝覆盖并给出外部修改警告。

### FR-4: Workspace Snapshot 和 Rollback

**优先级:** Must
**用户故事:** 作为 Agent，我想在长任务开始或关键阶段建立快照，并在异常时恢复，以便跨多个文件安全迭代。

#### 验收标准（EARS）

1. WHEN 调用 `workspace_snapshot` THEN 系统 SHALL 持久化快照 ID、任务 ID、Git 基线、文件 hash 和文件内容存储位置。
2. WHEN 调用 `workspace_rollback` THEN 系统 SHALL 先返回将被恢复的文件清单和冲突文件；只有无冲突或显式确认时才执行恢复。
3. WHEN rollback 的 staging 或替换失败 THEN 系统 SHALL 自动回滚本次 rollback 操作，并保留快照不变。
4. WHEN 任务开始 THEN 系统 SHALL 支持自动创建初始快照，但不得阻塞只读能力。

### FR-5: Session 终止原因和执行可观测性

**优先级:** Must
**用户故事:** 作为 Agent，我想知道命令是正常退出、超时、被杀、崩溃还是启动失败，以便决定是否重试或恢复。

#### 验收标准（EARS）

1. WHEN exec 命令结束 THEN 系统 SHALL 返回 session_id、status、termination_reason、exit_code、duration_ms、stdout、stderr、recoverable 和 suggestion。
2. WHEN 命令超时 THEN 系统 SHALL 标记 `termination_reason=timeout`、保留输出、说明已执行的终止动作，并提供 `read_output` 或重试建议。
3. WHEN 进程被用户终止、启动失败、权限拒绝或服务重启 THEN 系统 SHALL 使用不同的终止原因，不得统一返回 `Session terminated`。
4. WHEN exec/session 完成或失败 THEN 系统 SHALL 将操作、结果、持续时间、退出码、受影响工作区状态写入 Harness 事件日志。

### FR-6: 长任务状态和 Change Intelligence

**优先级:** Should
**用户故事:** 作为 Agent，我想知道每次变更做了什么以及为什么做，以便上下文压缩后仍能持续完成任务。

#### 验收标准（EARS）

1. WHEN patch、exec、snapshot、rollback 或验证完成 THEN 系统 SHALL 记录 operation_id、tool、affected_files、reason、result 和 verification 状态。
2. WHEN 调用 `change_summary` THEN 系统 SHALL 返回最近变更、变更原因、相关命令、验证结果、风险和可用 rollback/undo 动作。
3. WHEN task 更新步骤 THEN 系统 SHALL 持久化 completed_steps、pending_steps 和最近验证结果。

## 非功能需求

- **NFR-1（可靠性）**：任何单个文件预检、写入、事件日志或 session 错误都必须返回结构化错误；patch/rollback 不得留下已知的半修改状态。
- **NFR-2（可恢复性）**：Harness 状态文件、任务、事件和快照采用临时文件后原子替换；读取损坏时返回可定位的存储错误，不静默重置任务。
- **NFR-3（性能）**：`harness_status` 在普通工作区不超过 500ms；状态查询不得复制完整文件内容；快照内容按文件 hash 去重。
- **NFR-4（兼容性）**：现有工具名称、成功返回字段和只读调用方式保持兼容；新增字段采用可选/向后兼容 JSON 字段。
- **NFR-5（安全性）**：快照和 staging 路径必须位于 Harness 数据目录；rollback、undo 和 patch 继续复用工作区路径安全校验，不允许路径穿越。

## 依赖关系

- 依赖 `Harness`、`HarnessStore`、`HarnessEvent` 和 `dispatch::call_tool` 作为统一执行内核。
- 依赖现有 `Workspace` 路径解析、patch unified diff 解析和 session 管理。
- Git 信息继续通过当前 workspace 中的 Git 命令读取；Git 不可用时状态必须明确标记为 degraded。

## 检查清单

- [x] 已消化历史经验，并将组件级健康状态与结构化错误纳入设计。
- [x] 需求覆盖状态可见、能力隔离、事务、快照、session、Change Intelligence。
- [x] 每条需求有唯一 ID，并将在 design.md/tasks.md 中被引用。
- [x] 验收标准使用可执行的 EARS 形式。
- [x] 已明确范围边界和不做 AST/分布式高可用。
