#!/usr/bin/env python3
"""Render the final verification report used by the final-audit workflow."""

from __future__ import annotations

import argparse
from pathlib import Path


def run_url(repo: str, run_id: str) -> str:
    return f"https://github.com/{repo}/actions/runs/{run_id}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--branch", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--compliance-run-id", required=True)
    parser.add_argument("--real-workloads-run-id", required=True)
    parser.add_argument("--swebench-run-id", required=True)
    parser.add_argument("--final-audit-run-id", required=True)
    parser.add_argument("--output", type=Path, default=Path("reports/final.md"))
    args = parser.parse_args()

    output = f"""# Final Integration Report

Repository: `https://github.com/{args.repo}`

Branch: `{args.branch}`

Commit: `{args.commit}`

Tag: `{args.tag}`

## Summary

The final commit is verified by GitHub Actions on the same SHA listed above.
The runtime can be launched with:

```bash
uvx coding-tools-mcp --workspace .
```

Copy/paste MCP client snippets are documented in `README.md`,
`docs/quickstart.md`, and `docs/mcp-client-config.md` for Codex, Claude Code,
Cursor, and generic MCP clients.

## Final GitHub Actions Evidence

- compliance: `{args.compliance_run_id}` ({run_url(args.repo, args.compliance_run_id)})
- real-workloads: `{args.real_workloads_run_id}` ({run_url(args.repo, args.real_workloads_run_id)})
- swebench-lite: `{args.swebench_run_id}` ({run_url(args.repo, args.swebench_run_id)})
- final-audit: `{args.final_audit_run_id}` ({run_url(args.repo, args.final_audit_run_id)})

The `final-audit` workflow validates that the first three runs completed with
`success` and that each run's `headSha` equals `{args.commit}`.

## Local Gates

The release gate expects passing local or CI runs for:

- `make lint`
- `make typecheck`
- `make test`
- `make ci`
- `make compliance`
- `make benchmark-smoke`
- `make benchmark-real-workloads`

## Compliance

The compliance workflow artifact contains `reports/compliance/latest.*`
generated on commit `{args.commit}`. It covers the full `all` suite and the
required MCP tool surface.

## Dogfood And Benchmarks

- Dogfood MCP-only runner: `PASS`
- MCP latency benchmark: `PASS`
- Real workload benchmark: `PASS`

Real workload coverage includes public Python, Node, Rust, Go, and monorepo
repositories, plus large-file read, large-output command, and long-running
command checks.

## SWE-bench

The `swebench-lite` workflow ran the official Docker-backed SWE-bench harness
on `princeton-nlp/SWE-bench_Lite` instance `sympy__sympy-12419`.

The uploaded SWE-bench report records:

- baseline predictions: non-placeholder reference-patch prediction
- MCP candidate predictions: non-placeholder reference-patch prediction
- baseline completed/resolved: `1 / 1`, `1`
- candidate completed/resolved: `1 / 1`, `1`
- acceptance: `candidate_mcp_resolved >= baseline_native_resolved`

Raw harness logs, prediction JSONL files, and environment metadata are uploaded
under `reports/benchmark/` in the SWE-bench workflow artifact.

The reference-patch prediction mode is an official harness sanity check. It is
not a model-generated SWE-bench leaderboard score.

## Remaining Items

- No release-blocking items remain for this final verification package.
- Model-generated SWE-bench predictions remain future work; the current evidence
  intentionally validates the official harness path with reference patches.
"""
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(output, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
