# MCP Runtime Latency Benchmark

- Conclusion: **PASS**
- Endpoint: `http://127.0.0.1:54941/mcp`
- Iterations: `8`
- Exec iterations: `4`
- Warmup iterations: `2`
- Max MCP p95 threshold: `5000 ms`

## Metrics

| metric | samples | min ms | p50 ms | p95 ms | max ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| `mcp.tools_list` | 8 | 1.028 | 1.074 | 1.128 | 1.129 |
| `mcp.read_file` | 8 | 0.651 | 0.734 | 0.764 | 0.769 |
| `mcp.search_text` | 8 | 57.391 | 60.186 | 63.788 | 63.811 |
| `mcp.exec_command` | 4 | 46.139 | 46.632 | 46.678 | 46.68 |
| `native.read_text` | 8 | 0.028 | 0.043 | 0.06 | 0.066 |
| `native.search` | 8 | 4.034 | 4.063 | 4.167 | 4.17 |
| `native.exec_python` | 4 | 23.657 | 23.702 | 23.871 | 23.898 |

## Native Baseline Comparison

| operation | MCP p95 ms | native p95 ms | ratio |
| --- | ---: | ---: | ---: |
| `read_file` | 0.764 | 0.06 | 12.733 |
| `search_text` | 63.788 | 4.167 | 15.308 |
| `exec_command` | 46.678 | 23.871 | 1.955 |

## Failures

No failures recorded.

## Notes

- Native baselines are local developer-tool primitives, not equivalent MCP substitutes.
- Latency thresholds are intentionally broad; this smoke benchmark catches transport regressions and records trend evidence.
