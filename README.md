# Codex Tool Runtime MCP

Codex Tool Runtime MCP is a model-neutral coding-agent runtime MCP server. It exposes local coding primitives to any MCP client:

```text
inspect repo -> search/read files -> apply structured patches -> run tests/commands
-> interact with stdin sessions -> inspect git status/diff
```

It is not a `codex(prompt)` wrapper. It does not expose Codex accounts, memory, cloud tasks, web search, image generation, model routing, plugin marketplace, or subagent orchestration as MCP tools.

## Install

```bash
python -m pip install -e .
```

The package installs:

```bash
codex-tool-runtime-mcp
```

## Start

Streamable HTTP, the P0 transport:

```bash
codex-tool-runtime-mcp --workspace /path/to/repo --host 127.0.0.1 --port 8765
```

Environment variable form:

```bash
CODEX_TOOL_RUNTIME_WORKSPACE=/path/to/repo codex-tool-runtime-mcp
```

Stdio, implemented as P1:

```bash
codex-tool-runtime-mcp --stdio --workspace /path/to/repo
```

Image viewing is enabled by default. To disable it for a constrained deployment:

```bash
CODEX_TOOL_RUNTIME_ENABLE_VIEW_IMAGE=0 codex-tool-runtime-mcp --workspace /path/to/repo
```

Logs go to stderr. JSON-RPC protocol output is kept clean.

## MCP Client Examples

### Codex

```toml
[mcp_servers.codex_tool_runtime]
command = "codex-tool-runtime-mcp"
args = ["--stdio", "--workspace", "/path/to/repo"]
```

### Claude Code

```json
{
  "mcpServers": {
    "codex-tool-runtime": {
      "command": "codex-tool-runtime-mcp",
      "args": ["--stdio", "--workspace", "/path/to/repo"]
    }
  }
}
```

### Generic Streamable HTTP Client

Point the MCP client at:

```text
http://127.0.0.1:8765/mcp
```

Use MCP protocol version `2025-06-18`.

## Tools

P0 tools exposed by default:

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

Additional image tool exposed by default:

- `view_image`

## Safety Boundary

The runtime binds one workspace root per server process. Paths are workspace-relative by default. Absolute paths, `..` traversal, and symlink escapes are rejected. Recursive listing/search excludes `.git`, `.reference`, `node_modules`, `target`, `dist`, build outputs, virtualenvs, and common caches by default.

`exec_command` runs under policy controls with workspace-bound cwd, timeout, output caps, sensitive and loader/startup environment rejection, destructive command checks, network-looking command checks, indirect absolute-path checks, and session deadline enforcement. This is still not an OS/container sandbox; see [SECURITY.md](SECURITY.md).

## Compliance

```bash
make compliance
```

Current local result:

- report: [reports/compliance/latest.md](reports/compliance/latest.md)
- JSON: [reports/compliance/latest.json](reports/compliance/latest.json)
- status: `passed=true`
- tests: 44 run, 44 passed, 0 skips

GitHub Actions also runs compliance:

- latest verified run for the previous pushed commit: https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/25957328972

## Dogfood And Benchmark

Dogfood report:

- [reports/dogfood/codex-on-mcp.md](reports/dogfood/codex-on-mcp.md)
- conclusion: `PASS`

SWE-bench smoke/regression report:

- [reports/benchmark/swebench-regression.md](reports/benchmark/swebench-regression.md)
- conclusion: `INCONCLUSIVE`

The SWE-bench official harness was not run in this container because Docker and the `swebench` package are missing. Placeholder predictions and the smoke subset are checked in so the official command path is reproducible once infrastructure is available.

## Development Commands

```bash
make test-mcp-contract
make test-tool-golden
make test-security
make test-e2e
make test-codex-compat
make dogfood-mcp
make benchmark-smoke
make report
```
