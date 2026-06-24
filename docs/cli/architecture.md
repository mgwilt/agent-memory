# CLI Architecture

[CLI docs index](./README.md)

The CLI is split into transport and presentation layers:

```text
nestor -> nestor-client -> Nestor HTTP API -> Rust service
```

`nestor-client` owns typed API operations, URL construction, request serialization,
response decoding, timeout handling, and API error classification. It does not
know about terminals, help text, markdown, or stdout/stderr.

`nestor-cli` owns command parsing, progressive help, examples, text output, JSON
output, and corrective errors. Command modules call `nestor-client` operations
rather than constructing endpoint requests directly.

## Extension Rules

- Add a new API behavior as a typed operation in `nestor-client` first.
- Add a new CLI action as one command module plus one command registration.
- Add output modes in `output.rs`, not in individual command modules.
- Add examples in `examples.rs` so help and docs share canonical strings.
- Keep HTTP logic out of `nestor-cli`.
- Keep terminal rendering out of `nestor-client`.

See also [Progressive Disclosure](./progressive-disclosure.md),
[Command Reference](./commands.md), and the broader
[system architecture](../architecture.md).
