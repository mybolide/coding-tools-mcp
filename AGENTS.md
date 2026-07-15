<!-- mcp-probe:context begin — auto-generated; re-run init_project_context updates this block only -->
<!-- mcp-probe:context-version: 3.6.11 -->
## MCP (call first)
Requires mcp-probe-kit. Before coding, read Skill: @.agents/skills/mcp-probe-kit/SKILL.md (or [When to call MCP](.agents/skills/mcp-probe-kit/SKILL.md)) (Skill file auto-created on first MCP call).

- Unsure which MCP → `workflow` (returns firstTool)
- Feature → `start_feature` (searches memory first)
- Bug → `start_bugfix` (searches memory first)
- UI → `start_ui` (searches memory first)
- Unfamiliar code / impact → `code_insight` (context / impact / auto)
- Missing context → `init_project_context`
- Commit → `gencommit`

Context: before coding read [project-context](./docs/project-context.md) (links to `docs/project-context/`)
Graph: read [latest](./docs/graph-insights/latest.md) before large changes; refresh `code_insight` mode=auto save_to_docs=true
Memory (requires MEMORY_* env):
- Search: `start_*` auto-injects full memory hits; use `search_memory` mid-task; `read_memory_asset` for a specific id
- Store: do NOT use source_project/source_path for cross-repo pools; put paths in content; write keyword-rich summary
- Update: fix existing entries in place with `update_memory_asset` by asset_id (preserves ID)
- Cleanup: remove stale/wrong/duplicate entries with `delete_memory_asset` (confirm via `read_memory_asset` first)
- After verified bugfix → MUST `memorize_asset` type=`bugfix` (sections: symptom, root cause, fix, verification)
- Reusable feature/UI → `memorize_asset` type=`pattern`/`component`
<!-- mcp-probe:context end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **coding-tools-mcp** (2497 symbols, 5234 relationships, 210 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

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
