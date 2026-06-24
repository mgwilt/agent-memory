# Architecture Notes

Nestor uses a hybrid ACT-R design with Rust-owned scoring and Memgraph-backed
persistence.

## Ownership Boundary

Rust owns:

- current goal, retrieval, imaginal, and task buffers;
- per-agent session serialization;
- base-level activation, spreading activation, mismatch, noise, thresholding, and latency;
- production matching, conflict resolution, and utility updates.

Memgraph owns:

- durable chunks and slot/value graph;
- association edges used as spreading-activation inputs;
- practice history and optional audit events;
- production rule metadata and utility summaries;
- schema introspection and operational metrics.

## Runtime Profiles

The Rust service loads `RuntimeConfig` from environment variables, constructs a
repository, and validates the config before binding the API listener.
`NESTOR_REPOSITORY` defaults to `memgraph`; `memory` is reserved for explicit
test or local fixture runs. `NESTOR_PROFILE` accepts `development`, `staging`, or
`production`:

- `development` binds the API to `127.0.0.1:8080`, uses local Bolt at
  `bolt://127.0.0.1:7687`, and leaves Memgraph credentials optional for local
  development.
- `staging` binds on `0.0.0.0:8080`, uses a private `bolt+s://` Memgraph URI,
  enables TLS, and requires a credential source.
- `production` uses the staging hardening defaults, disables deterministic
  runtime seeding, rejects loopback Memgraph URIs, and requires TLS plus a
  credential source.

Common overrides are `NESTOR_API_BIND_ADDR`, `NESTOR_MEMGRAPH_URI`,
`NESTOR_MEMGRAPH_USER`, `NESTOR_MEMGRAPH_MAX_CONNECTIONS`,
`NESTOR_CANDIDATE_LIMIT`, `NESTOR_RETRIEVAL_THRESHOLD`, and
`NESTOR_DETERMINISTIC_SEED`. TLS is controlled with
`NESTOR_MEMGRAPH_TLS_ENABLED`, `NESTOR_MEMGRAPH_TLS_CA_FILE`, and
`NESTOR_MEMGRAPH_TLS_SERVER_NAME`.

`/readyz` runs a real repository health check. With the Memgraph backend, this is
a Bolt query against Memgraph; with the explicit in-memory backend it reports the
test repository as ready.

## Observability

The Rust API exposes Prometheus text exposition at `/metrics`. The service
metrics include retrieval hits and misses, last retrieval latency, last
candidate count, last activation-compute duration, session-lock contention, and
write conflicts. Prometheus scrapes the API through the `nestor` job in
`config/prometheus/prometheus.yml`.

Memgraph is started with `--metrics-format=OpenMetrics` and
`--metrics-port=9091`. Prometheus scrapes it inside the Compose network at
`memgraph:9091`, while the host mapping is bound to `127.0.0.1:9091` for local
development only. The Bolt port follows the same local-only host binding.

## TLS And Credentials

Do not commit Memgraph passwords, client certificates, private keys, generated
CA material, or local `.env` files. Runtime credentials should be supplied by
the deployment environment through one of these sources:

- `NESTOR_MEMGRAPH_PASSWORD`: a secret value provided by the process environment.
- `NESTOR_MEMGRAPH_PASSWORD_ENV`: the name of an environment variable that will
  contain the secret.
- `NESTOR_MEMGRAPH_PASSWORD_FILE`: a path mounted from a secret manager, such as
  `/run/secrets/memgraph-password`.

Production deployments should use a private `bolt+s://` Memgraph endpoint,
enable `NESTOR_MEMGRAPH_TLS_ENABLED=true`, set
`NESTOR_MEMGRAPH_TLS_SERVER_NAME` to the certificate identity, and mount any CA
bundle through `NESTOR_MEMGRAPH_TLS_CA_FILE`.

## Retrieval Flow

1. Normalize symbolic cues in Rust.
2. Fetch a bounded candidate set from Memgraph using indexed labels/properties.
3. Hydrate practice history and association summaries.
4. Compute activation in Rust.
5. Threshold and tie-break ranked candidates.
6. Commit the retrieval buffer on a hit when requested, then return hit or miss
   diagnostics.

This deliberately avoids graph-only ACT-R scoring. Dynamic activation math and
deterministic tests are the reasons to keep scoring in Rust.

## Memory System Flow

The CLI and HTTP API share the same memory path. The CLI owns terminal
interaction and output rendering, while the API owns request handling and calls
the Rust memory modules.

```mermaid
flowchart TD
  Agent["CLI user or HTTP client"] --> CLI["nestor CLI"]
  Agent --> HTTP["Nestor HTTP API"]
  CLI --> Client["nestor-client<br/>typed transport"]
  Client --> HTTP
  HTTP --> Session["nestor-session<br/>buffers and per-agent state"]
  HTTP --> Store["nestor-store<br/>candidate and persistence repository"]
  Store --> Graph[("Memgraph<br/>chunks, slots, practice, associations")]
  Store --> Core["nestor-core<br/>activation scoring"]
  Session --> Core
  HTTP --> Rules["nestor-rules<br/>production evaluation"]
  Core --> HTTP
  Rules --> HTTP
  HTTP --> Result["Response<br/>memory, diagnostics, metrics"]
  Result --> Agent
```

## Retrieval Scoring Internals

Nestor uses Memgraph to keep candidate generation bounded and durable, then
scores every candidate in Rust so retrieval remains deterministic, testable, and
explainable.

```mermaid
flowchart TD
  Request["RetrievalRequest<br/>agent_id, chunk_type, cue_slots, context_chunk_ids, now_ms"] --> Validate["Validate finite params<br/>candidate_limit, threshold, decay, noise, latency factor"]
  Validate --> Query["Candidate query<br/>normalize cue slots, bound by candidate_limit"]
  Query --> Fetch["Repository fetch_candidates<br/>indexed chunk type and cue slot filters"]
  Fetch --> Candidate["ChunkWithHistory<br/>chunk + practice_events + spread_score"]
  Candidate --> Loop{"Score each candidate"}

  subgraph Inputs["Candidate inputs"]
    Events["Practice events<br/>occurred_at_ms, weight"]
    SpreadInput["Association summary<br/>precomputed spread_score"]
    Slots["Candidate slots<br/>compared with cue_slots"]
    Seed["Optional deterministic_seed<br/>stable per chunk id"]
  end

  subgraph Match["Mismatch policy"]
    Policy{"Configured policy"}
    Disabled["Disabled<br/>P = 0"]
    Exact["Exact<br/>missing or unequal cue slots get -mismatch_penalty"]
    Partial["Partial<br/>slot similarity in [-1, 0] times mismatch_penalty"]
  end

  subgraph Activation["Activation calculation"]
    Base["Base level<br/>B = ln(sum_j weight_j * age_j^-d)"]
    Spread["Spreading<br/>S = spread_score"]
    PartialScore["Partial match<br/>P from mismatch policy"]
    Noise["Noise<br/>N = s * ln(u / (1 - u)); 0 when disabled"]
    Total["Total activation<br/>A = B + S + P + N"]
    Pass["Threshold check<br/>passes when A >= retrieval_threshold"]
    Probability["Probability<br/>noise_s <= 0: hard 0 or 1<br/>else 1 / (1 + exp((threshold - A) / s))"]
    Latency["Latency<br/>predicted_latency_ms = F * exp(-A)"]
  end

  Loop --> Events --> Base
  Loop --> SpreadInput --> Spread
  Loop --> Slots --> Policy
  Policy --> Disabled --> PartialScore
  Policy --> Exact --> PartialScore
  Policy --> Partial --> PartialScore
  Loop --> Seed --> Noise
  Base --> Total
  Spread --> Total
  PartialScore --> Total
  Noise --> Total
  Total --> Pass
  Total --> Probability
  Total --> Latency

  Pass --> Ranked["Rank candidates<br/>activation descending, chunk id ascending"]
  Ranked --> Best{"Best candidate passes?"}
  Best -->|yes| Commit["Optional commit_on_hit<br/>set retrieval buffer in repository and session"]
  Best -->|no candidates| NoCandidates["Miss: no_candidates"]
  Best -->|below threshold| ThresholdMiss["Miss: threshold<br/>include best_activation"]
  Commit --> Hit["Hit response<br/>chunk + B/S/P/N/A + probability + latency"]
  NoCandidates --> Miss["Miss response<br/>diagnostics + candidate_count"]
  ThresholdMiss --> Miss
```

The response preserves the component breakdown as `base_level`, `spreading`,
`partial_match`, `noise`, `activation`, `retrieval_probability`, and
`predicted_latency_ms`. This makes CLI and API results mechanically checkable
without requiring callers to infer why a memory was retrieved or missed.
