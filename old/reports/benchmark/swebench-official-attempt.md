# SWE-bench Smoke Regression Report

- Conclusion: **PASS**
- Dataset: `princeton-nlp/SWE-bench_Lite` split `test`
- Smoke subset: `/home/runner/work/codex-tool-runtime-mcp/codex-tool-runtime-mcp/benchmarks/swebench/subsets/smoke-lite-10.json`
- Raw log directory: `reports/benchmark/swebench-official-attempt/raw`
- Baseline predictions: `reports/benchmark/swebench-reference-predictions/baseline_reference.jsonl`
- Candidate predictions: `reports/benchmark/swebench-reference-predictions/candidate_reference.jsonl`
- Baseline resolved: `1`
- Candidate resolved: `1`
- Baseline completed: `1` / `1`
- Candidate completed: `1` / `1`

## Preflight

- docker: ok - docker version succeeded
- swebench package: ok - swebench harness help succeeded
- baseline predictions: 1 rows, placeholder=False
- candidate predictions: 1 rows, placeholder=False

## Instances

- `sympy__sympy-12419` (sympy)

## Evaluation Commands

```bash
/opt/hostedtoolcache/Python/3.11.15/x64/bin/python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --predictions_path reports/benchmark/swebench-reference-predictions/baseline_reference.jsonl --max_workers 1 --run_id codex_tool_runtime_native_smoke --instance_ids sympy__sympy-12419
/opt/hostedtoolcache/Python/3.11.15/x64/bin/python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --predictions_path reports/benchmark/swebench-reference-predictions/candidate_reference.jsonl --max_workers 1 --run_id codex_tool_runtime_mcp_smoke --instance_ids sympy__sympy-12419
```

## Harness Reports

### Baseline
- `logs/run_evaluation/codex_tool_runtime_native_smoke/baseline_native_reference_patch/sympy__sympy-12419/report.json`
- `reports/benchmark/swebench-official-attempt/raw/baseline-logs-run_evaluation`
### Candidate
- `logs/run_evaluation/codex_tool_runtime_mcp_smoke/candidate_mcp_reference_patch/sympy__sympy-12419/report.json`
- `reports/benchmark/swebench-official-attempt/raw/candidate-logs-run_evaluation`

## Limitations

