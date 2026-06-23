# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace for an ACT-R-inspired memory service backed by Memgraph. Source code lives under `crates/`: `actr-core` contains pure domain math and types, `actr-session` owns buffer/session state, `actr-rules` implements production rules, `actr-store` contains Memgraph schema and repository code, `actr-api` defines API DTOs/routes, and `actr-ops` holds config, health, and metrics helpers. Integration test scaffolding lives in `tests/integration/`, benchmark notes in `benches/`, runtime scripts in `scripts/`, documentation in `docs/`, and research/planning material in `research/` and `.goals/`.

## Build, Test, and Development Commands

- `cargo fmt --all`: format the workspace.
- `cargo check --workspace --all-targets`: type-check all crates and targets.
- `cargo clippy --workspace --all-targets -- -D warnings`: run lint checks as CI expects.
- `cargo test --workspace`: run all Rust tests.
- `make verify`: run format, check, clippy, and tests in sequence.
- `docker compose up -d`: start the local Memgraph, Prometheus, and Grafana stack.
- `./scripts/wait-for-memgraph.sh && ./scripts/bootstrap-memgraph.sh`: wait for Memgraph and apply migrations.

## Coding Style & Naming Conventions

Use standard Rust formatting with four-space indentation through `rustfmt`. Keep `actr-core` deterministic and free of I/O, clocks, database access, or HTTP concerns. Use `snake_case` for modules, functions, and variables; `PascalCase` for types and traits; and descriptive crate-local module names such as `activation`, `repository`, or `buffers`. Avoid `unwrap`, `dbg!`, and committed `todo!`; workspace lints deny them.

## Testing Guidelines

Place unit tests beside the code they verify and broader scenarios under `tests/integration/`. Use deterministic seeds for any noise-sensitive activation or retrieval behavior. Prefer tests that check ACT-R semantics directly: activation ordering, buffer mutation rules, production choice, and bounded Memgraph repository behavior. Run `cargo test --workspace` before submitting changes; use `make verify` for full local validation.

## Commit & Pull Request Guidelines

There is no existing commit history, so use clear imperative commit subjects such as `Add retrieval buffer tests` or `Wire Memgraph bootstrap`. Keep each commit focused. Pull requests should include a short problem/solution summary, linked issue or goal ID when relevant, test evidence, and notes for schema, API, or operational changes. Include screenshots only for UI or dashboard changes.

## Security & Configuration Tips

Do not commit secrets, local `.env` files, database dumps, or runtime volume data. Keep reusable examples in `.env.example` or docs. Memgraph migrations belong in `crates/actr-store/migrations/`; local data belongs in ignored Docker volumes or runtime directories.
