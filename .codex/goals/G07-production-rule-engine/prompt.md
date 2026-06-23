# Goal

Implement the ACT-R procedural memory engine.

# Context

Rules should match against session buffers and optional retrieved chunks, then be
ranked by enablement, specificity, learned utility, and deterministic tie-breaks.
Rule metadata belongs in Memgraph; execution belongs in Rust.

# Constraints

- Store rule metadata separately from execution state.
- Support enable/disable and version fields.
- Keep conflict resolution inspectable.
- Tests must avoid stochastic utility selection unless seeded.

# Done When

- Tests cover rule matching, conflict resolution, reward updates, disabled rules,
  and a no-match path.
