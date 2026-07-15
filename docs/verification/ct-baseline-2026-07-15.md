# CT-001～CT-006 当前行为验收基线

日期：2026-07-15
分支：`main`
基线提交：`c9d4846`
工作区状态：仅 `AGENTS.md`、`CLAUDE.md` 存在 GitNexus 索引统计变更；本基线没有修改业务代码。

## 1. 基线测试

当前 Rust 测试总数为 **86 个**，不是早期计划中的 49 个：单元测试 33 个、`call_tool_contract` 16 个、`call_tool_security` 24 个、`harness_state` 4 个、`harness_tool_contract` 9 个，全部通过。执行命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml`。前端 `rtk npm run check` 为 0 errors、0 warnings。

## 2. 当前工具清单

注册表共有 36 个高级工具。默认 `core` profile 暴露 20 个：`server_info`、`check_exec_environment`、`get_default_cwd`、`set_default_cwd`、`read_file`、`list_dir`、`list_files`、`search_text`、`apply_patch`、`exec_command`、`write_stdin`、`kill_session`、`read_output`、`git_status`、`git_diff`、`git_log`、`git_show`、`git_blame`、`request_permissions`、`view_image`。

`advanced` profile 暴露全部 36 个，额外包含：`harness_status`、`operation_log`、`project_state`、`start_task`、`update_task`、`pause_task`、`resume_task`、`finish_task`、`task_context`、`list_task_events`、`change_summary`、`patch_check`、`exec_health_check`。

`server_info`、MCP `tools/list`、Actions OpenAPI 都从注册表/profile 生成；代码层集合一致，但尚未启动独立 Actions HTTP 服务做运行时三方抓取比对，发布前需补 live 检查。

## 3. standalone、Task 与 Snapshot

- `exec_command`、普通 `apply_patch`、`dry_run` 无活动 Task 时可以执行；
- standalone 返回 `harness_mode: "standalone"`、`task_required: false`，普通开发不再被 `TASK_STATE_REQUIRED` 阻塞；
- 启动 Task 的测试确认不会创建 Workspace 持久化副本或 `snapshots` 目录；
- `apply_patch` 内部仍有一次操作内的临时备份用于失败恢复，不是工具级 Workspace Snapshot；长期恢复方向是 Git。

证据：`harness::state::tests::starting_task_does_not_create_workspace_copies`、`harness_tool_contract` standalone 断言、`patch.rs` 事务临时备份。

## 4. exec_command 当前返回

`pwd`、`ls`、`dir`、`which`、`echo` 走 native builtin，不创建子进程；典型字段为：`ok=true`、`execution_mode=native_builtin`、`status=exited`、`termination_reason=exited`、`exit_code=0`、`stdout`、`stderr`、`filesystem_scope=workspace`、`sandbox_enforced=false`、`execution_boundary=policy_only`、`child_process=false`。

普通命令通过 `tokio::process::Command` direct execution，不是 shell；典型字段为：`ok=true`、`execution_mode=direct`、`status=exited`、`exit_code=0`、`stdout`、`stderr`、`duration_ms`、`elapsed_ms`、`filesystem_scope=workspace`、`sandbox_enforced=false`、`execution_boundary=policy_only`、`child_process=true`。

当前异常语义尚未统一为 `transport_ok`、`command_ok`：非零退出通常仍返回工具层 `ok=true` 但没有 `command_ok`；程序不存在为 `COMMAND_REJECTED`；启动失败为 `COMMAND_SPAWN_FAILED`；timeout 为 `TIMEOUT` 工具错误；killed/session 使用 `termination_reason`。CT-003 未通过。

## 5. check_exec_environment

当前明确报告：`filesystem_sandbox.available=false`、`filesystem_sandbox.enforced=false`、`workspace_exec_available=true`、`workspace_exec_sandbox_enforced=false`、`workspace_exec_boundary=policy_only`。

结论：当前是策略限制，不是操作系统级 Workspace 文件系统沙箱，不能宣称具备真实边界。

## 6. Workspace 外读写、删除、执行

| 操作 | 当前结果 |
|---|---|
| Workspace 外 `read_file`、`list_dir`、`list_files`、`search_text` | 允许，外部只读测试通过 |
| `workdir` 指向 Workspace 外的 `exec_command` | policy 拒绝 |
| `filesystem_scope: host` | 拒绝，`EXTERNAL_EXECUTION_NOT_ALLOWED` |
| Patch 写入 Workspace 外 | 拒绝 |
| 解释器命令写入 Workspace 外 | policy 拒绝，但依赖命令文本识别 |
| 子进程/孙进程实际写入或删除 Workspace 外 | 未证明被 OS 阻止，`sandbox_enforced=false` |

CT-005 部分通过：入口策略有保护，真实 Windows 子进程边界尚未成立。

## 7. `.git`、`.github` 保护

当前 policy/工具层测试通过：Patch 删除 `.git` 始终拒绝；Patch 写入 `.git`、`.github` 拒绝；解释器删除 `.git`、`.github` 拒绝；解释器写入 `.git` 拒绝；普通 Workspace 文件修改和删除允许。上述不等价于 Windows ACL，CT-006 为策略层通过、OS 层待验证。

## 8. next_actions 当前状态

standalone 结果固定加入 `next_actions: ["retry", "inspect_error"]`，这两个值不是当前注册表中的 MCP 工具名。`exec_health_check` 失败路径还会返回“检查 exec worker 日志”“重启运行时”等普通文本动作；Harness 错误路径仍可能产生 `start_task`、`resume_task`、`harness_status`。成功/失败路径没有统一的“只推荐已暴露工具”校验，CT-004 未通过。

## 9. CT-001～CT-006 矩阵

| 编号 | 验收范围 | 当前结果 | 下一步 |
|---|---|---|---|
| CT-001 | 86 个现有测试、工具清单、profile 一致性 | 通过；live Actions 比对待补 | 发布前补 HTTP 抓取 |
| CT-002 | standalone 不依赖 Task，且不产生持久 Snapshot | 通过 | 保持 |
| CT-003 | exec 返回 `ok/transport_ok/command_ok` 统一语义 | 未通过 | 先修协议和测试 |
| CT-004 | `next_actions` 不引用未暴露工具 | 未通过 | 动态生成并校验 |
| CT-005 | Workspace 外读/写/删/执行边界 | 部分通过 | Windows-rs 原型验证 |
| CT-006 | `.git`、`.github` 保护 | 策略层通过，OS 层待验证 | 子进程/孙进程集成测试 |

## 10. 当前结论

第一阶段已冻结：86 个测试全通过；默认 profile 20 工具、advanced 36 工具；standalone 已解除 Task 门禁；没有生成持久 Workspace Snapshot，Patch 只使用一次操作内临时备份；exec 仍是 direct execution + `policy_only`，不是真实文件系统沙箱；最大确定性问题是 CT-003、CT-004，最大安全风险是 CT-005 的子进程/孙进程真实写入边界尚未验证。

下一步按原计划进入 CT-003、CT-004，不先增加 shell 模式，也不重新引入工具级 Snapshot/Rollback。
