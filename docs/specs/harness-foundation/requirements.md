# 需求文档：harness-foundation

## 功能概述

Coding Tools MCP Desktop 需要从“可调用的本地工具集合”升级为面向 ChatGPT 的本地 Coding Harness。Harness 必须在聊天上下文之外持续维护项目状态、任务进度、变更原因、验证证据和恢复点，并以统一、安全、可观察的方式向 MCP 与 Actions 暴露文件、命令和 Git 能力。

目标工作流为：

```text
连接工作区
→ 建立或恢复任务
→ 获取 Project State
→ 读取、修改、执行命令
→ 记录 Change Intelligence
→ 验证结果
→ 完成、暂停或回滚任务
```

## 历史经验与坑（来自记忆库）

- **可复用经验**：外部契约、持久化字段和 UI 展示字段必须使用稳定名称并建立覆盖测试，避免“代码已增加字段但映射或展示层未同步”。
- **必须规避的坑**：不得将模型推断当作事实保存；不得依赖客户端主动遵守提示词；不得在失败时静默丢失附件式的变更证据；所有状态流转必须由服务端规则约束。
- **直接同类经验**：暂无与 Coding Harness 状态、事务或 Change Intelligence 完全同类的历史资产，因此本规格以当前代码、旧版 MCP 合规契约和本轮用户反馈为主。

---

## 范围边界

### In Scope

- 统一 MCP 与 Actions 的工具注册、档位、注解和暴露策略。
- 提供实时 Project State 聚合工具。
- 提供可持久化 Task Session 状态机，并允许断线和应用重启后恢复。
- 服务端强制写工具依赖活动任务与有效 baseline。
- 提供结构化 Event Log、Change Set、Verification Record 和 Context Capsule。
- 提供文件级 checkpoint 和仅针对当前任务变更的 rollback。
- 使用文件哈希检测用户或外部程序的并发修改。
- 提供受控 Git 写操作和桌面 Pending Action 审批。
- 加固命令执行策略、环境脱敏、运行目录、超时和进程树清理。
- 在桌面端展示 Harness 状态、任务时间线、验证、审批和恢复入口。
- 建立 MCP、Actions、Windows、ChatGPT Connector 的端到端验证。

### Out of Scope

- 不实现模型推理、模型路由、子代理编排或云任务队列。
- 不保存完整聊天记录或用户提示词全文。
- 不提供跨工作区的长期个性化记忆。
- 不承诺回滚网络请求、数据库写入、全局软件安装、远程 Git push 或任意外部进程副作用。
- 第一版不允许多个客户端同时对同一工作区执行写任务。
- 第一版不提供无限制的通用 shell 或无限制 `git_command`。
- 第一版不以 Git commit、stash 或 `reset --hard` 作为事务实现基础。

---

## 核心不变量

1. 用户在任务开始前已有的修改永远不属于 Harness 本次任务，不得被自动回滚。
2. 用户或外部程序在任务执行期间修改文件后，Harness 不得静默覆盖。
3. 每个写操作必须关联活动 Task Session，并拥有可追溯的原因来源。
4. Project State 中的 Git 与文件状态必须实时计算，不得仅依赖缓存。
5. 任务元数据、事件和 checkpoint 存储在应用数据目录，不默认写入用户仓库。
6. 日志和事件不得保存密码、OAuth token、API key、密钥或敏感环境变量。
7. safe 和 trusted 模式不得自动执行破坏性 Git 操作。
8. 任务完成必须明确记录验证状态；没有验证时只能标记为 `completed_unverified`。

---

## 需求列表

### FR-1：统一工具注册和能力发现

**优先级：Must**

**用户故事：** 作为 ChatGPT 用户，我想在 MCP 与 Actions 中看到一致的 Git、命令和 Harness 工具，以便连接方式不会改变可用能力。

#### 验收标准（EARS）

1. WHEN MCP 或 Actions 生成工具清单 THEN 系统 SHALL 从同一工具注册表派生名称、Schema、注解、档位和传输可见性。
2. WHEN 工具档位为 `full` THEN MCP SHALL 暴露 `exec_command`、长命令 Session、Git 只读工具和 Harness 工具。
3. WHEN 工具档位为 `read-only` THEN 系统 SHALL 隐藏写工具，同时保留 Project State 和 Git 只读能力。
4. WHEN `server_info` 返回工具列表 THEN 该列表 SHALL 与当前传输的 `tools/list` 可见工具一致。
5. IF 工具配置在运行期间发生改变 THEN 系统 SHALL 要求重启服务或发送工具列表变更通知，不得继续返回旧配置而无提示。

### FR-2：提供实时 Project State

**优先级：Must**

**用户故事：** 作为 ChatGPT，我想在任务开始或恢复时读取完整项目状态，以便不依赖聊天上下文判断当前进度。

#### 验收标准（EARS）

1. WHEN 调用 `project_state` THEN 系统 SHALL 实时返回 workspace、branch、HEAD、upstream、ahead、behind、最近 commit 和工作树状态。
2. WHEN 存在活动任务 THEN `project_state` SHALL 返回任务目标、状态、完成步骤、待办步骤、最近变更、最近命令、验证结果和 checkpoint 摘要。
3. WHEN 工作树包含任务开始前的修改 THEN 系统 SHALL 将其标记为 `baseline_changes`。
4. WHEN 工作树包含当前任务产生的修改 THEN 系统 SHALL 将其标记为 `harness_changes`。
5. IF Git 不可用或工作区不是仓库 THEN 系统 SHALL 返回可用的文件与任务状态，并附带结构化警告。

### FR-3：建立 Task Session 状态机

**优先级：Must**

**用户故事：** 作为用户，我想让任务状态独立于单次 ChatGPT 对话，以便断线、换对话或重启后继续工作。

#### 验收标准（EARS）

1. WHEN 调用 `start_task` THEN 系统 SHALL 创建包含目标、baseline、时间和唯一 ID 的 Task Session。
2. WHEN 同一工作区已有可写活动任务 THEN 系统 SHALL 拒绝创建第二个可写任务并返回 `TASK_ALREADY_ACTIVE`。
3. WHEN任务正常流转 THEN 系统 SHALL 支持 `active`、`paused`、`verifying`、`completed`、`completed_unverified`、`failed`、`rolled_back` 状态。
4. WHEN MCP 断线或桌面应用重启 THEN 系统 SHALL 从持久化存储恢复非终态任务。
5. IF 请求非法状态迁移 THEN 系统 SHALL 返回 `INVALID_TASK_TRANSITION` 且不修改任务。

### FR-4：强制任务与 baseline 门禁

**优先级：Must**

**用户故事：** 作为用户，我想让 Harness 在执行写操作前建立状态基线，以便修改可追踪、可验证、可恢复。

#### 验收标准（EARS）

1. WHEN `apply_patch`、受控 Git 写操作或可能写文件的 `exec_command` 被调用 THEN 系统 SHALL 校验活动 Task Session 和 baseline。
2. IF 没有活动任务 THEN 系统 SHALL 返回 `TASK_STATE_REQUIRED`，不得执行写操作。
3. IF baseline 已过期或关键文件状态发生外部变化 THEN 系统 SHALL 返回 `BASELINE_STALE` 或 `FILE_CHANGED_EXTERNALLY`。
4. WHEN 只读工具被调用 THEN 系统 SHALL 允许无任务访问，但仍返回当前 task_id（如存在）。
5. WHEN 客户端不主动调用 `project_state` THEN 服务端 SHALL 通过门禁保证写操作不会绕过任务协议。

### FR-5：记录 Event Log 与 Change Intelligence

**优先级：Must**

**用户故事：** 作为用户，我想知道每个文件为什么被修改、执行了哪些命令以及验证结果，以便审查和恢复任务。

#### 验收标准（EARS）

1. WHEN 工具调用开始和结束 THEN 系统 SHALL 记录结构化事件、task_id、工具名、时间、结果和脱敏后的输入摘要。
2. WHEN 写操作影响文件 THEN 系统 SHALL 记录文件路径、操作类型、修改前后哈希和修改原因。
3. WHEN 原因来自用户、模型、继承任务目标或系统推断 THEN 系统 SHALL 标记来源为 `user_provided`、`model_provided`、`inherited` 或 `inferred`。
4. WHEN 执行测试、构建、检查或 lint 命令 THEN 系统 SHALL 记录 Verification Record，并关联 Change Set。
5. IF 输入包含敏感字段或环境变量 THEN 系统 SHALL 脱敏后记录，且不得保存原值。

### FR-6：生成有界 Context Capsule

**优先级：Must**

**用户故事：** 作为新的 ChatGPT 对话，我想获取精简但完整的任务恢复上下文，以便继续工作而不重新读取全部历史。

#### 验收标准（EARS）

1. WHEN 调用 `task_context` THEN 系统 SHALL 返回目标、baseline 摘要、已完成、待办、变更、验证、风险和建议下一步。
2. WHEN事件数量超过限制 THEN 系统 SHALL 对旧事件聚合并通过分页工具提供明细。
3. WHEN Capsule 超过配置的字节上限 THEN 系统 SHALL 按优先级压缩低价值内容，同时保留任务目标、当前状态、未完成事项和失败验证。
4. IF 没有活动任务 THEN 系统 SHALL 返回最近终态任务摘要或明确的空状态。

### FR-7：提供文件级 checkpoint 和 rollback

**优先级：Must**

**用户故事：** 作为用户，我想在修改或测试失败后恢复当前任务产生的文件变化，同时保留自己的原始修改。

#### 验收标准（EARS）

1. WHEN 创建 checkpoint THEN 系统 SHALL 保存当前任务涉及文件的路径、类型、哈希和恢复所需内容。
2. WHEN rollback 执行 THEN 系统 SHALL 只恢复 checkpoint 后由当前任务拥有的变更。
3. IF 文件在 checkpoint 后被用户或外部程序再次修改 THEN 系统 SHALL 返回 `ROLLBACK_CONFLICT`，不得覆盖该文件。
4. WHEN 变更包含新建、修改和删除文件 THEN rollback SHALL 分别支持删除新建文件、恢复原内容和恢复删除文件。
5. IF 操作包含网络、数据库、全局安装或远程 Git 副作用 THEN 系统 SHALL 标记 `rollback_capability` 为 `partial` 或 `unavailable`。

### FR-8：加固命令执行环境

**优先级：Must**

**用户故事：** 作为用户，我想让 ChatGPT 执行测试、构建和诊断命令，同时不泄露本机密钥或破坏工作区外环境。

#### 验收标准（EARS）

1. WHEN 创建新工作区配置 THEN 默认 permission mode SHALL 为 `safe`。
2. WHEN 执行命令 THEN 系统 SHALL 使用受控 PATH、脱敏环境、独立 HOME/TEMP/cache 和工作区内 cwd。
3. WHEN 命令超时或服务停止 THEN 系统 SHALL 终止整个进程树并清理 Session。
4. IF 命令包含危险 Git、特权、工作区逃逸、敏感环境或网络行为 THEN safe 模式 SHALL 拒绝或创建 Pending Action。
5. WHEN 命令持续运行 THEN MCP full 档位 SHALL 提供 read_output、write_stdin 和 kill_session 完整闭环。

### FR-9：提供受控 Git 写能力

**优先级：Should**

**用户故事：** 作为用户，我想让 ChatGPT 在确认后完成暂存、提交、切换分支和推送，以便形成完整交付闭环。

#### 验收标准（EARS）

1. WHEN 调用 Git 写工具 THEN 系统 SHALL 提供结构化 `git_add`、`git_commit`、`git_create_branch`、`git_switch` 和 `git_push`，不使用无限制通用 Git 工具。
2. WHEN 执行 `git_commit` 或 `git_push` THEN 系统 SHALL 创建 Pending Action 并等待桌面用户确认。
3. IF 请求为 reset、clean、force push、修改 remote 或修改全局 Git 配置 THEN safe 和 trusted 模式 SHALL 默认拒绝。
4. WHEN工作树存在未归属当前任务的修改 THEN Git 写工具 SHALL 显示影响范围并禁止默认包含这些修改。

### FR-10：提供桌面 Pending Action 审批

**优先级：Must**

**用户故事：** 作为桌面用户，我想审查 ChatGPT 请求的危险操作并明确允许或拒绝，以便不依赖客户端是否支持 MCP Elicitation。

#### 验收标准（EARS）

1. WHEN 工具需要用户确认 THEN 系统 SHALL 创建包含 action_id、task_id、原因、风险、影响和过期时间的 Pending Action。
2. WHEN 用户允许或拒绝 THEN 系统 SHALL 持久化决策、操作者和时间。
3. WHEN Pending Action 过期 THEN 系统 SHALL 自动拒绝且不得执行原操作。
4. WHEN客户端查询 action 状态 THEN 系统 SHALL 返回 `pending`、`approved`、`denied`、`expired` 或 `executed`。
5. IF批准后的工作区状态与申请时不同 THEN 系统 SHALL 重新校验并要求再次申请。

### FR-11：提供 Harness 桌面状态界面

**优先级：Should**

**用户故事：** 作为用户，我想在桌面应用中看到 ChatGPT 当前任务、变更、测试、checkpoint 和待审批操作，以便监督 Harness。

#### 验收标准（EARS）

1. WHEN 打开工作区页面 THEN 系统 SHALL 展示当前任务状态、branch、HEAD、修改数量和最近验证。
2. WHEN产生事件 THEN 时间线 SHALL 展示工具、结果、影响文件和原因摘要。
3. WHEN存在 checkpoint THEN 用户 SHALL 能查看可恢复范围并发起 rollback。
4. WHEN存在 Pending Action THEN UI SHALL 显示风险和影响，并提供允许、拒绝和过期状态。
5. IF 后端状态不可用 THEN UI SHALL 显示可恢复错误，不得将未知状态显示为成功。

### FR-12：建立迁移、合规和端到端验证

**优先级：Must**

**用户故事：** 作为维护者，我想通过自动化测试证明 Harness 状态、恢复和安全边界可靠，以便发布给真实 ChatGPT 用户。

#### 验收标准（EARS）

1. WHEN旧工作区数据加载 THEN 系统 SHALL 自动补齐 Harness 默认字段而不破坏现有配置。
2. WHEN执行合规测试 THEN 系统 SHALL 覆盖工具暴露、任务门禁、状态恢复、日志脱敏、并发冲突和危险命令拒绝。
3. WHEN执行 dogfood THEN 系统 SHALL 覆盖新对话恢复、失败回滚、Change Intelligence 和 Git/exec 闭环。
4. WHEN在 Windows 真机验证 THEN 系统 SHALL 证明进程树终止、路径边界和 Git 行为符合契约。
5. WHEN通过公网 ChatGPT Connector 验证 THEN 系统 SHALL 完成 OAuth、Project State、修改、测试和总结的完整流程。

---

## 非功能需求

- **NFR-1 性能**：普通规模仓库（10,000 个工作树条目以内）的 `project_state` P95 响应时间不超过 2 秒；超过限制时必须截断并标记。
- **NFR-2 持久性**：Task Session、审批和事件写入采用原子替换或追加日志；进程异常退出后不得产生无法解析的主状态文件。
- **NFR-3 安全**：敏感字段脱敏测试覆盖率为 100%；safe/trusted 模式下破坏性 Git 命令执行成功次数必须为 0。
- **NFR-4 隔离**：Harness 状态默认存储在应用数据目录，仓库内不得自动生成 `.coding-tools` 或类似目录。
- **NFR-5 兼容性**：保留现有 MCP 工具名称和输入字段；新增字段默认可选，旧客户端仍可完成只读操作。
- **NFR-6 可观测性**：每次工具调用必须拥有 operation_id；任务、Change Set、Verification 和 Pending Action 可通过 ID 串联。
- **NFR-7 容量**：单任务事件默认保留 10,000 条或 50 MiB，以先达到者为准；超限后归档并生成摘要。
- **NFR-8 可恢复性**：文件级 rollback 自动化测试必须覆盖已有脏工作区、新建文件、删除文件和外部修改冲突。
- **NFR-9 可用性**：Project State、任务和审批 UI 不得阻塞现有 MCP/Actions 服务启停功能。
- **NFR-10 跨平台**：Windows 为首要支持平台，同时保持 Linux/macOS 的编译和路径语义兼容。

---

## 依赖关系

- 依赖现有 `ToolContext`、统一 `call_tool` 入口、Workspace 路径边界和 SessionStore。
- 依赖现有 `git_status`、`git_diff`、`git_log` 与命令执行能力。
- 依赖 AppData/DataStore 的版本化迁移能力和应用配置目录解析。
- 依赖 Tauri command 作为桌面 UI 的状态、审批和回滚接口。
- 依赖 MCP `tools/list`、`tools/call` 和当前 OAuth/HTTP transport。
- 依赖现有 Rust 集成测试、旧版 Python 合规契约和 ChatGPT Connector 真机环境。

