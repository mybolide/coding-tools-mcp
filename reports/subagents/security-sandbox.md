# Security Sandbox Architecture Report

## Task Scope

Audit the current Coding Tool Runtime MCP Server security posture without changing server implementation code. This pass covers:

- Workspace root handling and unsafe root rejection.
- Path traversal checks for read, list, search, patch, image, git, and command cwd inputs.
- Symlink escape handling for reads, recursive walks, and writes.
- Command execution policy, including shell use, destructive commands, and outside-workspace access.
- Network and environment permission controls.
- Timeout, session lifecycle, and output truncation behavior.
- Destructive command protections.
- Security-focused compliance coverage.

Allowed edits for this audit were limited to `SECURITY.md`, `reports/subagents/security-sandbox.md`, and security-focused tests in `tests/compliance/test_security.py`. No intentional server implementation edits were made by this audit.

## Sources Read/Referenced

- `CODEX_GOAL_MODE_MCP_RUNTIME_TASK.md`: task requirements for the security-sandbox subagent and required report sections.
- `SECURITY.md`: existing security policy and target posture.
- `SPEC.md`: public runtime profile and workspace model.
- `codex_tool_runtime_mcp/server.py`: `Workspace`, path resolution, `apply_patch`, `exec_command`, `write_stdin`, `kill_session`, git helpers, tool schemas, HTTP and stdio dispatch.
- `tests/compliance/test_security.py`: existing and newly added security regression coverage.
- `tests/compliance/test_tool_golden.py`: golden behavior for read/list/search/patch/exec/git/session tools.
- `tests/compliance/test_e2e.py`: deterministic coding-loop and workspace escape coverage.
- `tests/compliance/test_support.py`: denial assertion helpers and structured payload parsing.
- `tests/compliance/mcp_client.py`: server startup environment, fixture secret injection, and transport behavior.
- `tests/compliance/fixtures.py`: fixture materialization, outside secret, symlink escape fixture, and git setup.
- Local verification:
  - Baseline before adding new tests: `make test-security` passed 5 tests.
  - First run after adding new regressions: `make test-security` failed, confirming command policy, network policy, environment policy, and returned-session timeout gaps in the active runtime.
  - Latest no-report run after concurrent workspace changes: `PYTHONDONTWRITEBYTECODE=1 python3 -m tests.compliance.runner --suite security` ran 12 tests and failed 6 failure records. The remaining observed failures were interpreter-mediated outside reads, risky environment variables, and returned-session timeout enforcement. The destructive-command and `http.client` network regression cases passed in that latest run, but command/network controls are still pattern-based rather than sandbox-enforced.

## Key Findings

### Critical: `exec_command` Can Read Outside The Workspace

`Workspace.resolve_existing` and `resolve_for_write` provide a reasonable centralized path boundary for direct tool path inputs. Direct `read_file`, `apply_patch`, command `workdir`, and simple command path escapes are covered by existing tests.

The boundary does not hold for command execution. `exec_command` starts a normal host process with `shell=True` and only classifies the command string before launch. A command such as:

```text
python -c "from pathlib import Path; print(Path('/outside/path').read_text())"
```

is allowed because the absolute path is embedded inside interpreter source rather than exposed as a standalone shell token. The added regression `test_exec_command_rejects_interpreter_mediated_outside_reads` demonstrated leakage of the outside fixture secret.

This is the top security issue. Command parsing cannot prove that arbitrary shells, interpreters, build tools, or test runners stay inside the workspace.

### High: Network Denial Is Still Pattern-Based

The current network policy searches the raw command string for URLs and common tokens such as `curl`, `wget`, `socket`, `urllib.request`, and `requests.`. That blocks simple cases, but it is not enforcement.

The first added regression run showed that a Python `http.client.HTTPConnection` command executed instead of being denied. A later no-report run passed that specific regression after concurrent implementation changes expanded the pattern list. That is useful coverage, but the control is still a finite string classifier around a host process.

Network isolation needs a process-level control, not a string pattern.

### High: Destructive Command Protection Is Still A Classifier

Existing tests blocked `rm -rf /`, `git reset --hard`, and `chmod -R 777 /`. The first added regression run showed these destructive variants executing:

- `rm -rf src`
- `git -C . reset --hard`
- `find . -maxdepth 1 -type f -delete`

A later no-report run passed those specific cases after concurrent implementation changes. The remaining risk is architectural: destructive protection still relies on recognizing command text before launching a shell. Interpreter-mediated deletion, build scripts, package scripts, and alternate command spellings remain in scope unless the process runs inside a sandbox and write/destructive grants are enforced below the shell.

### High: Returned Sessions Outlive Their Timeout

`exec_command` calculates a deadline before spawning, but once it returns a running session, that deadline is not stored on the `ExecSession` and is not enforced by `write_stdin`. A command launched with `timeout_ms=100` and `yield_time_ms=0` remains `running` after the deadline.

The added regression `test_exec_command_timeout_is_enforced_after_running_session_is_returned` confirmed this behavior. This creates resource exhaustion risk and undermines timeout guarantees for long-running commands and `tty=True` sessions.

### High: Environment Scrubbing Allows Risky User-Supplied Variables

The inherited server environment is mostly allowlisted, and fixture secrets such as `AWS_SECRET_ACCESS_KEY` are not leaked to child processes. However, user-supplied `env` entries are accepted unless their key matches the sensitive-name regex.

The added regression `test_exec_command_rejects_shell_startup_and_loader_environment` confirmed that `BASH_ENV`, `ENV`, `LD_PRELOAD`, and `PYTHONPATH` are accepted. These variables can alter shell startup, dynamic loading, or interpreter import behavior and should be denied by default or require an explicit grant.

### High: Output Buffers Are Not Bounded Internally

Tool responses apply `max_output_bytes` at snapshot time, but `ExecSession.stdout` and `ExecSession.stderr` are unbounded `bytearray` buffers. A process that writes continuously can consume server memory even when each response is truncated.

The policy should require per-session ring buffers, dropped-byte accounting, max session count, and cleanup for orphaned or idle sessions.

### Medium: Workspace Path Resolver Is Strong For Direct Tool Inputs But Still Has Gaps

Positive observations:

- The workspace root is canonicalized.
- `/` and the current home directory are rejected as roots.
- Direct absolute tool paths are denied in the current profile.
- `..` path components are rejected.
- Existing paths resolve symlinks before containment checks.
- New write targets validate the nearest existing parent.
- `apply_patch` rejects writes through final-path symlinks.
- Recursive listing/search do not follow unsafe symlinked files.

Remaining gaps:

- Other unsafe roots such as `/tmp`, `/var`, `/etc`, or drive roots are not fully rejected.
- Files are opened after path validation with ordinary path operations, leaving symlink race exposure for attackers who can mutate the workspace concurrently.
- `list_dir` may expose raw symlink target metadata, including host absolute paths.
- Future support for absolute paths inside the workspace would need careful component-based containment checks; the current implementation simply rejects them.

### Medium: Request Limits Are Mostly Schema Hints, Not Hard Server Validation

Tool schemas define maximums, but `tools/call` does not validate input schemas before dispatch. Handlers cast values directly from `args`, so malicious clients can request very large file reads, search result counts, output sizes, or JSON bodies unless each handler clamps them independently.

Additional issues:

- `read_file` reads the whole file before applying `max_bytes`.
- `search_text` reads whole files and can run unbounded regular expressions.
- HTTP request body size and JSON depth are not bounded before parsing.

### Medium: HTTP Exposure Depends On Operator Discipline

The default host is loopback, which is good. If an operator binds a non-loopback host, the server has no authentication. Origin checks help against some browser contexts, but non-browser clients can omit `Origin`.

The HTTP transport should remain local by default, and non-loopback binding should require explicit authentication and request size limits.

## Concrete Recommendations

1. Treat `exec_command` as unsafe until it runs inside a real sandbox.

   Use container, namespace, chroot, Landlock, seccomp, pledge/unveil, job-object, or equivalent platform controls. Mount only the workspace and runtime temp directory. Deny access to host home, cloud credentials, SSH material, package credentials, and sibling directories.

2. Enforce network denial below the process.

   Disable egress with network namespaces, firewall rules, container policy, or a broker. Treat loopback as network unless explicitly granted. Keep regex classification only for early permission prompts.

3. Replace command-string safety assumptions with permission classes.

   Prefer structured argv for allowed built-ins. Put all shell-string execution behind a permission gate. Require explicit grants for file-modifying commands, package installs, long-running servers, git history mutation, and destructive operations.

4. Block destructive workspace mutations by default.

   Deny or require grants for `rm -r`, `find -delete`, `git -C ... reset --hard`, `git clean`, checkout/restore discards, branch deletion, force push, recursive chmod/chown, and equivalent interpreter-mediated deletion. The sandbox should prevent outside-workspace deletion even if classification misses a case.

5. Harden environment handling.

   Start from a fixed allowlist and reject risky user-supplied variables by default. Deny `BASH_ENV`, `ENV`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_*`, `PYTHONPATH`, `NODE_OPTIONS`, package registry auth variables, proxy variables without network permission, and language-specific loader hooks unless an explicit policy grants them.

6. Store and enforce session deadlines.

   Add deadline, idle deadline, max lifetime, and permission state to `ExecSession`. Enforce them in `write_stdin`, polling, and a background reaper. Kill the process group when a deadline expires even if the client stops polling.

7. Bound session output in memory.

   Replace unbounded bytearrays with per-stream ring buffers. Report dropped bytes and truncation. Enforce max sessions per client and globally.

8. Enforce schemas and hard clamps server-side.

   Validate `tools/call` arguments against input schemas or duplicate hard clamps inside handlers. Clamp `max_bytes`, `max_results`, `max_entries`, `timeout_ms`, `yield_time_ms`, and request body sizes regardless of client behavior.

9. Make file operations race-resistant.

   Use anchored and no-follow filesystem operations where available. Re-check opened file metadata before reading or writing sensitive paths. Keep `apply_patch` transactional, but do not rely only on pre-open canonicalization.

10. Harden HTTP exposure.

    Keep loopback as the default. Require authentication and explicit operator opt-in for non-loopback binds. Add content-length, content-type, and JSON-depth limits. Do not treat `Mcp-Session-Id` as authentication.

## Risks

- Current `exec_command` can escape the intended workspace boundary through ordinary interpreter code. Do not expose it to untrusted MCP clients in its current form.
- Current network denial and destructive-command controls are bypassable because they rely on raw command string matching.
- Current returned sessions can exceed their configured timeout and can buffer unbounded output in server memory.
- The new security regressions intentionally fail until implementation hardening lands.
- Real sandboxing is platform-dependent and needs Linux, macOS, and Windows-specific design.
- Symlink race resistance cannot be solved completely with `Path.resolve()` checks alone.
- Test runners and package tools execute arbitrary project code and must be treated as command execution, not as safe read-only inspection.
- Existing worktree modifications outside this audit's allowed files were observed and were not reverted or edited by this pass.

## Action Items

1. Make the added security regressions pass:
   - `test_exec_command_rejects_interpreter_mediated_outside_reads`
   - `test_exec_command_rejects_destructive_workspace_mutations`
   - `test_exec_command_rejects_obfuscated_network_access`
   - `test_exec_command_rejects_shell_startup_and_loader_environment`
   - `test_exec_command_timeout_is_enforced_after_running_session_is_returned`

2. Implement an execution sandbox before advertising `exec_command` to untrusted clients.

3. Add an explicit permission state model and make `permission_grant_id` meaningful, or remove it from the schema until grants are implemented.

4. Add server-side schema validation and hard request clamps.

5. Add session deadline storage, a session reaper, process-group cleanup on server shutdown, max session counts, and bounded ring buffers.

6. Expand path tests for broken symlinks, symlinked parents, root prefix confusion, unsafe roots such as `/tmp`, and concurrent symlink swaps.

7. Add tests for HTTP request body limits, non-loopback binding behavior, and Origin/header assumptions.

8. Re-run the full `make compliance` gate only after security hardening is implemented. The current expanded security suite is expected to fail against the present server implementation.
