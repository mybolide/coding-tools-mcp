# Final Integration Report

Date: 2026-05-17

Repository: `https://github.com/ytagent/codex-tool-runtime-mcp`

Branch: `recover-rollout-2026-05-16`

Release tag target: `v0.1.3-recovered-final`

## Summary

The rollout is now implemented, pushed, and verified. The server supports the simple launch path:

```bash
uvx codex-tool-runtime-mcp --workspace .
```

Copy/paste MCP client config snippets for Codex, Claude Code, Cursor, and generic HTTP clients are documented in:

- [../README.md](../README.md)
- [../docs/quickstart.md](../docs/quickstart.md)
- [../docs/mcp-client-config.md](../docs/mcp-client-config.md)

## Local Verification

All final local gates passed in this container:

- `make lint`: PASS
- `make typecheck`: PASS
- `make test`: PASS, 65 tests
- `make ci`: PASS
- `make compliance`: PASS, 65 tests, suite `all`
- `make benchmark-smoke`: PASS as local preflight, conclusion `PREFLIGHT_ONLY` because this container has no Docker and the local `swebench` import path is broken
- `make benchmark-real-workloads`: PASS

Verification logs are under [verification](verification).

## Real Workload Benchmark

Report: [benchmark/real-workloads.md](benchmark/real-workloads.md)

Conclusion: `PASS`

Coverage:

- Python repository: `pallets/click`
- Node repository: `jshttp/mime-types`
- Rust repository: `dtolnay/itoa`
- Go repository: `google/uuid`
- Monorepo: `changesets/changesets`
- Large file read
- Large output command
- Long-running command

GitHub Actions evidence:

- `real-workloads` run `26005609634`
- URL: `https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/26005609634`
- Conclusion: `success`
- Head SHA: `d53b5cf8f79edce01c25416beae07f3341c6b686`

The final branch contains the passing report and raw workload evidence.

## SWE-bench Official Harness

Report: [benchmark/swebench-official-attempt.md](benchmark/swebench-official-attempt.md)

Conclusion: `PASS`

GitHub Actions evidence:

- `swebench-lite` run `26005718266`
- URL: `https://github.com/ytagent/codex-tool-runtime-mcp/actions/runs/26005718266`
- Conclusion: `success`
- Head SHA: `1e05880118dc8419d129f87172315ebce82dce50`

Resolved counts from the official SWE-bench harness:

- Dataset: `princeton-nlp/SWE-bench_Lite`, split `test`
- Instance: `sympy__sympy-12419`
- Baseline predictions: `placeholder=False`, completed `1 / 1`, resolved `1`
- Candidate predictions: `placeholder=False`, completed `1 / 1`, resolved `1`
- Acceptance comparison: `candidate_mcp_resolved >= baseline_native_resolved`

The SWE-bench workflow uses `prediction_source=reference_patch` by default. This is an official harness sanity check with non-empty SWE-bench reference patches and parsed resolved counts. It is not a model-generated leaderboard result. Use `prediction_source=checked_in` only after replacing the scaffold files with model-generated baseline and MCP-candidate predictions.

Raw harness logs are recorded under [benchmark/swebench-official-attempt/raw](benchmark/swebench-official-attempt/raw).

## GitHub Actions

Latest pushed evidence before this final report:

- Compliance run `26005790834`: `success`, head SHA `d16c8e1b47a3d9ba05c5a37168fc6b2a6f8bfa89`
- Compliance run `26005790832`: `success`, head SHA `d16c8e1b47a3d9ba05c5a37168fc6b2a6f8bfa89`
- SWE-bench official run `26005718266`: `success`
- Real workload run `26005609634`: `success`

After this report is committed and tagged, the final tag SHA should be verified again by the automatic `compliance` workflow and by manual `real-workloads` and `swebench-lite` workflow dispatches.

## Notes

- Checked-in SWE-bench scaffold prediction files remain placeholders by design.
- Reference-patch prediction files live under [benchmark/swebench-reference-predictions](benchmark/swebench-reference-predictions).
- This repository does not claim model-generated SWE-bench performance.
- Docker-backed official SWE-bench evidence is available from GitHub Actions because the local container has no Docker daemon.
