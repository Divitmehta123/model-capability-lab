# Model Capability Lab

A deterministic Rust workbench for deciding which model/runtime profile may execute a task. It scores declared observations, exposes capability gaps, measures tail latency, emits versioned JSON, and can fail CI when no model satisfies a deployment gate.

The included TSV is illustrative fixture data—not a vendor benchmark. Replace it with observations from your own eval harness.

## Run

```bash
cargo run -- --input fixtures/default.tsv
cargo run -- --format markdown
cargo run -- --format json
cargo run -- --min-score 80 --require tool_calls --require json_schema
```

Exit codes are stable: `0` for success, `1` when a gate has no eligible model, and `2` for invalid input or CLI usage. Rows are keyed by provider, model, and scenario; duplicates and malformed capability declarations are rejected.

## Input contract

Tab-separated columns: `provider`, `model`, `scenario`, `capability`, `weight`, `outcome`, `latency_ms`.

Capabilities are `tool_calls`, `vision`, `json_schema`, `streaming`, and `long_context`. Outcomes are `pass`, `fail`, or `unsupported`.

## Architecture

- `src/lib.rs`: parsing, invariants, aggregation, eligibility policy, and renderers.
- `src/main.rs`: minimal CLI boundary and meaningful exit codes.
- `fixtures/default.tsv`: replaceable, reviewable evidence input.

## Verify

```bash
cargo fmt -- --check
cargo test
cargo clippy -- -D warnings
cargo run -- --format markdown
```
