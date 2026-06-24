# Slots And JSON

[CLI docs index](./README.md)

Slot and cue values use an explicit type prefix:

```text
--slot key=symbol:value
--slot key=text:value
--slot key=number:12.5
--slot key=bool:true
--cue topic=symbol:preference
```

Use `symbol` for normalized symbolic matching. Use `text` for prose that should
round-trip as text. Use `number` and `bool` for typed facts.

## JSON Files

Any command with `--json-file` accepts exact API DTO JSON:

```sh
actr-memory chunk put ignored --json-file chunk.json
actr-memory retrieve --json-file - --format json
```

`-` reads JSON from stdin.

Example chunk:

```json
{
  "agent_id": "agent-1",
  "chunk_id": "mem-preference",
  "chunk_type": "fact",
  "now_ms": 1000,
  "slots": {
    "topic": { "type": "symbol", "value": "preference" }
  }
}
```

Common fixes:

- `topic=preference` is invalid; use `topic=symbol:preference`.
- `verified=true` is invalid; use `verified=bool:true`.
- `confidence=high` is invalid for numbers; use `confidence=number:0.9`.

See [Command Reference](./commands.md), [Workflows](./workflows.md), and
[Output And Errors](./output-and-errors.md).
