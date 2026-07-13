# 设计文档：harness-foundation

## 概述

本设计在现有 `tools::call_tool`、`ToolContext`、`DataStore` 和 Tauri 桌面外壳之上增加 Harness Kernel。Kernel 负责项目状态聚合、任务生命周期、变更事件、checkpoint、审批和上下文恢复；文件、Git 和命令工具继续作为执行器，不在传输层复制业务逻辑。

**对应需求：** FR-1、FR-2、FR-3、FR-4、FR-5、FR-6、FR-7、FR-8、FR-9、FR-10、FR-11、FR-12、NFR-1 至 NFR-10

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| Harness Kernel | Rust 模块 `src-tauri/src/harness/` | 与现有工具运行时共享进程、Workspace 和权限上下文 | FR-2 至 FR-10 |
| 主状态持久化 | 版本化 JSON + 原子替换 | 与现有 DataStore 风格一致，便于迁移和人工诊断 | FR-3、FR-10、FR-12 |
| 事件持久化 | 每任务 JSONL 追加日志 | 支持大量事件、故障恢复和分页读取 | FR-5、FR-6 |
| 文件标识 | SHA-256 内容哈希 | 用于 baseline、并发修改和 rollback 冲突判断 | FR-4、FR-7 |
| Checkpoint 内容 | 应用数据目录中的压缩文件快照 | 不依赖 Git 状态，不覆盖用户 baseline | FR-7 |
| 实时项目状态 | 调用 Git helper + 文件系统即时计算 | 避免缓存导致 branch、HEAD、dirty 状态过期 | FR-2 |
| 审批 | 后端 Pending Action + Tauri UI | 不依赖 ChatGPT Connector 的 Elicitation 支持 | FR-9、FR-10 |
| 工具注册 | 单一 `ToolDefinition` 注册表 | MCP、Actions、server_info 和策略从同一来源派生 | FR-1 |
| Context Capsule | 服务端确定性聚合 | 控制上下文体积，保留事实来源与失败状态 | FR-6 |

### 总体架构

```text
ChatGPT / GPT Actions
        │
        ▼
MCP / Actions Transport
        │ tools/list, tools/call
        ▼
统一工具注册表 ───────────────┐
        │                    │
        ▼                    ▼
tools::call_tool        Harness Gate
        │                    │
        ├─ read/search       ├─ active task
        ├─ apply_patch       ├─ fresh baseline
        ├─ exec_command      ├─ approval
        └─ git_*             └─ file ownership
        │
        ▼
Harness Kernel
  ├─ Project State（实时）
  ├─ Task Session（持久）
  ├─ Event / Change Set
  ├─ Verification
  ├─ Checkpoint / Rollback
  ├─ Context Capsule
  └─ Pending Action
        │
        ▼
应用数据目录 + 桌面 Harness UI
```

### 调用原则

1. MCP 与 Actions 只负责协议解析、认证和结果包装。
2. 工具是否可见由统一注册表和传输能力决定。
3. 工具执行前由 `call_tool` 统一调用 Harness Gate。
4. 工具执行前后由 Harness Event Recorder 记录 operation。
5. Project State 的 Git/文件部分实时计算；任务、事件和审批部分从持久化读取。

---

## 状态机设计

### Task Session 状态

```text
idle
  └─ start_task → active

active
  ├─ pause → paused
  ├─ begin_verification → verifying
  ├─ fail → failed
  └─ rollback → rolled_back

paused
  └─ resume → active

verifying
  ├─ verification_passed → completed
  ├─ finish_without_verification → completed_unverified
  └─ verification_failed → failed

failed
  ├─ resume → active
  └─ rollback → rolled_back
```

终态为 `completed`、`completed_unverified` 和 `rolled_back`。第一版同一 workspace_id 只允许一个 `active`、`paused`、`verifying` 或 `failed` 的可写任务。

### Pending Action 状态

```text
pending → approved → executed
pending → denied
pending → expired
approved → expired（执行前状态已变化）
```

批准不等于直接执行。执行前必须重新检查 task、baseline、文件哈希和风险条件。

---

## 数据模型

### HarnessIndex

```rust
pub struct HarnessIndex {
    pub schema_version: u32,
    pub workspaces: HashMap<String, WorkspaceHarnessState>,
}
```

### WorkspaceHarnessState

```rust
pub struct WorkspaceHarnessState {
    pub active_task_id: Option<String>,
    pub recent_task_ids: Vec<String>,
    pub pending_action_ids: Vec<String>,
    pub updated_at: String,
}
```

### TaskSession

```rust
pub struct TaskSession {
    pub id: String,
    pub workspace_id: String,
    pub objective: String,
    pub status: TaskStatus,
    pub baseline: ProjectBaseline,
    pub completed_steps: Vec<String>,
    pub pending_steps: Vec<String>,
    pub latest_change_id: Option<String>,
    pub latest_verification_id: Option<String>,
    pub checkpoint_ids: Vec<String>,
    pub rollback_capability: RollbackCapability,
    pub created_at: String,
    pub updated_at: String,
}
```

### ProjectBaseline

```rust
pub struct ProjectBaseline {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub worktree_fingerprint: String,
    pub entries: Vec<BaselineEntry>,
    pub captured_at: String,
}
```

`BaselineEntry` 至少包含相对路径、tracked/untracked、状态、是否存在、SHA-256、字节数。大文件只保存哈希和元数据，checkpoint 时再按策略决定是否保存内容。

### HarnessEvent

```rust
pub struct HarnessEvent {
    pub id: String,
    pub task_id: String,
    pub operation_id: String,
    pub kind: EventKind,
    pub tool_name: Option<String>,
    pub input_summary: serde_json::Value,
    pub result_summary: serde_json::Value,
    pub reason: Option<ReasonRecord>,
    pub affected_files: Vec<FileChangeRecord>,
    pub created_at: String,
}
```

### ChangeSet

```rust
pub struct ChangeSet {
    pub id: String,
    pub task_id: String,
    pub objective: String,
    pub reason: ReasonRecord,
    pub files: Vec<FileChangeRecord>,
    pub command_ids: Vec<String>,
    pub verification_ids: Vec<String>,
    pub risks: Vec<String>,
    pub rollback_capability: RollbackCapability,
    pub created_at: String,
}
```

### VerificationRecord

包含命令、类别（test/build/check/lint/custom）、开始结束时间、退出码、状态、输出引用、关联 change_id 和是否通过。命令输出继续由 SessionStore 分页保存，事件日志只保存摘要与引用。

### CheckpointManifest

包含 checkpoint_id、task_id、创建时间、文件项、内容存储位置、创建时哈希、预期当前哈希和 rollback capability。快照内容使用 zip 存储到 Harness 数据目录，禁止写入仓库。

### PendingAction

包含 action_id、task_id、operation、脱敏参数、原因、风险等级、影响文件、申请时状态指纹、状态、过期时间和决策记录。

---

## 存储布局

```text
<app-config>/harness/
├── index.json
└── workspaces/<workspace-id>/
    ├── state.json
    ├── tasks/<task-id>.json
    ├── events/<task-id>.jsonl
    ├── changes/<change-id>.json
    ├── verifications/<verification-id>.json
    ├── checkpoints/<checkpoint-id>.json
    ├── checkpoint-data/<checkpoint-id>.zip
    └── approvals/<action-id>.json
```

主 JSON 文件使用“写临时文件、flush、rename”原子替换。JSONL 每条事件独占一行；启动时忽略并报告最后一条不完整记录，不破坏之前事件。

状态文件采用独立 `schema_version`，不把高频事件直接并入现有 `profiles.json`，避免工作区配置保存与 Harness 事件写入互相阻塞。

---

## API 与工具设计

| 工具/接口 | 主要入参 | 主要出参 | 关联需求 |
|------|------|------|------|
| `project_state` | `include_recent_commits`、`max_files` | 实时 Git/文件状态、活动任务、最近验证 | FR-2 |
| `start_task` | `objective`、`completed_steps?`、`pending_steps?` | task、baseline、context capsule | FR-3、FR-4 |
| `update_task` | `task_id`、步骤和状态更新 | 更新后的任务 | FR-3 |
| `pause_task` | `task_id`、`reason` | paused task | FR-3 |
| `resume_task` | `task_id` | active task、新鲜状态 | FR-3 |
| `finish_task` | `task_id`、`summary`、`allow_unverified` | 终态任务和 change summary | FR-3、FR-5 |
| `task_context` | `task_id?`、`max_bytes` | 有界 Context Capsule | FR-6 |
| `list_task_events` | `task_id`、`cursor`、`limit` | 事件分页 | FR-5、FR-6 |
| `change_summary` | `task_id?`、`change_id?` | What/Why/Evidence/Verification/Risk | FR-5 |
| `create_checkpoint` | `task_id`、`reason` | checkpoint manifest | FR-7 |
| `rollback_checkpoint` | `checkpoint_id`、`reason` | 恢复结果、冲突列表 | FR-7 |
| `list_checkpoints` | `task_id` | checkpoint 摘要 | FR-7 |
| `git_add` | `task_id`、`paths`、`reason` | staged files | FR-9 |
| `git_commit` | `task_id`、`message`、`reason` | Pending Action 或 commit | FR-9、FR-10 |
| `git_create_branch` | `task_id`、`name`、`reason` | branch | FR-9 |
| `git_switch` | `task_id`、`name`、`reason` | Pending Action 或切换结果 | FR-9、FR-10 |
| `git_push` | `task_id`、`remote`、`branch`、`reason` | Pending Action 或 push 结果 | FR-9、FR-10 |
| Tauri `get_harness_state` | workspace id | UI 聚合状态 | FR-11 |
| Tauri `list_pending_actions` | workspace id | 审批列表 | FR-10、FR-11 |
| Tauri `decide_pending_action` | action id、decision | 决策结果 | FR-10、FR-11 |
| Tauri `rollback_checkpoint` | checkpoint id | 回滚结果 | FR-7、FR-11 |

### 统一工具定义

现有 tuple 注册表替换为结构体：

```rust
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub profile: ToolProfileVisibility,
    pub transports: ToolTransports,
    pub annotations: ToolAnnotations,
    pub input_schema: fn() -> Value,
    pub mutation: MutationClass,
    pub approval: ApprovalClass,
}
```

MCP、Actions OpenAPI、`server_info`、工具门禁和契约测试都从该定义派生。

---

## Harness Gate 设计

### 工具分类

- `ReadOnly`：文件读取、搜索、Project State、Git 只读、事件和任务查询。
- `TaskMutation`：apply_patch、可能修改文件的 exec、git_add、任务更新。
- `ApprovalMutation`：git_commit、git_switch、git_push、高风险命令。
- `Forbidden`：reset --hard、clean、force push、全局 Git 配置、工作区逃逸。

### 写操作门禁流程

```text
解析工具定义
→ 查找活动任务
→ 校验任务状态
→ 刷新 baseline / 目标文件哈希
→ 计算风险与审批要求
→ 记录 operation_started
→ 执行工具
→ 计算文件变化
→ 写入 Change Set / Verification
→ 记录 operation_finished
```

没有活动任务返回 `TASK_STATE_REQUIRED`。文件哈希不一致返回 `FILE_CHANGED_EXTERNALLY`。批准操作状态变化返回 `APPROVAL_STALE`。

### exec_command 写入分类

第一版采用保守分类：

- 明确只读命令可在无任务时执行，例如版本查询和 Git 只读子命令。
- 测试、构建、格式化、包管理器和未知命令视为可能写入，要求活动任务。
- `reason` 缺失时继承任务目标，并标记来源为 `inherited`。
- 执行前后比较工作树与文件哈希，生成副作用清单。

---

## Project State 计算

`project_state` 每次调用执行：

1. 读取 workspace 路径和 Harness workspace state。
2. 调用 Git helpers 获取 branch、HEAD、upstream、ahead/behind、status 和最近 commit。
3. 读取活动任务 baseline 和 Change Set。
4. 将当前工作树条目与 baseline 比较，分类为：
   - `baseline_changes`
   - `harness_changes`
   - `external_changes`
   - `unknown_changes`
5. 合并最近 Verification、Checkpoint、Pending Action 和下一步。
6. 按 max_files 截断并返回统计信息。

非 Git 仓库仍返回任务、文件变化、命令和验证状态。

---

## Checkpoint 与 rollback 设计

### 创建 checkpoint

- 收集当前任务已拥有或将要修改的文件。
- 记录路径、存在性、哈希、权限和文件类型。
- 对小于配置上限的普通文件保存内容。
- 对大文件、二进制或不可读文件标记 partial，并要求用户确认后继续。

### rollback

- 校验 task 与 checkpoint 仍属于当前 workspace。
- 对每个文件比较“当前哈希”和 Harness 记录的最后哈希。
- 匹配时恢复；不匹配时加入冲突列表。
- 任何冲突默认使该文件跳过，不覆盖。
- 回滚本身生成 Event 和 Change Set。

### 明确不回滚

- 网络请求结果。
- 数据库和外部服务写入。
- 全局包、系统软件和用户目录修改。
- 远程 Git push。
- 未被 Harness 记录的外部进程副作用。

---

## 命令与 Git 安全设计

### 默认策略

- 新工作区默认 `safe`。
- `trusted` 允许网络和更多开发命令，但仍阻止破坏性操作。
- `dangerous` 仅由桌面用户显式选择，并显示持续风险提示。

### 环境

- 继承最小 PATH、系统必需变量和 locale。
- HOME、TEMP、TMP、cache 指向应用数据目录下的每任务运行目录。
- 丢弃名称匹配 TOKEN、SECRET、PASSWORD、KEY、COOKIE、AUTH、CREDENTIAL 的变量。
- 禁止模型直接传入环境变量，后续若开放必须逐项审批。

### Git 子命令

- 自动允许：status、diff、log、show、blame、rev-parse、branch --show-current。
- 任务内允许：add、创建本地分支。
- 需要审批：commit、switch/checkout、push。
- 默认禁止：reset、clean、push --force、remote set-url、config --global、filter-branch。

---

## Context Capsule 设计

优先级从高到低：

1. objective、task status、branch、HEAD。
2. pending steps、失败验证、冲突和 Pending Action。
3. Harness 修改文件与原因。
4. 最近成功验证和完成步骤。
5. 历史事件摘要。

默认上限 32 KiB，允许客户端请求 8 KiB 至 128 KiB。超限时不截断 JSON 字段中间内容，而是减少列表项并返回 `truncated_sections`。

---

## 文件结构

### 新增后端文件

```text
src-tauri/src/harness/
├── mod.rs
├── model.rs
├── store.rs
├── state.rs
├── events.rs
├── changes.rs
├── checkpoint.rs
├── approval.rs
├── capsule.rs
└── tools.rs

src-tauri/src/commands/harness.rs
```

### 修改后端文件

```text
src-tauri/src/lib.rs
src-tauri/src/app_state.rs
src-tauri/src/data/model.rs
src-tauri/src/data/migrate.rs
src-tauri/src/tools/context.rs
src-tauri/src/tools/dispatch.rs
src-tauri/src/tools/registry.rs
src-tauri/src/tools/policy.rs
src-tauri/src/tools/exec.rs
src-tauri/src/tools/git.rs
src-tauri/src/mcp/server.rs
src-tauri/src/actions/listener.rs
src-tauri/src/actions/openapi.rs
src-tauri/src/commands/mod.rs
src-tauri/src/workspace/model.rs
src-tauri/src/platform/mod.rs
src-tauri/src/platform/windows/process.rs
```

### 前端文件

```text
src/lib/api/harness.ts
src/lib/types.ts
src/lib/components/HarnessStatePanel.svelte
src/lib/components/TaskTimeline.svelte
src/lib/components/CheckpointPanel.svelte
src/lib/components/PendingActionsPanel.svelte
src/routes/workspace/[id]/+page.svelte
src/app.css
```

### 测试文件

```text
src-tauri/tests/harness_state.rs
src-tauri/tests/harness_events.rs
src-tauri/tests/harness_checkpoint.rs
src-tauri/tests/harness_approval.rs
src-tauri/tests/harness_tool_contract.rs
src-tauri/tests/call_tool_security.rs
src-tauri/tests/call_tool_contract.rs
```

---

## 设计决策

### 决策 1：服务端强制任务协议（关联需求：FR-4）

**问题：** 提示 ChatGPT 先调用 Project State 无法保证模型遵守。

**选项：** 仅写 instructions；自动为所有写操作创建隐式任务；无任务时拒绝写操作。

**决策：** 无任务时拒绝写操作并返回 `TASK_STATE_REQUIRED`，响应附带下一步工具建议。桌面 UI 可提供“一键开始任务”。后续可增加显式配置允许隐式任务，但不作为默认行为。

### 决策 2：实时状态与持久状态分离（关联需求：FR-2、FR-3）

**问题：** 缓存 Git 状态会过期，全部实时计算又无法恢复任务目标。

**决策：** Git/文件状态实时计算；任务、事件、验证、审批和 checkpoint 持久化。

### 决策 3：单工作区单写任务（关联需求：FR-3）

**问题：** 多个客户端同时写入会破坏变更归属和 rollback。

**决策：** 第一版只允许一个非终态可写任务。其他客户端可以读取 Project State，但写工具返回 `TASK_LOCKED`。

### 决策 4：文件快照而非 Git reset（关联需求：FR-7）

**问题：** Git reset/stash 可能覆盖用户已有修改或改变索引状态。

**决策：** 第一版使用文件哈希和应用数据目录快照。未来可把临时 worktree 作为隔离执行模式，但不替换文件所有权规则。

### 决策 5：桌面审批为权威来源（关联需求：FR-10）

**问题：** ChatGPT Connector 对 MCP Elicitation 的支持不可作为硬依赖。

**决策：** Pending Action 由后端持久化，桌面 UI 决策；MCP 客户端轮询状态或重试原工具。

### 决策 6：不存完整聊天（关联需求：FR-5、NFR-3）

**问题：** 完整聊天体积大且可能包含敏感信息。

**决策：** 只保存显式 objective、reason、结构化工具事件和结果摘要；推断内容必须标记来源。

---

## 错误码

| 错误码 | 含义 |
|------|------|
| `TASK_STATE_REQUIRED` | 写操作缺少活动任务 |
| `TASK_ALREADY_ACTIVE` | 工作区已有可写任务 |
| `TASK_LOCKED` | 任务由其他客户端持有 |
| `INVALID_TASK_TRANSITION` | 非法状态迁移 |
| `BASELINE_STALE` | 项目 baseline 不再有效 |
| `FILE_CHANGED_EXTERNALLY` | 文件在 Harness 外被修改 |
| `CHECKPOINT_PARTIAL` | checkpoint 不能完整保存全部内容 |
| `ROLLBACK_CONFLICT` | rollback 会覆盖外部修改 |
| `APPROVAL_REQUIRED` | 操作需要桌面审批 |
| `APPROVAL_STALE` | 批准后项目状态已变化 |
| `APPROVAL_EXPIRED` | 审批已过期 |

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 大仓库 Project State 扫描过慢 | 高 | Git porcelain、条目上限、P95 指标、明确截断 |
| rollback 覆盖用户修改 | 严重 | baseline、每文件哈希、所有权记录、冲突默认跳过 |
| 命令产生不可观测副作用 | 高 | 保守分类、前后状态比较、partial/unavailable 标记 |
| 多客户端竞争写入 | 高 | 单写任务锁、client_id/lease、超时恢复 |
| 事件日志泄露秘密 | 严重 | 字段级脱敏、敏感模式测试、禁止完整聊天和环境原值 |
| Pending Action 被过期状态复用 | 高 | 申请时指纹、执行前重校验、短 TTL |
| 工具注册迁移导致 ChatGPT schema 漂移 | 中 | 兼容旧名称、统一注册表、MCP/Actions 契约测试 |
| Windows 进程树无法完全终止 | 高 | Job Object/进程树实现、真机超时测试 |
| 状态文件损坏 | 中 | 原子替换、schema_version、备份和恢复诊断 |
| 功能范围过大导致长期分支 | 高 | 按 Foundation、Recovery、Delivery 三个可交付切片实现 |

---

## 分阶段交付

### Slice A：Harness Foundation

- 统一工具注册。
- Project State。
- Task Session 和服务端门禁。
- Event Log、Change Set、Context Capsule 最小版本。

### Slice B：Recovery and Safety

- Checkpoint/rollback。
- 文件所有权和冲突检测。
- 命令环境、进程树和 Pending Action。

### Slice C：Git Delivery and UX

- 受控 Git 写工具。
- Harness UI、时间线和审批。
- 全量 dogfood、Windows 与 ChatGPT Connector 发布验证。

