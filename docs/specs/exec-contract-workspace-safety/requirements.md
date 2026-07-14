# 需求文档：exec-contract-workspace-safety

## 功能概述

为 AI Coding Agent 提供稳定、可判别的命令执行结果，并用回归测试锁定 Workspace 的安全边界。调用方无需依赖具体执行后端，即可区分 Harness 运行模式、命令执行模式、成功退出、失败退出和可恢复错误；同时允许工作区内的普通文件修改与删除，拒绝越界路径、符号链接逃逸和未经确认的关键项目资产删除。

## 历史经验与坑

- **可复用经验**：Workspace 是长期核心实体，Task 仅作为可选元数据；普通 Exec 不应自动创建全量 Snapshot，写入或高风险操作才触发快照。
- **必须规避的坑**：Exec 曾因每次命令前全量 Workspace Snapshot 导致 `git status` 超时；本次只调整结果契约和测试，不改变已验证的 Snapshot 触发策略。
- **必须规避的坑**：standalone 的 Harness 状态不能覆盖具体执行方式，否则 native builtin 会被误报为 standalone runner。
- **必须规避的坑**：没有真实的子进程文件系统隔离时，不能把 `workspace` scope 当作已安全执行；必须 fail-closed 并明确返回不可用原因。

## 术语定义

- **harness_mode**：工具是否依赖活动 Task；本功能的无 Task 调用值为 `standalone`。
- **execution_mode**：命令实际采用的执行后端；native 诊断值为 `native_builtin`，子进程直执行值为 `direct`。
- **关键资产**：`.git`、`.github`、锁文件、构建配置、README、LICENSE 等删除后会破坏项目身份或构建能力的文件/目录。

## 范围边界

**In Scope**

- 统一 native builtin 与 session runner 的成功结果字段。
- 保留 `elapsed_ms`，新增规范字段 `duration_ms`。
- 保留 standalone 的 Harness 元数据，但不覆盖 `execution_mode`。
- 覆盖普通文件修改、普通文件删除、关键资产删除确认、越界路径和符号链接逃逸测试。

**Out of Scope**

- 本次不新增 Shell 模式。
- 本次不改变命令白名单、Snapshot 策略、Task 生命周期或 FRP 进程管理。

## 需求列表

### FR-1：统一 Exec 成功结果契约

**优先级：** Must

**用户故事：** 作为 Coding Agent，我想从所有 Exec 后端收到一致字段，以便可靠判断命令结果和执行方式。

#### 验收标准

1. WHEN native builtin 成功执行 THEN 系统 SHALL 返回 `command`、`execution_mode`、`exit_code`、`stdout`、`stderr`、`duration_ms`、`status`。
2. WHEN session runner 成功执行 THEN 系统 SHALL 返回同一组字段，且 `execution_mode` 为 `direct`。
3. WHEN standalone 调度包装结果 THEN 系统 SHALL 添加 `harness_mode=standalone` 与 `task_required=false`，但 SHALL 保留底层 `execution_mode`。
4. 系统 SHALL 保留 `elapsed_ms` 作为兼容字段，其值与 `duration_ms` 一致。

### FR-2：保护 Workspace 边界

**优先级：** Must

**用户故事：** 作为项目所有者，我想让 Agent 在项目内自由开发，同时阻止其访问或修改项目外文件。

#### 验收标准

1. WHEN 修改工作区内普通文件 THEN 系统 SHALL 成功应用修改。
2. WHEN 删除工作区内普通文件 THEN 系统 SHALL 成功删除文件。
3. WHEN 目标路径为工作区外相对路径或绝对路径 THEN 系统 SHALL 拒绝操作并返回结构化安全错误。
4. WHEN 目标通过符号链接逃逸到工作区外 THEN 系统 SHALL 拒绝读写操作。

### FR-3：保护关键项目资产

**优先级：** Must

**用户故事：** 作为项目所有者，我想防止误删 Git 历史和构建入口，同时保留明确确认后的管理能力。

#### 验收标准

1. WHEN 删除关键项目资产且未提供 `confirm=true` THEN 系统 SHALL 拒绝操作并返回 `DANGEROUS_OPERATION_REQUIRES_CONFIRMATION`。
2. WHEN 删除普通文件且未提供确认 THEN 系统 SHALL 成功执行。
3. WHEN 删除关键项目资产且提供 `confirm=true` THEN 现有删除逻辑 SHALL 继续执行。

### FR-4：明确 Exec 子进程文件系统范围

**优先级：** Must

**用户故事：** 作为项目所有者，我想知道 Exec 子进程是否真正受到 Workspace 文件系统边界保护，避免 Python、Node 或 Git 绕过文件工具的路径限制。

#### 验收标准

1. WHEN `filesystem_scope` 未指定 THEN 系统 SHALL 使用 `workspace`，当前 Workspace 内的 Exec 不得因为 Task 或普通 permission mode 被关闭。
2. WHEN Workspace 子进程尚未受到 OS 级沙箱保护 THEN 系统 SHALL 在环境和执行结果中明确 `sandbox_enforced=false`，不得伪装成已完成隔离。
3. WHEN 请求 `host` scope THEN 系统 SHALL 同时要求 `confirm=true`，并明确该进程未受到 Workspace 文件系统隔离。
4. `.git` 和 `.github` 的删除或递归清空 SHALL 永远拒绝；普通 Workspace 文件仍保持默认开发权限。
5. 原生诊断命令不得因为上述闸门失去可用性；不需要启动子进程的命令仍可执行。

## 非功能需求

- **NFR-1（兼容性）**：不得移除已有 `elapsed_ms` 字段或改变既有错误码。
- **NFR-2（安全）**：安全测试必须在 Windows 和 Unix 路径语义下保持可判别；符号链接不可绕过 Workspace 边界。
- **NFR-3（可维护性）**：测试应通过公共 `call_tool` 入口验证行为，不依赖私有实现细节。
- **NFR-4（安全透明性）**：任何未隔离的子进程执行都必须通过显式 scope 和确认触发，并在响应中报告实际隔离状态。

## 依赖关系

- `src-tauri/src/tools/exec.rs` 的 native builtin、session runner 与结果合并逻辑。
- `src-tauri/src/tools/dispatch.rs` 的 standalone 元数据包装。
- `src-tauri/src/tools/patch.rs` 与 `workspace.rs` 的安全边界实现。
- `src-tauri/src/tools/policy.rs` 的 `filesystem_scope` 和确认校验。
- 现有 Rust 集成测试 fixture 与 tempfile。

## 检查清单

- [x] 已消化历史 Exec Snapshot 超时和 Workspace-first 经验。
- [x] 已覆盖成功、兼容字段和安全边界场景。
- [x] 每条需求有唯一 ID，并将在设计与任务中引用。
- [x] 范围边界和不做事项明确。
- [x] 非功能需求和依赖关系明确。
