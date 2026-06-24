# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace for an ACT-R-inspired memory service backed by Memgraph. Source code lives under `crates/`: `nestor-core` contains pure domain math and types, `nestor-session` owns buffer/session state, `nestor-rules` implements production rules, `nestor-store` contains Memgraph schema and repository code, `nestor-api` defines API DTOs/routes, and `nestor-ops` holds config, health, and metrics helpers. Integration test scaffolding lives in `tests/integration/`, benchmark notes in `benches/`, runtime scripts in `scripts/`, documentation in `docs/`, and research/planning material in `research/` and `.goals/`.

## Build, Test, and Development Commands

- `cargo fmt --all`: format the workspace.
- `cargo check --workspace --all-targets`: type-check all crates and targets.
- `cargo clippy --workspace --all-targets -- -D warnings`: run lint checks as CI expects.
- `cargo test --workspace`: run all Rust tests.
- `make verify`: run format, check, clippy, and tests in sequence.
- `docker compose up -d`: start the local Memgraph, Prometheus, and Grafana stack.
- `./scripts/wait-for-memgraph.sh && ./scripts/bootstrap-memgraph.sh`: wait for Memgraph and apply migrations.

## Coding Style & Naming Conventions

Use standard Rust formatting with four-space indentation through `rustfmt`. Keep `nestor-core` deterministic and free of I/O, clocks, database access, or HTTP concerns. Use `snake_case` for modules, functions, and variables; `PascalCase` for types and traits; and descriptive crate-local module names such as `activation`, `repository`, or `buffers`. Avoid `unwrap`, `dbg!`, and committed `todo!`; workspace lints deny them.

## Testing Guidelines

Place unit tests beside the code they verify and broader scenarios under `tests/integration/`. Use deterministic seeds for any noise-sensitive activation or retrieval behavior. Prefer tests that check ACT-R semantics directly: activation ordering, buffer mutation rules, production choice, and bounded Memgraph repository behavior. Run `cargo test --workspace` before submitting changes; use `make verify` for full local validation.

## Commit & Pull Request Guidelines

Use Conventional Commits for every commit subject: `<type>(<scope>): <description>`. Keep the description imperative, lower-case unless it starts with a proper noun, and under 72 characters when practical. Prefer scopes that match crates or repository areas, such as `core`, `session`, `rules`, `store`, `api`, `ops`, `docs`, `ci`, or `runtime`. Common types are `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `ci`, `build`, and `perf`; use `!` plus a body footer for breaking changes.

Keep each commit focused. Examples: `feat(rules): add retrieval condition matching`, `fix(store): bound candidate queries`, `docs: update Memgraph runbook`, and `ci: add workspace verification`. Pull requests should include a short problem/solution summary, linked issue or goal ID when relevant, test evidence, and notes for schema, API, or operational changes. Include screenshots only for UI or dashboard changes.

## Security & Configuration Tips

Do not commit secrets, local `.env` files, database dumps, or runtime volume data. Keep reusable examples in `.env.example` or docs. Memgraph migrations belong in `crates/nestor-store/migrations/`; local data belongs in ignored Docker volumes or runtime directories.
