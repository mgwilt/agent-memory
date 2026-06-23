# Goal

Build the retrieval pipeline from normalized cues to a ranked retrieval result.

# Context

Use Memgraph for bounded symbolic candidate generation and Rust for ACT-R
scoring: base-level activation, spreading activation, optional mismatch, noise,
thresholding, latency, and retrieval-buffer commit.

# Constraints

- Candidate set must be bounded, with a default cap of 200 chunks.
- Return score breakdowns.
- Deterministic seeds must make scores reproducible.
- Retrieval misses must be explicit.

# Done When

- Integration tests prove exact-match retrieval, threshold miss behavior,
  context-sensitive re-ranking, and score diagnostics.
