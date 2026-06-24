# Integration Tests

Package-level integration tests live under crate `tests/` directories. The
shared retrieval pipeline fixture in this directory is mounted by
`crates/nestor-store/tests/retrieval_pipeline.rs` so it runs with
`cargo test --workspace`.

Live Memgraph coverage is opt-in to keep normal test runs deterministic and
usable without Docker. CI runs these commands as mandatory coverage:

```sh
docker compose up -d memgraph
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/bootstrap-memgraph.sh
NESTOR_STORE_MEMGRAPH_TESTS=1 cargo test -p nestor-store --test memgraph_repository_live -- --nocapture
NESTOR_STORE_MEMGRAPH_TESTS=1 cargo test -p nestor-store --test memgraph_live -- --nocapture
NESTOR_STORE_MEMGRAPH_TESTS=1 cargo test -p nestor-api --test memgraph_runtime -- --nocapture
docker compose build api
docker compose up -d api
./scripts/demo-retrieval.sh
./scripts/demo-lifecycle.sh
```

The `memgraph_repository_live` test uses the async `neo4rs` runtime repository
directly. It verifies typed slot payload round-trips, practice and retrieval
events, candidate history, associations, buffers, production rules, reconnect
persistence, consolidation, forgetting, archive behavior, and soft delete.

The older `memgraph_live` test remains as a schema/Cypher smoke through
`mgconsole`. The API runtime test builds `ApiState::from_config()` against
Memgraph and proves memory state survives state reconstruction. The lifecycle
demo waits on `/readyz`, exercises the public lifecycle API over HTTP, restarts
the API container when compose is available, and verifies durable practice
history after restart.

Required scenarios:

- schema migration idempotency;
- chunk create/read/update/delete;
- append-only practice events under concurrency;
- context-sensitive spreading activation;
- retrieval threshold misses;
- production rule metadata persistence.
- API readiness failure and success;
- public rehearse, consolidate, and forget lifecycle flows;
- Docker Compose API restart persistence.
