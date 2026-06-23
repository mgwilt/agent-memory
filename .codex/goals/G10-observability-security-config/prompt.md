# Goal

Harden observability, security, and runtime configuration.

# Context

Expose Prometheus metrics from the Rust service, document Memgraph OpenMetrics,
support dev/stage/prod config profiles, and prepare TLS/auth settings without
committing secrets.

# Constraints

- Secrets must not be committed.
- Memgraph should stay private except for local development ports.
- Service metrics should include retrieval hits/misses, latency, candidates,
  activation compute time, session lock contention, and write conflicts.

# Done When

- `/metrics` exposes service metrics.
- Config validation tests pass.
- Deployment docs cover Memgraph TLS and credential handling.
