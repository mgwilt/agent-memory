use std::{cell::RefCell, cmp::Ordering, collections::BTreeMap};

use actr_core::{
    ActivationParams, AgentId, Chunk, ChunkId, ChunkType, MemoryError, MemoryResult, PracticeEvent,
    Slot, SlotValue,
};
use actr_rules::RuleId;
use actr_session::{BufferName, SessionState};
use actr_store::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, CreateChunk,
    DEFAULT_CANDIDATE_LIMIT, MemoryRepository, MismatchPolicy, PracticeEventWrite,
    ProductionRuleRecord, RetrievalMissReason, RetrievalRequest, RetrievalStatus, UpdateChunk,
    retrieve_chunk,
};

#[derive(Debug, Clone)]
struct StoredChunkState {
    chunk: Chunk,
    active: bool,
}

#[derive(Debug, Default)]
struct RecordingRepository {
    chunks: RefCell<BTreeMap<(AgentId, ChunkId), StoredChunkState>>,
    practice_events: RefCell<BTreeMap<String, PracticeEventWrite>>,
    associations: RefCell<BTreeMap<(AgentId, ChunkId, ChunkId, String), AssociationWrite>>,
    buffers: RefCell<BTreeMap<(AgentId, BufferName), ChunkId>>,
    candidate_queries: RefCell<Vec<CandidateQuery>>,
}

impl RecordingRepository {
    fn create_fact(&self, chunk_id: &str, topic: &str, created_at_ms: u64) -> MemoryResult<Chunk> {
        self.create_chunk(CreateChunk {
            chunk: Chunk::new(
                AgentId::from("agent-1"),
                ChunkId::from(chunk_id),
                ChunkType::from("fact"),
                created_at_ms,
            )
            .with_slot("topic", SlotValue::Symbol(topic.to_string())),
            initial_practice_event_id: format!("encode-{chunk_id}"),
        })
    }

    fn buffer_chunk(&self, agent_id: &AgentId, buffer_name: &BufferName) -> Option<ChunkId> {
        self.buffers
            .borrow()
            .get(&(agent_id.clone(), buffer_name.clone()))
            .cloned()
    }

    fn last_candidate_query(&self) -> Option<CandidateQuery> {
        self.candidate_queries.borrow().last().cloned()
    }
}

impl MemoryRepository for RecordingRepository {
    fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let key = (req.chunk.agent_id.clone(), req.chunk.chunk_id.clone());
        if self.chunks.borrow().contains_key(&key) {
            return Err(MemoryError::Conflict(format!(
                "chunk {} already exists",
                req.chunk.chunk_id.as_str()
            )));
        }
        self.practice_events.borrow_mut().insert(
            req.initial_practice_event_id.clone(),
            PracticeEventWrite {
                event_id: req.initial_practice_event_id,
                agent_id: req.chunk.agent_id.clone(),
                chunk_id: req.chunk.chunk_id.clone(),
                occurred_at_ms: req.chunk.created_at_ms,
                kind: "encode".to_string(),
                weight: 1.0,
            },
        );
        self.chunks.borrow_mut().insert(
            key,
            StoredChunkState {
                chunk: req.chunk.clone(),
                active: true,
            },
        );
        Ok(req.chunk)
    }

    fn get_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<Option<Chunk>> {
        Ok(self
            .chunks
            .borrow()
            .get(&(agent_id.clone(), chunk_id.clone()))
            .filter(|stored| stored.active)
            .map(|stored| stored.chunk.clone()))
    }

    fn update_chunk(&self, _req: UpdateChunk) -> MemoryResult<Chunk> {
        Err(MemoryError::Validation(
            "update_chunk is not needed by retrieval pipeline tests".to_string(),
        ))
    }

    fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()> {
        let mut chunks = self.chunks.borrow_mut();
        let stored = chunks
            .get_mut(&(agent_id.clone(), chunk_id.clone()))
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
        stored.active = false;
        Ok(())
    }

    fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
        req.validate()?;
        if self.practice_events.borrow().contains_key(&req.event_id) {
            return Err(MemoryError::Conflict(format!(
                "practice event {} already exists",
                req.event_id
            )));
        }
        self.practice_events
            .borrow_mut()
            .insert(req.event_id.clone(), req);
        Ok(())
    }

    fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
        req.validate()?;
        self.candidate_queries.borrow_mut().push(req.clone());

        let chunks = self.chunks.borrow();
        let events = self.practice_events.borrow();
        let associations = self.associations.borrow();
        let mut candidates = chunks
            .values()
            .filter(|stored| stored.active)
            .filter(|stored| stored.chunk.agent_id == req.agent_id)
            .filter(|stored| {
                req.chunk_type
                    .as_ref()
                    .is_none_or(|chunk_type| stored.chunk.chunk_type.as_str() == chunk_type)
            })
            .filter_map(|stored| {
                let cue_matches = normalized_cue_match_count(&stored.chunk, &req.cue_slots);
                if !req.cue_slots.is_empty() && cue_matches == 0 {
                    return None;
                }

                let spread_score = req
                    .context_chunk_ids
                    .iter()
                    .filter_map(|context_id| {
                        associations
                            .values()
                            .filter(|association| {
                                association.agent_id == req.agent_id
                                    && association.src_chunk_id == *context_id
                                    && association.dst_chunk_id == stored.chunk.chunk_id
                            })
                            .map(|association| association.strength)
                            .max_by(|left, right| {
                                left.partial_cmp(right).unwrap_or(Ordering::Equal)
                            })
                    })
                    .sum::<f64>();

                let practice_events = events
                    .values()
                    .filter(|event| {
                        event.agent_id == stored.chunk.agent_id
                            && event.chunk_id == stored.chunk.chunk_id
                    })
                    .map(|event| PracticeEvent {
                        occurred_at_ms: event.occurred_at_ms,
                        weight: event.weight,
                    })
                    .collect::<Vec<_>>();

                Some((
                    cue_matches,
                    spread_score,
                    stored.chunk.clone(),
                    practice_events,
                ))
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal))
                .then_with(|| left.2.chunk_id.cmp(&right.2.chunk_id))
        });

        Ok(candidates
            .into_iter()
            .take(req.candidate_limit)
            .map(
                |(_, spread_score, chunk, practice_events)| ChunkWithHistory {
                    chunk,
                    practice_events,
                    spread_score,
                },
            )
            .collect())
    }

    fn upsert_association(&self, req: AssociationWrite) -> MemoryResult<()> {
        req.validate()?;
        self.associations.borrow_mut().insert(
            (
                req.agent_id.clone(),
                req.src_chunk_id.clone(),
                req.dst_chunk_id.clone(),
                req.source.clone(),
            ),
            req,
        );
        Ok(())
    }

    fn set_buffer_current(&self, req: BufferSetCurrent) -> MemoryResult<()> {
        req.validate()?;
        self.buffers.borrow_mut().insert(
            (req.agent_id.clone(), req.buffer_name.clone()),
            req.chunk_id.clone(),
        );
        Ok(())
    }

    fn upsert_production_rule(
        &self,
        req: ProductionRuleRecord,
    ) -> MemoryResult<ProductionRuleRecord> {
        Ok(req)
    }

    fn get_production_rule(
        &self,
        _agent_id: &AgentId,
        _rule_id: &RuleId,
    ) -> MemoryResult<Option<ProductionRuleRecord>> {
        Ok(None)
    }
}

fn normalized_cue_match_count(chunk: &Chunk, cues: &[Slot]) -> usize {
    cues.iter()
        .filter(|cue| {
            chunk.slot(&cue.key).is_some_and(|value| {
                value.value_type() == cue.value.value_type()
                    && value.normalized() == cue.value.normalized()
            })
        })
        .count()
}

fn hit_request(now_ms: u64) -> RetrievalRequest {
    let mut request = RetrievalRequest::new(AgentId::from("agent-1"), now_ms);
    request.chunk_type = Some("fact".to_string());
    request.cue_slots = vec![Slot::new(
        "topic",
        SlotValue::Symbol("  ACT-R  ".to_string()),
    )];
    request.activation_params = ActivationParams {
        retrieval_threshold: -1.0,
        ..ActivationParams::deterministic()
    };
    request
}

fn snapshot_retrieval_chunk(session: &SessionState) -> Option<ChunkId> {
    session
        .snapshot()
        .into_iter()
        .find(|buffer| buffer.name == BufferName::Retrieval)
        .and_then(|buffer| buffer.chunk_id)
}

#[test]
fn retrieval_pipeline_exact_match_commits_retrieval_buffer() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    let chunk = repo.create_fact("ck-actr", "act-r", 1_000)?;
    let mut session = SessionState::new(AgentId::from("agent-1"));

    let outcome = retrieve_chunk(&repo, &mut session, hit_request(2_000))?;

    assert_eq!(outcome.status, RetrievalStatus::Hit);
    assert_eq!(
        outcome.hit.as_ref().map(|hit| hit.chunk.chunk_id.clone()),
        Some(chunk.chunk_id.clone())
    );
    assert_eq!(
        snapshot_retrieval_chunk(&session),
        Some(chunk.chunk_id.clone())
    );
    assert_eq!(
        repo.buffer_chunk(&AgentId::from("agent-1"), &BufferName::Retrieval),
        Some(chunk.chunk_id)
    );
    assert_eq!(
        repo.last_candidate_query()
            .map(|query| query.candidate_limit),
        Some(DEFAULT_CANDIDATE_LIMIT)
    );
    assert_eq!(
        outcome.diagnostics.normalized_cue_slots[0].value_norm,
        "act-r"
    );
    Ok(())
}

#[test]
fn retrieval_pipeline_threshold_miss_is_explicit_and_does_not_commit() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    repo.create_fact("ck-actr", "act-r", 1_000)?;
    let mut session = SessionState::new(AgentId::from("agent-1"));
    let mut request = hit_request(2_000);
    request.activation_params.retrieval_threshold = 10.0;

    let outcome = retrieve_chunk(&repo, &mut session, request)?;

    assert_eq!(outcome.status, RetrievalStatus::Miss);
    assert_eq!(
        outcome.miss.as_ref().map(|miss| miss.reason),
        Some(RetrievalMissReason::Threshold)
    );
    assert!(outcome.hit.is_none());
    assert!(outcome.ranked_candidates[0].score.activation < 10.0);
    assert_eq!(snapshot_retrieval_chunk(&session), None);
    assert_eq!(
        repo.buffer_chunk(&AgentId::from("agent-1"), &BufferName::Retrieval),
        None
    );
    Ok(())
}

#[test]
fn retrieval_pipeline_empty_candidate_miss_is_explicit() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    repo.create_fact("ck-rust", "rust", 1_000)?;
    let mut session = SessionState::new(AgentId::from("agent-1"));

    let outcome = retrieve_chunk(&repo, &mut session, hit_request(2_000))?;

    assert_eq!(outcome.status, RetrievalStatus::Miss);
    assert_eq!(
        outcome.miss.as_ref().map(|miss| miss.reason),
        Some(RetrievalMissReason::NoCandidates)
    );
    assert!(outcome.ranked_candidates.is_empty());
    assert_eq!(outcome.diagnostics.candidates_examined, 0);
    assert_eq!(snapshot_retrieval_chunk(&session), None);
    Ok(())
}

#[test]
fn retrieval_pipeline_context_sensitive_spread_reranks_candidates() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    let context = repo.create_fact("ck-context", "goal", 1_000)?;
    let weak = repo.create_fact("ck-weak", "act-r", 1_000)?;
    let strong = repo.create_fact("ck-strong", "act-r", 1_000)?;
    repo.upsert_association(AssociationWrite {
        agent_id: AgentId::from("agent-1"),
        src_chunk_id: context.chunk_id.clone(),
        dst_chunk_id: strong.chunk_id.clone(),
        source: "goal".to_string(),
        strength: 2.0,
        fan: 1,
        updated_at_ms: 2_000,
    })?;
    repo.upsert_association(AssociationWrite {
        agent_id: AgentId::from("agent-1"),
        src_chunk_id: context.chunk_id.clone(),
        dst_chunk_id: weak.chunk_id,
        source: "goal".to_string(),
        strength: 0.1,
        fan: 1,
        updated_at_ms: 2_000,
    })?;
    let mut session = SessionState::new(AgentId::from("agent-1"));
    let mut request = hit_request(2_000);
    request.context_chunk_ids = vec![context.chunk_id];

    let outcome = retrieve_chunk(&repo, &mut session, request)?;

    assert_eq!(outcome.status, RetrievalStatus::Hit);
    assert_eq!(
        outcome.hit.as_ref().map(|hit| hit.chunk.chunk_id.clone()),
        Some(strong.chunk_id.clone())
    );
    assert_eq!(outcome.ranked_candidates[0].chunk.chunk_id, strong.chunk_id);
    assert!(outcome.ranked_candidates[0].score.spreading > 1.9);
    assert!(
        outcome.ranked_candidates[0].score.activation
            > outcome.ranked_candidates[1].score.activation
    );
    Ok(())
}

#[test]
fn retrieval_pipeline_returns_score_diagnostics_and_reproducible_noise() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    let chunk = repo.create_fact("ck-actr", "act-r", 1_000)?;
    let mut first_session = SessionState::new(AgentId::from("agent-1"));
    let mut second_session = SessionState::new(AgentId::from("agent-1"));
    let mut request = hit_request(5_000);
    request.activation_params.retrieval_threshold = -10.0;
    request.activation_params.noise_s = 0.25;
    request.deterministic_seed = Some(42);
    request.mismatch_policy = MismatchPolicy::Exact;

    let first = retrieve_chunk(&repo, &mut first_session, request.clone())?;
    let second = retrieve_chunk(&repo, &mut second_session, request)?;

    assert_eq!(first.status, RetrievalStatus::Hit);
    assert_eq!(first.diagnostics.candidates_examined, 1);
    assert_eq!(first.diagnostics.deterministic_seed, Some(42));
    assert_eq!(
        first.hit.as_ref().map(|hit| hit.chunk.chunk_id.clone()),
        Some(chunk.chunk_id)
    );
    let first_score = &first.ranked_candidates[0].score;
    let second_score = &second.ranked_candidates[0].score;
    assert_eq!(first_score.noise, second_score.noise);
    assert_eq!(first_score.activation, second_score.activation);
    assert_eq!(first_score.partial_match, 0.0);
    assert!(first_score.retrieval_probability > 0.0);
    assert!(first_score.predicted_latency_ms.is_finite());
    Ok(())
}

#[test]
fn retrieval_pipeline_can_rank_without_committing_buffers() -> MemoryResult<()> {
    let repo = RecordingRepository::default();
    let chunk = repo.create_fact("ck-actr", "act-r", 1_000)?;
    let mut session = SessionState::new(AgentId::from("agent-1"));
    let mut request = hit_request(2_000);
    request.commit_on_hit = false;

    let outcome = retrieve_chunk(&repo, &mut session, request)?;

    assert_eq!(outcome.status, RetrievalStatus::Hit);
    assert_eq!(
        outcome.hit.as_ref().map(|hit| hit.chunk.chunk_id.clone()),
        Some(chunk.chunk_id)
    );
    assert_eq!(snapshot_retrieval_chunk(&session), None);
    assert_eq!(
        repo.buffer_chunk(&AgentId::from("agent-1"), &BufferName::Retrieval),
        None
    );
    assert_eq!(outcome.ranked_candidates.len(), 1);
    Ok(())
}
