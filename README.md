# ACT-R Agent Memory

This repository is scaffolded from the ACT-R research and engineering reports in
`research/`. It defines a Rust workspace for an ACT-R-inspired memory service for
LLM agents, backed by Memgraph for durable symbolic graph storage.

The architecture follows the reports' central split:

- Rust owns buffers, activation math, retrieval arbitration, production matching,
  utility updates, and per-agent session concurrency.
- Memgraph owns persisted chunks, slot/value relations, associations, practice
  history, production-rule metadata, and audit trails.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `actr-core` | Pure domain types, activation formulas, latency, thresholding, noise, utility math |
| `actr-session` | ACT-R buffer state and per-agent session serialization primitives |
| `actr-rules` | Symbolic production rule matching and utility ranking |
| `actr-store` | Memgraph schema, Cypher, repository contracts, migration registry |
| `actr-api` | Framework-neutral DTOs and route manifest for the HTTP API |
| `actr-ops` | Runtime config, health checks, metric names, ops constants |

The first scaffold intentionally avoids external crates so it can compile in a
restricted environment. Later goals add the recommended runtime stack: Tokio,
Axum, `thiserror`, `tracing`, `tower-http`, `neo4rs`, testcontainers, and
Criterion.

## Quick Start

```sh
cargo check --workspace --all-targets
cargo test --workspace
```

## Local Memgraph Runtime

Start the single-node local stack:

```sh
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
```

Memgraph listens on Bolt port `7687`. Its OpenMetrics endpoint is exposed on
`http://localhost:9091/metrics`, and Prometheus is available at
`http://localhost:9090`. Grafana is optional to avoid local port conflicts; start
it with `docker compose --profile dashboards up -d` when port `3000` is free.

The bootstrap script applies ordered Cypher migrations from
`crates/actr-store/migrations/`. It is safe to rerun during local development:
existing constraints and indexes are skipped, while other Cypher errors still
fail the script. The schema intentionally creates explicit indexes in addition
to constraints because Memgraph constraints do not create indexes.

Stop the stack while keeping named Docker volumes:

```sh
docker compose down
```

Remove local Memgraph and observability volume data:

```sh
docker compose down -v
```

## Codex Goals

Implementation is organized as repository-local goal packages under
`.codex/goals/`, matching the G01-G12 plan in
`research/ACT-R-Engineering-Plan.md`.
