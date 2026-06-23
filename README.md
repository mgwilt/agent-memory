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
| `actr-api` | Axum HTTP API, JSON DTOs, route manifest, and in-memory service wiring |
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

Build the service image, start the single-node local stack, bootstrap Memgraph,
and run the HTTP retrieval demo:

```sh
docker compose build api
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/demo-retrieval.sh
```

Run the reproducible Criterion benchmark suite for activation and retrieval hot
paths:

```sh
cargo bench -p actr-store --bench activation_retrieval
```

Create and compare local benchmark baselines with relative Criterion reports:

```sh
cargo bench -p actr-store --bench activation_retrieval -- --save-baseline local
cargo bench -p actr-store --bench activation_retrieval -- --baseline local
```

Print the API route manifest:

```sh
cargo run -p actr-api -- manifest
```

Start the local Axum API with the in-memory repository used by the current
service wiring:

```sh
cargo run -p actr-api -- serve
```

By default the server binds to `127.0.0.1:8080`. Override it with
`ACTR_API_BIND_ADDR`, for example `ACTR_API_BIND_ADDR=127.0.0.1:8090`.
When stdout is redirected, `cargo run -p actr-api` prints the route manifest so
automation can verify the binary without hanging on a long-running server.

## HTTP API Examples

Create or upsert a chunk:

```sh
curl -sS http://127.0.0.1:8080/v1/memory/chunks \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "chunk_id": "ck-actr",
    "chunk_type": "fact",
    "now_ms": 1000,
    "slots": {
      "topic": { "type": "symbol", "value": "act-r" }
    }
  }'
```

Retrieve with score diagnostics:

```sh
curl -sS http://127.0.0.1:8080/v1/memory/retrieve \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "chunk_type": "fact",
    "now_ms": 2000,
    "activation_threshold": -5.0,
    "cue_slots": [
      { "key": "topic", "value": { "type": "symbol", "value": "ACT-R" } }
    ]
  }'
```

Record practice, associate chunks, set a buffer, and evaluate a rule:

```sh
curl -sS http://127.0.0.1:8080/v1/memory/practice \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "chunk_id": "ck-actr",
    "event_id": "practice-1",
    "kind": "retrieve",
    "weight": 1.0,
    "occurred_at_ms": 2000
  }'

curl -sS http://127.0.0.1:8080/v1/memory/associate \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "src_chunk_id": "ck-actr",
    "dst_chunk_id": "ck-actr",
    "source": "goal",
    "strength": 1.5
  }'

curl -sS http://127.0.0.1:8080/v1/memory/buffers/goal \
  -X PUT \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "chunk_id": "ck-actr",
    "set_at_ms": 2500
  }'

curl -sS http://127.0.0.1:8080/v1/rules/evaluate \
  -H 'content-type: application/json' \
  -d '{
    "agent_id": "agent-1",
    "rules": [{
      "rule_id": "rule-1",
      "name": "goal fact present",
      "utility": 2.0,
      "conditions": [{ "buffer": "goal", "chunk_type": "fact" }]
    }]
  }'
```

Operational endpoints return JSON:

```sh
curl -sS http://127.0.0.1:8080/healthz
curl -sS http://127.0.0.1:8080/readyz
curl -sS http://127.0.0.1:8080/metrics
```

## Local Memgraph Runtime

Start the single-node local stack:

```sh
docker compose build api
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/demo-retrieval.sh
```

The stack starts the Rust API image, Memgraph, and Prometheus. The API listens
on `http://127.0.0.1:8080`, Memgraph listens on Bolt port `7687`, Memgraph
OpenMetrics is exposed on `http://127.0.0.1:9091/metrics`, and Prometheus is
available at `http://127.0.0.1:9090`. Grafana is optional to avoid local port
conflicts; start it with `docker compose --profile dashboards up -d` when port
`3000` is free.

The bootstrap script applies ordered Cypher migrations from
`crates/actr-store/migrations/`. It is safe to rerun during local development:
existing constraints and indexes are skipped, while other Cypher errors still
fail the script. The schema intentionally creates explicit indexes in addition
to constraints because Memgraph constraints do not create indexes.

The demo retrieval script talks to the HTTP API and validates a deterministic
chunk retrieval plus the retrieval hit metric. It uses the in-memory API
repository while Memgraph schema bootstrap is verified separately by the
bootstrap step.

Run the opt-in live Memgraph integration test after the stack is ready:

```sh
ACTR_STORE_MEMGRAPH_TESTS=1 cargo test -p actr-store --test memgraph_live -- --nocapture
```

Normal `cargo test --workspace` runs remain deterministic and do not require
Docker. The live test seeds a bounded retrieval fixture in Memgraph, verifies
association-driven candidate ordering through `mgconsole`, and removes the test
agent graph before returning.

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
