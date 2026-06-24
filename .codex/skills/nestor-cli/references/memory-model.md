# Memory Model

Use this reference before deciding how to represent information in Nestor. Nestor is an ACT-R-inspired durable memory service: the CLI writes and retrieves chunks through an HTTP API, while scoring and diagnostics explain why a memory hit or missed.

## Contents

- Core Concepts
- Choosing Slot Types
- Retrieval Semantics
- Practice, Rehearsal, And Associations
- Buffers
- Production Rules
- Consolidation And Forgetting

## Core Concepts

- Agent: `agent_id` namespaces all chunks, buffers, practice events, and rule evaluations.
- Chunk: stable memory unit with `chunk_id`, `chunk_type`, typed slots, timestamps, retrieval count, and base bias.
- Chunk type: symbolic grouping such as `fact`, `episode`, `goal`, or `semantic`.
- Slot: typed key/value attached to a chunk. Values are `symbol`, `text`, `number`, or `bool`.
- Cue: typed slot requested during retrieval.
- Context chunk: chunk id used as a spreading-activation source.

Use stable, meaningful IDs such as `ctx-goal`, `mem-preference`, or `episode-debugging-20260624`. Avoid random IDs unless the memory is intentionally disposable.

## Choosing Slot Types

- Use `symbol` for labels that should normalize and match, such as topics, subjects, task names, owners, and categories.
- Use `text` for prose that should round-trip without being treated as a normalized symbol.
- Use `number` for confidence, scores, timestamps carried as facts, and numeric attributes.
- Use `bool` for flags such as `verified`, `protected`, or `needs_followup`.

Prefer a small set of consistent keys: `topic`, `subject`, `detail`, `task`, `owner`, `status`, and `source` are easy for agents to cue later.

## Retrieval Semantics

Retrieval ranks candidate chunks and returns diagnostics. The activation shape is:

```text
activation = base_level + spreading + partial_match + noise
```

- Base level rises from encode, practice, rehearsal, and successful retrieval events.
- Spreading activation comes from associations between context chunks and candidate chunks.
- Partial match rewards or penalizes cue/slot similarity depending on matching settings.
- Noise is deterministic when a seed is provided and zero when disabled.
- A retrieval hit requires activation to pass the threshold.

For smoke tests, a low threshold such as `--threshold -10` proves plumbing. For realistic retrieval behavior, inspect the default threshold and the returned components.

Inspect these JSON fields:

```text
status
miss_reason
results[].chunk_id
results[].activation
results[].components.base_level
results[].components.spreading
results[].components.partial_match
results[].components.noise
results[].passes_threshold
diagnostics.candidates_examined
diagnostics.context_chunk_count
```

## Practice, Rehearsal, And Associations

Use `practice` to record a memory-use event with a kind such as `retrieve`. Use `rehearse` for first-class rehearsal. Both strengthen future retrieval through practice history.

Use `associate <src> <dst> --source <label> --strength <value>` when current context should help retrieve a related memory. A common pattern is a goal chunk as source and a fact or episode as destination:

```sh
nestor --agent agent-1 associate ctx-goal mem-preference --source goal --strength 1.25
```

Keep association strengths intentional. Use small positive values for normal context influence and inspect retrieval components to confirm the effect.

## Buffers

Buffers hold current cognitive state. Built-in names include:

```text
goal
retrieval
imaginal
task
```

Custom non-empty buffer names are also accepted. Use `buffer set goal <chunk-id>` when the task goal should become available to rule evaluation and retrieval context.

## Production Rules

Rules evaluate candidate productions against buffers and optionally the retrieved chunk. Use `rule eval` with `--rules-file` for realistic rule sets. A rule can match:

- buffer presence,
- buffer chunk id,
- buffer chunk type,
- retrieved chunk id,
- retrieved chunk type,
- retrieved chunk slots.

Returned diagnostics include selected rule, matches, candidates, utility, specificity, rank, and rejection reasons. If no rule matches, inspect buffers and the retrieved chunk condition before changing utilities.

## Consolidation And Forgetting

Use `consolidate` to create semantic summaries from overlapping chunks, usually episodes grouped by slots such as `topic` or `subject`.

Use `forget` to apply policy-driven soft delete or archive behavior. It can filter by type, recency cutoff, and base-level cutoff. Chunks marked or linked as protected may be returned as protected rather than forgotten.

Run lifecycle commands with scoped filters first. Avoid broad forgetting commands until retrieval, metrics, and expected protected chunks are understood.
