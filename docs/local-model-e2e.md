# Local Model E2E

Run the full local model memory workflow with:

```sh
pnpm e2e:agentic
```

The command:

- runs the deterministic Rust HTTP/formula integration test;
- verifies LM Studio is serving `qwen/qwen3.6-27b` at `http://localhost:1234/v1`;
- starts the Nestor API on an ephemeral localhost port;
- sends memory-write requests through AI SDK;
- exercises every HTTP API endpoint;
- validates created memory, retrieval behavior, production-rule selection, metrics, and retrieval formulas;
- sends answer-generation requests using retrieved memory; and
- writes run artifacts under `artifacts/e2e-agentic-memory/<timestamp>/`.

Useful overrides:

```sh
pnpm e2e:agentic -- --lmstudio-url http://localhost:1234/v1
pnpm e2e:agentic -- --model qwen/qwen3.6-27b
pnpm e2e:agentic -- --api-url http://127.0.0.1:8080
pnpm e2e:agentic -- --artifacts-dir /tmp/agent-memory-runs
```

Dependency installs use pnpm with exact versions and a `minimum-release-age` of
2880 minutes. Direct AI SDK dependencies are pinned in `package.json`, and the
resolved dependency tree is committed in `pnpm-lock.yaml`.
