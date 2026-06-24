#![forbid(unsafe_code)]

pub mod activation;
pub mod chunk;
pub mod error;
pub mod production;

pub use activation::{
    ActivationComponents, ActivationInput, ActivationOutput, ActivationParams,
    PartialMatchingParams, PracticeEvent, ScoredChunk, SlotSimilarity, SpreadingSource,
    base_level_activation, deterministic_noise, deterministic_unit_interval,
    exact_slot_match_score, partial_match_score, partial_match_score_with_similarities,
    rank_scored_chunks, retrieval_latency_ms, retrieval_probability, score_activation, score_chunk,
    slot_similarity, spreading_activation,
};
pub use chunk::{AgentId, Chunk, ChunkId, ChunkType, Slot, SlotValue};
pub use error::{MemoryError, MemoryResult};
pub use production::{
    ProductionUtility, UtilityUpdate, softmax_probabilities, update_utility_delta,
};
