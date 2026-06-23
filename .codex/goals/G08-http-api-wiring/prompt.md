# Goal

Expose the memory system as an HTTP API.

# Context

Use Axum in `actr-api` for chunk upsert, retrieval, practice recording,
association updates, rule evaluation, health, readiness, and metrics. DTOs should
mirror the report's request and response examples.

# Constraints

- Validate inputs.
- Return JSON only.
- Map domain errors to stable HTTP problem details.
- Include `/healthz`, `/readyz`, and `/metrics`.

# Done When

- End-to-end API tests pass.
- Example curl requests in docs match the implemented endpoints.
- `cargo run -p actr-api` starts the service.
