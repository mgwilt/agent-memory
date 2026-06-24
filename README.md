# Nestor

Nestor gives agents a durable memory surface they can use from a terminal,
local service, or companion app. The primary interface is the `nestor` CLI:
agents can write facts, retrieve relevant memories, reinforce useful memories,
connect context, inspect diagnostics, and evaluate simple production rules.

## What You Can Do

- Store typed memories as chunks with slots.
- Retrieve memories by cues and context, with score diagnostics.
- Record practice so repeated use changes retrieval strength.
- Associate chunks so current context can spread activation.
- Set buffers for goal, retrieval, imaginal, and task state.
- Evaluate production rules against memory and buffers.
- Run all of this locally through the CLI, HTTP API, or Docker stack.

## Start With The CLI

When Nestor is installed, use the CLI directly:

```sh
nestor guide
nestor doctor
nestor guide workflow
```

From this repository, run the same commands through Cargo:

```sh
cargo run -p nestor-cli -- guide
cargo run -p nestor-cli -- doctor
cargo run -p nestor-cli -- guide workflow
```

The CLI is intentionally progressive. Start with `nestor guide`, then drill into
only the command group or workflow you need:

```sh
nestor guide commands
nestor chunk --help
nestor retrieve --help
nestor guide slots
nestor guide docs
```

Full CLI docs live in [docs/cli/README.md](docs/cli/README.md).

## Common Workflows

Start or point at a Nestor API before running commands that call the service.
For local development:

```sh
cargo run -p nestor-api -- serve
```

In another terminal, check connectivity:

```sh
cargo run -p nestor-cli -- doctor
cargo run -p nestor-cli -- health
cargo run -p nestor-cli -- ready
```

Write one memory:

```sh
cargo run -p nestor-cli -- --agent agent-1 chunk put mem-preference \
  --type fact \
  --slot subject=symbol:eli \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"
```

Retrieve it:

```sh
cargo run -p nestor-cli -- --agent agent-1 retrieve \
  --type fact \
  --cue topic=symbol:preference \
  --threshold -10
```

Reinforce it and connect it to context:

```sh
cargo run -p nestor-cli -- --agent agent-1 practice mem-preference \
  --kind retrieve \
  --weight 2

cargo run -p nestor-cli -- --agent agent-1 associate ctx-goal mem-preference \
  --source goal \
  --strength 1.25
```

Set a buffer and inspect metrics:

```sh
cargo run -p nestor-cli -- --agent agent-1 buffer set goal ctx-goal
cargo run -p nestor-cli -- metrics --grep retrieval
```

For complete copy-pasteable workflows, see
[docs/cli/workflows.md](docs/cli/workflows.md).

## Run Nestor Locally

Use the in-memory local API when you want the fastest CLI loop:

```sh
cargo run -p nestor-api -- serve
```

Use Docker Compose when you want the local API, Memgraph, and Prometheus stack:

```sh
docker compose build api
docker compose up -d
./scripts/wait-for-memgraph.sh
./scripts/bootstrap-memgraph.sh
./scripts/demo-retrieval.sh
```

Default local endpoints:

- Nestor API: `http://127.0.0.1:8080`
- Memgraph Bolt: `127.0.0.1:7687`
- Memgraph metrics: `http://127.0.0.1:9091/metrics`
- Prometheus: `http://127.0.0.1:9090`

Operational details, runtime configuration, troubleshooting, and Docker cleanup
live in [docs/runbook.md](docs/runbook.md).

## Use The API

The CLI is the recommended starting point, but the HTTP API is available for
companion apps and agent runtimes.

Print the route manifest:

```sh
cargo run -p nestor-api -- manifest
```

Start the server:

```sh
cargo run -p nestor-api -- serve
```

Use `NESTOR_API_BIND_ADDR` to change the bind address:

```sh
NESTOR_API_BIND_ADDR=127.0.0.1:8090 cargo run -p nestor-api -- serve
```

For endpoint behavior, request and response shapes, and output contracts, start
with [docs/cli/commands.md](docs/cli/commands.md),
[docs/cli/slots-and-json.md](docs/cli/slots-and-json.md), and
[docs/cli/output-and-errors.md](docs/cli/output-and-errors.md).

## For Agents

Nestor is designed so agents do not need to load the entire manual up front.

- Use `nestor guide commands` for a compact command map.
- Use group help such as `nestor chunk --help` when choosing an action.
- Use leaf help such as `nestor retrieve --help` for exact arguments.
- Use `nestor guide workflow` for a complete end-to-end command sequence.
- Use `--format json` or `--format pretty-json` when the caller needs structured
  output.
- Use `--agent-footer` when running inside tools that benefit from explicit
  exit and duration markers.

See [docs/cli/progressive-disclosure.md](docs/cli/progressive-disclosure.md) for
the progressive-disclosure contract.

## Technical Documentation

- [CLI index](docs/cli/README.md): command map, workflows, examples, slots, and
  error handling.
- [CLI command reference](docs/cli/commands.md): every command and endpoint.
- [CLI workflows](docs/cli/workflows.md): quick checks and full memory flows.
- [Slots and JSON](docs/cli/slots-and-json.md): typed slot grammar and JSON
  file usage.
- [Output and errors](docs/cli/output-and-errors.md): stdout/stderr, JSON modes,
  exit codes, and recovery hints.
- [Runbook](docs/runbook.md): local stack, runtime config, and troubleshooting.
- [Architecture notes](docs/architecture.md): ownership boundaries, memory flow,
  scoring diagrams, runtime profiles, and observability.
- [Agentic E2E CLI](docs/agentic-e2e-cli.md): LM Studio and local-model
  integration workflow.

## Development Checks

Before submitting changes, run:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`make verify` runs the same verification sequence.
