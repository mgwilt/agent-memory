# Workflows

[CLI docs index](./README.md)

Each workflow links back to the [Command Reference](./commands.md).

## Quick Health Check

```sh
nestor doctor
nestor health
nestor ready
nestor metrics --grep retrieval_hits
```

Commands: [Operational Commands](./commands.md#operational-commands).

## Write And Retrieve One Fact

```sh
nestor --agent agent-1 chunk put mem-preference --type fact \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

nestor --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --threshold -10
```

Commands: [Chunk Put](./commands.md#chunk-put), [Retrieve](./commands.md#retrieve).

## Full Nestor Workflow

```sh
nestor --agent agent-1 chunk put ctx-goal --type goal \
  --slot task=symbol:answer-memory-question

nestor --agent agent-1 chunk put mem-preference --type fact \
  --slot subject=symbol:eli \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

nestor --agent agent-1 practice mem-preference --kind retrieve --weight 2

nestor --agent agent-1 associate ctx-goal mem-preference \
  --source goal \
  --strength 1.25

nestor --agent agent-1 buffer set goal ctx-goal

nestor --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --context ctx-goal \
  --threshold -10

nestor --agent agent-1 rule eval \
  --retrieved mem-preference \
  --rules-file rules.json

nestor metrics --grep retrieval_hits
```

Commands: [Chunk](./commands.md#chunk), [Practice](./commands.md#practice),
[Associate](./commands.md#associate), [Buffer Set](./commands.md#buffer-set),
[Retrieve](./commands.md#retrieve), [Rule Eval](./commands.md#rule-eval).

## JSON-File Driven Workflow

```sh
nestor chunk put ignored --json-file chunk.json
nestor retrieve --json-file retrieve.json --format json
nestor rule eval --json-file rule-eval.json --format pretty-json
```

Details: [Slots And JSON](./slots-and-json.md).

## Troubleshooting Failed Retrieval

```sh
nestor --agent agent-1 retrieve --type fact --cue topic=symbol:missing
nestor --agent agent-1 retrieve --type fact --threshold -10
nestor metrics --grep retrieval
```

Details: [Output And Errors](./output-and-errors.md).

## Agent Runner Usage

```sh
nestor --agent agent-1 --agent-footer retrieve \
  --type fact \
  --cue topic=symbol:preference
```

Details: [Progressive Disclosure](./progressive-disclosure.md) and
[Output And Errors](./output-and-errors.md).
