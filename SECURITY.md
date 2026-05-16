# Security Policy

This project exposes local coding-runtime primitives over MCP. The security boundary is the configured workspace root plus the runtime sandbox and permission policy. Tools must be designed as if MCP clients are untrusted unless explicitly configured otherwise.

## Current Implementation Caution

The policy below is the required security posture. The current compliance suite covers workspace traversal, symlink escape, direct and interpreter-mediated outside reads, risky environment variables, network-looking commands, destructive commands, output caps, and session deadlines. Even so, the `exec_command` tool must not be treated as a hard sandbox boundary by itself. It launches host processes and still relies partly on command-string classification, which is not a substitute for process-level isolation.

Until command execution is backed by OS-level sandboxing and stronger permission enforcement, operators should expose the server only to trusted local clients, bind HTTP to loopback, and run the server inside an external container or sandbox with no host secrets, no broad filesystem mounts, and network egress disabled by policy.

## Workspace Boundary

The workspace root is the only filesystem area available to runtime tools.

- The server must canonicalize the configured workspace root before serving requests.
- The canonical root is immutable for the lifetime of a session.
- All file paths, command working directories, patch targets, search roots, and git helper paths must resolve inside the canonical workspace root.
- The baseline public tool contract should prefer workspace-relative paths. If absolute paths are supported in a future profile, they must be allowed only when their final resolved target remains inside the workspace.
- Path checks must compare path components after canonicalization, not raw string prefixes.

Unsafe roots such as `/`, a user home directory, system directories, or drive roots should be rejected unless the operator explicitly opts in.

## Path Traversal and Symlinks

All path-taking tools must use the same resolver.

The resolver must:

- Reject NUL bytes and malformed platform-specific path syntax.
- Normalize `.` and `..`.
- Resolve symlinks before final containment checks.
- For new files, canonicalize the nearest existing parent directory and validate the new basename under that parent.
- Reject targets outside the workspace.

Symlink policy:

- Reading through a symlink is allowed only when the final target remains inside the workspace.
- Writing through symlinks is denied by default.
- `apply_patch` may modify only regular files inside the workspace.
- Creating, deleting, or replacing symlinks is denied in the baseline policy.
- Broken symlinks may be listed as metadata but must not be opened as files.

## Command Execution

Commands must run under a constrained execution policy.

- Default cwd is the workspace root.
- Supplied cwd must resolve inside the workspace.
- Processes should run in a process group so timeouts and cancellations kill descendants.
- Non-interactive execution is the default.
- Interactive sessions require explicit session creation and bounded stdin/output.
- Shell-string execution is higher risk than structured argv and must be permission-gated. Command-string parsing is only a classifier; it is not an enforcement boundary.

Read-only local inspection commands may be allowed by default. Commands that modify files, install dependencies, contact networks, rewrite git history, or delete data require explicit permission.

Destructive commands such as `git reset --hard`, `git -C . reset --hard`, `git clean -fdx`, recursive deletion including workspace-relative targets, `find -delete`, force push, branch deletion, privilege escalation, and host service management must require explicit approval or be denied outright.

Commands must not be able to read outside-workspace files indirectly through interpreters or nested shells, for example by passing absolute paths inside `python -c`, `/bin/sh -c`, build scripts, or test runners. Enforce this with the runtime sandbox, not by attempting to enumerate every dangerous command string.

## Network and Permission Model

The default posture is deny by default for risky capabilities.

Baseline capability defaults:

| Capability | Default |
| --- | --- |
| Read files inside workspace | Allow |
| Write files inside workspace | Require grant |
| Read-only local commands | Allow with timeout |
| File-modifying commands | Require grant |
| Network access | Deny |
| Destructive operations | Require explicit approval |
| Credential access | Deny |
| Outside-workspace access | Deny |

Approvals should be operation-scoped unless the operator grants a time-limited broader policy. Audit logs should record the tool, redacted arguments, cwd, permission class, decision, timestamp, and client identity when available.

Package installation is both network access and filesystem mutation. Test runners can also execute arbitrary project code, so they should run with the same sandbox and timeout controls as other commands.

Network denial must be enforced below the process, for example by container, namespace, firewall, broker, or platform sandbox controls. Regex checks for `curl`, URLs, or common Python modules are useful only as early classification hints and must not be the only control. Loopback connections count as network access unless an operator grants them explicitly.

## Environment Scrubbing

Commands should receive an allowlisted environment rather than inheriting the server process environment.

Allowed variables should be minimal, for example `PATH`, locale variables, a sandbox-safe `HOME`, a controlled `TMPDIR`, and explicit operator-configured values.

The runtime should deny or redact:

- API keys and tokens.
- Cloud credentials.
- SSH agent and key paths.
- Package registry credentials.
- Git credential helper configuration.
- Proxy variables unless network access is allowed.
- Shell startup injection variables.
- Dynamic loader and interpreter path injection variables such as `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_*`, `BASH_ENV`, `ENV`, `PYTHONPATH`, `RUBYLIB`, `NODE_OPTIONS`, and similar language-specific hooks unless explicitly granted.

Secret redaction is defense in depth and must not be treated as the primary protection.

## Timeouts and Output Limits

Every operation must be bounded.

Recommended defaults:

| Operation | Default timeout | Hard max | Output cap |
| --- | ---: | ---: | ---: |
| File read | 5s | 30s | 1 MiB per file |
| Search | 15s | 60s | 512 KiB or 2,000 matches |
| Patch apply | 10s | 60s | 256 KiB |
| One-shot command | 30s | 10m | 1 MiB stdout + 1 MiB stderr |
| Interactive idle timeout | 10m | 60m | Bounded ring buffer |
| Git diff/status | 10s | 60s | 1 MiB |

Truncated responses must say they were truncated. Command timeouts must terminate the process group, return timeout status, and include bounded stdout/stderr captured before termination.

If a command returns a `session_id` before it exits, the original deadline still applies. Polling or stdin writes must enforce the stored deadline, and idle cleanup must not be the only timeout mechanism.

## Session Lifecycle

Persistent sessions must be explicitly managed.

- Session ids must be opaque.
- Sessions are bound to one workspace root and permission state.
- `write_stdin` requires a live session id.
- Stdin after exit, timeout, cancellation, or close must be rejected.
- Output buffers must be bounded and cursor-based or sequence-based.
- Idle sessions must be closed automatically.
- Server shutdown must terminate child process groups and remove runtime-owned temporary directories.
- Session deadlines, maximum lifetime, maximum count, and output ring-buffer sizes must be enforced server-side even when the client stops polling.

## HTTP Exposure

The HTTP transport is intended for local MCP clients.

- Default bind address must remain loopback.
- Binding to non-loopback interfaces requires an explicit operator decision and authentication.
- Origin checks are defense in depth for browsers; non-browser clients can omit `Origin`.
- Request body size, content type, and JSON depth should be bounded before parsing.
- The `Mcp-Session-Id` header is not an authentication secret unless paired with transport authentication.

## Reporting Security Issues

Until a public disclosure process is established, report security issues privately to the repository maintainers. Do not open public issues containing exploit details, secrets, or host-specific paths.

Include:

- Affected tool or policy area.
- Minimal reproduction steps.
- Expected and actual behavior.
- Whether the issue escapes the workspace, exposes credentials, permits network access, bypasses approval, or survives timeout/cancellation.

## Residual Risks

- Shell commands and test runners may execute arbitrary project code.
- Symlink race resistance depends on platform support for anchored and no-follow file operations.
- Network controls require real sandbox enforcement, not only command classification.
- Secret redaction can miss transformed or fragmented secrets.
- Cross-platform filesystem semantics differ and need dedicated compliance coverage.
