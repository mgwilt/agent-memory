# Runbook

## Local Stack

Build the API image, then start the local single-node stack:

```sh
docker compose build api
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/demo-retrieval.sh
```

The stack includes the Rust API image, one Memgraph node, and Prometheus. Ports
are published on `127.0.0.1` only: API `8080`, Memgraph Bolt `7687`, Memgraph
OpenMetrics `9091`, and Prometheus `9090`. Prometheus scrapes Memgraph through
`memgraph:9091` and the API through `api:8080` on the private Compose network.

Run the retrieval demo against a non-default API URL when needed:

```sh
NESTOR_API_URL=http://127.0.0.1:8090 ./scripts/demo-retrieval.sh
```

Stop the stack:

```sh
docker compose down
```

## Verification

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.codex/goals/G11-deployment-handoff/verify.sh
```

## Runtime Configuration

Use `NESTOR_PROFILE=development`, `NESTOR_PROFILE=staging`, or
`NESTOR_PROFILE=production` to select validated defaults. Production rejects
loopback Memgraph URIs and requires TLS plus a credential source.

For staged or production deployments, provide Memgraph credentials through
runtime secrets rather than checked-in files:

```sh
NESTOR_PROFILE=production
NESTOR_MEMGRAPH_URI=bolt+s://memgraph.production.internal:7687
NESTOR_MEMGRAPH_TLS_ENABLED=true
NESTOR_MEMGRAPH_TLS_SERVER_NAME=memgraph.production.internal
NESTOR_MEMGRAPH_PASSWORD_FILE=/run/secrets/memgraph-password
```

Use `NESTOR_MEMGRAPH_TLS_CA_FILE` when the Memgraph certificate chain needs a
mounted CA bundle. Do not commit passwords, generated certificates, private
keys, or local `.env` files.

## Troubleshooting

- If schema bootstrap fails, confirm Memgraph is ready with
  `./scripts/wait-for-memgraph.sh`.
- If bootstrap reports `No Cypher migrations found`, run it from the repository
  root or set `MIGRATION_DIR` to the migrations directory.
- If bootstrap skips statements as already applied, that is expected on reruns;
  non-schema Cypher errors still fail the script.
- If Bolt connectivity fails, confirm port `7687` is free and the Compose
  service is healthy.
- If the API container exits immediately, rebuild it and confirm it is running
  the default `serve` command with `docker compose ps api`.
- If `scripts/demo-retrieval.sh` cannot reach the API, confirm port `8080` is
  free, `docker compose ps api` shows a running container, and
  `curl -fsS http://127.0.0.1:8080/healthz` returns JSON.
- If Memgraph metrics are missing, confirm Prometheus can reach `memgraph:9091`
  from the Compose network.
- If service metrics are missing, confirm the API is serving `/metrics` and that
  the `nestor` Prometheus target reaches `api:8080`.
- If retrieval tests become nondeterministic, check that noise uses deterministic
  seeds in test mode.
