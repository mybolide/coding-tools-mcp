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

Context: read [project-context](./docs/project-context.md) after Agent creates it (`init_project_context` + templates)
Graph: read [latest](./docs/graph-insights/latest.md) after Agent saves code_insight output (save_to_docs=true)
Memory (requires MEMORY_* env):
- Search: `start_*` auto-injects full memory hits; use `search_memory` mid-task; `read_memory_asset` for a specific id
- Store: do NOT use source_project/source_path for cross-repo pools; put paths in content; write keyword-rich summary
- Update: fix existing entries in place with `update_memory_asset` by asset_id (preserves ID)
- Cleanup: remove stale/wrong/duplicate entries with `delete_memory_asset` (confirm via `read_memory_asset` first)
- After verified bugfix → MUST `memorize_asset` type=`bugfix` (sections: symptom, root cause, fix, verification)
- Reusable feature/UI → `memorize_asset` type=`pattern`/`component`
<!-- mcp-probe:context end -->
