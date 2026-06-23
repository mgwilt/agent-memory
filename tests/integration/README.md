# Integration Tests

Future Memgraph-backed tests belong here or in package-level `tests/`
directories once G04-G09 introduce a concrete async driver and testcontainers.

Required scenarios from the research reports:

- schema migration idempotency;
- chunk create/read/update/delete;
- append-only practice events under concurrency;
- context-sensitive spreading activation;
- retrieval threshold misses;
- production rule metadata persistence.
