# Runtime And Verification

Use this reference when starting Nestor, checking service state, troubleshooting connectivity, or validating this Skill.

## Contents

- API Defaults And Environment
- Start The API
- Docker And Memgraph Stack
- Health, Readiness, Manifest, And Metrics
- Troubleshooting
- Verification Commands

## API Defaults And Environment

The CLI defaults to:

```text
Nestor API: http://127.0.0.1:8080
```

Useful environment variables:

```text
NESTOR_API_URL              CLI target API URL
NESTOR_AGENT_ID             default agent id for memory commands
NESTOR_API_BIND_ADDR        API bind address for serve/runtime
NESTOR_PROFILE              development, staging, production
NESTOR_REPOSITORY           memgraph by default; memory only for explicit local fixtures/tests
NESTOR_MEMGRAPH_URI         Bolt URI, commonly bolt://127.0.0.1:7687 locally
NESTOR_MEMGRAPH_USER        Memgraph user
NESTOR_MEMGRAPH_PASSWORD    direct secret value; do not commit
NESTOR_MEMGRAPH_PASSWORD_FILE secret-file source; do not commit
```

Production-style profiles require hardened Memgraph/TLS configuration. Do not commit secrets, local `.env` files, certificates, database dumps, or runtime volume data.

## Start The API

Fast repo loop:

```sh
cargo run -p nestor-api -- serve
```

Change bind address when needed:

```sh
NESTOR_API_BIND_ADDR=127.0.0.1:8090 cargo run -p nestor-api -- serve
```

Point the CLI at a non-default API:

```sh
nestor --api-url http://127.0.0.1:8090 doctor
```

## Docker And Memgraph Stack

Use the Compose stack when you need the API, Memgraph, and Prometheus together:

```sh
docker compose build api
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/demo-retrieval.sh
./scripts/demo-lifecycle.sh
```

Default local endpoints:

```text
API:              http://127.0.0.1:8080
Memgraph Bolt:   127.0.0.1:7687
Memgraph metrics http://127.0.0.1:9091/metrics
Prometheus:      http://127.0.0.1:9090
```

Run demos against another API URL:

```sh
NESTOR_API_URL=http://127.0.0.1:8090 ./scripts/demo-retrieval.sh
NESTOR_API_URL=http://127.0.0.1:8090 NESTOR_DEMO_RESTART_API=0 ./scripts/demo-lifecycle.sh
```

## Health, Readiness, Manifest, And Metrics

Use these checks before writing memories:

```sh
nestor --agent-footer doctor
nestor health --format json
nestor ready --format json
nestor manifest --format json
nestor metrics --grep retrieval
```

`health` checks liveness. `ready` checks dependencies. `manifest` prints routes without requiring manual source inspection. `metrics` returns Prometheus text; retrieval counters and latency metrics are useful after exercising memory flows.

If a Codex run tool reports `Operation not permitted` for localhost access, rerun the same API-backed command outside the sandbox. If the command returns exit code `6`, start or reconfigure the API and rerun `doctor`.

## Troubleshooting

- If schema bootstrap fails, run `./scripts/wait-for-memgraph.sh` first.
- If bootstrap reports no migrations, run it from the repo root or set `MIGRATION_DIR`.
- If Bolt connectivity fails, confirm port `7687` is free and Memgraph is healthy.
- If the API container exits, rebuild the API image and inspect `docker compose ps api`.
- If demos cannot reach the API, check port `8080`, `docker compose ps api`, and `curl -fsS http://127.0.0.1:8080/readyz`.
- If retrieval looks wrong, rerun with `--format json`, explicit `--seed`, `--diagnostics true`, and inspect activation components.

## Verification Commands

Skill validation:

```sh
python3 /Users/mike/.codex/skills/.system/skill-creator/scripts/quick_validate.py .codex/skills/nestor-cli
```

CLI discovery checks:

```sh
cargo run -p nestor-cli -- guide commands
target/debug/nestor guide slots
target/debug/nestor retrieve --help
```

Runtime checks:

```sh
target/debug/nestor --agent-footer doctor
target/debug/nestor manifest --format json
```

Workspace checks when changing repo code:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
