# 需求文档：workspace-first-harness

## 功能概述

将 Coding Harness 从 Task-first 简化为 Workspace-first。Workspace 是长期存在的核心实体，Task 仅作为可选的工作备注；Agent 可以像使用 IDE 一样直接读写、运行命令和操作 Git，同时获得操作记录、自动快照、恢复能力和危险操作保护。

## 历史经验与坑

- **可复用经验**：统一工具返回 `status`、`summary`、`next_actions` 和可恢复信息；原子 Patch、Undo 和 Snapshot 已有实现，继续复用。
- **必须规避的坑**：不能把 Task 生命周期当成 Workspace 权限。没有 Task 时不应返回 `TASK_STATE_REQUIRED`，尤其不能阻断 `dry_run`、Patch 和命令执行。

## 术语定义

- **Workspace**：当前打开的项目目录、Git、运行时、快照和操作历史的集合。
- **Operation**：一次文件、命令、Git、测试或恢复操作及其结果。
- **Capability**：基于 Workspace 边界、工具策略和危险程度计算的能力状态。
- **Task**：可选的长期工作备注，不是开发操作的前置条件。

## 范围边界

**In Scope**

- 以 Workspace 状态和 Capability 作为主观察面。
- 增加独立于 Task 的 Operation Log 工具。
- Patch、Undo 和明确写入意图的 Exec 前自动创建变更前快照，并返回恢复引用；普通查询 Exec 不触发全量快照。
- 保持 Workspace 路径边界；普通修改允许，删除关键项目资产和危险命令需要显式确认。
- 保留现有 Task API 作为兼容层。

**Out of Scope**

- 不删除现有 Task 数据和工具。
- 不实现跨机器分布式高可用。
- 不在本版本实现 AST 编辑器；保留 unified diff 接口。

## 需求列表

### FR-1: Workspace-first 能力

**优先级:** Must
**用户故事:** 作为 Agent，我想打开 Workspace 后直接开发，而不必先创建 Task。

#### 验收标准

1. WHEN 没有活动 Task THEN 系统 SHALL 允许 read、write、exec、Git 和测试工具运行。
2. WHEN 调用 `harness_status` THEN 系统 SHALL 返回 Workspace、Git、Capability、standalone 模式和可选的 start_task 建议。
3. IF 存在活动 Task 且其基线与当前工作区不一致 THEN 系统 SHALL 只阻止可能覆盖外部修改的操作。

### FR-2: Operation Log

**优先级:** Must
**用户故事:** 作为 Agent，我想查询最近做了什么、为什么做以及结果如何。

#### 验收标准

1. WHEN Patch、Exec、Snapshot、Rollback 或 Git 变更操作结束 THEN 系统 SHALL 写入 Workspace 级 Operation Log，不依赖 Task。
2. WHEN 调用 `operation_log` THEN 系统 SHALL 返回 operation_id、工具、时间、原因、结果、受影响文件、快照引用和恢复动作。

### FR-3: 自动 Snapshot 和恢复

**优先级:** Must
**用户故事:** 作为 Agent，我想在修改前自动获得恢复点，以便测试失败时回滚。

#### 验收标准

1. WHEN Patch 或 Undo 开始，或 Exec 传入 `snapshot=true`/匹配高风险命令 THEN 系统 SHALL 尽力创建变更前 Snapshot，并在结果中返回 snapshot_id。
2. IF Snapshot 创建失败 THEN 系统 SHALL 返回 warning 并继续执行不破坏 Workspace 边界的操作。
3. WHEN Rollback 存在冲突 THEN 系统 SHALL 先返回冲突清单，确认后才覆盖。

### FR-4: Workspace 安全边界和危险操作保护

**优先级:** Must
**用户故事:** 作为项目所有者，我想允许 Agent 正常修改代码，同时防止删除 Git 历史或工作区外文件。

#### 验收标准

1. WHEN 任意文件操作越过 workspace_root THEN 系统 SHALL 拒绝操作。
2. WHEN 删除 `.git`、`.github`、`.gitignore`、README、LICENSE 或构建配置等关键资产 THEN 系统 SHALL 要求显式确认。
3. WHEN 命令匹配 `git reset --hard`、`git clean`、递归删除或系统目录写入 THEN 系统 SHALL 要求显式确认或拒绝。
4. WHEN 普通修改关键文件 THEN 系统 SHALL 允许执行，并保留快照和操作记录。

## 非功能需求

- **NFR-1（兼容性）**：Task 工具、现有成功返回字段和 Workspace 路径规则保持兼容。
- **NFR-2（可靠性）**：自动快照和操作日志失败不得造成半写入；Patch/Undo/Rollback 继续使用事务提交。
- **NFR-3（可观测性）**：Operation Log 查询不得依赖聊天上下文或 Task ID。
- **NFR-4（性能）**：状态查询不复制完整文件内容；自动快照失败可降级，普通状态查询目标不超过 500ms。

## 依赖关系

- 复用 `HarnessStore`、`Workspace`、`dispatch::call_tool`、现有 Patch 事务和 Session 输出。
- 复用现有策略 allowlist，新增危险命令确认判断。

## 检查清单

- [x] Workspace 是主实体，Task 是可选元数据。
- [x] 覆盖操作记录、自动快照、边界保护和危险操作。
- [x] 每条需求有稳定 ID 和可测试验收标准。
