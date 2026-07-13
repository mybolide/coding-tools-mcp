# 任务清单：harness-foundation

## 概述

本清单按 Harness Foundation、Recovery and Safety、Git Delivery and UX 三个可独立验收的切片实施。每条任务必须先读取证据文件，完成后运行对应定向测试；禁止在规格闸门通过前编写业务实现。

> **二元禁令**：交付物中零容忍占位符、TODO 或省略实现。单文件超过 500 行必须拆分。

---

## 交付物清单（Scope-lock）

- 预计实现文件数：43
- 预计任务数：27
- 新增 Rust Harness 模块：10 个
- 新增 Tauri command 文件：1 个
- 新增前端 API/组件：5 个
- 新增 Rust 集成测试：5 个
- 修改现有 Rust/前端/测试文件：22 个

实现完成前必须回读“文件变更清单”。实际文件数增加超过 5 个时，需要更新设计文档说明拆分原因。

---

## 任务列表

## 阶段 1：规格与现状基线

- [ ] 1.1 固化 MCP 与 Actions 当前工具清单基线，覆盖 full/read-only
  - 证据块：`src-tauri/src/tools/registry.rs:3`、`src-tauri/src/mcp/server.rs:10`、`src-tauri/src/actions/listener.rs:142`
  - 文件：`src-tauri/tests/harness_tool_contract.rs`（预算 220 行）
  - 验收：测试记录当前工具清单，并明确 exec、session、Git 和 Harness 工具的目标可见性。
  - _需求：FR-1_ · _设计：统一工具定义_

- [ ] 1.2 建立 Harness 测试公共夹具，隔离应用数据目录和 Git 仓库
  - 证据块：`src-tauri/tests/common/mod.rs:1`、`src-tauri/src/platform/paths.rs:1`
  - 文件：`src-tauri/tests/common/mod.rs`（新增预算 160 行）
  - 验收：每个测试拥有独立 workspace、Harness root 和可控 Git baseline。
  - _需求：FR-12_ · _设计：存储布局_

- [ ] 1.3 增加 Harness schema 与状态迁移失败测试
  - 证据块：`src-tauri/src/data/model.rs:10`、`src-tauri/src/data/migrate.rs:1`
  - 文件：`src-tauri/tests/harness_state.rs`（预算 300 行）
  - 验收：覆盖首次创建、旧版本升级、损坏尾事件和应用重启恢复。
  - _需求：FR-3、FR-12_ · _设计：数据模型、存储布局_

### 阶段 1 检查点

- [ ] 新增测试先失败，证明当前代码缺少 Harness 能力。
- [ ] 当前已有 40 项 Rust 测试保持通过。
- [ ] 测试不读写真实用户应用数据目录。

---

## 阶段 2：统一工具注册与能力发现

- [ ] 2.1 将 tuple 工具表重构为 ToolDefinition 单一注册表
  - 证据块：`src-tauri/src/tools/registry.rs:3`、`src-tauri/src/tools/registry.rs:80`
  - 文件：`src-tauri/src/tools/registry.rs`（预算 420 行）
  - 验收：名称、Schema、档位、传输、注解、mutation 和 approval 从同一定义读取。
  - _需求：FR-1_ · _设计：统一工具定义_

- [ ] 2.2 统一 MCP、Actions、server_info 的工具过滤
  - 证据块：`src-tauri/src/mcp/server.rs:10`、`src-tauri/src/actions/openapi.rs:1`、`src-tauri/src/tools/dispatch.rs:68`
  - 文件：`src-tauri/src/mcp/server.rs`、`src-tauri/src/actions/listener.rs`、`src-tauri/src/actions/openapi.rs`、`src-tauri/src/tools/dispatch.rs`（合计预算 180 行）
  - 验收：同档位下工具名称一致；Actions 明确排除不支持的交互工具并返回原因。
  - _需求：FR-1_ · _设计：调用原则_

- [ ] 2.3 更新工作区默认工具和命令策略
  - 证据块：`src-tauri/src/workspace/model.rs:49`、`src-tauri/src/tools/policy.rs:12`
  - 文件：`src-tauri/src/workspace/model.rs`、`src-tauri/src/tools/policy.rs`（合计预算 100 行）
  - 验收：新工作区默认 safe；Actions 默认命令包含受控 git；旧配置迁移保持原值。
  - _需求：FR-1、FR-8、FR-12_ · _设计：默认策略_

### 阶段 2 检查点

- [ ] MCP full 能看到 exec、session、Git 和 Harness 查询工具。
- [ ] MCP read-only 隐藏写工具。
- [ ] `server_info.tools` 与 `tools/list` 一致。
- [ ] Actions OpenAPI 不出现未实现或不可执行的 operation。

---

## 阶段 3：Harness 数据模型与持久化

- [ ] 3.1 新建 Harness 核心模型并限制单文件规模
  - 证据块：`src-tauri/src/tools/context.rs:9`、`src-tauri/src/data/model.rs:10`
  - 文件：`src-tauri/src/harness/mod.rs`（≤100 行）、`src-tauri/src/harness/model.rs`（≤480 行）
  - 验收：实现 TaskStatus、TaskSession、Baseline、Event、ChangeSet、Verification、Checkpoint、PendingAction 和 schema_version。
  - _需求：FR-2 至 FR-7、FR-10_ · _设计：数据模型_

- [ ] 3.2 实现 HarnessStore 原子状态写入和 JSONL 事件追加
  - 证据块：`src-tauri/src/data/store.rs:25`、`src-tauri/src/data/migrate.rs:1`
  - 文件：`src-tauri/src/harness/store.rs`（≤480 行）、`src-tauri/src/harness/events.rs`（≤360 行）
  - 验收：原子写、事件分页、损坏尾行容错、容量限制和路径隔离测试通过。
  - _需求：FR-3、FR-5、FR-6、FR-12_ · _设计：存储布局_

- [ ] 3.3 将 HarnessStore 注入 AppState 与 ToolContext
  - 证据块：`src-tauri/src/app_state.rs:7`、`src-tauri/src/tools/context.rs:9`、`src-tauri/src/mcp/server.rs:75`
  - 文件：`src-tauri/src/app_state.rs`、`src-tauri/src/tools/context.rs`、`src-tauri/src/mcp/server.rs`、`src-tauri/src/lib.rs`（合计预算 160 行）
  - 验收：MCP 与桌面 command 共享同一 HarnessStore；多 workspace 状态隔离。
  - _需求：FR-3、FR-11_ · _设计：总体架构_

- [ ] 3.4 实现 Task Session 状态迁移与单写任务锁
  - 证据块：`src-tauri/src/harness/model.rs`、`src-tauri/src/runtime/supervisor.rs:42`
  - 文件：`src-tauri/src/harness/state.rs`（≤480 行）
  - 验收：合法迁移、非法迁移、重启恢复、第二写任务拒绝和 lease 过期测试通过。
  - _需求：FR-3_ · _设计：Task Session 状态_

### 阶段 3 检查点

- [ ] 应用重启后活动任务可恢复。
- [ ] 一个工作区无法创建第二个可写任务。
- [ ] 状态文件损坏不会影响 profiles 配置加载。
- [ ] Harness 数据不会写入用户仓库。

---

## 阶段 4：Project State、门禁和 Context Capsule

- [ ] 4.1 实现实时 Project State 聚合与 baseline 分类
  - 证据块：`src-tauri/src/tools/git.rs:9`、`src-tauri/src/tools/git.rs:99`、`src-tauri/src/tools/git.rs:161`
  - 文件：`src-tauri/src/harness/state.rs`（新增预算 220 行）、`src-tauri/src/harness/changes.rs`（≤420 行）
  - 验收：返回 branch、HEAD、最近 commit、dirty 分类、任务、验证、checkpoint 和审批；非 Git 仓库降级可用。
  - _需求：FR-2_ · _设计：Project State 计算_

- [ ] 4.2 实现 Harness MCP 工具和输入输出 Schema
  - 证据块：`src-tauri/src/tools/registry.rs:80`、`src-tauri/src/tools/dispatch.rs:19`
  - 文件：`src-tauri/src/harness/tools.rs`（≤480 行）、`src-tauri/src/tools/registry.rs`、`src-tauri/src/tools/dispatch.rs`（合计预算 260 行）
  - 验收：project_state、start/update/pause/resume/finish_task、task_context、list_task_events、change_summary 可调用。
  - _需求：FR-2、FR-3、FR-5、FR-6_ · _设计：API 与工具设计_

- [ ] 4.3 在 call_tool 前后接入任务门禁和事件记录
  - 证据块：`src-tauri/src/tools/dispatch.rs:19`、`src-tauri/src/tools/policy.rs:105`
  - 文件：`src-tauri/src/tools/dispatch.rs`、`src-tauri/src/harness/events.rs`、`src-tauri/src/harness/changes.rs`（合计预算 260 行）
  - 验收：无任务写操作返回 TASK_STATE_REQUIRED；外部修改返回 FILE_CHANGED_EXTERNALLY；只读操作保持兼容。
  - _需求：FR-4、FR-5_ · _设计：Harness Gate_

- [ ] 4.4 实现有界 Context Capsule 和事件压缩
  - 证据块：`src-tauri/src/harness/events.rs`、`src-tauri/src/tools/session.rs:218`
  - 文件：`src-tauri/src/harness/capsule.rs`（≤420 行）
  - 验收：32 KiB 默认上限、字段优先级、truncated_sections、事件分页和新对话恢复测试通过。
  - _需求：FR-6_ · _设计：Context Capsule 设计_

### 阶段 4 检查点

- [ ] 新对话调用 project_state/task_context 可恢复任务。
- [ ] 写工具无法绕过 active task 和 baseline。
- [ ] Change Set 可以回答改了什么、为什么改和如何验证。
- [ ] Project State P95 在测试仓库中不超过 2 秒。

---

## 阶段 5：Checkpoint 与 rollback

- [ ] 5.1 实现 checkpoint manifest、内容快照和容量策略
  - 证据块：`src-tauri/src/tools/patch.rs:1`、`src-tauri/src/tools/workspace.rs:1`
  - 文件：`src-tauri/src/harness/checkpoint.rs`（≤500 行）
  - 验收：保存新建、修改、删除文件状态；大文件和二进制标记 partial。
  - _需求：FR-7_ · _设计：创建 checkpoint_

- [ ] 5.2 实现仅回滚任务拥有变更的恢复算法
  - 证据块：`src-tauri/src/harness/checkpoint.rs`、`src-tauri/src/harness/changes.rs`
  - 文件：`src-tauri/src/harness/checkpoint.rs`（新增预算 260 行）、`src-tauri/tests/harness_checkpoint.rs`（≤480 行）
  - 验收：脏工作区、外部修改冲突、新建、删除、部分回滚和重复回滚测试通过。
  - _需求：FR-7、NFR-8_ · _设计：rollback_

- [ ] 5.3 暴露 checkpoint 工具和桌面 command
  - 证据块：`src-tauri/src/commands/mod.rs:1`、`src-tauri/src/lib.rs:20`
  - 文件：`src-tauri/src/commands/harness.rs`（≤420 行）、`src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/harness/tools.rs`（合计预算 220 行）
  - 验收：MCP 与桌面端可创建、列出和回滚 checkpoint，并返回冲突详情。
  - _需求：FR-7、FR-11_ · _设计：API 与工具设计_

### 阶段 5 检查点

- [ ] rollback 不改变任务 baseline 中的用户原始修改。
- [ ] 外部修改冲突默认跳过且明确报告。
- [ ] 不可恢复副作用明确标记 partial/unavailable。

---

## 阶段 6：命令安全、Git 写工具和审批

- [ ] 6.1 加固 exec 环境、运行目录与进程树清理
  - 证据块：`src-tauri/src/tools/exec.rs:11`、`src-tauri/src/tools/session.rs:15`、`src-tauri/src/platform/windows/process.rs:1`
  - 文件：`src-tauri/src/tools/exec.rs`、`src-tauri/src/tools/session.rs`、`src-tauri/src/platform/mod.rs`、`src-tauri/src/platform/windows/process.rs`（合计预算 420 行）
  - 验收：环境脱敏、独立 HOME/TEMP/cache、超时进程树终止和服务关闭清理测试通过。
  - _需求：FR-8_ · _设计：命令与 Git 安全设计_

- [ ] 6.2 实现 Git 子命令分类和危险操作拒绝
  - 证据块：`src-tauri/src/tools/policy.rs:105`、`src-tauri/src/tools/git.rs:1`
  - 文件：`src-tauri/src/tools/policy.rs`、`src-tauri/src/tools/git.rs`、`src-tauri/tests/call_tool_security.rs`（合计预算 300 行）
  - 验收：reset、clean、force push、remote set-url、config --global 在 safe/trusted 下全部拒绝。
  - _需求：FR-8、FR-9_ · _设计：Git 子命令_

- [ ] 6.3 实现 Pending Action 持久化、TTL 和状态指纹
  - 证据块：`src-tauri/src/harness/model.rs`、`src-tauri/src/harness/store.rs`
  - 文件：`src-tauri/src/harness/approval.rs`（≤480 行）、`src-tauri/tests/harness_approval.rs`（≤420 行）
  - 验收：批准、拒绝、过期、状态变化失效和重启恢复测试通过。
  - _需求：FR-10_ · _设计：Pending Action 状态_

- [ ] 6.4 实现结构化 Git 写工具并接入审批
  - 证据块：`src-tauri/src/tools/git.rs:9`、`src-tauri/src/harness/approval.rs`
  - 文件：`src-tauri/src/tools/git.rs`、`src-tauri/src/harness/tools.rs`、`src-tauri/src/tools/registry.rs`（合计预算 420 行）
  - 验收：git_add/create_branch 可按策略执行；commit/switch/push 创建审批；不包含非任务修改。
  - _需求：FR-9、FR-10_ · _设计：受控 Git 写能力_

### 阶段 6 检查点

- [ ] safe 为新工作区默认值。
- [ ] 敏感环境变量不会进入子进程或事件日志。
- [ ] Windows 超时命令不残留子进程。
- [ ] commit/push 未审批时不能执行。

---

## 阶段 7：桌面 Harness UI

- [ ] 7.1 新增 Harness API 和前端类型
  - 证据块：`src/lib/api/logs.ts:1`、`src/lib/types.ts:1`
  - 文件：`src/lib/api/harness.ts`（≤240 行）、`src/lib/types.ts`（新增预算 220 行）
  - 验收：状态、事件、checkpoint、审批和决策 DTO 与 Rust 序列化字段一致。
  - _需求：FR-10、FR-11_ · _设计：API 与工具设计_

- [ ] 7.2 新增 Harness 状态、时间线、checkpoint 和审批组件
  - 证据块：`src/routes/workspace/[id]/+page.svelte:515`、`src/lib/components/LogViewer.svelte:1`
  - 文件：`src/lib/components/HarnessStatePanel.svelte`（≤360 行）、`src/lib/components/TaskTimeline.svelte`（≤420 行）、`src/lib/components/CheckpointPanel.svelte`（≤320 行）、`src/lib/components/PendingActionsPanel.svelte`（≤360 行）
  - 验收：展示任务、Git 状态、原因、验证、冲突、审批风险和状态。
  - _需求：FR-11_ · _设计：Harness 桌面状态界面_

- [ ] 7.3 将 Harness 区域接入工作区页面并保持服务控制可用
  - 证据块：`src/routes/workspace/[id]/+page.svelte:515`、`src/routes/workspace/[id]/+page.svelte:588`
  - 文件：`src/routes/workspace/[id]/+page.svelte`、`src/app.css`（合计预算 260 行）
  - 验收：MCP/Actions 服务面板功能不回归；Harness 状态可刷新、恢复、回滚和审批。
  - _需求：FR-11、NFR-9_ · _设计：总体架构_

### 阶段 7 检查点

- [ ] `npm run check` 0 错误、0 警告。
- [ ] `npm run build` 通过。
- [ ] 未知或后端错误状态不会显示为成功。

---

## 阶段 8：合规、dogfood 和发布验证

- [ ] 8.1 完善工具契约和安全集成测试
  - 证据块：`src-tauri/tests/call_tool_contract.rs:1`、`src-tauri/tests/call_tool_security.rs:1`
  - 文件：`src-tauri/tests/call_tool_contract.rs`、`src-tauri/tests/call_tool_security.rs`、`src-tauri/tests/harness_tool_contract.rs`（合计预算 360 行）
  - 验收：工具清单、Schema、任务门禁、审批和危险命令覆盖。
  - _需求：FR-1、FR-4、FR-8、FR-9、FR-10、FR-12_ · _设计：错误码、风险评估_

- [ ] 8.2 增加 Change Intelligence 与事件脱敏测试
  - 证据块：`src-tauri/src/harness/events.rs`、`src-tauri/src/harness/changes.rs`
  - 文件：`src-tauri/tests/harness_events.rs`（≤460 行）
  - 验收：原因来源、命令验证关联、敏感字段脱敏、事件分页和容量归档通过。
  - _需求：FR-5、FR-6、NFR-3、NFR-7_ · _设计：Event Log 与 Context Capsule_

- [ ] 8.3 扩展 dogfood 场景覆盖上下文恢复和失败回滚
  - 证据块：`old/docs/dogfood.md:1`、`old/tests/compliance/test_dogfood.py:1`
  - 文件：`old/tests/compliance/test_dogfood.py`、`old/docs/dogfood.md`（合计预算 260 行）
  - 验收：新对话恢复任务、修改原因、测试失败、回滚和 Git/exec 闭环均有记录。
  - _需求：FR-12_ · _设计：分阶段交付_

- [ ] 8.4 执行 Windows 真机与 ChatGPT Connector 发布验证
  - 证据块：`README.md:1`、`old/docs/remote-mcp.md:1`
  - 文件：`docs/specs/harness-foundation/verification.md`（≤300 行）
  - 验收：OAuth、Project State、任务恢复、修改、测试、审批、总结和进程清理全链路通过。
  - _需求：FR-12、NFR-10_ · _设计：风险评估_

### 阶段 8 检查点

- [ ] `cargo test` 全量通过。
- [ ] `cargo clippy` 无新增警告。
- [ ] `npm run check` 和 `npm run build` 通过。
- [ ] Windows 真机和 ChatGPT Connector 验证记录完成。

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 统一工具定义 | 1.1、2.1、2.2、2.3、8.1 | 未开始 |
| FR-2 | Project State 计算 | 4.1、4.2 | 未开始 |
| FR-3 | Task Session 状态 | 1.3、3.1、3.2、3.3、3.4、4.2 | 未开始 |
| FR-4 | Harness Gate | 4.3、8.1 | 未开始 |
| FR-5 | Event Log、Change Set | 3.1、3.2、4.2、4.3、8.2 | 未开始 |
| FR-6 | Context Capsule | 3.2、4.2、4.4、8.2 | 未开始 |
| FR-7 | Checkpoint 与 rollback | 5.1、5.2、5.3 | 未开始 |
| FR-8 | 命令与 Git 安全 | 2.3、6.1、6.2、8.1 | 未开始 |
| FR-9 | 受控 Git 写能力 | 6.2、6.4、8.1 | 未开始 |
| FR-10 | Pending Action | 3.1、6.3、6.4、7.1、7.2、8.1 | 未开始 |
| FR-11 | Harness 桌面状态界面 | 3.3、5.3、7.1、7.2、7.3 | 未开始 |
| FR-12 | 迁移、合规和端到端 | 1.2、1.3、2.3、8.1、8.2、8.3、8.4 | 未开始 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/harness/mod.rs` | 新建 | ≤100 | Harness 模块导出 |
| `src-tauri/src/harness/model.rs` | 新建 | ≤480 | 核心模型与状态枚举 |
| `src-tauri/src/harness/store.rs` | 新建 | ≤480 | 原子状态和对象存储 |
| `src-tauri/src/harness/state.rs` | 新建 | ≤500 | 任务状态机和 Project State |
| `src-tauri/src/harness/events.rs` | 新建 | ≤360 | JSONL 事件、脱敏和分页 |
| `src-tauri/src/harness/changes.rs` | 新建 | ≤420 | baseline、文件归属和 Change Set |
| `src-tauri/src/harness/checkpoint.rs` | 新建 | ≤500 | 快照和 rollback |
| `src-tauri/src/harness/approval.rs` | 新建 | ≤480 | Pending Action |
| `src-tauri/src/harness/capsule.rs` | 新建 | ≤420 | Context Capsule |
| `src-tauri/src/harness/tools.rs` | 新建 | ≤480 | Harness 工具处理器 |
| `src-tauri/src/commands/harness.rs` | 新建 | ≤420 | Tauri Harness commands |
| `src-tauri/src/lib.rs` | 修改 | +40 | 注册模块和 commands |
| `src-tauri/src/app_state.rs` | 修改 | +60 | 注入 HarnessStore |
| `src-tauri/src/data/model.rs` | 修改 | +30 | 迁移元信息引用 |
| `src-tauri/src/data/migrate.rs` | 修改 | +80 | Harness schema 迁移入口 |
| `src-tauri/src/tools/context.rs` | 修改 | +50 | task/client/harness 上下文 |
| `src-tauri/src/tools/dispatch.rs` | 修改 | +180 | Gate、事件和分发 |
| `src-tauri/src/tools/registry.rs` | 修改 | ≤500 | 单一 ToolDefinition 注册表 |
| `src-tauri/src/tools/policy.rs` | 修改 | +180 | 默认 safe、Git 风险分类 |
| `src-tauri/src/tools/exec.rs` | 修改 | +220 | 环境、运行目录和副作用记录 |
| `src-tauri/src/tools/session.rs` | 修改 | +100 | 进程树和任务关联 |
| `src-tauri/src/tools/git.rs` | 修改 | ≤500 | 结构化 Git 写工具 |
| `src-tauri/src/mcp/server.rs` | 修改 | +70 | 统一工具清单和客户端上下文 |
| `src-tauri/src/actions/listener.rs` | 修改 | +40 | 统一工具过滤 |
| `src-tauri/src/actions/openapi.rs` | 修改 | +60 | 统一注册表生成 OpenAPI |
| `src-tauri/src/commands/mod.rs` | 修改 | +10 | 导出 Harness commands |
| `src-tauri/src/workspace/model.rs` | 修改 | +30 | safe 默认值与兼容迁移 |
| `src-tauri/src/platform/mod.rs` | 修改 | +20 | 进程树能力接口 |
| `src-tauri/src/platform/windows/process.rs` | 修改 | +160 | Windows 进程树终止 |
| `src/lib/api/harness.ts` | 新建 | ≤240 | Harness Tauri API |
| `src/lib/types.ts` | 修改 | +220 | Harness DTO |
| `src/lib/components/HarnessStatePanel.svelte` | 新建 | ≤360 | 状态总览 |
| `src/lib/components/TaskTimeline.svelte` | 新建 | ≤420 | 任务事件时间线 |
| `src/lib/components/CheckpointPanel.svelte` | 新建 | ≤320 | checkpoint 和 rollback |
| `src/lib/components/PendingActionsPanel.svelte` | 新建 | ≤360 | 审批 UI |
| `src/routes/workspace/[id]/+page.svelte` | 修改 | +220 | 集成 Harness 区域 |
| `src/app.css` | 修改 | +120 | Harness UI 样式 |
| `src-tauri/tests/harness_state.rs` | 新建 | ≤420 | 状态、迁移和恢复 |
| `src-tauri/tests/harness_events.rs` | 新建 | ≤460 | 事件、变更和脱敏 |
| `src-tauri/tests/harness_checkpoint.rs` | 新建 | ≤480 | checkpoint/rollback |
| `src-tauri/tests/harness_approval.rs` | 新建 | ≤420 | 审批状态机 |
| `src-tauri/tests/harness_tool_contract.rs` | 新建 | ≤420 | 工具清单和 Harness API 契约 |
| `src-tauri/tests/call_tool_contract.rs` | 修改 | +100 | 统一工具清单契约 |
| `src-tauri/tests/call_tool_security.rs` | 修改 | +160 | 门禁与命令安全 |

---

## 交付前自检

- [ ] 无占位符、TODO 或省略实现。
- [ ] 交付物数量与 Scope-lock 一致或已更新差异说明。
- [ ] 每个文件不超过 500 行。
- [ ] 每条任务回链需求和设计章节。
- [ ] 所有写工具经过任务、baseline、权限和事件门禁。
- [ ] Project State 的 Git/文件部分实时计算。
- [ ] rollback 不覆盖用户或外部修改。
- [ ] 事件日志和命令环境完成敏感信息脱敏。
- [ ] MCP、Actions、桌面 UI 和测试使用同一数据契约。
