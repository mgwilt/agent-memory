# Testing And Definition Of Done

[CLI docs index](./README.md)

## Test Matrix

- Client operation tests cover method/path/query construction and error mapping.
- Parser tests cover every command, slot values, JSON files, and config
  precedence.
- Help tests verify progressive-disclosure surfaces.
- Integration tests run the compiled CLI against an ephemeral API server.
- Docs tests validate crosslinks and command coverage.

Manual verification:

```sh
cargo run -p nestor-cli -- --help
cargo run -p nestor-cli -- guide commands
cargo run -p nestor-cli -- guide workflow
cargo test -p nestor-cli
```

Workspace verification:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Definition Of Done

- Root help lists all top-level commands.
- Every API route in `nestor_api::route_manifest()` is covered by CLI integration
  tests.
- Every leaf command has help, an example, and a docs section.
- `nestor guide commands` is compact enough for agent context.
- `nestor guide workflow` prints a copy-pasteable workflow.
- Every `docs/cli/` page links back to [the CLI index](./README.md).
- Root `README.md` links to `docs/cli/README.md`.
- JSON output is parseable for JSON-producing commands.
- Text output is stable and avoids debug formatting.
- Usage errors exit `2`.
- API/network failures exit `6`.
- Full workspace verification passes.

See [Command Reference](./commands.md) and [Output And Errors](./output-and-errors.md).
