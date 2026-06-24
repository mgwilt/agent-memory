use std::{
    cmp::Ordering,
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
};

use nestor_core::{
    AgentId, Chunk, ChunkId, MemoryError, MemoryResult, PracticeEvent, Slot, SlotValue,
    base_level_activation,
};
use nestor_ops::{MetricSample, RepositoryBackend, RuntimeConfig};
use nestor_rules::{RuleEngine, RuleId};
use nestor_session::{BufferName, BufferSnapshot, SessionRegistry};
use nestor_store::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, ConsolidateRequest,
    ConsolidationGroupReport, ConsolidationReport, CreateChunk, ForgetReport, ForgetRequest,
    MemgraphRepository, MemgraphRepositoryConfig, MemoryRepository, PracticeEventWrite,
    ProductionRuleRecord as StoreProductionRuleRecord, RetrievalPracticeWrite, UpdateChunk,
};

#[derive(Debug, Clone)]
struct StoredChunkState {
    chunk: Chunk,
    version: u64,
    active: bool,
    base_level_cache_stale: bool,
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
                base_level_cache_stale: false,
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

#[async_trait::async_trait]
impl MemoryRepository for ApiRepository {
    async fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
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
                base_level_cache_stale: false,
            },
        );
        Ok(req.chunk)
    }

    async fn upsert_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        ApiRepository::upsert_chunk(self, req)
    }

    async fn get_chunk(
        &self,
        agent_id: &AgentId,
        chunk_id: &ChunkId,
    ) -> MemoryResult<Option<Chunk>> {
        let store = lock_store(&self.inner)?;
        Ok(store
            .chunks
            .get(&(agent_id.clone(), chunk_id.clone()))
            .filter(|stored| stored.active)
            .map(|stored| stored.chunk.clone()))
    }

    async fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk> {
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

    async fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()> {
        let mut store = lock_store(&self.inner)?;
        let stored = store
            .chunks
            .get_mut(&(agent_id.clone(), chunk_id.clone()))
            .filter(|stored| stored.active)
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
        stored.active = false;
        Ok(())
    }

    async fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
        req.validate()?;
        let mut store = lock_store(&self.inner)?;
        active_chunk_from_store(&store, &req.agent_id, &req.chunk_id)?;
        if store.practice_events.contains_key(&req.event_id) {
            return Err(MemoryError::Conflict(format!(
                "practice event {} already exists",
                req.event_id
            )));
        }
        if let Some(stored) = store
            .chunks
            .get_mut(&(req.agent_id.clone(), req.chunk_id.clone()))
        {
            stored.base_level_cache_stale = true;
        }
        store.practice_events.insert(req.event_id.clone(), req);
        Ok(())
    }

    async fn record_successful_retrieval(&self, req: RetrievalPracticeWrite) -> MemoryResult<()> {
        req.validate()?;
        let event = PracticeEventWrite {
            event_id: req.event_id,
            agent_id: req.agent_id,
            chunk_id: req.chunk_id,
            occurred_at_ms: req.occurred_at_ms,
            kind: "retrieve".to_string(),
            weight: req.weight,
        };
        event.validate()?;
        let mut store = lock_store(&self.inner)?;
        if store.practice_events.contains_key(&event.event_id) {
            return Err(MemoryError::Conflict(format!(
                "practice event {} already exists",
                event.event_id
            )));
        }
        {
            let chunk = active_chunk_from_store_mut(&mut store, &event.agent_id, &event.chunk_id)?;
            chunk.chunk.record_retrieval(event.occurred_at_ms);
            chunk.base_level_cache_stale = true;
        }
        store.practice_events.insert(event.event_id.clone(), event);
        Ok(())
    }

    async fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
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
                    stored.base_level_cache_stale,
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
                |(_, spread_score, chunk, practice_events, base_level_cache_stale)| {
                    ChunkWithHistory {
                        chunk,
                        practice_events,
                        spread_score,
                        base_level_cache_stale,
                    }
                },
            )
            .collect())
    }

    async fn upsert_association(&self, req: AssociationWrite) -> MemoryResult<()> {
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

    async fn set_buffer_current(&self, req: BufferSetCurrent) -> MemoryResult<()> {
        req.validate()?;
        let mut store = lock_store(&self.inner)?;
        active_chunk_from_store(&store, &req.agent_id, &req.chunk_id)?;
        store.buffers.insert(
            (req.agent_id.clone(), req.buffer_name.clone()),
            req.chunk_id.clone(),
        );
        Ok(())
    }

    async fn upsert_production_rule(
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

    async fn get_production_rule(
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

    async fn consolidate(&self, req: ConsolidateRequest) -> MemoryResult<ConsolidationReport> {
        req.validate()?;
        let group_keys = effective_group_slot_keys(&req.group_slot_keys);
        let mut groups = BTreeMap::<String, Vec<Chunk>>::new();
        {
            let store = lock_store(&self.inner)?;
            for stored in store.chunks.values().filter(|stored| stored.active) {
                if stored.chunk.agent_id != req.agent_id {
                    continue;
                }
                if req
                    .chunk_type
                    .as_ref()
                    .is_some_and(|chunk_type| &stored.chunk.chunk_type != chunk_type)
                {
                    continue;
                }
                if is_bool_slot(&stored.chunk, "consolidated") {
                    continue;
                }
                if let Some(key) = consolidation_group_key(&stored.chunk, &group_keys) {
                    groups.entry(key).or_default().push(stored.chunk.clone());
                }
            }
        }

        let groups_considered = groups.len();
        let mut summaries = Vec::new();
        let mut store = lock_store(&self.inner)?;
        for (group_key, mut group_chunks) in groups {
            if group_chunks.len() < req.min_group_size {
                continue;
            }
            group_chunks.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));
            let source_chunk_ids = group_chunks
                .iter()
                .map(|chunk| chunk.chunk_id.clone())
                .collect::<Vec<_>>();
            let summary_id = ChunkId::from(format!(
                "summary-{}",
                stable_hex_hash(&[
                    req.agent_id.as_str(),
                    req.summary_chunk_type.as_str(),
                    &group_key
                ])
            ));
            let mut summary = Chunk::new(
                req.agent_id.clone(),
                summary_id.clone(),
                req.summary_chunk_type.clone(),
                req.now_ms,
            );
            for key in &group_keys {
                if let Some(value) = group_chunks[0].slot(key).cloned() {
                    summary.upsert_slot(key.clone(), value);
                }
            }
            summary.upsert_slot(
                "source_count",
                SlotValue::Number(source_chunk_ids.len() as f64),
            );
            summary.upsert_slot("consolidation_key", SlotValue::Text(group_key));
            store.chunks.insert(
                (req.agent_id.clone(), summary_id.clone()),
                StoredChunkState {
                    chunk: summary,
                    version: 1,
                    active: true,
                    base_level_cache_stale: false,
                },
            );
            for chunk_id in &source_chunk_ids {
                if let Some(stored) = store
                    .chunks
                    .get_mut(&(req.agent_id.clone(), chunk_id.clone()))
                {
                    stored
                        .chunk
                        .upsert_slot("consolidated", SlotValue::Bool(true));
                    stored.chunk.updated_at_ms = req.now_ms;
                }
                store.associations.insert(
                    (
                        req.agent_id.clone(),
                        summary_id.clone(),
                        chunk_id.clone(),
                        "consolidation".to_string(),
                    ),
                    AssociationWrite {
                        agent_id: req.agent_id.clone(),
                        src_chunk_id: summary_id.clone(),
                        dst_chunk_id: chunk_id.clone(),
                        source: "consolidation".to_string(),
                        strength: 1.0,
                        fan: source_chunk_ids.len() as u64,
                        updated_at_ms: req.now_ms,
                    },
                );
            }
            summaries.push(ConsolidationGroupReport {
                summary_chunk_id: summary_id,
                source_chunk_ids,
            });
        }

        Ok(ConsolidationReport {
            agent_id: req.agent_id,
            groups_considered,
            groups_consolidated: summaries.len(),
            summaries,
        })
    }

    async fn forget(&self, req: ForgetRequest) -> MemoryResult<ForgetReport> {
        req.validate()?;
        let mut store = lock_store(&self.inner)?;
        let mut examined = 0;
        let mut forgotten_chunk_ids = Vec::new();
        let mut archived_chunk_ids = Vec::new();
        let mut protected_chunk_ids = Vec::new();
        let event_values = store.practice_events.values().cloned().collect::<Vec<_>>();
        let association_values = store.associations.values().cloned().collect::<Vec<_>>();

        for ((agent_id, chunk_id), stored) in store.chunks.iter_mut() {
            if *agent_id != req.agent_id || !stored.active {
                continue;
            }
            if req
                .chunk_type
                .as_ref()
                .is_some_and(|chunk_type| &stored.chunk.chunk_type != chunk_type)
            {
                continue;
            }
            examined += 1;
            if is_bool_slot(&stored.chunk, "protected") {
                protected_chunk_ids.push(chunk_id.clone());
                continue;
            }
            let chunk_events = event_values
                .iter()
                .filter(|event| event.agent_id == *agent_id && event.chunk_id == *chunk_id)
                .map(|event| PracticeEvent {
                    occurred_at_ms: event.occurred_at_ms,
                    weight: event.weight,
                })
                .collect::<Vec<_>>();
            let last_practiced_at = chunk_events
                .iter()
                .map(|event| event.occurred_at_ms)
                .max()
                .unwrap_or(stored.chunk.updated_at_ms);
            if last_practiced_at > req.recency_cutoff_ms {
                continue;
            }
            let base_level = base_level_activation(&chunk_events, req.now_ms, 0.5);
            if base_level >= req.base_level_cutoff {
                continue;
            }
            let linked = association_values.iter().any(|association| {
                association.agent_id == *agent_id
                    && (association.src_chunk_id == *chunk_id
                        || association.dst_chunk_id == *chunk_id)
            });
            if linked && !req.allow_linked_forget {
                stored.chunk.upsert_slot("archived", SlotValue::Bool(true));
                stored.chunk.updated_at_ms = req.now_ms;
                archived_chunk_ids.push(chunk_id.clone());
            } else {
                stored.active = false;
                stored.chunk.updated_at_ms = req.now_ms;
                forgotten_chunk_ids.push(chunk_id.clone());
            }
        }

        Ok(ForgetReport {
            agent_id: req.agent_id,
            examined,
            forgotten_chunk_ids,
            archived_chunk_ids,
            protected_chunk_ids,
        })
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

#[derive(Clone)]
pub struct ApiState {
    pub repository: Arc<dyn MemoryRepository + Send + Sync>,
    pub sessions: Arc<SessionRegistry>,
    pub counters: Arc<ApiCounters>,
}

impl ApiState {
    pub fn new() -> Self {
        Self::with_repository(ApiRepository::default())
    }

    pub fn with_repository(repository: impl MemoryRepository + Send + Sync + 'static) -> Self {
        Self {
            repository: Arc::new(repository),
            sessions: Arc::new(SessionRegistry::default()),
            counters: Arc::new(ApiCounters::default()),
        }
    }

    pub async fn from_config(config: &RuntimeConfig) -> MemoryResult<Self> {
        match config.repository_backend {
            RepositoryBackend::Memgraph => {
                let password = config
                    .resolve_memgraph_password()
                    .map_err(MemoryError::Validation)?;
                let repository =
                    MemgraphRepository::connect(memgraph_repository_config(config, password))
                        .await?;
                Ok(Self::with_repository(repository))
            }
            RepositoryBackend::InMemory => Ok(Self::new()),
        }
    }

    pub fn session_snapshot(&self, agent_id: AgentId) -> MemoryResult<Vec<BufferSnapshot>> {
        self.sessions.snapshot(agent_id)
    }

    pub fn rule_engine(&self, rules: Vec<nestor_rules::ProductionRule>) -> RuleEngine {
        RuleEngine::new(rules)
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
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

fn active_chunk_from_store_mut<'a>(
    store: &'a mut MemoryStore,
    agent_id: &AgentId,
    chunk_id: &ChunkId,
) -> MemoryResult<&'a mut StoredChunkState> {
    store
        .chunks
        .get_mut(&(agent_id.clone(), chunk_id.clone()))
        .filter(|stored| stored.active)
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

fn effective_group_slot_keys(keys: &[String]) -> Vec<String> {
    if keys.is_empty() {
        vec![
            "topic".to_string(),
            "subject".to_string(),
            "entity".to_string(),
        ]
    } else {
        keys.iter().map(|key| key.trim().to_string()).collect()
    }
}

fn consolidation_group_key(chunk: &Chunk, keys: &[String]) -> Option<String> {
    let mut parts = Vec::new();
    for key in keys {
        if let Some(value) = chunk.slot(key) {
            parts.push(format!(
                "{key}={}:{}",
                value.value_type(),
                value.normalized()
            ));
        }
    }
    (!parts.is_empty()).then(|| parts.join("|"))
}

fn is_bool_slot(chunk: &Chunk, key: &str) -> bool {
    matches!(chunk.slot(key), Some(SlotValue::Bool(true)))
}

fn stable_hex_hash(parts: &[&str]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
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

fn memgraph_repository_config(
    config: &RuntimeConfig,
    password: String,
) -> MemgraphRepositoryConfig {
    MemgraphRepositoryConfig {
        uri: config.memgraph_uri.clone(),
        user: config.memgraph_user.clone(),
        password,
        database: "memgraph".to_string(),
        max_connections: config.memgraph_max_connections,
        tls_ca_file: config.memgraph_security.tls_ca_file.clone(),
        schema_info_enabled: true,
    }
}

#[cfg(test)]
mod tests {
    use nestor_ops::{MemgraphSecurityConfig, RepositoryBackend, RuntimeConfig};

    use super::*;

    #[tokio::test]
    async fn from_config_uses_explicit_in_memory_backend() -> MemoryResult<()> {
        let config = RuntimeConfig {
            repository_backend: RepositoryBackend::InMemory,
            ..RuntimeConfig::default()
        };

        let state = ApiState::from_config(&config).await?;

        state.repository.health_check().await
    }

    #[test]
    fn memgraph_config_mapping_preserves_runtime_settings() {
        let config = RuntimeConfig {
            memgraph_uri: "bolt+s://memgraph.example:7687".to_string(),
            memgraph_user: "nestor".to_string(),
            memgraph_max_connections: 7,
            memgraph_security: MemgraphSecurityConfig {
                tls_ca_file: Some("/etc/nestor/memgraph-ca.pem".to_string()),
                ..RuntimeConfig::default().memgraph_security
            },
            ..RuntimeConfig::default()
        };

        let repository_config = memgraph_repository_config(&config, "secret".to_string());

        assert_eq!(repository_config.uri, "bolt+s://memgraph.example:7687");
        assert_eq!(repository_config.user, "nestor");
        assert_eq!(repository_config.password, "secret");
        assert_eq!(repository_config.database, "memgraph");
        assert_eq!(repository_config.max_connections, 7);
        assert_eq!(
            repository_config.tls_ca_file.as_deref(),
            Some("/etc/nestor/memgraph-ca.pem")
        );
        assert!(repository_config.schema_info_enabled);
    }
}
