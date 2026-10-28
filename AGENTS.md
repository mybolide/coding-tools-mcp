<!-- mcp-probe:context begin — auto-generated; re-run init_project_context updates this block only -->
<!-- mcp-probe:context-version: 4.1.1 -->
## MCP（必须先调）
需已配置 mcp-probe-kit。写代码前先读 Skill：@.agents/skills/mcp-probe-kit/SKILL.md（或 [MCP 调用时机](.agents/skills/mcp-probe-kit/SKILL.md)）（首次 MCP 调用自动创建 Skill 文件）。

- 用户只说“继续 / 开始 / 往下做” → **先调用 `resume_plan`**；已知 `plan_id` 则传入，未知则只传 `project_root` 自动恢复最近的 active/blocked Plan；未确认无可恢复 Plan 前，禁止先用 Bash 探索、调用 `workflow` 或重新调用 `start_*`；恢复成功且 `mustContinue=true` 后禁止只汇报“已恢复”，必须立即执行 `nextStep/nextTool`，每步后调用 `plan_heartbeat`，直到阻断、取消或收敛
- 不确定用哪个 MCP → `workflow`（只返回工具选择指南；由 Agent 自己判断，必要时再澄清）
- 当前会话看不到 MCP 工具 → 读取 Skill 的“执行通道与自动降级”，通过 `.mcp-probe-kit/bin/probe.*` 调用同版本 CLI；不要要求用户安装
- 新功能 → `start_feature`（会先搜记忆）
- Bug → `start_bugfix`（会先搜记忆）
- UI → `start_ui`（会先搜记忆）
- 不熟代码 / 影响面 → `code_insight`（context / impact / auto）
- 缺上下文 → `init_project_context`
- 提交 → `gencommit`

上下文：写代码前先读 [project-context](./docs/project-context.md)（链到 `docs/project-context/` 各文档）
图谱：大改前读 [latest](./docs/graph-insights/latest.md)；过期 `code_insight` mode=auto save_to_docs=true
会话历史（默认开启；`MCP_HISTORY_SESSION=0|false|off` 关闭；详见 Skill「History Session 读写规则」）：
- **写**：新对话先 `history_session_bootstrap`（无宿主 key 时服务器铸 `hs_<uuid>` 开新档，禁止写入共享 `project-active`）；每轮最终回复前 `history_session_checkpoint`（必须回传上次 `session_key`/`expected_path` + `raw_user_input`/`user_intent`）
- **检查点最低清单**：`findings`/`decisions`/`files_changed`/`tests`/`runtime_state`/`next_actions` 须写满（无内容用明确占位句，禁止空数组交差）
- **读**：先用 bootstrap 有界 state；精确旧上下文再 `history_session_search` → `history_session_read`；不要全量灌历史
- 档案在 `docs/history-session/`（应 `.gitignore`，勿提交）；超行数/字节软轮转；与 Memory/Qdrant 分离；“继续”有 Plan → 仍先 `resume_plan`
记忆（需 MEMORY_QDRANT_URL 等已配置）：
- 检索：`start_*` 命中后**自动注入**历史经验全文；中途补查可用 `search_memory`；单条精读仍可用 `read_memory_asset`
- 沉淀：跨仓库共享**勿填** source_project/source_path；路径写进 content；summary 写检索关键词
- 修正：已有资产可用 `update_memory_asset` 按 asset_id 原地更新（保留 ID）
- 清理：过时/错误/重复沉淀可用 `delete_memory_asset`（删除前建议 `read_memory_asset` 确认）
- Bug 每轮验证后先准备成功/失败/证伪/回归候选并写入 `plan_heartbeat`；`converge` 通过后再 `memorize_asset`
- 功能/UI 验证后先准备候选并写入 `plan_heartbeat`；`converge` 通过后再 `memorize_asset` type=`pattern`/`component`
<!-- mcp-probe:context end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **coding-tools-mcp** (3598 symbols, 7608 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/coding-tools-mcp/context` | Codebase overview, check index freshness |
| `gitnexus://repo/coding-tools-mcp/clusters` | All functional areas |
| `gitnexus://repo/coding-tools-mcp/processes` | All execution flows |
| `gitnexus://repo/coding-tools-mcp/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
