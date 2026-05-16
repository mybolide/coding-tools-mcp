# Test Harness Subagent Report

## 2026-05-16 Acceptance Test Update

Scope for this pass was limited to `tests/compliance/**` and this report. I did not edit server implementation, release docs, commits, or pushes.

### Test Gaps Fixed

- Added a deterministic HTTP MCP client discovery assertion: a fresh client connected to the same server must complete `initialize`, call `tools/list`, and retrieve the same stable tool catalog as the original client.
- Kept stdio discovery covered through the current newline-delimited JSON-RPC stdio contract test, so clients that use stdio can catch the reported "cannot retrieve tools" failure mode.
- Added compliance report self-tests that patch report paths to a temporary directory and assert JSON/Markdown contents, category status, skipped tests, failures, and required-tool status without writing repo report artifacts.
- Strengthened golden assertions for `read_file` and `search_text` structured payloads, including stable path, encoding, line counts, query, match count, and truncation flags.
- Extended the deterministic MCP-only dogfood loop so the scripted agent retrieves the tool catalog before calling coding tools.
- Added E2E stdin/session coverage for polling a running process, natural exit, and structured error behavior on writes after session closure.
- Added security fixture coverage that read-only tools do not follow the malicious symlink escape and that `request_permissions` cannot silently grant dangerous access.

### Commands Run

- `python3 -m unittest tests.compliance.test_compliance_report`
  - Result: passed, 2 tests.
- `python3 -m py_compile tests/compliance/test_compliance_report.py tests/compliance/test_mcp_contract.py tests/compliance/test_tool_golden.py tests/compliance/test_security.py tests/compliance/test_e2e.py tests/compliance/test_dogfood.py tests/compliance/runner.py`
  - Result: passed.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m tests.compliance.runner --suite mcp-contract`
  - Result: failed, 11 tests run, 2 errors.
  - Passing coverage included fresh HTTP tool discovery and stdio `tools/list`.
  - Current blocker: every `tools/call` path hit JSON-RPC `-32603` with `name 'validate_arguments' is not defined`.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m tests.compliance.runner --suite all`
  - Result: failed, 43 tests run, 10 failures, 30 errors.
  - Dominant blocker: runtime `tools/call` errors with `name 'validate_arguments' is not defined`, so golden, security, E2E, Codex compatibility, and dogfood tool behavior cannot pass yet.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest tests.compliance.test_mcp_contract.MCPContractTests.test_fresh_http_clients_can_retrieve_stable_tool_catalog tests.compliance.test_mcp_contract.MCPContractTests.test_stdio_transport_uses_newline_delimited_json_rpc_only tests.compliance.test_compliance_report`
  - Result: passed, 4 tests. Python emitted non-fatal `ResourceWarning` messages for stdio subprocess streams.

### Changed Test Files

- `tests/compliance/runner.py`
- `tests/compliance/test_compliance_report.py`
- `tests/compliance/test_dogfood.py`
- `tests/compliance/test_e2e.py`
- `tests/compliance/test_mcp_contract.py`
- `tests/compliance/test_security.py`
- `tests/compliance/test_tool_golden.py`

### Current Handoff

The harness now catches the reported client tool-discovery failure independently of tool execution. The next implementation blocker is in the runtime: `tools/call` currently references undefined `validate_arguments`, causing structured tool behavior tests to fail before individual tool semantics are reached.

## Task Scope

- Role: `test-harness-engineer`.
- Built tests before runtime implementation.
- Owned changes under `tests/`, `Makefile` compliance targets, generated compliance report skeleton/output under `reports/compliance/`, and this report.
- Did not implement server runtime logic under `src/`.

## Materials Read

- `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`, especially sections 6-10 for P0 tool contract, fixtures, golden cases, security cases, E2E, Codex compatibility, and dogfood requirements.
- Local reference checkout under `.reference/openai-codex/`, with targeted searches around `apply_patch`, unified exec, MCP, truncation, and view image tests.

## Artifacts Produced

- `Makefile`
  - `make compliance`
  - `make test-mcp-contract`
  - `make test-tool-golden`
  - `make test-security`
  - `make test-e2e`
  - `make test-codex-compat`
  - `make dogfood-mcp`
  - `make report`
- `tests/compliance/runner.py`
  - stdlib-only unittest runner.
  - Writes `reports/compliance/latest.json` and `reports/compliance/latest.md`.
- `tests/compliance/mcp_client.py`
  - Streamable HTTP JSON-RPC MCP client.
  - Starts `codex-tool-runtime-mcp --workspace <fixture> --host 127.0.0.1 --port <port>` by default.
  - Supports `CODEX_TOOL_RUNTIME_SERVER_CMD` and `CODEX_TOOL_RUNTIME_SERVER_URL`.
- `tests/compliance/fixtures/`
  - `tiny-js-project`
  - `tiny-python-project`
  - `long-running-project`
  - `image-project`
  - `malicious-project`
- `tests/compliance/outside-secret.txt`
- `tests/compliance/codex_compat/semantic_vectors.json`
- Test modules:
  - `test_mcp_contract.py`
  - `test_tool_golden.py`
  - `test_security.py`
  - `test_e2e.py`
  - `test_codex_compat.py`
  - `test_dogfood.py`

## Key Coverage

- MCP contract:
  - `initialize`
  - `tools/list`
  - required P0 tools
  - forbidden product-layer tools
  - input schema sanity
  - structured success and failure shapes
  - unknown tool behavior
  - stdout pollution guard
- Golden tools:
  - `read_file`, `list_dir`, `list_files`, `search_text`
  - `apply_patch` add/update/delete/move/failure/path safety
  - `exec_command`, `write_stdin`, `kill_session`
  - `git_status`, `git_diff`
- Security:
  - traversal, absolute path, symlink escape
  - command workdir escape and shell attempts to read outside workspace
  - destructive command guardrails
  - network default policy
  - sensitive env stripping
  - concurrent read-only calls
- Deterministic E2E:
  - JS bugfix loop
  - Python function-add loop
  - long-running stdin loop
  - workspace escape loop
  - optional P1 `view_image`
- Codex compatibility:
  - Codex-style apply_patch envelope semantic vectors
  - exec/session/stdin behavior vectors
  - optional image behavior when exposed
- Dogfood skeleton:
  - deterministic MCP-only agent loop that records MCP tool calls and avoids direct filesystem/shell bypass.

## Expected Current Failures

`make compliance` currently fails before runtime assertions because no MCP runtime executable exists yet:

```text
MCP server command is unavailable. Set CODEX_TOOL_RUNTIME_SERVER_CMD or CODEX_TOOL_RUNTIME_SERVER_URL.
Default command: codex-tool-runtime-mcp --workspace <fixture> --host 127.0.0.1 --port <port>
```

Current generated report:

- `reports/compliance/latest.json`
- `reports/compliance/latest.md`

Current result:

- tests run: 29
- passed: false
- failures: 29
- expected reason: missing server runtime command/URL, not runtime semantic failures yet.

## Implementation Handoff

- Implement the P0 streamable HTTP MCP server and expose `codex-tool-runtime-mcp`.
- Keep logs on stderr. The tests assert stdout remains clean.
- Match these tool argument names:
  - `read_file`: `path`, `start_line`, `end_line`, `max_bytes`
  - `list_dir`: `path`, `include_hidden`, `max_results`
  - `list_files`: `glob`, `path`, `max_results`
  - `search_text`: `query`, `path`, `glob`, `context_lines`, `max_results`
  - `apply_patch`: `patch`
  - `exec_command`: `cmd`, `workdir`, `timeout_ms`, `max_output_bytes`, `tty`
  - `write_stdin`: `session_id`, `chars`
  - `kill_session`: `session_id`
  - `git_diff`: `path`, `max_bytes`
- Return MCP tool results with `content` and preferably `structuredContent`.
- For tool-level denials, either return MCP `isError=true` with structured content or a JSON-RPC error with `code` and `message`.

## Risks

- The security tests intentionally require more than path normalization; `exec_command` must prevent command-level outside-workspace reads, not only invalid `workdir`.
- Network-denial behavior must be deterministic, otherwise the suite can become environment-dependent.
- The Python fixture uses a local `pytest.py` shim so the compliance suite does not depend on installing external pytest.
- P1 `view_image` tests are skipped only when the tool is absent; if the tool is exposed, it must pass.

## Action Items

- Implementation engineer: implement server runtime until `make compliance` reaches semantic failures, then iterate against the failing cases.
- Security engineer: review command-denial expectations before runtime ships.
- Benchmark/dogfood engineer: reuse `test_dogfood.py` as the deterministic MCP-only path and extend it for benchmark reporting.
