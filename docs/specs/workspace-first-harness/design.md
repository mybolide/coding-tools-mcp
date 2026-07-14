# 设计文档：workspace-first-harness

## 概述

保留现有 Harness 存储和 Task API，但将普通工具执行的生命周期从 Task 中解耦。`call_tool` 以 Workspace 为上下文，按 Capability 和操作风险决定执行；Task 存在时提供额外基线校验，Task 不存在时进入 standalone 模式。

**对应需求:** FR-1, FR-2, FR-3, FR-4, NFR-1 至 NFR-4。

## 技术方案

| 能力 | 实现 | 原则 |
|------|------|------|
| Workspace 状态 | 复用 `Harness` 和 `harness_status` | Workspace 必须独立于 Task 可用 |
| Operation Log | `HarnessStore` 下独立 JSONL | 以 operation_id 查询，不依赖 Task |
| Snapshot | 复用现有 SnapshotManifest 和事务写入 | 变更前尽力创建，失败可降级 |
| Capability | `dispatch` + Workspace/Policy 检查 | 普通开发默认允许，危险操作单独确认 |
| 兼容层 | 保留 Task API 和旧事件文件 | 不破坏旧客户端和历史记录 |

## 架构设计

```text
Conversation / MCP / Actions
            ↓
    tools::dispatch::call_tool
            ↓
 Workspace Context + Capability
   ├─ boundary / policy check
   ├─ pre-change snapshot
   ├─ operation log
   ├─ transactional patch / exec / git
   └─ optional Task metadata and baseline
            ↓
      HarnessStore / Workspace
```

Workspace 是唯一必需上下文。Task 只在存在时参与 baseline 检查和 task 事件，不存在时不生成伪 Task。

## 数据模型

新增 `OperationRecord`，持久化到 `workspaces/<workspace_id>/operations.jsonl`：

| 字段 | 类型 | 说明 |
|------|------|------|
| `operation_id` | string | 全局唯一操作 ID |
| `tool` | string | 调用工具 |
| `kind` | string | started/completed/failed |
| `task_id` | optional string | 兼容 Task 的关联信息 |
| `reason` | optional string | 操作原因 |
| `result` | JSON | 结果摘要、退出码或错误 |
| `affected_files` | array | 文件及 before/after hash |
| `snapshot_id` | optional string | 变更前恢复点 |
| `created_at` | string | 时间戳 |

## API 设计

| 工具 | 入参 | 出参 |
|------|------|------|
| `harness_status` | 无 | Workspace、Capability、standalone、Task 摘要 |
| `operation_log` | cursor、limit | Workspace 级 Operation Record 列表 |
| `workspace_snapshot` | reason | snapshot_id、文件数、Workspace 基线 |
| `workspace_rollback` | snapshot_id、confirm | 冲突清单或恢复结果 |
| `apply_patch` | 现有 patch、dry_run、confirm | 现有字段 + snapshot_id、operation_id |
| `exec_command` | 现有命令、confirm | 现有字段 + snapshot_id、operation_id |

## 执行流程

```text
调用工具
  ↓
路径/策略/危险操作检查
  ↓
变更操作？→ 自动 Snapshot（失败则 warning）
  ↓
执行事务或进程
  ↓
写 Workspace Operation Log
  ↓
返回结果、operation_id、snapshot_id、恢复动作
```

## 关键保护

- 所有路径继续由 `Workspace` 解析，禁止绝对路径、路径穿越和 symlink escape。
- 普通修改 `.gitignore`、README、LICENSE 和构建配置允许执行。
- 删除核心资产和危险命令必须有 `confirm: true`；没有确认时返回 `DANGEROUS_OPERATION_REQUIRES_CONFIRMATION`。
- Task 基线冲突只影响可能覆盖文件的操作，不影响读、Git 查询、状态和 Operation Log。

## 向后兼容

- 保留 `start_task`、`task_context`、`change_summary` 等工具。
- 保留现有 Task JSON 和 task event 文件。
- 新 Operation Log 使用独立文件和新工具，不改变旧事件读取格式。
- 没有 Task 的成功操作标记 `harness_mode=standalone`。

## 文件结构

```text
src-tauri/src/harness/
├── model.rs       # OperationRecord 与现有状态模型
├── state.rs       # Workspace 状态、快照和操作入口
├── store.rs       # Task、Snapshot、Operation Log 持久化
└── tools.rs       # harness_status、operation_log、快照工具
src-tauri/src/tools/
├── dispatch.rs    # Capability、自动快照和 Operation Log 编排
├── policy.rs      # 命令 allowlist 与危险命令确认
├── workspace.rs   # Workspace 边界和关键路径判断
├── patch.rs       # 原子 Patch、Undo 与关键删除保护
└── registry.rs    # 工具及参数 schema
```

## 测试策略

- 无 Task 的 read/write/exec/Git 全部可用。
- Operation Log 在无 Task、Task 存在、进程重启后均可读取。
- Patch/Undo 自动 Snapshot；Exec 仅在 `snapshot=true` 或高风险命令时创建 Snapshot；Snapshot 失败时操作按降级策略执行。
- 工作区外路径、关键文件删除、危险命令和普通关键文件修改分别验证。
- 保持 Rust 全量测试、Clippy 和前端检查通过。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 写入型 Exec 自动快照消耗磁盘 | 中 | 普通查询不快照；明确写入或高风险 Exec 才创建，配合 hash 去重和清理入口 |
| Operation Log 过大 | 中 | JSONL 分页、限制结果摘要和输出引用 |
| 危险命令规则误判 | 中 | 只拦高置信破坏模式，普通命令继续走现有 allowlist |
| 旧 Task 事件兼容 | 低 | 新旧日志分开存储，保留旧 API |

## 检查清单

- [x] Workspace-first 数据流已定义。
- [x] Operation Log、Snapshot、Capability 和安全边界已定义。
- [x] 旧 Task API 的兼容策略已定义。
