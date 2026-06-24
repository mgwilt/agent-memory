---
name: nestor-cli
description: >-
  Use when Codex needs to use, script, troubleshoot, or explain the Nestor CLI
  as an agent memory tool in the agent-memory repository: storing chunks,
  retrieving memories, practicing or rehearsing memories, associating context,
  setting buffers, evaluating rules, consolidating or forgetting memories,
  checking health, readiness, and metrics, or choosing between CLI flags and
  JSON-file API payloads. Do not use for unrelated Rust refactors or general
  repository maintenance that does not involve the Nestor CLI memory workflow.
---

# Nestor CLI

Use Nestor through the CLI first. Treat existing repo docs and `nestor ... --help` as canonical; this skill routes you to the right surface and common agent-safe patterns.

## Progressive Disclosure

This skill is intentionally layered:

1. **Metadata**: The `name` and `description` above are the discovery layer. They tell the agent when the Nestor CLI workflow is relevant.
2. **SKILL.md**: This file is the activation layer. Keep it to command-prefix selection, first checks, gotchas, and reference routing.
3. **References**: Load exactly the one-hop reference file needed for the current task. Do not read all references up front.

## Quick Start

1. Choose the command prefix:
   - Use `nestor` when it is installed on `PATH`.
   - In this repo, use `cargo run -p nestor-cli --` before a binary exists.
   - After building, use `target/debug/nestor` for faster repeated calls.
2. Orient with `nestor guide commands`, `nestor guide workflow`, and `nestor guide slots`. When using Cargo, put CLI arguments after `--`.
3. Run `nestor --agent-footer doctor` before API-backed memory commands. If the API is unavailable, start `nestor serve` or the Docker/Memgraph stack, then read `references/runtime-and-verification.md`.
4. Prefer `--format json` or `--format pretty-json` when another tool must parse output. Use `--agent-footer` when a run tool benefits from explicit `[exit:N | duration]` markers.

## Reference Routing

- Read `references/cli-tool-use.md` when you need to compose commands, choose flags, use typed slots, parse JSON output, pass `--json-file`, interpret exit codes, or run a CLI memory workflow.
- Read `references/memory-model.md` before creating or reshaping chunks, choosing slot types, designing retrieval cues/context, interpreting activation diagnostics, setting buffers, evaluating rules, consolidating, or forgetting.
- Read `references/runtime-and-verification.md` only when the task involves starting/checking the API, Docker/Memgraph, environment variables, health/readiness/metrics, localhost sandbox failures, or validation commands.

## Gotchas

- Use typed values: `key=symbol:value`, `key=text:value`, `key=number:12.5`, and `key=bool:true`. Use `symbol` for normalized matching and `text` for prose that should round-trip.
- Include an agent id for memory operations through `--agent <id>` or `NESTOR_AGENT_ID`.
- Use unique chunk IDs and event IDs in examples to avoid colliding with existing local state.
- Use `--json-file <path>` or `--json-file -` for exact API-shaped payloads or larger requests.
- For retrieval diagnostics, inspect `status`, `miss_reason`, `results[].components`, `passes_threshold`, and `diagnostics`.
- If a command fails, preserve stderr hints and follow the suggested `nestor guide ...` command before changing inputs.
- Do not invent undocumented CLI flags; check leaf help such as `nestor retrieve --help`.
