# CLI Tool Use

Use this reference for Nestor command syntax, agent-friendly output, and practical workflows. The canonical surfaces are still `nestor guide ...`, leaf `--help`, and `docs/cli/`.

## Contents

- Command Prefix
- Progressive Discovery
- Global Flags
- Typed Slots And Cues
- JSON File Mode
- Common Workflows
- Exit Codes

## Command Prefix

- Use `nestor` when the binary is installed.
- From this repo before building, use `cargo run -p nestor-cli -- <args>`.
- After a build, use `target/debug/nestor <args>` for faster repeated calls.
- If the Codex sandbox blocks localhost probes with `Operation not permitted`, rerun API-backed commands outside the sandbox.

## Progressive Discovery

Start shallow and drill down only as needed:

```sh
nestor --help
nestor guide commands
nestor guide workflow
nestor guide slots
nestor chunk --help
nestor retrieve --help
nestor rule eval --help
```

Top-level commands:

```text
guide, serve, manifest, doctor, health, ready, metrics,
chunk, retrieve, practice, rehearse, consolidate, forget,
associate, buffer, rule
```

Only `guide` is purely local. Most memory and ops commands contact the API.

## Global Flags

```text
--api-url <URL>        API base URL, default http://127.0.0.1:8080
--agent <AGENT_ID>     Default agent id, also supported by NESTOR_AGENT_ID
--format <FORMAT>      text, json, pretty-json
--timeout-ms <MS>      HTTP timeout, default 5000
--agent-footer         Append [exit:N | duration] for LLM run tools
--verbose              Print request metadata to stderr
```

Prefer `--format json` for machine parsing. Prefer `--agent-footer` when command status must be visible in combined tool output.

## Typed Slots And Cues

Slots and cues require explicit types:

```sh
--slot topic=symbol:preference
--slot detail=text:"strong black coffee"
--slot confidence=number:0.9
--slot verified=bool:true
--cue topic=symbol:preference
```

Use `symbol` for normalized matching, `text` for prose, and typed `number` or `bool` for structured facts. Invalid examples such as `topic=preference` or `verified=true` should be corrected to typed values.

## JSON File Mode

Use `--json-file <path>` or `--json-file -` when flags become unwieldy or when matching API DTOs exactly. Slot values in JSON use tagged objects:

```json
{
  "agent_id": "agent-1",
  "chunk_id": "mem-preference",
  "chunk_type": "fact",
  "now_ms": 1000,
  "slots": {
    "topic": { "type": "symbol", "value": "preference" },
    "detail": { "type": "text", "value": "strong black coffee" }
  }
}
```

Examples:

```sh
nestor chunk put ignored --json-file chunk.json
nestor retrieve --json-file - --format json
nestor rule eval --json-file rule-eval.json --format pretty-json
```

## Common Workflows

Health and readiness:

```sh
nestor --agent-footer doctor
nestor health --format json
nestor ready --format json
nestor metrics --grep retrieval
```

Write and retrieve one fact:

```sh
nestor --agent agent-1 chunk put mem-preference --type fact \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

nestor --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --threshold -10 \
  --format json
```

Full memory loop:

```sh
nestor --agent agent-1 chunk put ctx-goal --type goal \
  --slot task=symbol:answer-memory-question

nestor --agent agent-1 chunk put mem-preference --type fact \
  --slot subject=symbol:eli \
  --slot topic=symbol:preference \
  --slot detail=text:"strong black coffee"

nestor --agent agent-1 practice mem-preference --kind retrieve --weight 2
nestor --agent agent-1 rehearse mem-preference --weight 1
nestor --agent agent-1 associate ctx-goal mem-preference --source goal --strength 1.25
nestor --agent agent-1 buffer set goal ctx-goal

nestor --agent agent-1 retrieve --type fact \
  --cue topic=symbol:preference \
  --context ctx-goal \
  --threshold -10 \
  --result-limit 3 \
  --format json
```

Production rule evaluation:

```sh
nestor --agent agent-1 rule eval \
  --retrieved mem-preference \
  --rules-file rules.json \
  --format json
```

Lifecycle maintenance:

```sh
nestor --agent agent-1 consolidate --type episode --group-slot topic

nestor --agent agent-1 forget --type fact \
  --recency-cutoff-ms 1000 \
  --base-level-cutoff -4
```

## Exit Codes

```text
0 success
2 CLI usage or local validation
3 API bad_request
4 API not_found
5 API conflict
6 API unavailable, network failure, or timeout
7 invalid API response or internal CLI error
```

Errors print to stderr and include a corrective hint such as `Explore: nestor guide slots`. Follow that hint before broadening the diagnosis.
