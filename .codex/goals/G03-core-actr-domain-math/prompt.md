# Goal

Implement the ACT-R core math and domain model.

# Context

Add chunk, slot, activation component, scored chunk, retrieval threshold, latency
estimator, deterministic noise, partial matching, and utility update functions in
`actr-core`.

# Constraints

- Pure functions only.
- Deterministic mode for tests.
- Document formulas in Rustdoc when public behavior stabilizes.
- No database, HTTP, clock, or runtime dependencies in this crate.

# Done When

- `cargo test -p actr-core` passes.
- Tests cover base-level scoring, spreading composition, threshold misses,
  latency monotonicity, deterministic noise, partial matching, and utility math.
