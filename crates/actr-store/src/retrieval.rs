use actr_core::{
    ActivationInput, ActivationOutput, ActivationParams, AgentId, Chunk, ChunkId, MemoryError,
    MemoryResult, PartialMatchingParams, Slot, SlotSimilarity, deterministic_noise,
    exact_slot_match_score, partial_match_score_with_similarities, score_activation,
};
use actr_session::{BufferName, SessionState};

use crate::repository::{
    BufferSetCurrent, CandidateQuery, ChunkWithHistory, DEFAULT_CANDIDATE_LIMIT, MemoryRepository,
    StoredSlot, bounded_candidate_limit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalStatus {
    Hit,
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalMissReason {
    NoCandidates,
    Threshold,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MismatchPolicy {
    Disabled,
    Exact,
    Partial {
        params: PartialMatchingParams,
        similarities: Vec<SlotSimilarity>,
    },
}

impl Default for MismatchPolicy {
    fn default() -> Self {
        Self::Partial {
            params: PartialMatchingParams::default(),
            similarities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalRequest {
    pub agent_id: AgentId,
    pub now_ms: u64,
    pub chunk_type: Option<String>,
    pub cue_slots: Vec<Slot>,
    pub context_chunk_ids: Vec<ChunkId>,
    pub candidate_limit: usize,
    pub activation_params: ActivationParams,
    pub mismatch_policy: MismatchPolicy,
    pub deterministic_seed: Option<u64>,
    pub commit_on_hit: bool,
}

impl RetrievalRequest {
    pub fn new(agent_id: AgentId, now_ms: u64) -> Self {
        Self {
            agent_id,
            now_ms,
            chunk_type: None,
            cue_slots: Vec::new(),
            context_chunk_ids: Vec::new(),
            candidate_limit: DEFAULT_CANDIDATE_LIMIT,
            activation_params: ActivationParams::deterministic(),
            mismatch_policy: MismatchPolicy::default(),
            deterministic_seed: None,
            commit_on_hit: true,
        }
    }

    pub fn validate(&self) -> MemoryResult<()> {
        if self.agent_id.as_str().trim().is_empty() {
            return Err(MemoryError::Validation(
                "agent_id must not be empty".to_string(),
            ));
        }
        if self.activation_params.retrieval_threshold.is_finite()
            && self.activation_params.decay_d.is_finite()
            && self.activation_params.noise_s.is_finite()
            && self.activation_params.latency_factor_ms.is_finite()
            && self.activation_params.mismatch_penalty.is_finite()
        {
            bounded_candidate_limit(self.candidate_limit).map_or_else(
                || {
                    Err(MemoryError::Validation(format!(
                        "candidate_limit must be between 1 and {DEFAULT_CANDIDATE_LIMIT}"
                    )))
                },
                |_| Ok(()),
            )
        } else {
            Err(MemoryError::Validation(
                "activation parameters must be finite".to_string(),
            ))
        }
    }

    pub fn candidate_query(&self) -> CandidateQuery {
        CandidateQuery {
            agent_id: self.agent_id.clone(),
            chunk_type: self.chunk_type.clone(),
            cue_slots: self.cue_slots.clone(),
            context_chunk_ids: self.context_chunk_ids.clone(),
            candidate_limit: self.candidate_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalScoreBreakdown {
    pub base_level: f64,
    pub spreading: f64,
    pub partial_match: f64,
    pub noise: f64,
    pub activation: f64,
    pub retrieval_probability: f64,
    pub predicted_latency_ms: f64,
    pub passes_threshold: bool,
}

impl From<ActivationOutput> for RetrievalScoreBreakdown {
    fn from(output: ActivationOutput) -> Self {
        Self {
            base_level: output.components.base_level,
            spreading: output.components.spreading,
            partial_match: output.components.partial_match,
            noise: output.components.noise,
            activation: output.activation,
            retrieval_probability: output.retrieval_probability,
            predicted_latency_ms: output.predicted_latency_ms,
            passes_threshold: output.passes_threshold,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedRetrievalCandidate {
    pub chunk: Chunk,
    pub score: RetrievalScoreBreakdown,
    pub practice_event_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalHit {
    pub chunk: Chunk,
    pub score: RetrievalScoreBreakdown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalMiss {
    pub reason: RetrievalMissReason,
    pub threshold: f64,
    pub best_activation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalDiagnostics {
    pub candidates_examined: usize,
    pub candidate_limit: usize,
    pub normalized_cue_slots: Vec<StoredSlot>,
    pub context_chunk_count: usize,
    pub deterministic_seed: Option<u64>,
    pub threshold: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalOutcome {
    pub agent_id: AgentId,
    pub status: RetrievalStatus,
    pub hit: Option<RetrievalHit>,
    pub miss: Option<RetrievalMiss>,
    pub ranked_candidates: Vec<RankedRetrievalCandidate>,
    pub diagnostics: RetrievalDiagnostics,
}

pub fn retrieve_chunk<R: MemoryRepository>(
    repository: &R,
    session: &mut SessionState,
    request: RetrievalRequest,
) -> MemoryResult<RetrievalOutcome> {
    request.validate()?;
    let query = request.candidate_query();
    query.validate()?;
    let normalized_cue_slots = query.normalized_cue_slots();
    let candidates = repository.fetch_candidates(query)?;
    let ranked_candidates = rank_retrieval_candidates(candidates, &request);
    let diagnostics = RetrievalDiagnostics {
        candidates_examined: ranked_candidates.len(),
        candidate_limit: request.candidate_limit,
        normalized_cue_slots,
        context_chunk_count: request.context_chunk_ids.len(),
        deterministic_seed: request.deterministic_seed,
        threshold: request.activation_params.retrieval_threshold,
    };

    let Some(best) = ranked_candidates.first() else {
        return Ok(RetrievalOutcome {
            agent_id: request.agent_id,
            status: RetrievalStatus::Miss,
            hit: None,
            miss: Some(RetrievalMiss {
                reason: RetrievalMissReason::NoCandidates,
                threshold: request.activation_params.retrieval_threshold,
                best_activation: None,
            }),
            ranked_candidates,
            diagnostics,
        });
    };

    if !best.score.passes_threshold {
        return Ok(RetrievalOutcome {
            agent_id: request.agent_id,
            status: RetrievalStatus::Miss,
            hit: None,
            miss: Some(RetrievalMiss {
                reason: RetrievalMissReason::Threshold,
                threshold: request.activation_params.retrieval_threshold,
                best_activation: Some(best.score.activation),
            }),
            ranked_candidates,
            diagnostics,
        });
    }

    if request.commit_on_hit {
        repository.set_buffer_current(BufferSetCurrent {
            agent_id: request.agent_id.clone(),
            buffer_name: BufferName::Retrieval,
            chunk_id: best.chunk.chunk_id.clone(),
            set_at_ms: request.now_ms,
        })?;
        session.commit_retrieval(
            best.chunk.chunk_id.clone(),
            best.chunk.chunk_type.clone(),
            request.now_ms,
        );
    }

    Ok(RetrievalOutcome {
        agent_id: request.agent_id,
        status: RetrievalStatus::Hit,
        hit: Some(RetrievalHit {
            chunk: best.chunk.clone(),
            score: best.score.clone(),
        }),
        miss: None,
        ranked_candidates,
        diagnostics,
    })
}

fn rank_retrieval_candidates(
    candidates: Vec<ChunkWithHistory>,
    request: &RetrievalRequest,
) -> Vec<RankedRetrievalCandidate> {
    let mut scored = candidates
        .into_iter()
        .map(|candidate| {
            let partial_match_score = mismatch_score(&candidate.chunk, request);
            let noise = request.deterministic_seed.map_or(0.0, |seed| {
                deterministic_noise(
                    seed,
                    candidate.chunk.chunk_id.as_str(),
                    request.activation_params.noise_s,
                )
            });
            let output = score_activation(&ActivationInput {
                now_ms: request.now_ms,
                practice_events: candidate.practice_events.clone(),
                spread_score: candidate.spread_score,
                partial_match_score,
                noise,
                params: request.activation_params,
            });

            RankedRetrievalCandidate {
                chunk: candidate.chunk,
                score: output.into(),
                practice_event_count: candidate.practice_events.len(),
            }
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .activation
            .total_cmp(&left.score.activation)
            .then_with(|| left.chunk.chunk_id.cmp(&right.chunk.chunk_id))
    });
    scored
}

fn mismatch_score(candidate: &Chunk, request: &RetrievalRequest) -> f64 {
    match &request.mismatch_policy {
        MismatchPolicy::Disabled => 0.0,
        MismatchPolicy::Exact => exact_slot_match_score(
            candidate,
            &request.cue_slots,
            request.activation_params.mismatch_penalty,
        ),
        MismatchPolicy::Partial {
            params,
            similarities,
        } => partial_match_score_with_similarities(
            candidate,
            &request.cue_slots,
            similarities,
            *params,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actr_core::{ChunkType, PracticeEvent, SlotValue};

    #[test]
    fn request_uses_default_candidate_cap() {
        let request = RetrievalRequest::new(AgentId::from("agent-1"), 1_000);

        assert_eq!(request.candidate_limit, DEFAULT_CANDIDATE_LIMIT);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn ranking_uses_activation_then_chunk_id() {
        let request = RetrievalRequest {
            activation_params: ActivationParams {
                retrieval_threshold: -10.0,
                ..ActivationParams::deterministic()
            },
            ..RetrievalRequest::new(AgentId::from("agent-1"), 2_000)
        };
        let candidates = vec![
            ChunkWithHistory {
                chunk: Chunk::new(
                    AgentId::from("agent-1"),
                    ChunkId::from("b"),
                    ChunkType::from("fact"),
                    1_000,
                ),
                practice_events: vec![PracticeEvent::new(1_000)],
                spread_score: 0.0,
            },
            ChunkWithHistory {
                chunk: Chunk::new(
                    AgentId::from("agent-1"),
                    ChunkId::from("a"),
                    ChunkType::from("fact"),
                    1_000,
                ),
                practice_events: vec![PracticeEvent::new(1_000)],
                spread_score: 0.0,
            },
        ];

        let ranked = rank_retrieval_candidates(candidates, &request);

        assert_eq!(ranked[0].chunk.chunk_id, ChunkId::from("a"));
    }

    #[test]
    fn exact_mismatch_policy_penalizes_wrong_slots() {
        let chunk = Chunk::new(
            AgentId::from("agent-1"),
            ChunkId::from("ck"),
            ChunkType::from("fact"),
            1_000,
        )
        .with_slot("topic", SlotValue::Symbol("rust".to_string()));
        let mut request = RetrievalRequest::new(AgentId::from("agent-1"), 2_000);
        request.cue_slots = vec![Slot::new(
            "topic",
            SlotValue::Symbol("memgraph".to_string()),
        )];
        request.mismatch_policy = MismatchPolicy::Exact;

        assert_eq!(
            mismatch_score(&chunk, &request),
            -request.activation_params.mismatch_penalty
        );
    }
}
