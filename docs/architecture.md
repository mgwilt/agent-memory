# Architecture Notes

The scaffold follows the engineering plan's hybrid ACT-R design.

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

## Retrieval Flow

1. Normalize symbolic cues in Rust.
2. Fetch a bounded candidate set from Memgraph using indexed labels/properties.
3. Hydrate practice history and association summaries.
4. Compute activation in Rust.
5. Threshold, tie-break, and commit the retrieval buffer.
6. Record a practice event and optional audit data transactionally.

This deliberately avoids graph-only ACT-R scoring. The research reports identify
dynamic activation math and deterministic tests as the reasons to keep scoring in
Rust.
