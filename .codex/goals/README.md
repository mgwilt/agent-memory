# Codex Goals

Each directory is an atomic implementation package following the report's
`Goal, Context, Constraints, Done when` convention.

Required files:

- `goal.yaml`: id, dependencies, verification command, and artifact paths.
- `prompt.md`: prompt text for a Codex implementation pass.
- `output.schema.json`: stable final-output schema for non-interactive runs.
- `verify.sh`: local verification command for the goal.
- `artifacts.txt`: expected changed or created paths.

The dependency graph mirrors `research/ACT-R-Engineering-Plan.md`.
