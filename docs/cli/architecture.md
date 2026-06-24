# CLI Architecture

[CLI docs index](./README.md)

The CLI is split into transport and presentation layers:

```text
actr-memory -> actr-client -> ACT-R HTTP API -> Rust service
```

`actr-client` owns typed API operations, URL construction, request serialization,
response decoding, timeout handling, and API error classification. It does not
know about terminals, help text, markdown, or stdout/stderr.

`actr-cli` owns command parsing, progressive help, examples, text output, JSON
output, and corrective errors. Command modules call `actr-client` operations
rather than constructing endpoint requests directly.

## Extension Rules

- Add a new API behavior as a typed operation in `actr-client` first.
- Add a new CLI action as one command module plus one command registration.
- Add output modes in `output.rs`, not in individual command modules.
- Add examples in `examples.rs` so help and docs share canonical strings.
- Keep HTTP logic out of `actr-cli`.
- Keep terminal rendering out of `actr-client`.

See also [Progressive Disclosure](./progressive-disclosure.md),
[Command Reference](./commands.md), and the broader
[system architecture](../architecture.md).
