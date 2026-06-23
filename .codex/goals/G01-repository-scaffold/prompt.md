# Goal

Scaffold the ACT-R memory system repository as a Cargo workspace.

# Context

Create the six authoritative crates from the engineering plan: `actr-core`,
`actr-session`, `actr-rules`, `actr-store`, `actr-api`, and `actr-ops`.

# Constraints

- Rust stable.
- No business logic beyond narrow compile-time scaffolding and pure ACT-R math helpers.
- No high availability work.
- Include `AGENTS.md` with build, test, lint, and done criteria.
- Keep modules minimal but compilable.

# Done When

- `cargo check --workspace --all-targets` passes.
- Every crate builds.
- `AGENTS.md` exists.
- `.codex/goals/template/` exists.
