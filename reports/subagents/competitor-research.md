# Competitor Research: Coding Agent Runtime Interfaces

Research date: 2026-05-16
Role: competitor-researcher

## Task Scope

Compare public coding-agent, CLI, and agent-computer-interface designs against this repository's goal: a model-neutral MCP server that exposes local coding runtime primitives, not a product-level agent wrapper.

The scope is limited to designs relevant to:

- file read, directory listing, glob/search, structured editing, shell execution, stdin/session handling, git status/diff, and output truncation
- permission and sandbox controls for filesystem, shell, network, credentials, and destructive operations
- subagent and parallel-work patterns that can inform internal dogfood/validation without becoming public P0 MCP tools
- MCP contract/Inspector practices that can strengthen compliance testing

No implementation code was edited for this research. This report is the only file changed by this subtask.

## Sources Read, Cloned, or Referenced

No repositories were cloned. Public repositories were referenced through official docs and GitHub pages only, so `.reference/` was not modified.

Local project sources read:

- `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`
- `README.md`
- `SPEC.md`
- `docs/profile.md`
- `docs/profile-v0.1.md`
- `SECURITY.md`
- `COMPLIANCE.md`
- `BENCHMARK.md`
- `docs/research/reference-review.md`
- `reports/subagents/mcp-contract.md`
- `reports/subagents/security-sandbox.md`
- `reports/subagents/test-harness.md`
- previous `reports/subagents/competitor-research.md`

Public MCP references:

- MCP 2025-06-18 overview, tools, transports, schema, and elicitation specs: https://modelcontextprotocol.io/specification/2025-06-18/basic/index, https://modelcontextprotocol.io/specification/2025-06-18/server/tools, https://modelcontextprotocol.io/specification/2025-06-18/basic/transports, https://modelcontextprotocol.io/specification/2025-06-18/schema, https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation
- MCP Inspector docs and repo: https://modelcontextprotocol.io/docs/tools/inspector and https://github.com/modelcontextprotocol/inspector
- MCPJam Inspector CLI docs as a secondary CI-oriented inspector reference: https://docs.mcpjam.com/cli/overview and https://docs.mcpjam.com/cli/reference

Public competitor references:

- OpenCode docs and repo pages: https://opencode.ai/docs/, https://opencode.ai/docs/tools/, https://opencode.ai/docs/agents/, https://github.com/anomalyco/opencode
- Claude Code docs: subagents, MCP, permissions, permission modes, settings, hooks: https://code.claude.com/docs/en/sub-agents, https://code.claude.com/docs/en/mcp, https://code.claude.com/docs/en/permissions, https://code.claude.com/docs/en/permission-modes, https://code.claude.com/docs/en/settings, https://code.claude.com/docs/en/agent-sdk/hooks
- Gemini CLI docs: tools, filesystem tools, shell, checkpointing, sandboxing, MCP servers, subagents: https://google-gemini.github.io/gemini-cli/docs/tools/, https://google-gemini.github.io/gemini-cli/docs/tools/file-system.html, https://google-gemini.github.io/gemini-cli/docs/tools/shell.html, https://google-gemini.github.io/gemini-cli/docs/cli/checkpointing.html, https://google-gemini.github.io/gemini-cli/docs/cli/sandbox.html, https://google-gemini.github.io/gemini-cli/docs/tools/mcp-server.html, https://github.com/google-gemini/gemini-cli/blob/main/docs/core/subagents.md, https://developers.googleblog.com/subagents-have-arrived-in-gemini-cli/
- Aider docs: edit formats, repo map, git integration, commands, lint/test: https://aider.chat/docs/more/edit-formats.html, https://aider.chat/docs/repomap.html, https://aider.chat/docs/git.html, https://aider.chat/docs/usage/commands.html, https://aider.chat/docs/usage/lint-test.html
- SWE-agent docs and paper: https://swe-agent.com/0.7/, https://swe-agent.com/0.7/background/aci/, https://arxiv.org/abs/2405.15793
- mini-SWE-agent SWE-bench docs: https://mini-swe-agent.com/latest/usage/swebench/
- Public analysis: Claude Code design-space tech report: https://arxiv.org/abs/2604.14228

## Key Findings

### MCP Spec and Inspector Baseline

MCP 2025-06-18 supports this project's intended contract shape. Tools are model-controlled, discovered through `tools/list`, invoked through `tools/call`, and described with `inputSchema`, optional `outputSchema`, optional `annotations`, and result content. The spec says tool-originated failures should be returned inside the tool result with `isError: true`, while unknown tools and malformed protocol requests remain JSON-RPC errors.

Streamable HTTP is the right P0 transport for the profile. The spec requires a single MCP endpoint, HTTP POST for client messages, optional GET/SSE for server messages, `Accept: application/json, text/event-stream`, protocol-version headers after initialization, localhost binding for local servers, Origin validation, and authentication before non-loopback exposure. Stdio remains useful for P1 compatibility but must keep stdout as pure MCP JSON-RPC and send logs to stderr.

Tool annotations are useful display hints, not a security boundary. The tools spec explicitly requires clients to treat annotations as untrusted unless the server is trusted. For this runtime, annotations should communicate risk, while the server enforces path, permission, sandbox, timeout, and output policies.

Elicitation is a reasonable protocol hook for permission prompts, but it is client-dependent and cannot request secrets. `request_permissions` should therefore keep the current profile's structured fallback: return explicit `ELICITATION_UNSUPPORTED`, `PERMISSION_REQUIRED`, `PERMISSION_DENIED`, or a scoped grant result instead of silently widening policy.

MCP Inspector is directly useful for compliance and developer debugging. The official Inspector runs a web UI plus proxy and supports stdio, SSE, and Streamable HTTP. Inspector-style CLI checks should cover `initialize`, `tools/list`, schema validation, tool-call envelope validation, and representative success/error calls.

### File Read, Search, Edit, Shell, and Diff Patterns

OpenCode exposes a low-level coding tool split that closely resembles this project's target surface: `read`, `list`, `glob`, `grep`, experimental `lsp`, `edit`, `write`, `apply_patch`, and `bash`. `read` supports line ranges; `grep` supports regex and file pattern filtering; `edit` uses exact string replacement; `apply_patch` embeds project-relative paths in patch markers such as add/update/move/delete. Its `bash` tool is broad enough to run package installs and git commands, so it is a useful capability reference but not a safe default.

Claude Code's public docs describe Read/Grep/Glob-style file operations, Bash as a first-class tool, read/write permission rules, `/diff`-style review workflows, and MCP tool access. Its command permission docs show a mature but product-specific approach: wildcard Bash rules, compound-command splitting, wrapper stripping, path rules with symlink target checks, and hooks before tool use. The docs also warn that file permission rules do not constrain arbitrary subprocesses; OS sandboxing is needed for that.

Gemini CLI has the closest named filesystem surface to this project: `list_directory`, `read_file`, `glob`, `search_file_content`, `write_file`, and `replace`. File tools operate under a `rootDirectory`. Search uses `git grep` when available and ignores common nuisance directories. `write_file` and `replace` require confirmation and show diffs. `replace` is exact-string based but can attempt model-assisted edit correction; that is valuable in a product loop but too nondeterministic for this runtime's `apply_patch` contract.

Gemini CLI's `run_shell_command` returns command, directory, stdout, stderr, error, exit code, signal, and background PIDs. It can use a PTY for interactive commands when enabled, and it supports command restrictions through configuration. This maps well to the local `exec_command`/`write_stdin`/`kill_session` split, except this project should expose managed session IDs instead of raw background PIDs.

Aider is strongest on edit/diff ergonomics and repo context. It uses model-specific edit formats including whole-file, search/replace blocks, and simplified unified diff. It has `/diff`, `/undo`, `/git`, `/run`, `/test`, `/lint`, auto-lint/test feedback, and a repo map that summarizes important symbols and ranks context under a token budget. Its automatic git commits make undo easy for one user, but they are unsafe for a shared worktree runtime.

SWE-agent shows that custom agent-computer interfaces matter. Its ACI uses a purpose-built file viewer, small paginated reads, scrolling, search within files, directory search, edit-time linting, and explicit feedback for empty command output. The paper reports that interface design materially affects benchmark performance. This supports a tool-shaped MCP runtime rather than a single generic terminal.

mini-SWE-agent deliberately simplifies to a bash-centric interface for SWE-bench. Its docs encourage multiple independent bash calls in one turn, Docker or Singularity/Apptainer environments, non-persistent subshell state, output caps with head/tail elision, and final patch submission through `git diff`. These patterns are useful for benchmark harness discipline but too broad as the public P0 runtime surface.

### Permission and Sandbox Designs

OpenCode has a simple permission vocabulary: `allow`, `ask`, and `deny`, with keys for read, edit, glob, grep, list, bash, task, external directory, web fetch/search, LSP, skill, and other tools. The grouping is borrowable, but its documented default of all tools enabled without permission is too permissive for arbitrary MCP clients.

Claude Code has the richest permission model. It layers hooks, deny rules, permission modes, allow rules, and a runtime callback. It supports `default`, `acceptEdits`, `plan`, `auto`, `dontAsk`, and `bypassPermissions` modes, plus sandbox restrictions for filesystem and network access. Important lessons: deny rules must take precedence; symlink checks must consider both link and target; command-string rules are not sufficient OS enforcement; and bypass-style modes belong only in isolated containers or VMs.

Gemini CLI combines confirmations, command include/exclude settings, MCP server `trust`, MCP `includeTools`/`excludeTools`, and sandboxing. Its sandbox methods include macOS Seatbelt and Docker/Podman. It also notes that MCP servers must be available inside the sandbox when sandboxing is active. Borrow the layered sandbox posture, but avoid treating `trust: true` as an equivalent to server-side safety.

Aider primarily relies on git snapshots/undo and explicit user-selected chat files, not a fine-grained sandbox. That is acceptable for an interactive pair programmer but insufficient for a reusable MCP server that may be connected to unknown clients.

SWE-agent and mini-SWE-agent rely on controlled benchmark environments, usually Docker or Singularity. Their isolation is environmental rather than prompt-based. This supports the project security report's conclusion that shell autonomy eventually needs real OS/container enforcement, not only string classification.

### Subagent and Parallel Work Designs

OpenCode supports agents with per-agent permissions and a `task` delegation primitive. Task permissions can restrict which subagents an agent may invoke, and hidden subagents can be invoked programmatically if permissions allow. This is a useful internal orchestration model, but exposing it as an MCP tool would expand this project's scope beyond runtime primitives.

Claude Code subagents have isolated context, model choice, tool allowlists/denylists, scoped MCP servers, hooks, max turns, memory, background execution, and isolation/worktree options. Claude's docs also mention that subagents do not automatically inherit parent permissions, which prevents accidental privilege propagation but can multiply prompts. The strongest borrowable idea is subagent scoping by tools and MCP servers, not the product-level agent marketplace.

Gemini CLI exposes subagents as tools to the main agent, with automatic delegation and explicit `@agent` delegation. Subagents run in their own context loops, have explicit tool access, can define inline MCP servers, and cannot recursively call other subagents. The recursion guard is directly borrowable for any internal dogfood runner.

Aider's architect/editor mode is a useful two-role workflow, but not general subagent orchestration. It separates planning from editing and can use different models, which is a good pattern for internal review but not a P0 MCP tool.

SWE-agent parallelism comes mostly from benchmark runners rather than collaborative agents. mini-SWE-agent supports batch workers and parallel independent bash calls, which is relevant for benchmark throughput and compliance concurrency tests.

### Borrowable Designs

- MCP spec: schema-first `tools/list` and `tools/call`, `structuredContent`, `outputSchema`, `isError` tool failures, Streamable HTTP, clean stdio, elicitation capability checks, and untrusted annotations.
- MCP Inspector: scriptable contract probes over real transports; validate tool inventory, schemas, result envelopes, and representative errors in CI.
- OpenCode: concise low-level tool split; `apply_patch` with project-relative paths embedded in patch markers; capability groups for read/search/edit/bash/external-directory/MCP; per-agent permission overrides.
- Claude Code: deny-first permission layering; plan/read-only mode as a workflow concept; subagents with isolated context and scoped tools/MCP servers; hook-like pre/post checks; worktree isolation for parallel experiments; symlink target-aware path rules.
- Gemini CLI: root-directory model; diff preview before writes; checkpoint snapshots in a shadow git repo before mutating tools; structured shell results; PTY-gated interactivity; Docker/Podman/Seatbelt sandbox options; MCP include/exclude and trust flags as client-side controls.
- Aider: repo map as a future context-efficiency tool; strict edit formats as evidence for deterministic edits; `/diff`, `/undo`, `/test`, and `/lint` ergonomics; failed test output fed back into repair loops.
- SWE-agent: small custom ACI beats generic terminal-only interaction; paginated file viewing; concise search results; edit-time linting; explicit empty-output messages; benchmark discipline.
- mini-SWE-agent: Docker/Singularity environment abstraction, workers for SWE-bench smoke/regression, output caps with head/tail elision, and final patch verification.

### Designs Unsuitable Here

- A public `spawn_subagent`, `task`, or agent-team MCP tool. The project brief explicitly excludes subagent orchestration from P0, and permission/worktree/provenance semantics would be unclear.
- OpenCode's default-all-tools-enabled posture. This server may be connected to arbitrary MCP clients and must default to bounded read-only/local-safe behavior.
- Claude Code's `bypassPermissions` and opaque auto classifier as runtime policy. They are product-level conveniences, not deterministic server-side contract behavior.
- Gemini's model-assisted edit correction inside `apply_patch`. The runtime must be parseable, deterministic, and compliance-testable.
- Aider's auto-commit-on-edit behavior. This repo is explicitly multi-agent and dirty-worktree aware; the runtime should expose status/diff and never commit or revert unless separately requested by a user-level workflow.
- mini-SWE-agent's single unrestricted bash tool as the P0 interface. It is compact for benchmark agents but too powerful for a reusable local MCP runtime.
- Product-layer features such as memory, account/login, cloud task queues, web search/fetch, image generation, model routing, marketplace/plugin install, browser/computer-use agents, or high-level `codex(prompt)` wrappers.
- Client-only safety based on MCP annotations, `trust` flags, or UI prompts. The server must enforce workspace, path, environment, permission, sandbox, timeout, and output limits.

## Project-Specific Recommendations

1. Keep the current P0 tool list unchanged: `read_file`, `list_dir`, `list_files`, `search_text`, `apply_patch`, `exec_command`, `write_stdin`, `kill_session`, `git_status`, `git_diff`, and `request_permissions`. Treat `view_image` as P1 and keep product-layer tools absent.
2. Use workspace-relative public paths, canonicalize every path, reject `..`, reject absolute paths unless explicitly configured and still inside the root, and deny symlink escapes. This is stricter and better for a server contract than Gemini's common absolute-path examples.
3. Keep `search_text` backed by `rg` or an equivalent fast engine and return structured path, line, and preview records with truncation metadata. Avoid returning huge raw grep blobs.
4. Preserve Codex/OpenCode-style patch envelopes for `apply_patch`, including add/update/delete/move. Validate every affected path before edits and define atomic or rollback behavior for partial failures.
5. Split shell support into one-shot `exec_command`, managed long-running sessions, `write_stdin`, and `kill_session`. Return stdout/stderr separately, exit code/signal, timeout state, elapsed time, truncation flags, and opaque session IDs.
6. Do not expose raw background PIDs. `kill_session` should terminate only server-owned process groups.
7. Add Inspector-compatible checks to `make test-mcp-contract` when practical. Validate initialize, `tools/list`, tool schemas, output schemas, representative success calls, and representative structured error calls over Streamable HTTP.
8. Keep permission enforcement server-side. Use MCP annotations and client confirmations only as presentation hints. Default-deny network, destructive commands, credential access, and outside-workspace access.
9. Model `request_permissions` after MCP elicitation but keep the structured fallback for clients without elicitation. Permission grants should be scoped to operation, workspace, capability, timeout, and client/session where possible.
10. Borrow Gemini checkpointing only as optional internal preimage/rollback metadata, not as git commits. In a shared worktree, the safest default is status/diff visibility plus atomic patches.
11. Use subagents internally for dogfood and review with Claude/Gemini-style scoped tool access. Record MCP calls and reject direct filesystem/shell bypass, but do not expose subagent spawning through MCP.
12. Defer repo-map, symbol map, and LSP features to P1/P2. OpenCode and Aider show these are valuable, but the v0.1 contract should first remain small, deterministic, and compliance-complete.

## Risks

- Shell safety cannot be solved by command-string parsing. Compound commands, interpreters, package scripts, redirection, wrappers, and test runners can bypass prefix rules unless an OS/container sandbox and scrubbed environment enforce boundaries.
- Path safety can fail through symlinks, absolute paths, `..`, hardlinks, generated temp files, case-insensitive filesystems, and time-of-check/time-of-use races. The resolver must be shared by all tools.
- MCP annotations and client `trust` settings are not security controls. A malicious or buggy client can still call dangerous tools directly.
- Elicitation support is uneven across MCP clients. Permission-required operations must have deterministic non-elicitation behavior.
- Concurrent edits can race in a multi-agent worktree. `apply_patch` needs conflict detection, affected-file reporting, and clear failure/rollback semantics.
- Output growth can hide important failures or break clients. File read, search, shell, session, and diff outputs need hard caps and explicit truncation metadata.
- Auto-commit, undo, and restore designs from single-user agents can destroy user or peer-agent work if copied directly.
- Reference repos and benchmark fixtures can pollute project history if cloned outside `.reference/`; this pass avoided cloning.
- Future LSP/repo-map/browser/image extensions can pull the runtime toward product-agent scope unless they remain separate, feature-gated, and covered by compliance tests.

## Action Items

1. Keep compliance tests aligned with MCP 2025-06-18: initialize, `tools/list`, schemas, output schemas, structured success/error results, unknown-tool errors, clean stdio, and Streamable HTTP headers.
2. Add or maintain tests for traversal, absolute path denial, symlink escape, binary reads, search/list exclusions, patch path validation, patch atomicity, command timeout, output truncation, session write/kill, git status/diff caps, and no forbidden product tools.
3. Add Inspector or Inspector-like CLI probes to the contract gate once dependency policy is settled.
4. Document command policy in the same capability vocabulary used by implementation: workspace read, workspace write, read-only command, mutating command, network, destructive, credential access, and outside-workspace access.
5. For dogfood, use scoped subagents or an MCP-only runner that can access only this server's tools; log all tool calls and flag any direct filesystem/shell bypass.
6. For benchmark work, copy mini-SWE-agent's output discipline and environment abstraction, but compare native baseline vs MCP candidate through the same runner and do not claim SWE-bench pass without official harness evidence.
7. Keep P1/P2 ideas separated: `view_image`, repo/symbol map, LSP, stronger container sandboxing, and checkpoint/rollback metadata.
8. Revisit this report when MCP protocol version, Claude/Gemini/OpenCode subagent semantics, or Inspector CLI behavior changes, because those areas are actively evolving.
