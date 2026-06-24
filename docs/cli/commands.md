# Command Reference

[CLI docs index](./README.md)

Global options:

```text
--api-url <URL>        API base URL
--agent <AGENT_ID>     Default agent id
--format <FORMAT>      text, json, pretty-json
--timeout-ms <MS>      HTTP timeout
--agent-footer         Append [exit:N | duration]
--verbose              Print request metadata to stderr
```

See [Slots And JSON](./slots-and-json.md), [Workflows](./workflows.md), and
[Output And Errors](./output-and-errors.md).

## Guide

Purpose: agent-oriented command map and examples.

Endpoint: none.

Example:

```sh
nestor guide workflow
```

Related: [Progressive Disclosure](./progressive-disclosure.md).

## Serve

Purpose: start the local API server.

Endpoint: local API listener.

Example:

```sh
nestor serve --bind 127.0.0.1:8090
```

Related: [Architecture](./architecture.md).

## Operational Commands

Purpose: inspect service health and metrics.

Endpoints:

- `manifest`: local route manifest
- `doctor`: `/healthz`, `/readyz`, `/metrics`
- `health`: `GET /healthz`
- `ready`: `GET /readyz`
- `metrics`: `GET /metrics`

Examples:

```sh
nestor doctor
nestor metrics --grep retrieval_hits
```

Related: [Output And Errors](./output-and-errors.md).

## Chunk

Purpose: create, inspect, patch, and delete chunks.

### Chunk Put

Endpoint: `POST /v1/memory/chunks`.

Required: `--agent`, `<CHUNK_ID>`, `--type`.

Options: `--slot`, `--now-ms`, `--json-file`.

Example:

```sh
nestor --agent agent-1 chunk put mem-preference --type fact \
  --slot topic=symbol:preference
```

Related: [Slots And JSON](./slots-and-json.md), [Workflows](./workflows.md).

### Chunk Get

Endpoint: `GET /v1/memory/chunks/{chunk_id}`.

Required: `--agent`, `<CHUNK_ID>`.

Example:

```sh
nestor --agent agent-1 chunk get mem-preference
```

Related: [Workflows](./workflows.md).

### Chunk Patch

Endpoint: `PATCH /v1/memory/chunks/{chunk_id}`.

Required: `--agent`, `<CHUNK_ID>`, `--expected-version`.

Options: `--slot`, `--json-file`.

Example:

```sh
nestor --agent agent-1 chunk patch mem-preference \
  --expected-version 1 \
  --slot verified=bool:true
```

Related: [Slots And JSON](./slots-and-json.md).

### Chunk Delete

Endpoint: `DELETE /v1/memory/chunks/{chunk_id}`.

Required: `--agent`, `<CHUNK_ID>`, `--yes`.

Example:

```sh
nestor --agent agent-1 chunk delete old-fact --yes
```

Related: [Output And Errors](./output-and-errors.md).

## Retrieve

Endpoint: `POST /v1/memory/retrieve`.

Alternate endpoint: `POST /v1/memory/retrieve/stream` with
`--endpoint stream`. The stream endpoint currently returns the same JSON shape.

Required: `--agent`.

Options: `--type`, `--cue`, `--context`, `--candidate-limit`,
`--result-limit`, `--threshold`, `--noise-s`, `--partial-matching`,
`--diagnostics`, `--seed`, `--commit`, `--now-ms`, `--json-file`.

Example:

```sh
nestor --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --context ctx-goal \
  --threshold -10
```

Related: [Slots And JSON](./slots-and-json.md), [Workflows](./workflows.md).

## Practice

Endpoint: `POST /v1/memory/practice`.

Required: `--agent`, `<CHUNK_ID>`, `--kind`.

Options: `--weight`, `--at-ms`, `--event-id`, `--json-file`.

Example:

```sh
nestor --agent agent-1 practice mem-preference --kind retrieve --weight 2
```

Related: [Workflows](./workflows.md).

## Rehearse

Endpoint: `POST /v1/memory/rehearse`.

Required: `--agent`, `<CHUNK_ID>`.

Options: `--weight`, `--at-ms`, `--event-id`, `--json-file`.

Example:

```sh
nestor --agent agent-1 rehearse mem-preference --weight 1
```

Related: [Workflows](./workflows.md).

## Consolidate

Endpoint: `POST /v1/memory/consolidate`.

Required: `--agent`.

Options: `--type`, `--summary-type`, `--group-slot`, `--min-group-size`,
`--now-ms`, `--json-file`.

Example:

```sh
nestor --agent agent-1 consolidate --type episode --group-slot topic
```

Related: [Workflows](./workflows.md).

## Forget

Endpoint: `POST /v1/memory/forget`.

Required: `--agent`.

Options: `--type`, `--now-ms`, `--recency-cutoff-ms`,
`--base-level-cutoff`, `--allow-linked`, `--json-file`.

Example:

```sh
nestor --agent agent-1 forget --type fact \
  --recency-cutoff-ms 1000 \
  --base-level-cutoff -4
```

Related: [Output And Errors](./output-and-errors.md).

## Associate

Endpoint: `POST /v1/memory/associate`.

Required: `--agent`, `<SRC_CHUNK_ID>`, `<DST_CHUNK_ID>`, `--source`,
`--strength`.

Options: `--fan`, `--at-ms`, `--json-file`.

Example:

```sh
nestor --agent agent-1 associate ctx-goal mem-preference \
  --source goal \
  --strength 1.25
```

Related: [Workflows](./workflows.md).

## Buffer

### Buffer Set

Endpoint: `PUT /v1/memory/buffers/{buffer_name}`.

Required: `--agent`, `<BUFFER_NAME>`, `<CHUNK_ID>`.

Options: `--at-ms`, `--json-file`.

Example:

```sh
nestor --agent agent-1 buffer set goal ctx-goal
```

Related: [Workflows](./workflows.md).

## Rule

### Rule Eval

Endpoint: `POST /v1/rules/evaluate`.

Required: `--agent`.

Options: `--candidate-rule`, `--rules-file`, `--retrieved`, `--json-file`.

Example:

```sh
nestor --agent agent-1 rule eval \
  --retrieved mem-preference \
  --rules-file rules.json
```

Related: [Workflows](./workflows.md), [Slots And JSON](./slots-and-json.md).
