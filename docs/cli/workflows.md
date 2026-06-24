# Workflows

[CLI docs index](./README.md)

Each workflow links back to the [Command Reference](./commands.md).

## Quick Health Check

```sh
actr-memory doctor
actr-memory health
actr-memory ready
actr-memory metrics --grep retrieval_hits
```

Commands: [Operational Commands](./commands.md#operational-commands).

## Write And Retrieve One Fact

```sh
actr-memory --agent agent-1 chunk put mem-preference --type fact \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

actr-memory --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --threshold -10
```

Commands: [Chunk Put](./commands.md#chunk-put), [Retrieve](./commands.md#retrieve).

## Full ACT-R Workflow

```sh
actr-memory --agent agent-1 chunk put ctx-goal --type goal \
  --slot task=symbol:answer-memory-question

actr-memory --agent agent-1 chunk put mem-preference --type fact \
  --slot subject=symbol:eli \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

actr-memory --agent agent-1 practice mem-preference --kind retrieve --weight 2

actr-memory --agent agent-1 associate ctx-goal mem-preference \
  --source goal \
  --strength 1.25

actr-memory --agent agent-1 buffer set goal ctx-goal

actr-memory --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --context ctx-goal \
  --threshold -10

actr-memory --agent agent-1 rule eval \
  --retrieved mem-preference \
  --rules-file rules.json

actr-memory metrics --grep retrieval_hits
```

Commands: [Chunk](./commands.md#chunk), [Practice](./commands.md#practice),
[Associate](./commands.md#associate), [Buffer Set](./commands.md#buffer-set),
[Retrieve](./commands.md#retrieve), [Rule Eval](./commands.md#rule-eval).

## JSON-File Driven Workflow

```sh
actr-memory chunk put ignored --json-file chunk.json
actr-memory retrieve --json-file retrieve.json --format json
actr-memory rule eval --json-file rule-eval.json --format pretty-json
```

Details: [Slots And JSON](./slots-and-json.md).

## Troubleshooting Failed Retrieval

```sh
actr-memory --agent agent-1 retrieve --type fact --cue topic=symbol:missing
actr-memory --agent agent-1 retrieve --type fact --threshold -10
actr-memory metrics --grep retrieval
```

Details: [Output And Errors](./output-and-errors.md).

## Agent Runner Usage

```sh
actr-memory --agent agent-1 --agent-footer retrieve \
  --type fact \
  --cue topic=symbol:preference
```

Details: [Progressive Disclosure](./progressive-disclosure.md) and
[Output And Errors](./output-and-errors.md).
