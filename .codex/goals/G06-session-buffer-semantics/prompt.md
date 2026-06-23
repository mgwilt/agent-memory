# Goal

Implement per-agent session state with ACT-R-like buffer semantics.

# Context

Goal, retrieval, imaginal, and task buffers live in memory. Each buffer holds one
current chunk. One cognitive step should mutate an agent session at a time.

# Constraints

- No database writes from buffer structs directly.
- Expose deterministic APIs for tests.
- Preserve serial mutation per agent while allowing many sessions overall.

# Done When

- Unit tests demonstrate serialized mutation, buffer replacement, clearing, and
  retrieval commit behavior under concurrent requests.
