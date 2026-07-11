# MCP Dogfood Runner

`mcp_deterministic_runner.py` is a deterministic MCP-only agent. It prepares a
temporary fixture workspace and may start a local MCP server, but the coding
loop itself uses only HTTP MCP calls:

1. `initialize`
2. `tools/list`
3. `tools/call` for read/search/patch/exec/stdin/diff

Example:

```bash
python benchmarks/dogfood/mcp_deterministic_runner.py \
  --endpoint http://127.0.0.1:8765/mcp \
  --server-command "coding-tools-mcp --workspace {workspace} --port 8765"
```

If no MCP server is reachable, the generated report is `INCONCLUSIVE` rather
than a claimed dogfood pass.
