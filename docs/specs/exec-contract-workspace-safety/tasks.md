# 任务清单：exec-contract-workspace-safety

## 概述

实现 Exec 结果契约统一和 Workspace 安全回归，严格限定在现有工具链与测试范围内。

## 交付物清单（Scope-lock）

- **预计新建文件数**：0 个生产文件，3 个规格文件已生成。
- **预计修改文件数**：5 个源码/测试文件。
- **预计新增或修改函数数**：约 4 个函数及 8 个测试用例。
- **交付物逐项列举**：
  1. `src-tauri/src/tools/exec.rs` 的统一结果字段。
  2. `src-tauri/src/tools/dispatch.rs` 的 Harness 元数据包装修正。
  3. `src-tauri/tests/call_tool_contract.rs` 的 native/direct 契约测试。
  4. `src-tauri/tests/harness_tool_contract.rs` 的 standalone direct 契约测试。
  5. `src-tauri/tests/call_tool_security.rs` 的 Workspace 安全回归测试。
  6. `src-tauri/src/tools/exec.rs` 的未隔离子进程 fail-closed 闸门与安全返回契约。

## 任务列表

### 阶段 1：准备工作

- [x] 1.1 读取 Exec、Dispatch、Patch、Workspace 和现有测试，确认结果字段与保护逻辑。
  - **证据块**：`src-tauri/src/tools/exec.rs` 当前 native builtin 只有 `elapsed_ms` 且设置 `execution_mode=native_builtin`；`src-tauri/src/tools/dispatch.rs` 当前 standalone 包装会覆盖 `execution_mode=standalone`；`src-tauri/src/tools/patch.rs` 已在删除关键文件时要求确认。
  - **涉及文件**：上述生产文件只读；不改动。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：Exec 结果模型、Workspace 安全模型_

### 阶段 2：核心实现

- [x] 2.1 统一 native builtin 与 direct runner 的 Exec 结果字段，并保留兼容字段。
  - **证据块**：`run_native_diagnostic` 返回 `execution_mode=native_builtin`；`merge_exec_result` 当前只写入 `elapsed_ms`；`dispatch` 会改写执行模式。
  - **涉及文件**：`src-tauri/src/tools/exec.rs`（约 20 行修改）、`src-tauri/src/tools/dispatch.rs`（约 3 行修改）。
  - _需求：FR-1_ ｜ _设计：Exec 结果模型、决策 1、决策 2_

### 阶段 3：集成测试

- [x] 3.1 增加 native 与 direct Exec 契约断言。
  - **证据块**：`src-tauri/tests/call_tool_contract.rs` 已覆盖 `pwd/ls`；`src-tauri/tests/harness_tool_contract.rs` 已覆盖无 Task 的 `git status`。
  - **涉及文件**：`src-tauri/tests/call_tool_contract.rs`、`src-tauri/tests/harness_tool_contract.rs`，约 30 行。
  - _需求：FR-1_ ｜ _设计：测试策略_

- [x] 3.2 增加普通修改、普通删除、关键删除确认和越界路径回归测试。
  - **证据块**：`src-tauri/tests/call_tool_security.rs` 已覆盖 README 删除确认和 traversal patch；`src-tauri/tests/common/mod.rs` 提供 tempfile fixture。
  - **涉及文件**：`src-tauri/tests/call_tool_security.rs`，约 70 行。
  - _需求：FR-2、FR-3_ ｜ _设计：Workspace 安全模型、测试策略_

- [x] 3.3 运行 Rust、Clippy、前端检查和 diff whitespace 检查，确认仅影响预期范围。
  - **证据块**：项目现有验证命令为 `cargo test`、`cargo clippy`、`npm run check` 和 `git diff --check`。
  - **涉及文件**：无新增代码文件。
  - _需求：NFR-1、NFR-2、NFR-3_ ｜ _设计：风险评估_

### 阶段 4：子进程文件系统安全边界

- [x] 4.1 在真实 Windows 文件系统沙箱尚未启用时，保持 Workspace 内 Exec 可用，并在结果中明确未隔离状态；`.git/.github` 删除由 Patch/Policy 永久拒绝。
  - **证据块**：Workspace 子进程可执行并返回 `sandbox_enforced=false`；`host` scope 要求 `confirm=true`；仓库保护资产删除测试通过。
  - **涉及文件**：`src-tauri/src/tools/exec.rs`、`src-tauri/src/tools/policy.rs`、`src-tauri/src/tools/registry.rs`、相关安全测试。
  - _需求：FR-4、NFR-4_ ｜ _设计：Exec 子进程安全闸_
- [ ] 4.2 实现 Windows-rs 原生进程文件系统授权，使 Python、Node、Git、PowerShell 及其子进程继承 Workspace/外部授权范围。
  - **验收重点**：越界读取、越界写入、`.git/.github`、junction、symlink、UNC 和 `\\?\\` 路径均不能绕过授权。
  - _需求：FR-2、FR-4_ ｜ _设计：Exec 子进程安全闸_
- [ ] 4.3 使用 Job Object 管理整个进程树的终止、超时和资源上限；不将 Job Object 单独当作文件系统沙箱。
  - _需求：FR-4、NFR-4_ ｜ _设计：Exec 子进程安全闸_

## 检查点

- [x] 阶段 1 完成：已确认现有契约冲突和安全实现。
- [x] 阶段 2 完成：native 与 direct 均输出统一字段，dispatch 不覆盖执行模式。
- [x] 阶段 3 完成：新增回归测试通过，完整验证命令通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | Exec 结果模型 | 2.1、3.1 | 已完成 |
| FR-2 | Workspace 安全模型 | 3.2 | 已完成 |
| FR-3 | Workspace 安全模型 | 3.2 | 已完成 |
| FR-4 | Exec 子进程安全闸 | 4.1、4.2、4.3 | 4.1 已完成，4.2/4.3 待完成 |
| NFR-1 | 决策 2、风险评估 | 2.1、3.3 | 已完成 |
| NFR-2 | 测试策略 | 3.2、3.3 | 已完成 |
| NFR-3 | 测试策略 | 3.1、3.2、3.3 | 已完成 |
| NFR-4 | Exec 子进程安全闸 | 4.1、4.2、4.3 | 4.1 已完成，4.2/4.3 待完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `src-tauri/src/tools/exec.rs` | 修改 | 20 | 统一 Exec 结果字段 |
| `src-tauri/src/tools/dispatch.rs` | 修改 | 3 | 只附加 Harness 元数据 |
| `src-tauri/tests/call_tool_contract.rs` | 修改 | 20 | native/direct 字段断言 |
| `src-tauri/tests/harness_tool_contract.rs` | 修改 | 15 | standalone 不覆盖执行模式 |
| `src-tauri/tests/call_tool_security.rs` | 修改 | 70 | Workspace 安全回归 |

## 检查清单

- [x] 交付物清单已填。
- [x] 每条任务包含证据块、文件和预算、需求与设计回链。
- [x] 需求覆盖矩阵已填。
- [x] 无占位符、TODO 或省略号占位。
