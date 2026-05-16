# MCP Contract Architect Report

## Task Scope

Verify the current MCP protocol contract for `/root/workspace` against:

- The local task brief and repository docs.
- The current `codex_tool_runtime_mcp/server.py` behavior.
- The compliance harness under `tests/compliance`.
- Current official MCP and Python MCP SDK expectations for tools, structured output, annotations, Streamable HTTP, and stdio JSON-RPC.

No server implementation changes were made. Contract edits were limited to:

- `SPEC.md`
- `reports/subagents/mcp-contract.md`
- `tests/compliance/test_mcp_contract.py`

Observed unrelated working-tree edits in `codex_tool_runtime_mcp/server.py`, `tests/compliance/mcp_client.py`, and generated report files were left untouched.

## Sources Read/Referenced

Local repository files:

- `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`
- `codex_tool_runtime_mcp/server.py`
- `SPEC.md`
- `COMPLIANCE.md`
- `docs/profile-v0.1.md`
- `tests/compliance/test_mcp_contract.py`
- `tests/compliance/mcp_client.py`
- `tests/compliance/test_support.py`
- `tests/compliance/test_tool_golden.py`
- `tests/compliance/test_security.py`
- `tests/compliance/test_e2e.py`
- `tests/compliance/test_codex_compat.py`
- `tests/compliance/test_dogfood.py`

Official upstream references:

- MCP 2025-06-18 tools: https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- MCP 2025-06-18 lifecycle: https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle
- MCP 2025-06-18 transports: https://modelcontextprotocol.io/specification/2025-06-18/basic/transports
- MCP 2025-06-18 schema: https://modelcontextprotocol.io/specification/2025-06-18/schema
- MCP latest checked, 2025-11-25 tools: https://modelcontextprotocol.io/specification/2025-11-25/server/tools
- MCP latest checked, 2025-11-25 lifecycle: https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
- MCP latest checked, 2025-11-25 transports: https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
- MCP latest checked, 2025-11-25 schema: https://modelcontextprotocol.io/specification/2025-11-25/schema
- Official Python SDK README: https://github.com/modelcontextprotocol/python-sdk
- Official Python SDK API reference: https://modelcontextprotocol.github.io/python-sdk/api/

Local Python environment note: the `mcp` package is not installed in this workspace, so SDK verification used official docs and API references rather than importing SDK types locally.

## Tool Surface

The implementation advertises these coding runtime tools:

- `read_file`
- `list_dir`
- `list_files`
- `search_text`
- `apply_patch`
- `exec_command`
- `write_stdin`
- `kill_session`
- `git_status`
- `git_diff`
- `request_permissions`
- `view_image` when image support is enabled

The surface correctly excludes product-layer capabilities such as Codex login, account/keyring management, cloud tasks, memory, web search, image generation, plugin marketplace/connector installation, model routing, and high-level `codex(prompt)` wrappers.

Current working-tree note: an unowned server edit makes `view_image` enabled by default and an unowned test-harness edit adds it to `REQUIRED_TOOLS`. This differs from the older profile language that treated `view_image` as P1/feature-gated.

## Input/Output Schema

`tools/list` entries include the Python SDK-compatible fields:

- `name`
- `title`
- `description`
- `inputSchema`
- `outputSchema`
- `annotations`

Input schemas are JSON Schema object shapes and broadly match the local profile. The implementation does not centrally validate all calls against those schemas. Practical effects:

- Current dirty-tree blocker: `Runtime.call_tool` now calls `validate_arguments(name, args)`, but that function is not defined, causing valid `tools/call` requests to fail with JSON-RPC `-32603`.
- `additionalProperties: false` is advertised but not consistently enforced.
- Numeric min/max constraints are mostly enforced only where handler code casts and checks them.
- Some malformed arguments become structured tool errors rather than JSON-RPC `-32602`.

The most important contract gap is `outputSchema`: every tool currently advertises the same minimal schema with only `ok` and `additionalProperties: false`. Actual `structuredContent` returns tool-specific fields such as `content`, `entries`, `matches`, `stdout`, `stderr`, `diff`, `error`, and `warnings`. MCP 2025-06-18 says that when a tool provides an output schema, structured results must conform to it, and the Python SDK validates structured results against output schemas. The current implementation therefore should either:

- publish real per-tool output schemas that include success and error fields, or
- remove/relax `outputSchema` until the schemas are accurate.

## Error Model

The intended split is correct:

- Protocol errors for malformed JSON-RPC, unknown methods, invalid `tools/call` shape, and unknown tools.
- Tool execution errors as `tools/call` results with `isError: true`, `structuredContent.ok: false`, and structured `error`.

Verified behavior:

- Unknown tool names return JSON-RPC `-32602` with `error.data.reason = "unknown_tool"`.
- Tool execution failures include `content`, `structuredContent`, `isError: true`, `error.code`, `error.message`, `error.category`, `error.retryable`, and `error.details`.
- Text content mirrors JSON serialization of `structuredContent`, matching the MCP structured-output backward-compatibility recommendation.

Gaps:

- `run_stdio` maps parse and invalid-shape failures to generic `-32603` in some cases rather than precise JSON-RPC errors such as `-32700` or `-32600`.
- Stdio `tools/call` does not validate that `arguments` is an object before dispatching.
- `request_permissions` always returns `ELICITATION_UNSUPPORTED` as a tool error and does not yet implement MCP elicitation or the full profile-level `status: "unsupported"` structured response.

## Annotations

The implementation emits the standard MCP `ToolAnnotations` hint keys:

- `title`
- `readOnlyHint`
- `destructiveHint`
- `idempotentHint`
- `openWorldHint`

The annotation posture is consistent with the runtime:

- Read-only/idempotent/closed-world: `read_file`, `list_dir`, `list_files`, `search_text`, `git_status`, `git_diff`, and `view_image`.
- Write/destructive/non-idempotent: `apply_patch`, `exec_command`, and `kill_session`.
- Write/non-destructive/non-idempotent: `write_stdin`.
- Permission workflow hint: `request_permissions` is read-only but non-idempotent.
- `exec_command` is correctly marked `openWorldHint: true`.

Risk reminder: MCP annotations are hints only. They must not be treated as the security boundary.

## Transports

Streamable HTTP:

- Endpoint is `/mcp`.
- POST accepts JSON-RPC requests and notifications.
- Notifications return `202`.
- GET returns `405`, which is allowed when no SSE stream is provided.
- Responses are JSON objects rather than SSE.
- Server binds to loopback by default and validates `Origin` for loopback origins.
- Logs go to stderr; HTTP response bodies are not polluted with debug output.

HTTP gaps:

- The server emits `Mcp-Session-Id`, but does not require it on later requests.
- The server does not reject invalid or unsupported `MCP-Protocol-Version` headers after initialization.
- `tools/list` ignores pagination cursors; this is acceptable for the small fixed list only if documented as unpaginated.

stdio:

- `--stdio` reads newline-delimited JSON-RPC from stdin.
- stdout responses are single-line JSON-RPC messages.
- `notifications/initialized` produces no response.
- stdout remains MCP-message-only on the verified happy path.

stdio gaps:

- Parse errors and non-object JSON request shapes need stricter JSON-RPC error mapping.
- There is no SDK transport adapter; behavior is implemented directly in `run_stdio`/`StdioDispatcher`.

## Risks

- Output schema drift is the highest SDK compatibility risk because Python MCP SDK servers validate structured output against `outputSchema`.
- The current dirty tree has a hard `tools/call` regression from an undefined `validate_arguments` reference in `server.py`.
- The repository now has a contract mismatch around whether `view_image` is P1 optional or a default required tool.
- Hand-rolled schema validation can drift from `inputSchema`, especially for `additionalProperties`, ranges, and type errors.
- HTTP version-header handling is looser than the 2025-06-18 transport spec.
- Elicitation is documented but not implemented.
- `view_image` does not emit MCP image content for `output: "mcp_image"`; it returns text/data URL only.
- Stdio is clean on normal requests, but malformed input behavior is not fully JSON-RPC-compliant.

## Action Items

1. Decide whether `view_image` is P0/default or P1/feature-gated, then align `server.py`, `tests/compliance/mcp_client.py`, `SPEC.md`, `docs/profile-v0.1.md`, and `COMPLIANCE.md`.
2. Define or remove the new `validate_arguments` call so valid `tools/call` requests execute again.
3. Replace the minimal `outputSchema` with accurate per-tool schemas, including success fields and the structured error object, or omit `outputSchema` until schemas are accurate.
4. Add implementation-level argument validation against each tool `inputSchema`; return JSON-RPC `-32602` for malformed `tools/call` params and schema-invalid arguments where the profile requires protocol errors.
5. Tighten Streamable HTTP version handling: reject invalid/unsupported `MCP-Protocol-Version` headers after initialization.
6. Tighten stdio error handling for parse errors, non-object JSON messages, and non-object `tools/call.arguments`.
7. Implement or explicitly downgrade `request_permissions` elicitation behavior in the profile.
8. Emit MCP image content for `view_image` when `output: "mcp_image"` is requested, or update the profile to document data-URL-only behavior.
9. Keep the added contract tests for annotations, output schema presence, structured-content text mirroring, structured tool errors, and stdio newline JSON-RPC behavior.
