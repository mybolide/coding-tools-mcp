# Real Workload MCP Benchmark

- Conclusion: **PARTIAL**
- Workloads: `5`
- Raw log directory: `reports/benchmark/real-workloads/raw`

## Workloads

- `python-click` (python): **PASS**
  - repo: `https://github.com/pallets/click.git`
  - commit: `c9e94136b39b1fad4a47ac0d4478fa372bd07ff8`
  - list_files: `PASS`
  - read_file: `PASS`
  - search_text: `PASS`
  - exec_command: `PASS`
- `node-mime-types` (node): **PASS**
  - repo: `https://github.com/jshttp/mime-types.git`
  - commit: `3a1187ee21388efe46b916d4991589d595bcb26c`
  - list_files: `PASS`
  - read_file: `PASS`
  - search_text: `PASS`
  - exec_command: `PASS`
- `rust-itoa` (rust): **SKIP**
  - repo: `https://github.com/dtolnay/itoa.git`
  - reason: `required executable not found: cargo`
- `go-uuid` (go): **SKIP**
  - repo: `https://github.com/google/uuid.git`
  - reason: `required executable not found: go`
- `monorepo-changesets` (monorepo): **PASS**
  - repo: `https://github.com/changesets/changesets.git`
  - commit: `372523f4c2ee4ffeb8330d444d47ffb6d0af5126`
  - list_files: `PASS`
  - read_file: `PASS`
  - search_text: `PASS`
  - exec_command: `PASS`
  - large_file: `PASS`
  - large_output: `PASS`
  - long_test: `PASS`

## Coverage

- Python repository
- Node repository
- Rust repository
- Go repository
- Monorepo
- Large file read
- Large output command
- Long-running command
