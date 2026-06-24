use std::{
    cmp::Ordering,
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
};

use nestor_core::{AgentId, Chunk, ChunkId, MemoryError, MemoryResult, PracticeEvent, Slot};
use nestor_ops::MetricSample;
use nestor_rules::{RuleEngine, RuleId};
use nestor_session::{BufferName, BufferSnapshot, SessionRegistry};
use nestor_store::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, CreateChunk,
    MemoryRepository, PracticeEventWrite, ProductionRuleRecord as StoreProductionRuleRecord,
    UpdateChunk,
};

#[derive(Debug, Clone)]
struct StoredChunkState {
    chunk: Chunk,
    version: u64,
    active: bool,
}

#[derive(Debug, Default)]
struct MemoryStore {
    chunks: BTreeMap<(AgentId, ChunkId), StoredChunkState>,
    practice_events: BTreeMap<String, PracticeEventWrite>,
    associations: BTreeMap<(AgentId, ChunkId, ChunkId, String), AssociationWrite>,
    buffers: BTreeMap<(AgentId, BufferName), ChunkId>,
    rules: BTreeMap<(AgentId, RuleId), StoreProductionRuleRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct ApiRepository {
    inner: Arc<Mutex<MemoryStore>>,
}

impl ApiRepository {
    pub fn upsert_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let key = (req.chunk.agent_id.clone(), req.chunk.chunk_id.clone());
        let practice = PracticeEventWrite {
            event_id: req.initial_practice_event_id,
            agent_id: req.chunk.agent_id.clone(),
            chunk_id: req.chunk.chunk_id.clone(),
            occurred_at_ms: req.chunk.created_at_ms,
            kind: "encode".to_string(),
            weight: 1.0,
        };
        practice.validate()?;

        let mut store = lock_store(&self.inner)?;
        store
            .practice_events
            .insert(practice.event_id.clone(), practice);
        store.chunks.insert(
            key,
            StoredChunkState {
                chunk: req.chunk.clone(),
                version: 1,
                active: true,
            },
        );
        Ok(req.chunk)
    }

    pub fn buffer_snapshot(
        &self,
        agent_id: &AgentId,
        buffer_name: &BufferName,
    ) -> MemoryResult<Option<ChunkId>> {
        let store = lock_store(&self.inner)?;
        Ok(store
            .buffers
            .get(&(agent_id.clone(), buffer_name.clone()))
            .cloned())
    }
}

impl MemoryRepository for ApiRepository {
    fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let key = (req.chunk.agent_id.clone(), req.chunk.chunk_id.clone());
        let practice = PracticeEventWrite {
            event_id: req.initial_practice_event_id,
            agent_id: req.chunk.agent_id.clone(),
            chunk_id: req.chunk.chunk_id.clone(),
            occurred_at_ms: req.chunk.created_at_ms,
            kind: "encode".to_string(),
            weight: 1.0,
        };
        practice.validate()?;

        let mut store = lock_store(&self.inner)?;
        if store.chunks.contains_key(&key) {
            return Err(MemoryError::Conflict(format!(
                "chunk {} already exists",
                req.chunk.chunk_id.as_str()
            )));
        }
        store
            .practice_events
            .insert(practice.event_id.clone(), practice);
        store.chunks.insert(
            key,
            StoredChunkState {
                chunk: req.chunk.clone(),
                version: 1,
                active: true,
            },
        );
        Ok(req.chunk)
    }

    fn get_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<Option<Chunk>> {
        let store = lock_store(&self.inner)?;
        Ok(store
            .chunks
            .get(&(agent_id.clone(), chunk_id.clone()))
            .filter(|stored| stored.active)
            .map(|stored| stored.chunk.clone()))
    }

    fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let key = (req.agent_id.clone(), req.chunk_id.clone());
        let mut store = lock_store(&self.inner)?;
        let stored = store
            .chunks
            .get_mut(&key)
            .filter(|stored| stored.active)
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk_id.as_str())))?;
        if stored.version != req.expected_version {
            return Err(MemoryError::Conflict(format!(
                "chunk {} version conflict",
                req.chunk_id.as_str()
            )));
        }

        stored.chunk.slots.clear();
        for slot in req.slots {
            stored.chunk.upsert_slot(slot.key, slot.value);
        }
        stored.version = stored.version.saturating_add(1);
        stored.chunk.updated_at_ms = stored.chunk.updated_at_ms.saturating_add(1);
        Ok(stored.chunk.clone())
    }

    fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()> {
        let mut store = lock_store(&self.inner)?;
        let stored = store
            .chunks
            .get_mut(&(agent_id.clone(), chunk_id.clone()))
            .filter(|stored| stored.active)
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
        stored.active = false;
        Ok(())
    }

    fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
        req.validate()?;
        let mut store = lock_store(&self.inner)?;
        active_chunk_from_store(&store, &req.agent_id, &req.chunk_id)?;
        if store.practice_events.contains_key(&req.event_id) {
            return Err(MemoryError::Conflict(format!(
                "practice event {} already exists",
                req.event_id
            )));
        }
        store.practice_events.insert(req.event_id.clone(), req);
        Ok(())
    }

    fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
        req.validate()?;
        let store = lock_store(&self.inner)?;
        let mut candidates = store
            .chunks
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
                        store
                            .associations
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

                let practice_events = store
                    .practice_events
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
        let mut store = lock_store(&self.inner)?;
        active_chunk_from_store(&store, &req.agent_id, &req.src_chunk_id)?;
        active_chunk_from_store(&store, &req.agent_id, &req.dst_chunk_id)?;
        store.associations.insert(
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
        let mut store = lock_store(&self.inner)?;
        active_chunk_from_store(&store, &req.agent_id, &req.chunk_id)?;
        store.buffers.insert(
            (req.agent_id.clone(), req.buffer_name.clone()),
            req.chunk_id.clone(),
        );
        Ok(())
    }

    fn upsert_production_rule(
        &self,
        req: StoreProductionRuleRecord,
    ) -> MemoryResult<StoreProductionRuleRecord> {
        req.validate()?;
        let mut store = lock_store(&self.inner)?;
        store.rules.insert(
            (req.agent_id.clone(), req.rule.rule_id.clone()),
            req.clone(),
        );
        Ok(req)
    }

    fn get_production_rule(
        &self,
        agent_id: &AgentId,
        rule_id: &RuleId,
    ) -> MemoryResult<Option<StoreProductionRuleRecord>> {
        let store = lock_store(&self.inner)?;
        Ok(store
            .rules
            .get(&(agent_id.clone(), rule_id.clone()))
            .cloned())
    }
}

#[derive(Debug, Default)]
pub struct ApiCounters {
    retrieval_hits: AtomicU64,
    retrieval_misses: AtomicU64,
    session_lock_contention: AtomicU64,
    write_conflicts: AtomicU64,
    last_retrieve_latency_us: AtomicU64,
    last_candidates_examined: AtomicU64,
    last_activation_compute_us: AtomicU64,
}

impl ApiCounters {
    pub fn record_retrieval_hit(&self) {
        self.retrieval_hits.fetch_add(1, AtomicOrdering::Relaxed);
    }

    pub fn record_retrieval_miss(&self) {
        self.retrieval_misses.fetch_add(1, AtomicOrdering::Relaxed);
    }

    pub fn record_retrieval_observation(
        &self,
        latency_ms: f64,
        candidates_examined: usize,
        activation_compute_ms: f64,
    ) {
        self.last_retrieve_latency_us.store(
            milliseconds_to_microseconds(latency_ms),
            AtomicOrdering::Relaxed,
        );
        self.last_candidates_examined.store(
            u64::try_from(candidates_examined).unwrap_or(u64::MAX),
            AtomicOrdering::Relaxed,
        );
        self.last_activation_compute_us.store(
            milliseconds_to_microseconds(activation_compute_ms),
            AtomicOrdering::Relaxed,
        );
    }

    pub fn record_session_lock_contention(&self) {
        self.session_lock_contention
            .fetch_add(1, AtomicOrdering::Relaxed);
    }

    pub fn record_write_conflict(&self) {
        self.write_conflicts.fetch_add(1, AtomicOrdering::Relaxed);
    }

    pub fn retrieval_hits(&self) -> u64 {
        self.retrieval_hits.load(AtomicOrdering::Relaxed)
    }

    pub fn retrieval_misses(&self) -> u64 {
        self.retrieval_misses.load(AtomicOrdering::Relaxed)
    }

    pub fn retrieve_latency_ms(&self) -> f64 {
        microseconds_to_milliseconds(self.last_retrieve_latency_us.load(AtomicOrdering::Relaxed))
    }

    pub fn candidates_examined(&self) -> u64 {
        self.last_candidates_examined.load(AtomicOrdering::Relaxed)
    }

    pub fn activation_compute_ms(&self) -> f64 {
        microseconds_to_milliseconds(
            self.last_activation_compute_us
                .load(AtomicOrdering::Relaxed),
        )
    }

    pub fn session_lock_contention(&self) -> u64 {
        self.session_lock_contention.load(AtomicOrdering::Relaxed)
    }

    pub fn write_conflicts(&self) -> u64 {
        self.write_conflicts.load(AtomicOrdering::Relaxed)
    }

    pub fn metric_samples(&self) -> Vec<MetricSample> {
        vec![
            MetricSample {
                name: "nestor_memory_retrieve_latency_ms",
                value: self.retrieve_latency_ms(),
            },
            MetricSample {
                name: "nestor_memory_candidates_examined",
                value: self.candidates_examined() as f64,
            },
            MetricSample {
                name: "nestor_memory_activation_compute_ms",
                value: self.activation_compute_ms(),
            },
            MetricSample {
                name: "nestor_memory_retrieval_hits_total",
                value: self.retrieval_hits() as f64,
            },
            MetricSample {
                name: "nestor_memory_retrieval_misses_total",
                value: self.retrieval_misses() as f64,
            },
            MetricSample {
                name: "nestor_memory_session_lock_contention_total",
                value: self.session_lock_contention() as f64,
            },
            MetricSample {
                name: "nestor_memory_write_conflicts_total",
                value: self.write_conflicts() as f64,
            },
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct ApiState {
    pub repository: ApiRepository,
    pub sessions: Arc<SessionRegistry>,
    pub counters: Arc<ApiCounters>,
}

impl ApiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn session_snapshot(&self, agent_id: AgentId) -> MemoryResult<Vec<BufferSnapshot>> {
        self.sessions.snapshot(agent_id)
    }

    pub fn rule_engine(&self, rules: Vec<nestor_rules::ProductionRule>) -> RuleEngine {
        RuleEngine::new(rules)
    }
}

fn lock_store(store: &Mutex<MemoryStore>) -> MemoryResult<std::sync::MutexGuard<'_, MemoryStore>> {
    store
        .lock()
        .map_err(|_| MemoryError::Conflict("api memory store lock poisoned".to_string()))
}

fn active_chunk_from_store(
    store: &MemoryStore,
    agent_id: &AgentId,
    chunk_id: &ChunkId,
) -> MemoryResult<Chunk> {
    store
        .chunks
        .get(&(agent_id.clone(), chunk_id.clone()))
        .filter(|stored| stored.active)
        .map(|stored| stored.chunk.clone())
        .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))
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

fn milliseconds_to_microseconds(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let max_milliseconds = u64::MAX as f64 / 1_000.0;
    if value >= max_milliseconds {
        return u64::MAX;
    }
    (value * 1_000.0).round() as u64
}

fn microseconds_to_milliseconds(value: u64) -> f64 {
    value as f64 / 1_000.0
}
