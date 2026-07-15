# CT-003 / CT-004 协议修复验收

日期：2026-07-15
基线文档：[ct-baseline-2026-07-15.md](./ct-baseline-2026-07-15.md)

## 修复范围

本轮只处理两个确定性协议问题，没有改变 Workspace 边界模型、Task 生命周期或 Snapshot 策略：

- CT-003：统一 `exec_command`、session、native builtin 的执行结果语义；
- CT-004：standalone 和 Harness 错误路径不再推荐当前 `tools/list` 未暴露的工具名。

## CT-003 结果协议

执行层现在明确区分三种状态：

```json
{
  "ok": true,
  "transport_ok": true,
  "command_ok": false,
  "status": "exited",
  "exit_code": 1,
  "stdout": "",
  "stderr": "..."
}
```

- `ok`：MCP 工具调用和结果封装成功；
- `transport_ok`：执行请求已进入并完成执行层处理；
- `command_ok`：实际命令是否成功；
- 正常退出且 `exit_code=0` 时为 `true`；
- 正常退出且非零退出码时为 `false`；
- 进程仍运行时为 `null`；
- timeout、killed、crashed、spawn failure 时为 `false`。

覆盖的路径：

- native `pwd` / `ls`：成功结果包含 `transport_ok=true`、`command_ok=true`；
- direct 命令成功：三项语义一致；
- direct 命令非零退出：`ok=true`、`transport_ok=true`、`command_ok=false`；
- timeout：保留 session 输出，`command_ok=false`；
- killed：保留终止原因，`command_ok=false`；
- 程序不存在、启动失败：转换为统一的执行失败结果，而不是丢失执行上下文。

## CT-004 next_actions

- standalone 成功路径返回 `next_actions: []`，恢复说明放在普通文本字段 `recovery_hint`；
- standalone 失败路径同样返回空 `next_actions`，不再出现 `retry`、`inspect_error` 等伪工具名；
- Harness 状态中的 `next_actions` 会按当前 profile 的注册表过滤；
- core profile 不会再返回 `harness_status`、`operation_log`、`start_task` 等未暴露工具；
- `exec_health_check` 中的诊断动作仍是普通文本提示，不作为 MCP 工具调用建议。

## 验证结果

执行命令：

```text
rtk cargo check --manifest-path src-tauri/Cargo.toml
rtk cargo test --manifest-path src-tauri/Cargo.toml
rtk npm run check
```

结果：

- Rust 编译通过；
- Rust 测试 **90 个全部通过**；
- 前端检查 0 errors、0 warnings。

新增/强化回归测试包括：

- native/direct 成功结果字段；
- 非零退出码语义；
- timeout；
- killed session；
- 程序不存在和启动失败的统一失败结果；
- standalone 成功/失败不推荐未暴露工具。

## 未包含在本轮

- Windows-rs 真实文件系统边界验证仍属于 CT-005；
- `.git` / `.github` 的 OS 层保护仍属于 CT-006；
- 暂不增加 shell execution；
- 不重新引入工具级 Snapshot/Rollback；
- `AGENTS.md`、`CLAUDE.md` 的 GitNexus 统计变更仍需单独处理。
