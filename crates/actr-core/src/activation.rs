use crate::{
    chunk::{Chunk, ChunkId, Slot, SlotValue},
    error::{MemoryError, MemoryResult},
};

const EPSILON_SECONDS: f64 = 1e-6;
const EPSILON_SCORE: f64 = 1e-12;
const EPSILON_PROBABILITY: f64 = 1e-12;
const MAX_EXPONENT: f64 = 709.0;

/// Parameters for ACT-R declarative-memory scoring.
///
/// The activation equation implemented by [`score_activation`] is:
///
/// `A_i = B_i + S_i + P_i + noise`
///
/// where `B_i` is base-level activation, `S_i` is spreading activation,
/// `P_i` is partial-match adjustment, and `noise` is an optional deterministic
/// activation perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivationParams {
    /// Decay parameter `d` in `B_i = ln(sum(t_j^-d))`.
    pub decay_d: f64,
    /// Minimum activation required for a retrieval hit.
    pub retrieval_threshold: f64,
    /// Logistic noise scale `s` used for retrieval probability.
    pub noise_s: f64,
    /// Latency factor `F` in `latency = F * e^-A`.
    pub latency_factor_ms: f64,
    /// Default mismatch penalty used by partial matching helpers.
    pub mismatch_penalty: f64,
}

impl Default for ActivationParams {
    fn default() -> Self {
        Self {
            decay_d: 0.5,
            retrieval_threshold: 0.0,
            noise_s: 0.0,
            latency_factor_ms: 350.0,
            mismatch_penalty: 0.5,
        }
    }
}

impl ActivationParams {
    /// Returns default scoring parameters with stochastic noise disabled.
    pub fn deterministic() -> Self {
        Self {
            noise_s: 0.0,
            ..Self::default()
        }
    }
}

/// A retrieval practice event contributing to base-level activation.
///
/// Each event contributes `weight * age_seconds^-d` to the base-level sum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeEvent {
    pub occurred_at_ms: u64,
    pub weight: f64,
}

impl PracticeEvent {
    pub fn new(occurred_at_ms: u64) -> Self {
        Self {
            occurred_at_ms,
            weight: 1.0,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Input to [`score_activation`].
#[derive(Debug, Clone, PartialEq)]
pub struct ActivationInput {
    pub now_ms: u64,
    pub practice_events: Vec<PracticeEvent>,
    pub spread_score: f64,
    pub partial_match_score: f64,
    pub noise: f64,
    pub params: ActivationParams,
}

/// Individual activation terms retained for score diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivationComponents {
    pub base_level: f64,
    pub spreading: f64,
    pub partial_match: f64,
    pub noise: f64,
}

impl ActivationComponents {
    /// Sums the activation components into a final activation score.
    pub fn total(self) -> f64 {
        self.base_level + self.spreading + self.partial_match + self.noise
    }
}

/// Complete declarative-memory scoring result for one chunk candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivationOutput {
    pub components: ActivationComponents,
    pub activation: f64,
    pub retrieval_probability: f64,
    pub predicted_latency_ms: f64,
    pub passes_threshold: bool,
}

impl ActivationOutput {
    /// Returns a threshold miss error when this score is below `threshold`.
    pub fn require_threshold(&self, threshold: f64) -> MemoryResult<()> {
        if self.passes_threshold {
            Ok(())
        } else {
            Err(MemoryError::ThresholdMiss {
                activation: self.activation,
                threshold,
            })
        }
    }
}

/// Score diagnostics for a concrete chunk candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredChunk {
    pub chunk_id: ChunkId,
    pub output: ActivationOutput,
}

impl ScoredChunk {
    pub fn new(chunk_id: ChunkId, output: ActivationOutput) -> Self {
        Self { chunk_id, output }
    }

    pub fn activation(&self) -> f64 {
        self.output.activation
    }

    pub fn require_threshold(&self, threshold: f64) -> MemoryResult<()> {
        self.output.require_threshold(threshold)
    }
}

/// One source of spreading activation.
///
/// ACT-R spreading activation is composed as `sum_j W_j * S_ji`, where
/// `weight` is the source attention weight and `strength` is the association
/// strength from that source to the candidate chunk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpreadingSource {
    pub weight: f64,
    pub strength: f64,
}

impl SpreadingSource {
    pub fn new(weight: f64, strength: f64) -> Self {
        Self { weight, strength }
    }

    pub fn contribution(self) -> f64 {
        self.weight * self.strength
    }
}

/// Parameters for partial matching.
///
/// Slot similarities use ACT-R's common convention where `0.0` means identical
/// and negative values represent mismatch severity. The final contribution is
/// `mismatch_penalty * similarity`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartialMatchingParams {
    pub mismatch_penalty: f64,
    pub missing_slot_penalty: f64,
}

impl Default for PartialMatchingParams {
    fn default() -> Self {
        Self {
            mismatch_penalty: ActivationParams::default().mismatch_penalty,
            missing_slot_penalty: ActivationParams::default().mismatch_penalty,
        }
    }
}

/// A domain-specific similarity override for one slot/value comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotSimilarity {
    pub key: String,
    pub requested: SlotValue,
    pub candidate: SlotValue,
    pub similarity: f64,
}

impl SlotSimilarity {
    pub fn new(
        key: impl Into<String>,
        requested: SlotValue,
        candidate: SlotValue,
        similarity: f64,
    ) -> Self {
        Self {
            key: key.into(),
            requested,
            candidate,
            similarity,
        }
    }

    fn matches(&self, key: &str, requested: &SlotValue, candidate: &SlotValue) -> bool {
        self.key == key && self.requested == *requested && self.candidate == *candidate
    }
}

/// Computes base-level activation `B_i = ln(sum_j weight_j * t_j^-d)`.
///
/// Event age is measured in seconds from `now_ms` and floored to a small
/// epsilon to keep coincident or future timestamps finite and deterministic.
pub fn base_level_activation(events: &[PracticeEvent], now_ms: u64, decay_d: f64) -> f64 {
    let decay = decay_d.max(0.0);
    let base_sum = events
        .iter()
        .map(|event| {
            let age_ms = now_ms.saturating_sub(event.occurred_at_ms);
            let age_seconds = (age_ms as f64 / 1_000.0).max(EPSILON_SECONDS);
            event.weight.max(0.0) * age_seconds.powf(-decay)
        })
        .sum::<f64>()
        .max(EPSILON_SCORE);

    base_sum.ln()
}

/// Computes spreading activation as `sum_j W_j * S_ji`.
pub fn spreading_activation(sources: &[SpreadingSource]) -> f64 {
    sources.iter().map(|source| source.contribution()).sum()
}

/// Computes retrieval probability from activation and threshold.
///
/// With positive `noise_s`, this is the logistic form
/// `1 / (1 + exp((threshold - activation) / noise_s))`. With `noise_s <= 0`,
/// the function becomes deterministic and returns `1.0` for threshold hits and
/// `0.0` for misses.
pub fn retrieval_probability(activation: f64, threshold: f64, noise_s: f64) -> f64 {
    if noise_s <= 0.0 {
        return if activation >= threshold { 1.0 } else { 0.0 };
    }

    let exponent = ((threshold - activation) / noise_s).clamp(-MAX_EXPONENT, MAX_EXPONENT);
    1.0 / (1.0 + exponent.exp())
}

/// Estimates retrieval latency as `F * e^-A`.
pub fn retrieval_latency_ms(activation: f64, latency_factor_ms: f64) -> f64 {
    latency_factor_ms.max(0.0) * (-activation).exp()
}

/// Scores one activation input and returns all diagnostic components.
pub fn score_activation(input: &ActivationInput) -> ActivationOutput {
    let base_level =
        base_level_activation(&input.practice_events, input.now_ms, input.params.decay_d);
    let components = ActivationComponents {
        base_level,
        spreading: input.spread_score,
        partial_match: input.partial_match_score,
        noise: input.noise,
    };
    let activation = components.total();
    let predicted_latency_ms = retrieval_latency_ms(activation, input.params.latency_factor_ms);

    ActivationOutput {
        components,
        activation,
        retrieval_probability: retrieval_probability(
            activation,
            input.params.retrieval_threshold,
            input.params.noise_s,
        ),
        predicted_latency_ms,
        passes_threshold: activation >= input.params.retrieval_threshold,
    }
}

/// Scores an activation input and attaches the candidate chunk identifier.
pub fn score_chunk(chunk: &Chunk, input: &ActivationInput) -> ScoredChunk {
    ScoredChunk::new(chunk.chunk_id.clone(), score_activation(input))
}

/// Returns scored chunks sorted by descending activation and then by chunk id.
pub fn rank_scored_chunks(mut chunks: Vec<ScoredChunk>) -> Vec<ScoredChunk> {
    chunks.sort_by(|left, right| {
        right
            .activation()
            .total_cmp(&left.activation())
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    chunks
}

/// Produces deterministic logistic activation noise for a seed and chunk id.
///
/// The returned value is `noise_s * ln(u / (1 - u))`, where `u` is a stable
/// hash-derived value in `(0, 1)`. Passing `scale <= 0` disables noise.
pub fn deterministic_noise(seed: u64, chunk_id: &str, scale: f64) -> f64 {
    if scale <= 0.0 {
        return 0.0;
    }

    let unit = deterministic_unit_interval(seed, chunk_id)
        .clamp(EPSILON_PROBABILITY, 1.0 - EPSILON_PROBABILITY);
    scale * (unit / (1.0 - unit)).ln()
}

/// Deterministically maps a seed and label into the open interval `(0, 1)`.
pub fn deterministic_unit_interval(seed: u64, label: &str) -> f64 {
    let mut hash = seed ^ 0xcbf2_9ce4_8422_2325;
    for byte in label.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    ((hash as f64) + 0.5) / ((u64::MAX as f64) + 1.0)
}

/// Computes a default ACT-R-style slot similarity in `[-1.0, 0.0]`.
///
/// Exact symbolic, text, numeric, and boolean matches return `0.0`. Numeric
/// mismatches degrade proportionally up to `-1.0`; other mismatches return
/// `-1.0`.
pub fn slot_similarity(requested: &SlotValue, candidate: &SlotValue) -> f64 {
    match (requested, candidate) {
        (SlotValue::Symbol(left), SlotValue::Symbol(right))
        | (SlotValue::Text(left), SlotValue::Text(right)) => {
            if left.trim().eq_ignore_ascii_case(right.trim()) {
                0.0
            } else {
                -1.0
            }
        }
        (SlotValue::Number(left), SlotValue::Number(right)) => {
            let denominator = left.abs().max(right.abs()).max(1.0);
            -((left - right).abs() / denominator).min(1.0)
        }
        (SlotValue::Bool(left), SlotValue::Bool(right)) => {
            if left == right {
                0.0
            } else {
                -1.0
            }
        }
        _ => -1.0,
    }
}

/// Computes partial-match adjustment for requested slots.
pub fn partial_match_score(
    candidate: &Chunk,
    requested: &[Slot],
    params: PartialMatchingParams,
) -> f64 {
    partial_match_score_with_similarities(candidate, requested, &[], params)
}

/// Computes partial-match adjustment with domain-specific similarity overrides.
pub fn partial_match_score_with_similarities(
    candidate: &Chunk,
    requested: &[Slot],
    similarities: &[SlotSimilarity],
    params: PartialMatchingParams,
) -> f64 {
    let mismatch_penalty = params.mismatch_penalty.max(0.0);
    let missing_slot_penalty = params.missing_slot_penalty.max(0.0);

    requested
        .iter()
        .map(|slot| match candidate.slot(&slot.key) {
            Some(value) => {
                let similarity = similarities
                    .iter()
                    .find(|rule| rule.matches(&slot.key, &slot.value, value))
                    .map_or_else(
                        || slot_similarity(&slot.value, value),
                        |rule| rule.similarity,
                    )
                    .clamp(-1.0, 0.0);

                mismatch_penalty * similarity
            }
            None => -missing_slot_penalty,
        })
        .sum()
}

/// Computes exact-match penalties for compatibility with callers that do not
/// want graded similarities.
pub fn exact_slot_match_score(candidate: &Chunk, requested: &[Slot], mismatch_penalty: f64) -> f64 {
    requested
        .iter()
        .map(|slot| match candidate.slot(&slot.key) {
            Some(value) if slot_values_match(value, &slot.value) => 0.0,
            Some(_) | None => -mismatch_penalty.max(0.0),
        })
        .sum()
}

fn slot_values_match(left: &SlotValue, right: &SlotValue) -> bool {
    left.value_type() == right.value_type() && left.normalized() == right.normalized()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{AgentId, Chunk, ChunkId, ChunkType, SlotValue};

    #[test]
    fn base_level_matches_single_event_formula() {
        let now_ms = 11_000;
        let events = [PracticeEvent::new(1_000)];
        let score = base_level_activation(&events, now_ms, 0.5);

        assert!((score - 10.0_f64.powf(-0.5).ln()).abs() < 1e-9);
    }

    #[test]
    fn spreading_activation_composes_weighted_sources() {
        let sources = [
            SpreadingSource::new(0.4, 1.5),
            SpreadingSource::new(0.6, -0.25),
        ];

        assert!((spreading_activation(&sources) - 0.45).abs() < 1e-12);
    }

    #[test]
    fn higher_activation_has_lower_latency() {
        let params = ActivationParams::default();
        let low = score_activation(&ActivationInput {
            now_ms: 10_000,
            practice_events: vec![PracticeEvent::new(1_000)],
            spread_score: 0.0,
            partial_match_score: 0.0,
            noise: 0.0,
            params,
        });
        let high = score_activation(&ActivationInput {
            spread_score: 2.0,
            ..ActivationInput {
                now_ms: 10_000,
                practice_events: vec![PracticeEvent::new(1_000)],
                spread_score: 0.0,
                partial_match_score: 0.0,
                noise: 0.0,
                params,
            }
        });

        assert!(high.activation > low.activation);
        assert!(high.predicted_latency_ms < low.predicted_latency_ms);
        assert!(
            retrieval_latency_ms(high.activation, 350.0)
                < retrieval_latency_ms(low.activation, 350.0)
        );
    }

    #[test]
    fn threshold_miss_is_explicit() {
        let output = score_activation(&ActivationInput {
            now_ms: 10_000,
            practice_events: vec![PracticeEvent::new(1_000)],
            spread_score: 0.0,
            partial_match_score: 0.0,
            noise: 0.0,
            params: ActivationParams {
                retrieval_threshold: 10.0,
                ..ActivationParams::default()
            },
        });

        assert!(!output.passes_threshold);
        assert_eq!(
            output.require_threshold(10.0),
            Err(MemoryError::ThresholdMiss {
                activation: output.activation,
                threshold: 10.0
            })
        );
    }

    #[test]
    fn deterministic_noise_is_reproducible() {
        assert_eq!(
            deterministic_noise(42, "ck-1", 0.25),
            deterministic_noise(42, "ck-1", 0.25)
        );
        assert_ne!(
            deterministic_noise(42, "ck-1", 0.25),
            deterministic_noise(42, "ck-2", 0.25)
        );
        assert_eq!(deterministic_noise(42, "ck-1", 0.0), 0.0);
    }

    #[test]
    fn deterministic_probability_becomes_hard_threshold() {
        assert_eq!(retrieval_probability(1.0, 0.0, 0.0), 1.0);
        assert_eq!(retrieval_probability(-1.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn exact_partial_matching_penalizes_mismatches() {
        let chunk = Chunk::new(
            AgentId("agent".to_string()),
            ChunkId("ck".to_string()),
            ChunkType("episodic".to_string()),
            0,
        )
        .with_slot("topic", SlotValue::Symbol("memgraph".to_string()));
        let requested = [Slot::new("topic", SlotValue::Symbol("act-r".to_string()))];

        assert_eq!(exact_slot_match_score(&chunk, &requested, 0.5), -0.5);
    }

    #[test]
    fn exact_partial_matching_uses_normalized_typed_values() {
        let chunk = Chunk::new(
            AgentId("agent".to_string()),
            ChunkId("ck".to_string()),
            ChunkType("episodic".to_string()),
            0,
        )
        .with_slot("topic", SlotValue::Symbol("act-r".to_string()));
        let requested = [Slot::new(
            "topic",
            SlotValue::Symbol("  ACT-R  ".to_string()),
        )];

        assert_eq!(exact_slot_match_score(&chunk, &requested, 0.5), 0.0);
    }

    #[test]
    fn partial_matching_supports_numeric_and_custom_similarity() {
        let chunk = Chunk::new(
            AgentId("agent".to_string()),
            ChunkId("ck".to_string()),
            ChunkType("episodic".to_string()),
            0,
        )
        .with_slot("size", SlotValue::Number(12.0))
        .with_slot("topic", SlotValue::Symbol("declarative".to_string()));
        let requested = [
            Slot::new("size", SlotValue::Number(10.0)),
            Slot::new("topic", SlotValue::Symbol("procedural".to_string())),
            Slot::new("missing", SlotValue::Bool(true)),
        ];
        let similarities = [SlotSimilarity::new(
            "topic",
            SlotValue::Symbol("procedural".to_string()),
            SlotValue::Symbol("declarative".to_string()),
            -0.25,
        )];
        let score = partial_match_score_with_similarities(
            &chunk,
            &requested,
            &similarities,
            PartialMatchingParams {
                mismatch_penalty: 2.0,
                missing_slot_penalty: 0.75,
            },
        );

        assert!((score - (-2.0 * (2.0 / 12.0) - 0.5 - 0.75)).abs() < 1e-12);
    }

    #[test]
    fn scored_chunks_rank_by_activation_then_chunk_id() {
        let chunks = rank_scored_chunks(vec![
            ScoredChunk::new(
                ChunkId("b".to_string()),
                ActivationOutput {
                    components: ActivationComponents {
                        base_level: 0.0,
                        spreading: 0.0,
                        partial_match: 0.0,
                        noise: 0.0,
                    },
                    activation: 1.0,
                    retrieval_probability: 1.0,
                    predicted_latency_ms: 1.0,
                    passes_threshold: true,
                },
            ),
            ScoredChunk::new(
                ChunkId("a".to_string()),
                ActivationOutput {
                    components: ActivationComponents {
                        base_level: 0.0,
                        spreading: 0.0,
                        partial_match: 0.0,
                        noise: 0.0,
                    },
                    activation: 1.0,
                    retrieval_probability: 1.0,
                    predicted_latency_ms: 1.0,
                    passes_threshold: true,
                },
            ),
        ]);

        assert_eq!(chunks[0].chunk_id, ChunkId("a".to_string()));
    }
}
