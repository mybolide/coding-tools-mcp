# Coding Tools MCP Desktop

Rust + Tauri 2 desktop client for local coding tools exposed via **MCP** (Streamable HTTP) and **ChatGPT Actions** (OpenAPI gateway). Both surfaces share a single `tools::call_tool` implementation aligned with the Python reference in `old/`.

## Features

- Workspace management with keyring-backed secrets
- Embedded MCP listener (`/mcp`) with OAuth / Bearer / no-auth
- Embedded Actions gateway (`/openapi.json`, `/actions/{tool}`)
- FRP + Cloudflare tunnel supervision (auto `frpc` / `cloudflared`)
- Global FRP profiles + per-workspace subdomains
- Health checks, log viewer, tool profiles & permission modes

## Prerequisites

- Rust stable (2021 edition)
- Node.js 20+
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

## Development

```bash
# 1) Install frontend deps (first time only)
npm install

# 2) Start desktop app — pick ONE:
npm run desktop          # recommended
npm start                # alias
cargo tauri dev          # needs: cargo install tauri-cli --locked (already done on this machine)
dev-desktop.cmd          # Windows double-click / cmd

# Type-check Svelte
npm run check

# Rust tests
cd src-tauri && cargo test
```

> **Why `cargo tauri dev` failed before:** Rust does not ship Tauri CLI. Install once with `cargo install tauri-cli --locked`, or use `npm run desktop` (uses `@tauri-apps/cli` from `node_modules`).

> **Port 1420 in use:** Stop the previous `tauri dev` window/terminal, or kill the process holding port 1420, then retry.

Do **not** run `npm run dev` alone — that only starts Vite without the desktop shell.

Default ports: **MCP 28766**, **Actions 8787**.

## Project layout

| Path | Purpose |
|------|---------|
| `src-tauri/src/tools/` | Shared tool kernel (`call_tool`) |
| `src-tauri/src/mcp/` | MCP HTTP server |
| `src-tauri/src/actions/` | Actions OpenAPI gateway |
| `src-tauri/src/tunnel/` | FRP / Cloudflare supervisors |
| `src-tauri/src/settings/` | Global app settings (FRP profiles) |
| `src/` | SvelteKit UI |
| `old/` | Python reference implementation |

## GPT Actions setup

1. Start Actions service for a workspace
2. Configure tunnel (FRP global profile + subdomain, or Cloudflare)
3. Open workspace → **Actions 认证** → copy OpenAPI URL into GPT editor → Actions → Import from URL
4. Set auth: None / API Key / OAuth per [OpenAI Help Center](https://help.openai.com/en/articles/9442513)

## Legacy migration

On first launch, if the new profile store is empty, workspaces are imported from:

`~/.coding-tools-mcp-desktop/profiles.json`

Secrets from `secrets.json` are migrated into the OS keyring.

## License

Apache-2.0
