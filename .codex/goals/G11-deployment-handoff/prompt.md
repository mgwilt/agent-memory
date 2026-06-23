# Goal

Create the deployment artifact and handoff package.

# Context

A fresh checkout should be able to build the service image, start the local
stack, bootstrap schema, and run a demo retrieval scenario.

# Constraints

- Single-node Memgraph only.
- Keep runbooks practical and command-oriented.
- Include troubleshooting for schema bootstrap, Bolt connectivity, and metrics.

# Done When

- The Docker image builds.
- Compose starts the local stack.
- Bootstrap scripts run.
- A demo retrieval script passes.
