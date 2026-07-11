---
name: goal
description: >-
  Drives Coding Tools MCP desktop development to completion against old/ parity.
  Use when the user invokes /goal, says 开始开发直到完成, or asks to finish the
  rust-desktop-client spec with shared MCP/Actions tool core aligned to old/.
disable-model-invocation: true
---

# /goal — 开发直到完成

开始开发吧，直到完成。

## 架构约束

- **工具共用底层**：MCP 与 Actions 必须且只能调用 `tools::call_tool`（一点不能差）；策略校验在 `call_tool` 内统一完成。
- **Actions 暴露层**：`validate_actions_exposure` 仅判断工具是否在 OpenAPI 白名单，不得重复做 exec/patch 参数校验。
- **旧版对齐**：工具名、schema、ALLOWED_TOOLS、行为以 `old/coding_tools_mcp/server.py` 与 `old/coding_tools_actions/policies.py` 为准。
- **exec_command**：禁止默认「整条命令丢进 bash/sh」；优先 **argv 直启**（`Command::new(exe).args(...)`），扩大白名单（pytest/python/cargo/npm/go/msbuild/dotnet/gradle…），与旧版 allowlist 一致并覆盖 Windows 可执行名。
- **单文件 <500 行**；用 `platform` trait，禁止 PowerShell。
- **双端口**：MCP 与 Actions 默认 28766，用户可改；占用时弹窗提醒。

## 完成定义（按 tasks.md）

**优先级：底层逻辑 > 能跑通 > 测试（可选）**

1. **阶段 2（核心）**：`tools/` 与旧版行为对齐；MCP/Actions 共用；P0 工具逻辑完整（exec 直启、session、git 全家桶、patch、文件工具）。
2. **阶段 3**：隧道 FRP + Cloudflare 监督；健康检查 MVP；与 runtime 集成。
3. **阶段 4**：隧道/认证配置 UI（非旧版布局）。
4. **验收**：`cargo build` + `npm run check` 通过即可手工验证主路径。

**测试（低优先级，用户不要求则不做）**：不主动移植 `old/tests/compliance/`；仅在为修 bug 时补最小单测。`cargo test` 有则用，无则不强求全绿。

## 执行流程

1. 读 `docs/specs/rust-desktop-client/tasks.md` 找下一项 `[ ]`。
2. 读 `old/` 对应证据块，再改 Rust。
3. 每完成一项：`cargo build`（必须）；测试仅在有回归风险或用户要求时跑。
4. 未 `cargo build` 绿不停止；阻塞则记到 `docs/specs/rust-desktop-client/blockers.md` 并继续其他项。
5. 多 agent 时按目录分工：`tools/`、`mcp/`、`actions/`、`tunnel/`、`src/` UI，集成冲突由主 agent 合并。

## 参考路径

| 主题 | 旧版 |
|------|------|
| 工具注册表 | `old/coding_tools_mcp/server.py` TOOL_REGISTRY |
| Actions 白名单 | `old/coding_tools_actions/policies.py` |
| exec 策略 | `old/coding_tools_mcp/server.py` exec_command / _check_command_policy |
| 合规测试 | `old/tests/compliance/` |
