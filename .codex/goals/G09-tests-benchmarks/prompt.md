# Goal

Add comprehensive tests and reproducible benchmarks for the ACT-R memory service.

# Context

Use deterministic unit tests for core math and rules, Memgraph-backed integration
tests for repositories and retrieval, and Criterion benchmarks for activation and
retrieval hot paths.

# Constraints

- Deterministic seeds only.
- Bounded fixture sizes.
- No flaky timing assertions.
- Benchmark regression gates should focus on relative changes, not absolute local
  machine timing.

# Done When

- `cargo test --workspace` passes.
- Integration tests run against Memgraph.
- Benchmark targets produce stable baseline reports.
