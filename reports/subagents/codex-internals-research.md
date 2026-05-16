# Codex Internals Research

## Task Scope

Research OpenAI Codex internals that are relevant to a coding-agent runtime MCP server. The target server is the lower-level runtime described in `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`, not a wrapper around the Codex CLI or ChatGPT product features.

This report covers:

- Codex local tool capabilities.
- Which capabilities should be MCP-ified for this project.
- Which Codex capabilities should not be exposed.
- Tests and fixtures that can be reused or migrated.
- `apply_patch` semantics and limits.
- Shell, exec, session, and stdin semantics.
- Whether `view_image` is P1-worthy.
- Current project state in `/root/workspace` as of this review.

## Sources Read, Cloned, Referenced

- Project brief: `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`.
- Current project implementation and tests:
  - `codex_tool_runtime_mcp/server.py`
  - `docs/profile-v0.1.md`
  - `SPEC.md`
  - `Makefile`
  - `pyproject.toml`
  - `tests/compliance/mcp_client.py`
  - `tests/compliance/runner.py`
  - `tests/compliance/test_mcp_contract.py`
  - `tests/compliance/test_tool_golden.py`
  - `tests/compliance/test_security.py`
  - `tests/compliance/test_e2e.py`
  - `tests/compliance/test_codex_compat.py`
  - `tests/compliance/test_dogfood.py`
  - `tests/compliance/codex_compat/semantic_vectors.json`
  - `reports/compliance/latest.md`
  - `reports/dogfood/codex-on-mcp.md`
  - `reports/benchmark/swebench-regression.md`
- Local clone: `.reference/openai-codex`, shallow clone of `https://github.com/openai/codex`, reviewed at short commit `de9c5c0` (`Fix Windows doctor npm root probe (#22967)`). This directory is intentionally under `.reference/` and is ignored by `.gitignore`.
- Codex CLI and MCP server:
  - `.reference/openai-codex/codex-rs/README.md`
  - `.reference/openai-codex/codex-rs/mcp-server/src/codex_tool_config.rs`
  - `.reference/openai-codex/codex-rs/mcp-server/src/message_processor.rs`
  - `.reference/openai-codex/codex-rs/mcp-server/src/lib.rs`
  - `.reference/openai-codex/codex-rs/mcp-server/tests/suite/codex_tool.rs`
- `apply_patch` implementation and tests:
  - `.reference/openai-codex/codex-rs/apply-patch/src/parser.rs`
  - `.reference/openai-codex/codex-rs/apply-patch/src/lib.rs`
  - `.reference/openai-codex/codex-rs/apply-patch/src/invocation.rs`
  - `.reference/openai-codex/codex-rs/apply-patch/apply_patch_tool_instructions.md`
  - `.reference/openai-codex/codex-rs/apply-patch/tests/suite/tool.rs`
  - `.reference/openai-codex/codex-rs/apply-patch/tests/suite/scenarios.rs`
  - `.reference/openai-codex/codex-rs/apply-patch/tests/fixtures/scenarios/`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/apply_patch.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/apply_patch_tests.rs`
- Shell, unified exec, sessions, and stdin:
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/shell_spec.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/shell.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/shell/shell_command.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/unified_exec.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/unified_exec/write_stdin.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/runtimes/unified_exec.rs`
  - `.reference/openai-codex/codex-rs/core/src/unified_exec/mod.rs`
  - `.reference/openai-codex/codex-rs/core/src/unified_exec/process_manager.rs`
  - `.reference/openai-codex/codex-rs/core/src/unified_exec/process.rs`
  - `.reference/openai-codex/codex-rs/core/src/unified_exec/head_tail_buffer.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/unified_exec_tests.rs`
  - `.reference/openai-codex/codex-rs/core/src/unified_exec/mod_tests.rs`
- Image viewing:
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/view_image.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/handlers/view_image_spec.rs`
  - `.reference/openai-codex/codex-rs/core/src/original_image_detail.rs`
  - `.reference/openai-codex/codex-rs/core/src/context_manager/history_tests.rs`
- Sandbox, approvals, and protocol:
  - `.reference/openai-codex/codex-rs/protocol/src/protocol.rs`
  - `.reference/openai-codex/codex-rs/protocol/src/exec_output.rs`
  - `.reference/openai-codex/codex-rs/protocol/src/exec_output_tests.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/sandboxing.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/sandboxing_tests.rs`
  - `.reference/openai-codex/codex-rs/core/src/tools/network_approval.rs`
  - `.reference/openai-codex/docs/sandbox.md`
  - `.reference/openai-codex/docs/config.md`
- Public OpenAI docs used as context from the task brief:
  - `https://developers.openai.com/codex/mcp`
  - `https://developers.openai.com/codex/security`
  - `https://developers.openai.com/codex/config-reference`
  - `https://platform.openai.com/docs/guides/tools-apply-patch`
  - `https://developers.openai.com/api/docs/guides/tools-shell`

The operational details below come primarily from the public Codex repository because it contains the concrete schemas, handlers, and tests.

## Key Findings

### 1. Existing Codex Local Tool Abilities

Codex has local coding-agent primitives, but its own public MCP server is not that primitive surface.

The model-visible local capabilities in the reviewed source include:

- Command execution:
  - Legacy one-shot `shell_command`.
  - Unified `exec_command`.
  - `write_stdin` for live unified-exec sessions.
- File editing:
  - `apply_patch` as a freeform patch tool.
  - Shell interception for `apply_patch` command-shaped calls.
- Local image loading:
  - `view_image` for image files on disk, returning a data URL and image detail hint to the model path.
- Permissions:
  - `request_permissions`, feature-gated by approval configuration.
  - Approval and sandbox plumbing for exec, patch, and network cases.
- Planning and orchestration:
  - `update_plan`.
  - Goal tools.
  - Multi-agent tools such as `spawn_agent`, `send_input`, `wait_agent`, `close_agent`, and newer agent-job variants.
- MCP client capability:
  - Codex can call tools exposed by configured MCP servers.
- MCP resource helper tools:
  - Codex has handlers for listing and reading MCP resources from configured servers.

The reviewed Codex core does not primarily expose separate low-level file read/list/search tools. It relies heavily on shell commands such as `rg`, `sed`, `ls`, and `git` for read-only filesystem work. That is acceptable in a full CLI agent, but it is too broad for this project as a runtime MCP contract. This project should expose dedicated read-only file and search primitives so clients do not need shell access for basic inspection.

### 2. The Existing Codex MCP Server Is A High-Level Wrapper

`codex-rs/mcp-server` exposes two tools:

- `codex`: starts or continues a Codex agent task from a prompt.
- `codex-reply`: sends a follow-up prompt to an existing Codex thread.

The `codex` tool accepts product/runtime controls such as `prompt`, optional `model`, `profile`, `cwd`, `approval_policy`, `sandbox`, `config`, `base_instructions`, `developer_instructions`, and `compact_prompt`. Its output schema is a high-level `{ threadId, content }` shape. `tools/list` returns only `codex` and `codex-reply`; unknown tools return a tool-level error result.

This is useful as a reference for JSON-RPC stdio hygiene, output schemas, elicitation wiring, and approval request shape, but it is the wrong MCP surface for this project. The project brief explicitly calls for runtime primitives, not a Codex-in-Codex wrapper.

Transport detail worth borrowing: the Codex MCP server uses stdin/stdout for JSON-RPC, keeps protocol output on stdout, and sends tracing/logging to stderr. This project follows that pattern for `--stdio` and logs HTTP server messages to stderr.

### 3. What Should Be MCP-ized

The project should MCP-ify the lower-level runtime operations from the brief:

- `read_file`, `list_dir`, `list_files`, and `search_text` as safe, workspace-bound read-only tools.
- `apply_patch` using Codex-style patch grammar, with project-specific workspace and symlink enforcement.
- `exec_command` and `write_stdin` using Codex unified-exec semantics as the baseline.
- `kill_session` as a project-specific addition. Codex has internal process termination and LRU pruning, but no model-visible `kill_session` tool in the reviewed local tool surface.
- `git_status` and `git_diff` as structured git primitives so clients do not need shell for routine review.
- `request_permissions` as a structured approval path, not as silent privilege escalation.
- `view_image` as P1, feature-gated.

Current project state:

- `codex_tool_runtime_mcp/server.py` exposes all P0 tools by default over Streamable HTTP and has a `--stdio` mode.
- `view_image` exists. In the current dirty working tree, CLI startup enables it by default unless `CODEX_TOOL_RUNTIME_ENABLE_VIEW_IMAGE=0`, which broadens the default P0 tool catalog beyond the earlier P1-gated plan.
- `tools/list` does not expose Codex product wrappers or forbidden product-layer tools.
- Tool annotations are present for read-only, destructive, idempotent, and open-world hints.
- `reports/compliance/latest.md` currently records an older PASS for 29 tests, but the working tree is now dirty from concurrent implementation/test edits. A fresh `CODEX_TOOL_RUNTIME_SERVER_CMD='python -m codex_tool_runtime_mcp --workspace {workspace} --host 127.0.0.1 --port {port}' python -m tests.compliance.runner --suite mcp-contract` run fails because `Runtime.call_tool()` calls `validate_arguments(...)`, which is not currently defined.

### 4. What Should Not Be Exposed

Do not expose the following as runtime MCP tools:

- High-level Codex tools such as `codex`, `codex-reply`, or equivalent agent wrappers.
- ChatGPT or Codex account login, token, keyring, billing, or paid-routing features.
- Codex cloud tasks, remote queues, or cloud-environment management.
- Memory, personalization, user profile, or cross-session preference storage.
- Web search, arbitrary network fetch, browser automation, or image generation.
- Plugin marketplace, connector installation, or tool discovery/install flows.
- Model selection or OpenAI account routing controls.
- Subagent orchestration tools such as `spawn_agent`, `spawn_agents_on_csv`, or `agent_jobs`.
- MCP resource proxy helpers unless the project explicitly broadens from runtime primitives to resource brokerage.
- Planning/goal tools such as `update_plan` and goal APIs as external runtime tools.
- Raw unrestricted shell or full Codex config override surfaces.
- Base/developer instruction injection through MCP.

Several of these are useful inside Codex as a product, but they would blur the runtime boundary and expand the security model beyond the project brief.

### 5. `apply_patch` Semantics And Limits

Codex `apply_patch` uses a custom envelope grammar. The relevant grammar in `parser.rs` is:

```text
start: begin_patch environment_id? hunk+ end_patch
begin_patch: "*** Begin Patch" LF
environment_id: "*** Environment ID: " filename LF
end_patch: "*** End Patch" LF?
hunk: add_hunk | delete_hunk | update_hunk
add_hunk: "*** Add File: " filename LF add_line+
delete_hunk: "*** Delete File: " filename LF
update_hunk: "*** Update File: " filename LF change_move? change?
filename: /(.+)/
add_line: "+" /(.+)/ LF -> line
change_move: "*** Move to: " filename LF
change: (change_context | change_line)+ eof_line?
change_context: ("@@" | "@@ " /(.+)/) LF
change_line: ("+" | "-" | " ") /(.+)/ LF
eof_line: "*** End of File" LF
```

Important upstream behavior:

- It supports add, delete, update, and move/rename via `*** Move to:`.
- It supports an optional `*** Environment ID: ...` header for multi-environment Codex sessions. This project should omit that unless it later supports multiple workspaces.
- The parser and invocation layer are lenient in practice. They tolerate whitespace-padded markers and can extract heredoc forms such as `apply_patch <<'PATCH'`.
- The tool instructions tell the model to use relative file references only. The parser and tests accept both relative and absolute hunk paths, so this project must enforce workspace-relative or workspace-contained paths at the MCP layer.
- Add can overwrite an existing file upstream and records the overwritten content in the delta.
- Delete fails on missing files and on directories; it is not recursive deletion.
- Update requires an existing file.
- Move writes the destination and then removes the source; it can overwrite the destination.
- Parent directories for added or moved files may be created as needed.
- Empty patches fail with "No files were modified."
- Application is not atomic across hunks upstream. If an early hunk succeeds and a later hunk fails, the earlier file changes remain. Codex exposes failure with a delta; tests confirm partial success is left on disk.
- On write failure, delta exactness can become false because the filesystem may have changed before the error.
- Successful CLI output is a summary such as `Success. Updated the following files:` followed by `A`, `M`, and `D` entries.

Current project state:

- The server implements add, update, delete, move, dry-run, workspace path rejection, symlink write rejection, and transactional staged writes with rollback.
- The server is intentionally stricter than upstream Codex in at least one important case: `*** Add File` rejects an existing destination, while upstream allows overwriting.
- The server does not implement the full upstream parser. It requires a direct `*** Begin Patch` / `*** End Patch` body, does not support environment ids, and does not implement Codex's full heredoc extraction surface.
- This stricter behavior is acceptable for P0 if it is documented as project policy and covered by compatibility tests.

Recommendation: keep the Codex patch envelope and operation names for compatibility, but keep the project-level safety policy stricter than Codex where needed. In particular, retain workspace containment, symlink escape rejection, explicit structured errors, and transactional rollback. Add a compatibility note that duplicate add and non-atomic partial success intentionally diverge from upstream.

### 6. Shell, Exec, Session, And Stdin Semantics

Codex has two execution paths:

- Legacy `shell_command`:
  - One-shot command execution.
  - Parameters include `command`, `workdir`, `timeout_ms`, optional `login`, sandbox/approval fields, and permission fields.
  - No model-visible persistent session id or stdin path.
- Unified `exec_command`:
  - Parameters include `cmd`, optional `workdir`, `shell`, `login`, `tty`, `yield_time_ms`, `max_output_tokens`, sandbox/approval fields, and optional `environment_id`.
  - Description: "Runs a command in a PTY, returning output or a session ID for ongoing interaction."
  - Despite that description, `tty` defaults false. Codex allocates a PTY only when requested.
- `write_stdin`:
  - Parameters include `session_id`, optional `chars`, `yield_time_ms`, and `max_output_tokens`.
  - Empty `chars` means poll without writing.
  - Nonempty `chars` requires the original process to have been started with `tty: true`; otherwise Codex reports that stdin is closed and tells the model to rerun with `tty=true`.

Unified exec output schema includes:

- `chunk_id`
- `wall_time_seconds`
- `exit_code`
- `session_id`
- `original_token_count`
- `output`

Behavior worth carrying into this project:

- If a command exits within the initial yield window, the response includes `exit_code` and no session id is needed.
- If the command is still running after the yield window, the response includes `session_id`.
- A later `write_stdin` call returns more output and either a still-running `session_id` or a final `exit_code`.
- An empty `write_stdin` call is a poll.
- Once a process exits and is removed, subsequent writes return unknown-process errors in Codex.
- `yield_time_ms` is clamped. Codex constants include minimum 250 ms, empty-poll minimum 5000 ms, maximum 30000 ms for ordinary yields, and a default background terminal timeout of 300000 ms.
- Default max output is 10000 tokens, with a hard unified-exec output byte cap of 1 MiB.
- Codex limits unified exec processes to 64 and prunes older background processes, protecting the most recent ones.
- Workdir is resolved against the active environment cwd.
- Login shell usage can be disabled. If disabled, `login: true` is rejected.
- Production session ids are randomized in a broad numeric range; tests use deterministic ids.
- Output is buffered with truncation behavior tested by head-tail buffer tests.

Current project state:

- `exec_command` uses `cmd`, workspace-bound `workdir`, `timeout_ms`, `yield_time_ms`, `max_output_bytes`, optional `stdin`, `tty`, and sanitized `env`.
- Long-running commands return a random string `session_id`, and `write_stdin` can write or poll a server-managed session.
- `kill_session` is implemented as a project-specific process-group termination tool.
- Output is split into `stdout` and `stderr` instead of Codex's single `output` string, and caps use bytes rather than tokens.
- The implementation starts subprocesses with `shell=True`, pipes, and `start_new_session=True`; it does not allocate a real PTY even when `tty: true`.
- The implementation does not currently store the original `tty` flag on `ExecSession`, so a long-running non-TTY process with an open stdin pipe could be writable through `write_stdin`. Codex rejects nonempty writes unless the original exec used `tty=true`.

Recommendation: continue using the unified-exec model, but tighten compatibility by storing `tty` on sessions and rejecting nonempty `write_stdin` for non-TTY sessions. If closer Codex parity is needed, add real PTY support or document `tty` as "interactive session mode" rather than true terminal allocation.

### 7. Sandbox And Approval Notes

Codex protocol has approval policies such as `untrusted`, deprecated `on-failure`, `on-request`, `granular`, and `never`. Sandbox policies include `danger-full-access`, `read-only`, `workspace-write`, and external sandbox modes. Workspace-write policy has details for writable roots and protected metadata paths.

This project should not clone the entire Codex product sandbox stack for P0. It should implement a smaller, enforceable runtime policy:

- One canonical workspace root.
- Shared path resolver for every filesystem, git, patch, image, and cwd input.
- Absolute paths rejected by default, or accepted only after canonical workspace containment in a future opt-in mode.
- No symlink escape.
- No writes outside the workspace.
- Network denied by default.
- Explicit approval flow for higher-risk exec cases.
- Output, timeout, process count, and session idle limits.

Current project state:

- The shared `Workspace` resolver rejects absolute paths, `..`, and symlink escapes for file and patch operations.
- Command policy blocks obvious network, destructive command, sensitive env, and path escape patterns before execution.
- `request_permissions` currently returns structured `ELICITATION_UNSUPPORTED`, which is acceptable for clients without elicitation if dangerous operations remain blocked.
- The command sandbox is policy-based, not OS-isolated. Regex guardrails reduce accidental risk but do not provide a complete sandbox.

The Codex approval/sandbox source is still useful as a vocabulary and as a reminder that permissions must be field-driven, not inferred from command strings alone.

### 8. Reusable Or Migratable Tests

Best upstream test sources to migrate or adapt:

- `codex-rs/apply-patch/tests/fixtures/scenarios/`
  - Golden scenarios for add, update, delete, multi-op patches, multi-chunk patches, move to new directory, overwrite behavior, Unicode, EOF markers, whitespace-padded markers, missing context, missing files, delete-directory failure, trailing newline handling, and partial success.
- `codex-rs/apply-patch/tests/suite/tool.rs`
  - CLI-style success/error output and partial failure behavior.
- `codex-rs/apply-patch/src/parser.rs` tests
  - Parser grammar, lenient heredoc handling, hunk path parsing, and absolute-path acceptance. For this project, keep parser acceptance tests only where intentionally supported and add MCP policy tests that reject absolute paths escaping the workspace.
- `codex-rs/apply-patch/src/invocation.rs` tests
  - If shell interception is ever added, migrate only the safe direct and `cd <path> && apply_patch <<'PATCH'` forms. Do not expose broad shell parser behavior as an MCP API unless it is required.
- `codex-rs/core/src/tools/handlers/apply_patch_tests.rs`
  - Handler-level verification, environment-id behavior, streamed diff consumption, hook payload shaping, and apply-patch shell interception.
- `codex-rs/core/src/tools/handlers/unified_exec_tests.rs`
  - Command resolution, explicit shell behavior, login rejection, hook payloads, and `write_stdin` preserving original exec call identity.
- `codex-rs/core/src/unified_exec/mod_tests.rs`
  - Session persistence, multiple session isolation, timeout then poll retrieval, head-tail buffering, pause extending yield, and remote/local process contracts.
- `codex-rs/core/src/unified_exec/head_tail_buffer_tests.rs`
  - Head-tail retention behavior for large command outputs.
- `codex-rs/protocol/src/exec_output_tests.rs`
  - Exec output encoding/decoding behavior.
- `codex-rs/core/src/tools/handlers/view_image_spec.rs`
  - `path` and optional `detail` schema behavior.
- `codex-rs/core/src/context_manager/history_tests.rs`
  - Image payload sizing and capping behavior, including original-detail image accounting.
- `codex-rs/core/src/tools/sandboxing_tests.rs`
  - Workspace-write and sandbox-policy expectations.

Current project state:

- `tests/compliance/codex_compat/semantic_vectors.json` already migrates a useful subset of apply-patch semantic vectors: add, update, delete, move, context mismatch, absolute path rejection, and traversal rejection.
- `tests/compliance/test_codex_compat.py` covers those vectors plus session semantics and optional `view_image`.
- The current compatibility tests do not yet cover upstream duplicate-add overwrite behavior, partial-success divergence, heredoc leniency, whitespace-padded markers, head-tail buffering, or non-TTY `write_stdin` rejection.

Migration guidance:

- Do not copy OpenAI tests verbatim without checking license and attribution requirements.
- Port scenario ideas and expected behavior into this repo's own fixtures.
- For every intentional divergence from Codex, add a test and document whether the project is stricter or simply incomplete.
- Add project-specific tests for MCP schemas, workspace confinement, symlink escape rejection, permission denials, structured failure shapes, and JSON-RPC error boundaries.

### 9. Is `view_image` P1-Worthy?

Yes. `view_image` is P1-worthy, but not P0.

Reasons to include it in P1:

- Frontend and UI coding agents need to inspect screenshots, generated assets, visual regressions, and design references.
- Codex already treats local image viewing as a useful model-facing primitive.
- The implementation boundary is manageable: it reads a local file, validates image bytes, and returns image content with a detail hint.

Reasons not to make it P0:

- Core coding workflows can be handled with text file tools, patching, exec, git status/diff, and tests.
- Image handling needs separate file size, MIME, decode, dimension, and prompt payload limits.
- It increases the blast radius of path handling if implemented before the shared workspace resolver is hardened.

Current project state:

- `view_image` is implemented and has a CLI/env control, but in the current dirty working tree it is exposed by default unless `CODEX_TOOL_RUNTIME_ENABLE_VIEW_IMAGE=0`; this should be treated as an in-flight policy change, not final P1 completion.
- It validates workspace path containment, size, and basic image type from bytes for PNG/GIF/JPEG, with extension fallback for other image MIME guesses.
- It returns a structured `data_url`, `mime_type`, dimensions when known, and byte count.
- It ignores the declared `output` argument and does not emit MCP image content blocks; the tool description says "MCP image content", so the P1 contract should be tightened before calling this complete.
- It does not expose Codex's `detail: high|original` behavior yet.

P1 requirements:

- Workspace-contained paths only.
- File must be a regular image file.
- MIME/type detection from bytes, not only extension.
- File size and pixel dimension limits.
- Optional `detail` support only if the client/model path can consume it.
- Return MCP image content or a structured data URL, matching the profile contract and tool description.

## Project-Specific Recommendations

1. Keep the default P0 surface as runtime primitives, not as a Codex wrapper:
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

2. Keep high-level product features out:
   - Do not expose `codex`, `codex-reply`, model selection, cloud tasks, subagents, memory, web search, connector install flows, or instruction injection.

3. Document intentional Codex divergences:
   - `apply_patch` is transactional in this project; upstream is not.
   - Duplicate `Add File` is rejected in this project; upstream can overwrite.
   - The project rejects absolute paths and traversal at the MCP layer; upstream parser accepts absolute paths in some contexts.
   - The project currently uses byte output caps and separate stdout/stderr instead of Codex token caps and a single `output` field.

4. Tighten `exec_command` / `write_stdin` compatibility:
   - Store `tty` on session state.
   - Reject nonempty `write_stdin` unless the session was started with `tty=true`.
   - Add idle/session cleanup and a process-count cap comparable to Codex's 64-process limit.
   - Decide whether real PTY support is required or whether the profile should call this an interactive pipe session.

5. Keep `kill_session` as a project-specific extension:
   - Codex needs process cleanup internally, but this MCP runtime needs a client-visible termination primitive.
   - Continue terminating the process group, not only the parent process.

6. Keep basic file and git reads first-class:
   - This is safer than forcing clients through shell.
   - It also makes MCP schemas and permissions easier to validate.

7. Continue expanding migrated compatibility tests:
   - Add upstream apply-patch scenario coverage for whitespace-padded markers, trailing newline behavior, delete-directory failure, duplicate add divergence, move-overwrite divergence, and partial-failure divergence.
   - Add unified-exec tests for non-TTY stdin rejection, empty polling, output truncation, process count, and closed-session cleanup.

8. Keep `view_image` as P1 unless the profile is explicitly revised:
   - Restore disabled-by-default behavior or update `docs/profile-v0.1.md`, `SPEC.md`, tests, and security notes to say `view_image` is now default.
   - Before enabling in default dogfood, align the return shape with the profile: either true MCP image content or explicitly documented data URL output.

## Risks

- `apply_patch` parser compatibility can accidentally allow paths the model instructions say not to use. The server must enforce path policy after parsing.
- The current project parser is narrower than Codex's parser. That is safer, but any "Codex-compatible" claim must call out unsupported environment-id, heredoc, and whitespace-leniency cases.
- Upstream Codex non-atomic patch behavior conflicts with this project's "no half products" requirement. The current transactional behavior is preferable, but compatibility tests must mark the divergence.
- Shell execution is inherently broad. Dedicated file/git tools reduce the need for shell but do not remove the need for command sandboxing.
- Regex-based command blocking is not a complete sandbox. A determined command can still hide network or filesystem behavior unless OS-level sandboxing is added.
- Interactive sessions can outlive the request that created them. They need idle cleanup, process-tree termination, ownership checks, and bounded output.
- Current `tty` is not a true PTY. Interactive programs that require terminal behavior may not work even though simple stdin workflows pass.
- If non-TTY sessions remain writable, the project diverges from Codex and broadens stdin behavior more than intended.
- The current dirty working tree has a blocking runtime regression: `tools/call` can return JSON-RPC `-32603` because `validate_arguments` is referenced but missing.
- The current dirty working tree exposes `view_image` by default, which conflicts with the earlier P1-gated recommendation unless the profile is deliberately changed.
- Output truncation can hide important failure lines. Return truncation metadata and prefer head-tail retention for long-running commands.
- `view_image` can become an unintended file exfiltration path if path handling is weaker than the text file tools.
- Copying the high-level Codex MCP server shape would violate the project boundary and create a much larger product/security surface.

## Action Items

1. Fix or complete the in-flight `validate_arguments` implementation before trusting any compliance report; fresh mcp-contract currently fails at `tools/call`.
2. Decide whether `view_image` is still P1-gated or default-on. If P1, restore disabled-by-default behavior; if default-on, update the profile, SPEC, security model, and tests accordingly.
3. Add compatibility vectors for documented `apply_patch` divergences: duplicate add rejection, upstream partial-success divergence, move overwrite behavior, whitespace-padded markers, and trailing newline handling.
4. Add a non-TTY `write_stdin` rejection test and update session state to store the original `tty` flag.
5. Add session lifecycle limits: max active sessions, idle timeout, and explicit cleanup of exited sessions.
6. Decide and document whether `exec_command.tty=true` means a real PTY or only "keep stdin open for interaction"; implement real PTY if terminal semantics are required.
7. Align `view_image` output with the profile and tool description before marking P1 complete: MCP image content blocks or clearly documented structured data URL.
8. Add `view_image` tests with the feature flag enabled, including non-image rejection, traversal rejection, size cap, and MIME detection.
9. Keep `.reference/openai-codex` uncommitted; use it only as a local reference source.
10. Keep the current P0 no-product-wrapper boundary enforced in `tools/list` tests.
