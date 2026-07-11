# Coding Tools MCP Spec

This repository implements the `coding-tools-mcp-v0.1` profile defined in [docs/profile-v0.1.md](docs/profile-v0.1.md).

## Scope

The server exposes coding runtime primitives over MCP. It is intentionally lower-level than a product agent wrapper. Clients can inspect, edit, test, and review a workspace, but cannot ask this server to route prompts to an external coding agent, manage accounts, search the web, spawn subagents, or operate cloud tasks.

## Protocol

- MCP profile target: `2025-06-18`
- Latest upstream MCP specification checked during contract review: `2025-11-25`
- P0 transport: Streamable HTTP at `/mcp`
- stdio transport: newline-delimited JSON-RPC via `--stdio`
- Initialize response advertises tools and logging only.
- `tools/list` returns a stable default P0 tool set.
- `tools/call` returns MCP `content`, `structuredContent`, and `isError`.

The implementation is a hand-rolled JSON-RPC MCP server rather than a Python MCP SDK
server. Its wire shape should remain compatible with the Python SDK `Tool` and
`CallToolResult` models: tools expose `name`, `title`, `description`,
`inputSchema`, `outputSchema`, and `annotations`; tool results expose `content`,
optional `structuredContent`, and optional `isError`.

## Workspace Model

Startup selects one workspace root:

```bash
coding-tools-mcp --workspace /path/to/repo
```

All path inputs are workspace-relative. The server rejects absolute paths by default, rejects `..`, canonicalizes existing paths, canonicalizes the nearest existing parent for writes, and rejects symlink escapes.

## Tool Set

Default P0 tools:

- `read_file`: read UTF-8 text slices with line and byte limits.
- `list_dir`: list directory entries with default exclusions.
- `list_files`: list files by glob with result caps.
- `search_text`: literal or regex text search with context and truncation.
- `apply_patch`: patch envelope for add, update, delete, and move.
- `exec_command`: run bounded workspace commands and return final output or session id.
- `write_stdin`: write to server-managed running sessions.
- `kill_session`: terminate server-managed sessions.
- `git_status`: structured git working tree status.
- `git_diff`: bounded unified diff with path filters.
- `request_permissions`: structured non-granting permission path when elicitation is unavailable.

P1:

- `view_image`: image data output. The current implementation enables it by default and can disable it with `CODING_TOOLS_MCP_ENABLE_VIEW_IMAGE=0`.

## Forbidden Capabilities

The implementation must not expose:

- External agent login, account, token, or keyring management.
- External agent memory or personalization.
- External agent cloud tasks or remote queues.
- Web search or arbitrary network fetch as a direct tool.
- Image generation.
- Subagent orchestration.
- Model routing or paid account selection.
- Plugin marketplace or connector installation.
- High-level prompt wrapper tools.

## Error Model

Tool execution failures return `isError: true` and structured content:

```json
{
  "ok": false,
  "error": {
    "code": "PATH_OUTSIDE_WORKSPACE",
    "message": "Path escapes the configured workspace.",
    "category": "security",
    "retryable": false,
    "details": {}
  }
}
```

Unknown JSON-RPC methods and malformed protocol-level requests use JSON-RPC errors.

## Contract Status

Contract verification on 2026-05-16 covers the current implementation with
compliance tests for:

- `initialize`, repeated/fresh-client `tools/list`, and all required tools.
- Tool `inputSchema`, permissive `outputSchema`, annotations, structured
  success/error results, unknown-tool errors, and text mirrors of
  `structuredContent`.
- Streamable HTTP and stdio happy paths, including clean stdout for stdio and
  rejection of unsupported `MCP-Protocol-Version` headers on HTTP.
- Central argument validation against each advertised `inputSchema`.
- `view_image` returning both structured image metadata/data URL and MCP image
  content when `output: "mcp_image"`.

Known protocol limitation: `request_permissions` uses a structured
`ELICITATION_UNSUPPORTED` fallback because MCP elicitation support varies across
clients and is not implemented in this server yet. It never silently grants
dangerous permissions.

## Implementation Notes

- Runtime implementation: [coding_tools_mcp/server.py](coding_tools_mcp/server.py)
- Test profile: [tests/compliance](tests/compliance)
- Current profile document: [docs/profile-v0.1.md](docs/profile-v0.1.md)
