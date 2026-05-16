# Final Report

## Summary

This repository implements a Codex-style coding runtime MCP server for workspace-confined coding primitives. It is not a Codex product wrapper.

Repository:

- https://github.com/ytagent/codex-tool-runtime-mcp

Current pushed evidence before this report update:

- Latest verified CI commit: `16ab9ace68b1241f1f2a2b63a1b62c35102e95da`
- CI run: https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/25957328972
- CI conclusion: `success`
- Release tag: `v0.1.0`

Current local verification after this report update:

- Implementation/test/docs commit verified by `make compliance`: `0f8534df65c0797544421e47974c4df14176df81`
- Compliance report refresh commit: `c85398aa5e833e443fbd93fdd3e66fcb2d3c8c09`
- Release tag for this iteration: `v0.1.1`
- `make compliance`: `44` tests run, `44` passed, `0` skipped.
- Required tools, including `view_image`: all passed.
- The final assistant response records the pushed tag target after push.

## Implemented Tools

Default P0 tools:

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

Additional tool:

- `stdio` transport implemented.
- `view_image` implemented and enabled by default; set `CODEX_TOOL_RUNTIME_ENABLE_VIEW_IMAGE=0` to disable it.

Forbidden product-layer tools are not exposed.

## Subagents

Completed subagent reports:

- `codex-internals-researcher`: [reports/subagents/codex-internals-research.md](subagents/codex-internals-research.md)
- `competitor-researcher`: [reports/subagents/competitor-research.md](subagents/competitor-research.md)
- `mcp-contract-architect`: [reports/subagents/mcp-contract.md](subagents/mcp-contract.md)
- `security-sandbox-architect`: [reports/subagents/security-sandbox.md](subagents/security-sandbox.md)
- `test-harness-engineer`: [reports/subagents/test-harness.md](subagents/test-harness.md)
- `implementation-engineer`: [reports/subagents/implementation.md](subagents/implementation.md)
- `benchmark-engineer`: [reports/subagents/benchmark.md](subagents/benchmark.md)
- `release-docs-engineer`: [reports/subagents/release-docs.md](subagents/release-docs.md)
- CI verification: [reports/subagents/ci.md](subagents/ci.md)

Several later GPT-5.5 xhigh subagents requested by the user were attempted for CI, compliance verification, dogfood/benchmark verification, release docs, and review. The platform returned usage-limit errors before they could produce artifacts; this is recorded in the relevant reports.

## Compliance

Command:

```bash
make compliance
```

Evidence:

- [reports/compliance/latest.json](compliance/latest.json)
- [reports/compliance/latest.md](compliance/latest.md)
- status: `passed=true`
- tests: 44 run, 44 passed, 0 skips
- required tools: all passed, including `view_image`
- security: passed
- e2e: passed
- Codex dogfood: passed

GitHub Actions:

- https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/25957328972
- conclusion: `success`

## Dogfood

Evidence:

- [reports/dogfood/codex-on-mcp.md](dogfood/codex-on-mcp.md)
- [reports/dogfood/codex-on-mcp.json](dogfood/codex-on-mcp.json)
- conclusion: `PASS`

The deterministic MCP-only runner used MCP calls for search/read, patch, command execution, stdin sessions, and diff inspection. It reports no direct filesystem or shell bypass during task execution.

## Benchmark

Evidence:

- [reports/benchmark/swebench-regression.md](benchmark/swebench-regression.md)
- [reports/benchmark/swebench-regression.json](benchmark/swebench-regression.json)
- subset: [benchmarks/swebench/subsets/smoke-lite-10.json](../benchmarks/swebench/subsets/smoke-lite-10.json)
- predictions:
  - [benchmarks/swebench/predictions/baseline_native.jsonl](../benchmarks/swebench/predictions/baseline_native.jsonl)
  - [benchmarks/swebench/predictions/candidate_mcp.jsonl](../benchmarks/swebench/predictions/candidate_mcp.jsonl)

Conclusion:

```text
INCONCLUSIVE
```

The official SWE-bench harness was not run because Docker and the `swebench` Python package are unavailable in this environment, and the checked-in prediction files are placeholders. The project does not claim SWE-bench PASS.

## Security Limitations

- Command safety is policy-based, not a full OS/container sandbox.
- Network-looking commands and destructive command patterns are blocked or permission-required, but shell string policy is not a complete isolation boundary.
- `request_permissions` returns `ELICITATION_UNSUPPORTED` unless future client approval integration is added.
- Workspace escape is not grantable.
- `view_image` is enabled by default and remains workspace-confined.

## Prompt-To-Artifact Checklist

| Requirement | Evidence |
| --- | --- |
| GitHub repo created and pushed | https://github.com/ytagent/codex-tool-runtime-mcp |
| Frequent commits/pushes | git history includes initialization, security, research/tests, implementation, hardening, CI, reports |
| Subagents used | reports under `reports/subagents/` |
| Reference research covers Codex and competitors | `reports/subagents/codex-internals-research.md`, `competitor-research.md`, `docs/research/reference-review.md` |
| MCP profile written | `docs/profile-v0.1.md`, `docs/profile.md` |
| P0 tools implemented | `codex_tool_runtime_mcp/server.py`, `tools/list` compliance tests |
| Forbidden product tools absent | `test_tools_list_excludes_forbidden_product_layer_tools` |
| `make compliance` passes | `reports/compliance/latest.json`, local 44-test run, CI run `25957328972` |
| Dogfood complete | `reports/dogfood/codex-on-mcp.md` |
| SWE-bench/benchmark report complete | `reports/benchmark/swebench-regression.md`, conclusion INCONCLUSIVE |
| Docs complete | `README.md`, `SPEC.md`, `COMPLIANCE.md`, `SECURITY.md`, `BENCHMARK.md`, this report |
| CI verification | `.github/workflows/compliance.yml`, run `25957328972` |

## Follow-Up Roadmap

1. Add an OS/container sandbox backend for `exec_command`.
2. Integrate MCP elicitation for permission grants.
3. Replace SWE-bench placeholder predictions with real native and MCP candidate runs.
4. Run official SWE-bench Lite smoke with Docker and `swebench`.
5. Expand Codex `apply_patch` compatibility vectors.
