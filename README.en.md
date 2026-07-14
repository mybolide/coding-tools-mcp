# Coding Tools MCP Desktop

[中文](README.md) | [English](README.en.md)

Coding Tools MCP Desktop is a local AI coding workspace built with Rust and
Tauri 2. It exposes project files, command execution, and Git capabilities
through MCP to ChatGPT, Codex, and other MCP clients.

The Rust implementation uses the Python implementation in `old/` as its
behavioral reference. The core workflow is:

```text
Open a workspace
    ↓
Read the code
    ↓
Modify files
    ↓
Run commands and tests
    ↓
Inspect Git status and diffs
```

## Current capabilities

- Rust + Tauri 2 desktop client
- MCP Streamable HTTP server
- ChatGPT Actions OpenAPI gateway
- OAuth, Bearer Token, and no-auth modes
- Workspace default directory and subdirectory switching
- File reading, directory browsing, text search, and image viewing
- Atomic patches, patch preflight checks, and structured change results
- Workspace command execution with TTY, stdin, timeouts, and paginated output
- Git status, diff, log, show, and blame
- FRP and Cloudflare tunnel management
- Global FRP profiles and workspace configuration

## Default tool set

The default `core` profile exposes the stable core tools compatible with the
Python reference implementation:

| Category | Tools |
| --- | --- |
| File reading | `read_file`, `list_dir`, `list_files`, `search_text`, `view_image` |
| File modification | `apply_patch`, `patch_check` |
| Command execution | `exec_command`, `write_stdin`, `read_output`, `kill_session` |
| Git | `git_status`, `git_diff`, `git_log`, `git_show`, `git_blame` |
| Environment | `server_info`, `check_exec_environment`, `get_default_cwd`, `set_default_cwd` |
| Permissions | `request_permissions` |

Advanced Harness, task-state, and operation-history tools remain in the code
but are not part of the default tool surface. Use the explicit `advanced`
profile when they are needed.

## Permission boundaries

- Normal files inside the Workspace can be read, created, modified, deleted,
  and executed.
- Outside the Workspace, `read_file`, `list_dir`, `list_files`, `search_text`,
  and `view_image` provide read-only access.
- `.git` and `.github` cannot be written by ordinary file tools or Patch.
- Git is the single source of truth for recovery; Workspace snapshots and
  tool-level Undo are not used.
- `exec_command.filesystem_scope` defaults to and currently accepts only
  `workspace`.

Windows OS-level filesystem isolation for child processes is still under
development. `sandbox_enforced: false` is the honest current status; static
command checks must not be treated as a complete sandbox.

## Requirements

- Rust stable (2021 edition)
- Node.js 20+
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for
  your operating system

## Development

Install frontend dependencies:

```bash
npm install
```

Start the desktop development environment:

```bash
npm run desktop
```

On Windows, you can also run:

```text
dev-desktop.cmd
```

Other useful commands:

```bash
npm run check
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

Do not use `npm run dev` alone to validate the desktop application. It starts
Vite without the Tauri shell.

Default ports:

- MCP: `28766`
- Actions: `8787`
- Vite: `1420`

## Project layout

| Path | Purpose |
| --- | --- |
| `src-tauri/src/tools/` | Shared tool kernel and `call_tool` |
| `src-tauri/src/mcp/` | MCP HTTP server |
| `src-tauri/src/actions/` | Actions OpenAPI gateway |
| `src-tauri/src/tunnel/` | FRP / Cloudflare tunnel management |
| `src-tauri/src/settings/` | Global settings and FRP profiles |
| `src/` | SvelteKit UI |
| `old/` | Python reference implementation and compatibility tests |

## ChatGPT Actions

1. Start the Actions service for the target Workspace.
2. Configure an FRP or Cloudflare tunnel.
3. Copy the OpenAPI URL from the Workspace Actions authentication page.
4. Import the OpenAPI document in the GPT editor.
5. Select None, API Key, or OAuth authentication as appropriate.

## Legacy migration

When the new profile store is empty on first launch, the application attempts
to import the legacy configuration from:

```text
~/.coding-tools-mcp-desktop/profiles.json
```

Secrets from `secrets.json` are migrated to the operating system keyring.

## License

Apache-2.0
