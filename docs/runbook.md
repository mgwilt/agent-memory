# Runbook

## Local Stack

Start Memgraph and observability services:

```sh
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
```

Memgraph Bolt and OpenMetrics ports are published on `127.0.0.1` only:
`127.0.0.1:7687` and `127.0.0.1:9091`. Prometheus scrapes Memgraph through the
private Compose service name `memgraph:9091`.

Stop the stack:

```sh
docker compose down
```

## Verification

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Runtime Configuration

Use `ACTR_PROFILE=development`, `ACTR_PROFILE=staging`, or
`ACTR_PROFILE=production` to select validated defaults. Production rejects
loopback Memgraph URIs and requires TLS plus a credential source.

For staged or production deployments, provide Memgraph credentials through
runtime secrets rather than checked-in files:

```sh
ACTR_PROFILE=production
ACTR_MEMGRAPH_URI=bolt+s://memgraph.production.internal:7687
ACTR_MEMGRAPH_TLS_ENABLED=true
ACTR_MEMGRAPH_TLS_SERVER_NAME=memgraph.production.internal
ACTR_MEMGRAPH_PASSWORD_FILE=/run/secrets/memgraph-password
```

Use `ACTR_MEMGRAPH_TLS_CA_FILE` when the Memgraph certificate chain needs a
mounted CA bundle. Do not commit passwords, generated certificates, private
keys, or local `.env` files.

## Troubleshooting

- If schema bootstrap fails, confirm Memgraph is ready with
  `./scripts/wait-for-memgraph.sh`.
- If Bolt connectivity fails, confirm port `7687` is free and the Compose
  service is healthy.
- If Memgraph metrics are missing, confirm Prometheus can reach `memgraph:9091`
  from the Compose network.
- If service metrics are missing, confirm the API is serving `/metrics` and that
  the `actr-memory` Prometheus target reaches `host.docker.internal:8080`.
- If retrieval tests become nondeterministic, check that noise uses deterministic
  seeds in test mode.
