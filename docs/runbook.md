# Runbook

## Local Stack

Start Memgraph and observability services:

```sh
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
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
```

## Troubleshooting

- If schema bootstrap fails, confirm Memgraph is ready with
  `./scripts/wait-for-memgraph.sh`.
- If Bolt connectivity fails, confirm port `7687` is free and the Compose
  service is healthy.
- If metrics are missing, confirm Prometheus can reach `memgraph:9091` from the
  Compose network.
- If retrieval tests become nondeterministic, check that noise uses deterministic
  seeds in test mode.
