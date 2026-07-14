# 设计文档：exec-contract-workspace-safety

## 概述

本设计覆盖 FR-1 至 FR-4 及 NFR-1 至 NFR-4。实现保持当前 Workspace-first Harness、direct child-process Exec、事务化 Patch 和关键文件保护，不引入新的生命周期门禁。

## 技术方案

### Exec 结果模型

所有成功的 `exec_command` 结果在返回前补齐以下字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `command` | string | 用户传入的完整命令文本 |
| `execution_mode` | string | `native_builtin` 或 `direct` |
| `exit_code` | integer | 子进程退出码；native builtin 为 0 |
| `stdout` | string | 标准输出 |
| `stderr` | string | 标准错误 |
| `duration_ms` | integer | 本次执行耗时 |
| `elapsed_ms` | integer | 兼容旧客户端的同值字段 |
| `status` | string | 当前成功结果为 `exited` |

`dispatch` 只负责附加 `harness_mode` 和 `task_required`，不改写已有 `execution_mode`。session runner 在 `merge_exec_result` 中设置 `execution_mode=direct`；native builtin 直接设置 `execution_mode=native_builtin`。

### Workspace 安全模型

Patch 继续先完成解析、路径解析、符号链接检查和 staged 内容准备，随后一次性提交变更。测试通过 `call_tool` 验证：普通文件可改可删；关键资产删除由现有 `confirm` 门禁保护；越界路径由 Workspace 解析层拒绝。

### Exec 子进程安全闸

当前构建尚未提供 Windows 原生文件系统沙箱。默认 `filesystem_scope=workspace` 允许 Workspace 内正常开发，Python、Node、Git、PowerShell 等子进程可以运行，但必须明确 `sandbox_enforced=false`。`.git` 和 `.github` 的删除或递归清空由 Patch/Policy 两层永久拒绝。原生诊断命令在进程内完成，不经过该闸门。

`filesystem_scope=host` 仅在调用方显式传入 `confirm=true` 时允许，并返回 `sandbox_enforced=false`。这只是受确认保护的兼容调试模式，不代表 Workspace 隔离已经完成。

后续 Windows-rs 实现应以进程令牌、AppContainer 或等效的 Windows 文件系统授权机制约束整个进程树；Job Object 只负责生命周期和资源控制，不能单独作为文件系统沙箱。

## 数据流

```text
exec_command
  ├─ native diagnostic ──> normalize result(native_builtin)
  └─ session runner ─────> normalize result(direct)
             ↓
       dispatch metadata
       (harness_mode only)

apply_patch
  ├─ parse all hunks
  ├─ resolve and validate every path
  ├─ reject unsafe/critical deletion
  └─ commit staged files atomically

exec_command (child process)
  ├─ workspace scope ────────────────────────> allowed, explicitly unenforced transition mode
  ├─ host scope + confirm=true ───────────────> explicitly unsandboxed process
  └─ native builtin ──────────────────────────> no child process required
```

## 文件结构

- `src-tauri/src/tools/exec.rs`：统一 Exec 字段和执行模式。
- `src-tauri/src/tools/dispatch.rs`：移除对底层 `execution_mode` 的覆盖。
- `src-tauri/src/tools/policy.rs`：校验 Exec 文件系统 scope 与 host 确认。
- `src-tauri/tests/call_tool_contract.rs`：增加 native 结果契约断言。
- `src-tauri/tests/harness_tool_contract.rs`：增加 standalone session 结果契约断言。
- `src-tauri/tests/call_tool_security.rs`：增加普通修改/删除、关键删除和越界路径回归测试。
- `src-tauri/tests/common/mod.rs`：复用现有 fixture，不改变生产代码边界。

## 设计决策

### 决策 1：区分 Harness 模式和执行模式

**问题：** `dispatch` 当前把 `execution_mode` 统一写成 `standalone`，导致调用方无法知道命令是否走 native builtin。

**选项：**

1. 继续复用 `execution_mode` 表示 Harness 状态。
2. 使用 `harness_mode` 表示 Harness 状态，让 `execution_mode` 表示真实后端。

**决策：** 选择选项 2。

**理由：** 两个字段语义正交，兼容已有 standalone 元数据并恢复真实诊断能力。

### 决策 2：新增 duration_ms，保留 elapsed_ms

**问题：** 新客户端需要统一命名，旧客户端仍读取 `elapsed_ms`。

**决策：** 新增 `duration_ms`，同时写入同值 `elapsed_ms`，避免协议破坏。

### 决策 3：在真实沙箱完成前对 Workspace 子进程 fail-closed

**问题：** 文件工具的 Workspace 边界无法限制由 Python、Node 或 PowerShell 启动的子进程。

**决策：** 默认 `workspace` scope 不允许启动未隔离子进程；仅原生诊断命令继续可用，`host` scope 需要显式确认。

**理由：** “允许执行但没有实际文件系统边界”会给 Agent 造成错误安全感。短期降低部分 Exec 能力，比静默允许跨项目读写更可控。

## 测试策略

- Exec 契约：native `pwd/ls` 和 direct `python --version` 检查字段集合、模式、退出码和两个时长字段。
- 修改：对普通文件应用 unified patch，断言磁盘内容变化。
- 删除：删除普通文件，断言文件不存在。
- 关键保护：删除 README 未确认失败，错误码为 `DANGEROUS_OPERATION_REQUIRES_CONFIRMATION`。
- 越界：使用 `../outside-secret.txt` 的 patch，断言安全类错误。
- 验证命令：Rust 全量测试、Clippy、前端检查和 diff whitespace 检查。
- 子进程安全闸：默认 workspace scope 返回 `EXEC_SANDBOX_UNAVAILABLE`；host scope 无确认时拒绝，确认后明确 `sandbox_enforced=false`。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 旧客户端只读取 `elapsed_ms` | 中 | 保留旧字段并保持同值 |
| dispatch 仍覆盖执行模式 | 高 | 增加 standalone native 与 direct 两类断言 |
| 安全测试依赖平台路径 | 中 | 使用现有 tempfile fixture 和相对路径；符号链接测试保留平台条件 |
| 测试修改触碰用户未提交改动 | 高 | 仅新增测试和局部契约字段，不重置或覆盖其他改动 |
