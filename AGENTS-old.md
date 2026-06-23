# Agent Instructions

## Project Shape

This repository implements an ACT-R-inspired memory service for LLM agents.
The authoritative crate names are:

- `actr-core`: pure chunk types, activation math, latency, noise, partial matching, utility math.
- `actr-session`: per-agent session state, buffers, and one-step-at-a-time mutation semantics.
- `actr-rules`: symbolic production rules, matching, conflict resolution, and utility updates.
- `actr-store`: Memgraph schema, Cypher, repository traits, and future driver adapters.
- `actr-api`: HTTP/API DTO and route surface; Axum wiring belongs here in G08.
- `actr-ops`: config, health, metrics, deployment helpers, and runbook-oriented code.

Rust computes ACT-R scoring and production choice. Memgraph persists chunks,
slot/value relations, associations, practice history, rule metadata, and audit
records. Do not move activation math into Cypher.

## Commands

- Format: `cargo fmt --all`
- Check: `cargo check --workspace --all-targets`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Test: `cargo test --workspace`
- Full local verification: `make verify`

## Engineering Conventions

- Keep `actr-core` pure and deterministic. Database, HTTP, clocks, and randomness
  should be injected from higher layers.
- Use deterministic seeds in tests for any noise-sensitive behavior.
- Keep Memgraph queries bounded. Candidate generation belongs in `actr-store`;
  final scoring and thresholding belong in Rust.
- Preserve ACT-R buffer semantics: one current chunk per buffer and one cognitive
  step mutating an agent session at a time.
- Treat `.codex/goals/*` as implementation work packages. Each goal should have
  a prompt, schema, verification script, artifact list, and dependency metadata.

## Done Means

A change is not done until the relevant `cargo test` scope passes, workspace
`cargo check` passes, and any scaffolded script or goal metadata still points to
real repository paths.
