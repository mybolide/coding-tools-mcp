# 设计文档：harness-high-availability

## 概述

在现有 Harness Foundation 上增加一层统一的观察和恢复契约，并把高风险文件修改纳入事务服务。状态查询不依赖聊天上下文；读能力与写能力分开计算；所有失败都携带可执行的恢复信息。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, NFR-1 至 NFR-5。

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 状态契约 | Rust 结构体序列化为 JSON | 保证 MCP 与 Actions 观察面一致 | FR-1, FR-2 |
| 持久化 | HarnessStore 下的原子 JSON/二进制文件 | 复用现有目录和原子写入模式 | FR-3, FR-4, NFR-2 |
| Patch 事务 | preflight → staging → atomic replace → recovery | 不改变现有 unified diff 输入，降低兼容风险 | FR-3 |
| 快照 | hash 元数据 + 内容文件 + manifest | 支持多文件恢复和内容去重 | FR-4, NFR-3 |
| Session 原因 | 生命周期状态 + 终止原因枚举 | 区分 timeout、killed、crash、spawn_failed、exited | FR-5 |
| 事件日志 | 现有 JSONL 扩展字段 | 保留历史日志兼容，并支持 Change Intelligence | FR-6 |

### 架构设计

```text
MCP / Actions
      ↓
tools::dispatch::call_tool
      ├─ harness::tools::harness_status
      ├─ capability evaluator
      ├─ patch_check/apply/undo ──→ transactional workspace writer
      ├─ workspace_snapshot/rollback → snapshot store
      └─ exec/session ─────────────→ structured termination + event log
                                      ↓
                               HarnessStore (atomic)
```

`call_tool` 仍是唯一入口。状态工具、只读工具和无任务模式下的开发工具不依赖活动任务。存在活动 Task 时，`apply_patch`、`undo_last_patch` 和 `exec_command` 继续执行基线校验，并在拒绝时附带 `harness_status`。成功且关联 Task 的操作更新 expected fingerprint，并写入 affected files、reason 和 verification 引用；无任务操作仍可执行，但明确标记为 standalone，不伪造任务事件。

### 状态与能力模型

新增 `HarnessStatus`、`CapabilityStatus` 和 `HarnessReason` 序列化模型：

- `task`: `id`、`state`、`objective`、`updated_at`、`writable`。
- `workspace`: `workspace_id`、branch、head、baseline_matches、external_changes。
- `capabilities`: `read`、`write`、`exec`、`git`、`network`，每项包含 `status`、`reason`、`recoverable`。
- `next_actions`: 稳定工具名数组，如 `start_task`、`resume_task`、`refresh_baseline`、`workspace_snapshot`。

错误信封统一增加：

```json
{
  "ok": true,
  "status": "success",
  "summary": "当前为无任务模式，操作不会进入任务事件流",
  "mode": "standalone",
  "harness": { "capabilities": {}, "next_actions": ["start_task", "read_file"] }
}
```

### Patch 事务

`patch_check` 复用现有 diff parser 和 hunk applicator，输出每个文件的 before hash、after hash、hunk 结果以及冲突。`apply_patch` 对全部文件生成 staged 内容；全部成功后将 staged 文件写入同目录临时文件并替换。替换前保留原内容，若任一替换失败，按已替换顺序恢复原内容；恢复失败时返回 `ROLLBACK_INCOMPLETE` 和恢复文件清单。

每次成功 patch 保存 `ChangeSet` 以及 before 内容到 Harness store。`undo_last_patch` 只允许在当前 after hash 与 ChangeSet 一致时执行，避免覆盖外部修改。

### Snapshot/rollback

快照目录位于 Harness store 的 `workspaces/<workspace_id>/snapshots/<snapshot_id>/`，包含 `manifest.json` 与按 hash 命名的内容文件。manifest 记录相对路径、存在性、sha256、大小、Git branch/head 和创建原因。rollback 先计算冲突，再按与 patch 相同的 staging/atomic replace 流程执行。

### Session 结构化状态

`ExecSession` 增加 `termination_reason`、`finished_at`、`killed_by_user` 和 `spawn_error` 等状态。统一映射：正常退出为 `exited`，超时为 `timeout`，显式 kill 为 `killed`，无法启动为 `spawn_failed`，无退出码的异常结束为 `crashed`，服务生命周期中断为 `server_restart`。输出保留现有 tail buffer，并在结果中给出 stdout/stderr 引用。

### Change Intelligence

事件记录必须生成 operation_id，写操作记录 affected_files 和 before/after hash；exec 记录 command 摘要、duration、exit_code 和工作区 fingerprint 变化。`change_summary` 聚合这些事件和最近的 ChangeSet、VerificationRecord，返回“做了什么、为什么、验证结果、风险、可恢复动作”。

## 数据模型

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| `HarnessStatus` | struct | 可序列化 | 当前任务、工作区和能力状态 |
| `CapabilityStatus` | struct | status/reason/recoverable | 五类能力的独立状态 |
| `ChangeSet` | 已有 struct | before/after 一致 | 一次可撤销修改的记录 |
| `SnapshotManifest` | 新 struct | snapshot_id 唯一 | 快照元数据与文件清单 |
| `TerminationReason` | enum | 稳定 snake_case | session 结束原因 |

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| `harness_status` | MCP tool | `max_events?` | `HarnessStatus` | FR-1, FR-2 |
| `patch_check` | MCP tool | `patch` | 预检结果，不改文件 | FR-3 |
| `undo_last_patch` | MCP tool | `task_id?`, `change_id?` | 变更和冲突 | FR-3 |
| `workspace_snapshot` | MCP tool | `task_id?`, `reason?` | snapshot_id、manifest 摘要 | FR-4 |
| `workspace_rollback` | MCP tool | `snapshot_id`, `confirm` | 恢复结果和文件清单 | FR-4 |
| `exec_command` | 现有 tool 扩展返回 | 现有入参兼容 | 结构化 session 结果 | FR-5 |
| `change_summary` | 现有 tool 扩展返回 | `task_id?` | Change Intelligence | FR-6 |

## 文件结构

```text
src-tauri/src/harness/
├── model.rs          # 状态、能力、快照和终止原因模型
├── state.rs          # 状态计算、快照入口、ChangeSet 管理
├── store.rs          # 快照内容、变更记录和原子持久化
└── tools.rs          # Harness 工具与统一错误状态
src-tauri/src/tools/
├── dispatch.rs       # 统一状态注入、能力门禁和事件记录
├── patch.rs          # patch_check、原子 apply、undo
├── session.rs        # session 终止原因和结构化结果
└── exec.rs           # timeout/spawn/kill 结果与 Harness 事件
```

测试优先放在对应模块的 `#[cfg(test)]`，跨模块行为放在现有 Rust 集成测试目录；不新增第二套工具入口。

## 设计决策

### 决策 1: 先做单机持久化高可用（关联需求: FR-1, FR-3, FR-4）

**问题**：当前 failure 会让 Agent 无法判断任务是否还能继续。

**选项**：
1. 引入外部数据库/服务：可扩展，但增加部署和断网故障面。
2. 复用 HarnessStore，使用原子 manifest 和内容文件：部署零依赖，适合桌面客户端。

**决策**：选择选项 2。高可用目标先覆盖客户端进程重启、上下文丢失、patch 失败和外部修改。

### 决策 2: 不自动恢复外部修改（关联需求: FR-2, FR-3, FR-4）

**问题**：自动覆盖外部修改会把“恢复”变成数据丢失。

**决策**：冲突时只读继续可用，写入/rollback 明确拒绝，要求 Agent 先查看 diff 或显式确认。

### 决策 3: 保持工具成功返回兼容，仅扩展字段（关联需求: NFR-4）

**问题**：客户端已有工具依赖当前 JSON 字段。

**决策**：新增 `summary`、`next_actions`、`harness` 等字段，不删除原字段；错误统一化只扩大信息量。

## 测试策略

- Harness 单测：无任务状态、暂停任务、基线失效、状态持久化损坏、能力矩阵和 next_actions。
- Patch 单测：多文件全成功、多文件 hunk 失败、路径穿越、替换失败恢复、undo hash 冲突。
- Snapshot 单测：创建/重启后读取、添加/删除文件、冲突 rollback、rollback 失败恢复。
- Session 单测：正常退出、非零退出码、timeout、kill、spawn failure、输出截断和结构化字段。
- Dispatch 集成测试：Read/Git 无任务可用，Write/Exec 被拒绝时携带状态，成功操作写入事件。
- 回归门禁：`cargo check`、`cargo clippy --all-targets`、`cargo test`、`npm run check`。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Windows 原子替换语义与文件占用 | 高 | 使用同目录临时文件、保留备份、失败恢复测试 |
| 快照占用磁盘 | 中 | hash 去重、限制单任务快照数、返回清理建议 |
| 现有事件 JSONL 向后兼容 | 中 | serde default、追加字段、不改变旧字段含义 |
| GitNexus bridge 仓库名不匹配 | 中 | 以源码和本地 graph-insights 为准，改动前补充 CLI impact/detect-changes |
| 外部工具已改变工作区 | 高 | before/after hash 检查，冲突时只读降级 |

## 检查清单

- [x] 技术方案与现有 Rust/Tauri 架构一致。
- [x] 每条 FR 均已覆盖。
- [x] 文件结构使用现有真实模块，并标明新增能力归属。
- [x] API、数据模型、测试策略和风险已定义。
