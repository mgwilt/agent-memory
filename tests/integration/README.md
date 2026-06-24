# Integration Tests

Package-level integration tests live under crate `tests/` directories. The
shared retrieval pipeline fixture in this directory is mounted by
`crates/nestor-store/tests/retrieval_pipeline.rs` so it runs with
`cargo test --workspace`.

Live Memgraph coverage is opt-in to keep normal test runs deterministic and
usable without Docker:

```sh
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
NESTOR_STORE_MEMGRAPH_TESTS=1 cargo test -p nestor-store --test memgraph_live -- --nocapture
```

The live G09 test seeds a bounded retrieval fixture in Memgraph, verifies
retrieval-relevant association ordering through `mgconsole`, and deletes the
test agent's graph before returning.

Required scenarios from the research reports:

- schema migration idempotency;
- chunk create/read/update/delete;
- append-only practice events under concurrency;
- context-sensitive spreading activation;
- retrieval threshold misses;
- production rule metadata persistence.
