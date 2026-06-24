# Benchmarks

Criterion benchmarks live here and are wired through the `nestor-store` package so
workspace `--all-targets` checks compile them.

Run the deterministic activation and retrieval hot-path suite:

```sh
cargo bench -p nestor-store --bench activation_retrieval
```

Create a local baseline report, then compare future runs against it:

```sh
cargo bench -p nestor-store --bench activation_retrieval -- --save-baseline local
cargo bench -p nestor-store --bench activation_retrieval -- --baseline local
```

Regression gates should use Criterion's relative baseline comparisons. Avoid
absolute timing thresholds because local CPU, thermal, and scheduler conditions
vary across developer and CI machines.

Current benchmark targets:

- activation scoring over bounded practice histories of 8, 32, and 128 events;
- retrieval candidate ranking for 50, 100, and 200 chunks;
- deterministic retrieval noise and partial-match scoring with fixed seeds.
