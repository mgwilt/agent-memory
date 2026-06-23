# Goal

Add a local Memgraph development stack for the ACT-R memory project.

# Context

Use Docker Compose for the non-HA topology from the reports: one Memgraph
instance, local persistent volumes, Prometheus scraping, and deterministic schema
bootstrap scripts.

# Constraints

- No high availability or cluster routing.
- Expose Bolt and metrics ports.
- Keep schema bootstrap idempotent enough for repeated local development.
- Preserve the report detail that constraints do not replace indexes.

# Done When

- `docker compose up -d` starts Memgraph.
- Bootstrap Cypher creates constraints and indexes.
- A readiness script confirms the database responds.
- README documents startup and teardown.
