# Compliance

The one-command acceptance gate is:

```bash
make compliance
```

It runs:

- `make test-mcp-contract`
- `make test-tool-golden`
- `make test-security`
- `make test-e2e`
- `make test-codex-compat`
- `make dogfood-mcp`
- compliance report self-tests

## Current Result

Latest local report files:

- [reports/compliance/latest.json](reports/compliance/latest.json)
- [reports/compliance/latest.md](reports/compliance/latest.md)

Current status in `latest.json`:

- `passed`: `true`
- `tests_run`: `43`
- required tools, including `view_image`: all `passed`
- `security`: `passed`
- `e2e`: `passed`
- `codex_dogfood`: `passed`

There are no skipped tests in the current default profile.

## CI Evidence

GitHub Actions workflow:

- [.github/workflows/compliance.yml](.github/workflows/compliance.yml)

Latest verified run:

- https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/25957328972
- conclusion: `success`
- head SHA at run time: `16ab9ace68b1241f1f2a2b63a1b62c35102e95da`

## Coverage

The suite verifies:

- MCP initialize, tools/list, tools/call, schemas, structured success/failure output, unknown tool behavior, and stdout protocol cleanliness.
- Fresh-client `tools/list` discovery, stdio transport, unsupported HTTP protocol-version rejection, tool annotations, and mirrored structured/text tool results.
- Tool golden cases for read/list/search/patch/exec/stdin/kill/git status/git diff/image.
- Security cases for traversal, absolute paths, symlink escape, command workdir escape, direct and interpreter-mediated outside reads, destructive command policy, obfuscated network access, risky env rejection, session timeout enforcement, stdout JSON-RPC pollution, request-permission non-grants, and concurrent read-only calls.
- Deterministic E2E loops for JavaScript bugfix, Python function add, long-running stdin, session close behavior, workspace escape denial, and image viewing.
- Codex compatibility vectors for patch envelope, exec/session/stdin behavior, and image viewing.
- MCP-only dogfood without direct filesystem or shell bypass during task execution.
- Compliance report generation semantics.

## Running Individual Gates

```bash
make test-mcp-contract
make test-tool-golden
make test-security
make test-e2e
make test-codex-compat
make dogfood-mcp
make benchmark-smoke
```
