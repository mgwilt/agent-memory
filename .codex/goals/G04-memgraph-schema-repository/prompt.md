# Goal

Implement the Memgraph persistence layer for chunks, slot values, associations,
practice events, buffers, and production rules.

# Context

Use explicit Cypher migration files and repository traits. Later driver wiring
should use an async Bolt-compatible Rust driver such as `neo4rs`, with one shared
pooled graph client and short-lived request-scoped transactions.

# Constraints

- Create both constraints and indexes; Memgraph constraints do not create indexes.
- Keep candidate queries bounded.
- Use request-scoped transactions for causally coupled writes.
- Keep activation scoring out of Cypher.

# Done When

- Repository tests can upsert a chunk, fetch it, persist associations, append
  practice events, and enforce uniqueness constraints against Memgraph.
- Migrations are ordered and safe to run in local development.
