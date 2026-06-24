use std::collections::BTreeMap;

use nestor_core::{
    AgentId, Chunk, ChunkId, ChunkType, MemoryError, MemoryResult, PracticeEvent, Slot, SlotValue,
};
use nestor_rules::{
    ProductionRule, ProductionRuleMetadata, RuleId, RuleRewardUpdate, apply_reward_to_rule,
};
use nestor_session::BufferName;

pub const DEFAULT_CANDIDATE_LIMIT: usize = 200;
pub const MAX_CANDIDATE_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateChunk {
    pub chunk: Chunk,
    pub initial_practice_event_id: String,
}

impl CreateChunk {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.chunk.agent_id.as_str())?;
        require_non_empty("chunk_id", self.chunk.chunk_id.as_str())?;
        require_non_empty("chunk_type", self.chunk.chunk_type.as_str())?;
        require_non_empty("initial_practice_event_id", &self.initial_practice_event_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateChunk {
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub expected_version: u64,
    pub slots: Vec<Slot>,
}

impl UpdateChunk {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("chunk_id", self.chunk_id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PracticeEventWrite {
    pub event_id: String,
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub occurred_at_ms: u64,
    pub kind: String,
    pub weight: f64,
}

impl PracticeEventWrite {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("event_id", &self.event_id)?;
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("chunk_id", self.chunk_id.as_str())?;
        require_non_empty("kind", &self.kind)?;
        if self.weight.is_finite() && self.weight >= 0.0 {
            Ok(())
        } else {
            Err(MemoryError::Validation(
                "practice event weight must be finite and non-negative".to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalPracticeWrite {
    pub event_id: String,
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub occurred_at_ms: u64,
    pub weight: f64,
}

impl RetrievalPracticeWrite {
    pub fn validate(&self) -> MemoryResult<()> {
        PracticeEventWrite {
            event_id: self.event_id.clone(),
            agent_id: self.agent_id.clone(),
            chunk_id: self.chunk_id.clone(),
            occurred_at_ms: self.occurred_at_ms,
            kind: "retrieve".to_string(),
            weight: self.weight,
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateQuery {
    pub agent_id: AgentId,
    pub chunk_type: Option<String>,
    pub cue_slots: Vec<Slot>,
    pub context_chunk_ids: Vec<ChunkId>,
    pub candidate_limit: usize,
}

impl CandidateQuery {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            chunk_type: None,
            cue_slots: Vec::new(),
            context_chunk_ids: Vec::new(),
            candidate_limit: DEFAULT_CANDIDATE_LIMIT,
        }
    }

    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        if self.bounded_limit().is_some() {
            Ok(())
        } else {
            Err(MemoryError::Validation(format!(
                "candidate_limit must be between 1 and {MAX_CANDIDATE_LIMIT}"
            )))
        }
    }

    pub fn bounded_limit(&self) -> Option<usize> {
        bounded_candidate_limit(self.candidate_limit)
    }

    pub fn normalized_cue_slots(&self) -> Vec<StoredSlot> {
        stored_slots_from_slots(&self.cue_slots)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkWithHistory {
    pub chunk: Chunk,
    pub practice_events: Vec<PracticeEvent>,
    pub spread_score: f64,
    pub base_level_cache_stale: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssociationWrite {
    pub agent_id: AgentId,
    pub src_chunk_id: ChunkId,
    pub dst_chunk_id: ChunkId,
    pub source: String,
    pub strength: f64,
    pub fan: u64,
    pub updated_at_ms: u64,
}

impl AssociationWrite {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("src_chunk_id", self.src_chunk_id.as_str())?;
        require_non_empty("dst_chunk_id", self.dst_chunk_id.as_str())?;
        require_non_empty("source", &self.source)?;
        if self.strength.is_finite() {
            Ok(())
        } else {
            Err(MemoryError::Validation(
                "association strength must be finite".to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BufferSetCurrent {
    pub agent_id: AgentId,
    pub buffer_name: BufferName,
    pub chunk_id: ChunkId,
    pub set_at_ms: u64,
}

impl BufferSetCurrent {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("chunk_id", self.chunk_id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionRuleRecord {
    pub agent_id: AgentId,
    pub rule: ProductionRule,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_reward: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidateRequest {
    pub agent_id: AgentId,
    pub chunk_type: Option<ChunkType>,
    pub summary_chunk_type: ChunkType,
    pub group_slot_keys: Vec<String>,
    pub min_group_size: usize,
    pub now_ms: u64,
}

impl ConsolidateRequest {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("summary_chunk_type", self.summary_chunk_type.as_str())?;
        if self.min_group_size < 2 {
            return Err(MemoryError::Validation(
                "min_group_size must be at least 2".to_string(),
            ));
        }
        if self.group_slot_keys.iter().any(|key| key.trim().is_empty()) {
            return Err(MemoryError::Validation(
                "group_slot_keys must not contain empty keys".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationGroupReport {
    pub summary_chunk_id: ChunkId,
    pub source_chunk_ids: Vec<ChunkId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationReport {
    pub agent_id: AgentId,
    pub groups_considered: usize,
    pub groups_consolidated: usize,
    pub summaries: Vec<ConsolidationGroupReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForgetRequest {
    pub agent_id: AgentId,
    pub chunk_type: Option<ChunkType>,
    pub now_ms: u64,
    pub recency_cutoff_ms: u64,
    pub base_level_cutoff: f64,
    pub allow_linked_forget: bool,
}

impl ForgetRequest {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        if self.base_level_cutoff.is_finite() {
            Ok(())
        } else {
            Err(MemoryError::Validation(
                "base_level_cutoff must be finite".to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgetReport {
    pub agent_id: AgentId,
    pub examined: usize,
    pub forgotten_chunk_ids: Vec<ChunkId>,
    pub archived_chunk_ids: Vec<ChunkId>,
    pub protected_chunk_ids: Vec<ChunkId>,
}

impl ProductionRuleRecord {
    pub fn validate(&self) -> MemoryResult<()> {
        require_non_empty("agent_id", self.agent_id.as_str())?;
        require_non_empty("rule_id", self.rule.rule_id.0.as_str())?;
        require_non_empty("name", &self.rule.name)?;
        if self.rule.utility.is_finite() && self.avg_reward.is_finite() {
            Ok(())
        } else {
            Err(MemoryError::Validation(
                "production rule utility and reward must be finite".to_string(),
            ))
        }
    }

    pub fn metadata(&self) -> ProductionRuleMetadata {
        self.rule.metadata()
    }

    pub fn record_reward(
        &mut self,
        reward: f64,
        learning_rate: f64,
        succeeded: bool,
    ) -> RuleRewardUpdate {
        let attempts = self.success_count.saturating_add(self.failure_count);
        self.avg_reward = if attempts == 0 {
            reward
        } else {
            ((self.avg_reward * attempts as f64) + reward) / attempts.saturating_add(1) as f64
        };
        if succeeded {
            self.success_count = self.success_count.saturating_add(1);
        } else {
            self.failure_count = self.failure_count.saturating_add(1);
        }
        apply_reward_to_rule(&mut self.rule, reward, learning_rate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSlot {
    pub key: String,
    pub value_type: &'static str,
    pub value_norm: String,
    pub value_hash: String,
    pub value_symbol: Option<String>,
    pub value_text: Option<String>,
    pub value_number: Option<String>,
    pub value_bool: Option<bool>,
}

impl StoredSlot {
    pub fn from_slot(slot: &Slot) -> Self {
        let (value_symbol, value_text, value_number, value_bool) = match &slot.value {
            SlotValue::Symbol(value) => (Some(value.clone()), None, None, None),
            SlotValue::Text(value) => (None, Some(value.clone()), None, None),
            SlotValue::Number(value) => (None, None, Some(format!("{value:.17}")), None),
            SlotValue::Bool(value) => (None, None, None, Some(*value)),
        };
        Self {
            key: slot.key.clone(),
            value_type: slot.value.value_type(),
            value_norm: slot.value.normalized(),
            value_hash: slot_value_hash(&slot.value),
            value_symbol,
            value_text,
            value_number,
            value_bool,
        }
    }

    pub fn slot_value(&self) -> MemoryResult<SlotValue> {
        match self.value_type {
            "symbol" => self
                .value_symbol
                .clone()
                .or_else(|| Some(self.value_norm.clone()))
                .map(SlotValue::Symbol)
                .ok_or_else(|| {
                    MemoryError::Serialization("symbol slot missing payload".to_string())
                }),
            "text" => self
                .value_text
                .clone()
                .or_else(|| Some(self.value_norm.clone()))
                .map(SlotValue::Text)
                .ok_or_else(|| MemoryError::Serialization("text slot missing payload".to_string())),
            "number" => self
                .value_number
                .as_ref()
                .unwrap_or(&self.value_norm)
                .parse::<f64>()
                .map(SlotValue::Number)
                .map_err(|error| {
                    MemoryError::Serialization(format!("number slot payload is invalid: {error}"))
                }),
            "bool" => self
                .value_bool
                .or_else(|| self.value_norm.parse::<bool>().ok())
                .map(SlotValue::Bool)
                .ok_or_else(|| MemoryError::Serialization("bool slot missing payload".to_string())),
            other => Err(MemoryError::Serialization(format!(
                "unknown slot value type {other}"
            ))),
        }
    }
}

pub fn stored_slots_from_chunk(chunk: &Chunk) -> Vec<StoredSlot> {
    chunk
        .slots
        .iter()
        .map(|(key, value)| {
            StoredSlot::from_slot(&Slot {
                key: key.clone(),
                value: value.clone(),
            })
        })
        .collect()
}

pub fn stored_slots_from_slots(slots: &[Slot]) -> Vec<StoredSlot> {
    let mut by_key = BTreeMap::new();
    for slot in slots {
        by_key.insert(slot.key.clone(), StoredSlot::from_slot(slot));
    }
    by_key.into_values().collect()
}

pub fn slot_value_hash(value: &SlotValue) -> String {
    stable_hex_hash(&[value.value_type(), "\0", &value.normalized()])
}

pub fn chunk_slot_hash(slots: &BTreeMap<String, SlotValue>) -> String {
    let mut parts = Vec::with_capacity(slots.len() * 4);
    for (key, value) in slots {
        parts.push(key.clone());
        parts.push("\0".to_string());
        parts.push(value.value_type().to_string());
        parts.push(value.normalized());
    }
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    stable_hex_hash(&part_refs)
}

pub fn bounded_candidate_limit(candidate_limit: usize) -> Option<usize> {
    match candidate_limit {
        1..=MAX_CANDIDATE_LIMIT => Some(candidate_limit),
        _ => None,
    }
}

#[async_trait::async_trait]
pub trait MemoryRepository {
    async fn health_check(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk>;

    async fn upsert_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk>;

    async fn get_chunk(
        &self,
        agent_id: &AgentId,
        chunk_id: &ChunkId,
    ) -> MemoryResult<Option<Chunk>>;

    async fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk>;

    async fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()>;

    async fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()>;

    async fn record_successful_retrieval(&self, req: RetrievalPracticeWrite) -> MemoryResult<()>;

    async fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>>;

    async fn upsert_association(&self, req: AssociationWrite) -> MemoryResult<()>;

    async fn set_buffer_current(&self, req: BufferSetCurrent) -> MemoryResult<()>;

    async fn upsert_production_rule(
        &self,
        req: ProductionRuleRecord,
    ) -> MemoryResult<ProductionRuleRecord>;

    async fn get_production_rule(
        &self,
        agent_id: &AgentId,
        rule_id: &RuleId,
    ) -> MemoryResult<Option<ProductionRuleRecord>>;

    async fn consolidate(&self, req: ConsolidateRequest) -> MemoryResult<ConsolidationReport>;

    async fn forget(&self, req: ForgetRequest) -> MemoryResult<ForgetReport>;
}

fn require_non_empty(field: &str, value: &str) -> MemoryResult<()> {
    if value.trim().is_empty() {
        Err(MemoryError::Validation(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
        collections::{BTreeMap, BTreeSet},
        sync::{Mutex, MutexGuard},
    };

    use nestor_core::{ChunkType, SlotValue};
    use nestor_rules::{BufferCondition, RuleId};

    use super::*;

    #[derive(Debug, Clone)]
    struct StoredChunkState {
        chunk: Chunk,
        version: u64,
        active: bool,
        base_level_cache_stale: bool,
    }

    #[derive(Debug, Default)]
    struct InMemoryRepository {
        chunks: Mutex<BTreeMap<(AgentId, ChunkId), StoredChunkState>>,
        practice_events: Mutex<BTreeMap<String, PracticeEventWrite>>,
        associations: Mutex<BTreeMap<(AgentId, ChunkId, ChunkId, String), AssociationWrite>>,
        buffers: Mutex<BTreeMap<(AgentId, BufferName), ChunkId>>,
        rules: Mutex<BTreeMap<(AgentId, RuleId), ProductionRuleRecord>>,
    }

    impl InMemoryRepository {
        fn active_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<Chunk> {
            lock(&self.chunks)?
                .get(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .map(|stored| stored.chunk.clone())
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> MemoryResult<MutexGuard<'_, T>> {
        mutex
            .lock()
            .map_err(|_| MemoryError::Conflict("test repository lock poisoned".to_string()))
    }

    #[async_trait::async_trait]
    impl MemoryRepository for InMemoryRepository {
        async fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
            req.validate()?;
            let key = (req.chunk.agent_id.clone(), req.chunk.chunk_id.clone());
            let mut chunks = lock(&self.chunks)?;
            if chunks.contains_key(&key) {
                return Err(MemoryError::Conflict(format!(
                    "chunk {} already exists",
                    req.chunk.chunk_id.as_str()
                )));
            }

            let practice = PracticeEventWrite {
                event_id: req.initial_practice_event_id,
                agent_id: req.chunk.agent_id.clone(),
                chunk_id: req.chunk.chunk_id.clone(),
                occurred_at_ms: req.chunk.created_at_ms,
                kind: "encode".to_string(),
                weight: 1.0,
            };
            practice.validate()?;
            lock(&self.practice_events)?.insert(practice.event_id.clone(), practice);
            chunks.insert(
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
            lock(&self.practice_events)?.insert(practice.event_id.clone(), practice);
            lock(&self.chunks)?.insert(
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

        async fn get_chunk(
            &self,
            agent_id: &AgentId,
            chunk_id: &ChunkId,
        ) -> MemoryResult<Option<Chunk>> {
            Ok(self
                .chunks
                .lock()
                .map_err(|_| MemoryError::Conflict("test repository lock poisoned".to_string()))?
                .get(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .map(|stored| stored.chunk.clone()))
        }

        async fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk> {
            req.validate()?;
            let key = (req.agent_id.clone(), req.chunk_id.clone());
            let mut chunks = lock(&self.chunks)?;
            let stored = chunks
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
            stored.chunk.updated_at_ms = stored.chunk.updated_at_ms.saturating_add(1);
            stored.version = stored.version.saturating_add(1);
            Ok(stored.chunk.clone())
        }

        async fn soft_delete_chunk(
            &self,
            agent_id: &AgentId,
            chunk_id: &ChunkId,
        ) -> MemoryResult<()> {
            let mut chunks = lock(&self.chunks)?;
            let stored = chunks
                .get_mut(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
            stored.active = false;
            Ok(())
        }

        async fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
            req.validate()?;
            let chunk_key = (req.agent_id.clone(), req.chunk_id.clone());
            if !self
                .chunks
                .lock()
                .map_err(|_| MemoryError::Conflict("test repository lock poisoned".to_string()))?
                .get(&chunk_key)
                .is_some_and(|stored| stored.active)
            {
                return Err(MemoryError::NotFound(format!(
                    "chunk {}",
                    req.chunk_id.as_str()
                )));
            }
            let mut events = lock(&self.practice_events)?;
            if events.contains_key(&req.event_id) {
                return Err(MemoryError::Conflict(format!(
                    "practice event {} already exists",
                    req.event_id
                )));
            }
            events.insert(req.event_id.clone(), req);
            if let Some(stored) = lock(&self.chunks)?.get_mut(&chunk_key) {
                stored.base_level_cache_stale = true;
            }
            Ok(())
        }

        async fn record_successful_retrieval(
            &self,
            req: RetrievalPracticeWrite,
        ) -> MemoryResult<()> {
            req.validate()?;
            let chunk_key = (req.agent_id.clone(), req.chunk_id.clone());
            let mut chunks = lock(&self.chunks)?;
            let stored = chunks
                .get_mut(&chunk_key)
                .filter(|stored| stored.active)
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk_id.as_str())))?;
            let event = PracticeEventWrite {
                event_id: req.event_id,
                agent_id: req.agent_id,
                chunk_id: req.chunk_id,
                occurred_at_ms: req.occurred_at_ms,
                kind: "retrieve".to_string(),
                weight: req.weight,
            };
            event.validate()?;
            let mut events = lock(&self.practice_events)?;
            if events.contains_key(&event.event_id) {
                return Err(MemoryError::Conflict(format!(
                    "practice event {} already exists",
                    event.event_id
                )));
            }
            stored.chunk.record_retrieval(event.occurred_at_ms);
            stored.base_level_cache_stale = true;
            events.insert(event.event_id.clone(), event);
            Ok(())
        }

        async fn fetch_candidates(
            &self,
            req: CandidateQuery,
        ) -> MemoryResult<Vec<ChunkWithHistory>> {
            req.validate()?;
            let chunks = lock(&self.chunks)?;
            let events = lock(&self.practice_events)?;
            let associations = lock(&self.associations)?;
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
                    let cue_matches = cue_match_count(&stored.chunk, &req.cue_slots);
                    if !req.cue_slots.is_empty() && cue_matches == 0 {
                        return None;
                    }

                    let spread_score: f64 = req
                        .context_chunk_ids
                        .iter()
                        .filter_map(|context_id| {
                            associations
                                .values()
                                .filter(|assoc| {
                                    assoc.agent_id == req.agent_id
                                        && assoc.src_chunk_id == *context_id
                                        && assoc.dst_chunk_id == stored.chunk.chunk_id
                                })
                                .map(|assoc| assoc.strength)
                                .max_by(|left, right| {
                                    left.partial_cmp(right).unwrap_or(Ordering::Equal)
                                })
                        })
                        .sum();

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
            self.active_chunk(&req.agent_id, &req.src_chunk_id)?;
            self.active_chunk(&req.agent_id, &req.dst_chunk_id)?;
            lock(&self.associations)?.insert(
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
            self.active_chunk(&req.agent_id, &req.chunk_id)?;
            lock(&self.buffers)?.insert(
                (req.agent_id.clone(), req.buffer_name.clone()),
                req.chunk_id.clone(),
            );
            Ok(())
        }

        async fn upsert_production_rule(
            &self,
            req: ProductionRuleRecord,
        ) -> MemoryResult<ProductionRuleRecord> {
            req.validate()?;
            lock(&self.rules)?.insert(
                (req.agent_id.clone(), req.rule.rule_id.clone()),
                req.clone(),
            );
            Ok(req)
        }

        async fn get_production_rule(
            &self,
            agent_id: &AgentId,
            rule_id: &RuleId,
        ) -> MemoryResult<Option<ProductionRuleRecord>> {
            Ok(self
                .rules
                .lock()
                .map_err(|_| MemoryError::Conflict("test repository lock poisoned".to_string()))?
                .get(&(agent_id.clone(), rule_id.clone()))
                .cloned())
        }

        async fn consolidate(&self, req: ConsolidateRequest) -> MemoryResult<ConsolidationReport> {
            req.validate()?;
            let group_keys = effective_group_slot_keys(&req.group_slot_keys);
            let mut groups = BTreeMap::<String, Vec<Chunk>>::new();
            {
                let chunks = lock(&self.chunks)?;
                for stored in chunks.values().filter(|stored| stored.active) {
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

                let mut chunks = lock(&self.chunks)?;
                chunks.insert(
                    (req.agent_id.clone(), summary_id.clone()),
                    StoredChunkState {
                        chunk: summary,
                        version: 1,
                        active: true,
                        base_level_cache_stale: false,
                    },
                );
                for chunk_id in &source_chunk_ids {
                    if let Some(stored) = chunks.get_mut(&(req.agent_id.clone(), chunk_id.clone()))
                    {
                        stored
                            .chunk
                            .upsert_slot("consolidated", SlotValue::Bool(true));
                        stored.chunk.updated_at_ms = req.now_ms;
                    }
                }
                drop(chunks);

                for chunk_id in &source_chunk_ids {
                    lock(&self.associations)?.insert(
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
            let mut examined = 0;
            let mut forgotten_chunk_ids = Vec::new();
            let mut archived_chunk_ids = Vec::new();
            let mut protected_chunk_ids = Vec::new();
            let events = lock(&self.practice_events)?;
            let associations = lock(&self.associations)?;
            let mut chunks = lock(&self.chunks)?;

            for ((agent_id, chunk_id), stored) in chunks.iter_mut() {
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
                let chunk_events = events
                    .values()
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
                let base_level = nestor_core::base_level_activation(&chunk_events, req.now_ms, 0.5);
                if base_level >= req.base_level_cutoff {
                    continue;
                }
                let linked = associations.values().any(|association| {
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

    fn cue_match_count(chunk: &Chunk, cues: &[Slot]) -> usize {
        cues.iter()
            .filter(|cue| chunk.slot(&cue.key) == Some(&cue.value))
            .count()
    }

    fn chunk(id: &str, topic: &str) -> Chunk {
        Chunk::new(
            AgentId::from("agent-1"),
            ChunkId::from(id),
            ChunkType::from("fact"),
            1_000,
        )
        .with_slot("topic", SlotValue::Symbol(topic.to_string()))
    }

    #[test]
    fn slot_hashes_are_deterministic_and_type_sensitive() {
        let symbol_hash = slot_value_hash(&SlotValue::Symbol("one".to_string()));
        let text_hash = slot_value_hash(&SlotValue::Text("one".to_string()));

        assert_eq!(
            symbol_hash,
            slot_value_hash(&SlotValue::Symbol(" one ".to_string()))
        );
        assert_ne!(symbol_hash, text_hash);
    }

    #[test]
    fn stored_slots_are_sorted_by_key() {
        let slots = stored_slots_from_slots(&[
            Slot::new("z", SlotValue::Bool(true)),
            Slot::new("a", SlotValue::Number(1.0)),
        ]);

        assert_eq!(slots[0].key, "a");
        assert_eq!(slots[1].key, "z");
    }

    #[test]
    fn stored_slots_preserve_original_typed_payloads() -> MemoryResult<()> {
        let slots = stored_slots_from_slots(&[
            Slot::new("symbol", SlotValue::Symbol(" ACT-R ".to_string())),
            Slot::new("text", SlotValue::Text("Mixed Case".to_string())),
            Slot::new("number", SlotValue::Number(42.25)),
            Slot::new("bool", SlotValue::Bool(true)),
        ]);
        let by_key = slots
            .into_iter()
            .map(|slot| (slot.key.clone(), slot))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(by_key["symbol"].value_norm, "act-r");
        assert_eq!(
            by_key["symbol"].slot_value()?,
            SlotValue::Symbol(" ACT-R ".to_string())
        );
        assert_eq!(
            by_key["text"].slot_value()?,
            SlotValue::Text("Mixed Case".to_string())
        );
        assert_eq!(by_key["number"].slot_value()?, SlotValue::Number(42.25));
        assert_eq!(by_key["bool"].slot_value()?, SlotValue::Bool(true));
        Ok(())
    }

    #[test]
    fn candidate_limit_is_bounded() {
        assert_eq!(bounded_candidate_limit(1), Some(1));
        assert_eq!(bounded_candidate_limit(DEFAULT_CANDIDATE_LIMIT), Some(200));
        assert_eq!(bounded_candidate_limit(0), None);
        assert_eq!(bounded_candidate_limit(MAX_CANDIDATE_LIMIT + 1), None);
    }

    #[tokio::test]
    async fn repository_contract_persists_core_memory_graph_shapes() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let chunk_a = repo
            .create_chunk(CreateChunk {
                chunk: chunk("ck-a", "rust"),
                initial_practice_event_id: "event-a".to_string(),
            })
            .await?;
        let chunk_b = repo
            .create_chunk(CreateChunk {
                chunk: chunk("ck-b", "memgraph"),
                initial_practice_event_id: "event-b".to_string(),
            })
            .await?;

        repo.append_practice_event(PracticeEventWrite {
            event_id: "event-c".to_string(),
            agent_id: AgentId::from("agent-1"),
            chunk_id: chunk_a.chunk_id.clone(),
            occurred_at_ms: 2_000,
            kind: "retrieve".to_string(),
            weight: 1.0,
        })
        .await?;
        repo.upsert_association(AssociationWrite {
            agent_id: AgentId::from("agent-1"),
            src_chunk_id: chunk_a.chunk_id.clone(),
            dst_chunk_id: chunk_b.chunk_id.clone(),
            source: "goal".to_string(),
            strength: 0.75,
            fan: 1,
            updated_at_ms: 2_000,
        })
        .await?;
        repo.set_buffer_current(BufferSetCurrent {
            agent_id: AgentId::from("agent-1"),
            buffer_name: BufferName::Goal,
            chunk_id: chunk_a.chunk_id.clone(),
            set_at_ms: 2_000,
        })
        .await?;

        let fetched = repo
            .get_chunk(&AgentId::from("agent-1"), &ChunkId::from("ck-a"))
            .await?;
        let candidates = repo
            .fetch_candidates(CandidateQuery {
                agent_id: AgentId::from("agent-1"),
                chunk_type: Some("fact".to_string()),
                cue_slots: vec![Slot::new(
                    "topic",
                    SlotValue::Symbol("memgraph".to_string()),
                )],
                context_chunk_ids: vec![chunk_a.chunk_id],
                candidate_limit: 10,
            })
            .await?;

        assert_eq!(fetched, Some(chunk("ck-a", "rust")));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].chunk.chunk_id, ChunkId::from("ck-b"));
        assert_eq!(candidates[0].practice_events.len(), 1);
        assert_eq!(candidates[0].spread_score, 0.75);
        Ok(())
    }

    #[tokio::test]
    async fn repository_contract_enforces_uniqueness_and_versions() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let create = CreateChunk {
            chunk: chunk("ck-a", "rust"),
            initial_practice_event_id: "event-a".to_string(),
        };

        repo.create_chunk(create.clone()).await?;
        assert!(matches!(
            repo.create_chunk(create).await,
            Err(MemoryError::Conflict(_))
        ));
        assert!(matches!(
            repo.append_practice_event(PracticeEventWrite {
                event_id: "event-a".to_string(),
                agent_id: AgentId::from("agent-1"),
                chunk_id: ChunkId::from("ck-a"),
                occurred_at_ms: 2_000,
                kind: "retrieve".to_string(),
                weight: 1.0,
            })
            .await,
            Err(MemoryError::Conflict(_))
        ));
        assert!(matches!(
            repo.update_chunk(UpdateChunk {
                agent_id: AgentId::from("agent-1"),
                chunk_id: ChunkId::from("ck-a"),
                expected_version: 99,
                slots: Vec::new(),
            })
            .await,
            Err(MemoryError::Conflict(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn production_rules_are_part_of_repository_contract() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let record = ProductionRuleRecord {
            agent_id: AgentId::from("agent-1"),
            rule: ProductionRule::new(
                RuleId("rule-1".to_string()),
                "retrieve fact",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(1.25),
            success_count: 3,
            failure_count: 1,
            avg_reward: 0.5,
        };

        repo.upsert_production_rule(record.clone()).await?;
        let fetched = repo
            .get_production_rule(&AgentId::from("agent-1"), &RuleId("rule-1".to_string()))
            .await?;

        assert_eq!(fetched, Some(record));
        Ok(())
    }

    #[test]
    fn production_rule_record_updates_learned_metadata() {
        let mut record = ProductionRuleRecord {
            agent_id: AgentId::from("agent-1"),
            rule: ProductionRule::new(
                RuleId::from("rule-1"),
                "retrieve fact",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(2.0)
            .with_version(4),
            success_count: 1,
            failure_count: 1,
            avg_reward: 2.0,
        };

        let update = record.record_reward(6.0, 0.25, true);

        assert_eq!(update.updated_utility, 3.0);
        assert_eq!(record.rule.version, 5);
        assert_eq!(record.success_count, 2);
        assert_eq!(record.failure_count, 1);
        assert!((record.avg_reward - (10.0 / 3.0)).abs() < 1e-12);
        assert_eq!(record.metadata().rule_id, RuleId::from("rule-1"));
    }

    #[tokio::test]
    async fn successful_retrieval_records_practice_and_stale_cache() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        repo.create_chunk(CreateChunk {
            chunk: chunk("ck-a", "rust"),
            initial_practice_event_id: "event-a".to_string(),
        })
        .await?;

        repo.record_successful_retrieval(RetrievalPracticeWrite {
            event_id: "retrieve-a".to_string(),
            agent_id: AgentId::from("agent-1"),
            chunk_id: ChunkId::from("ck-a"),
            occurred_at_ms: 2_000,
            weight: 1.0,
        })
        .await?;

        let fetched = repo
            .get_chunk(&AgentId::from("agent-1"), &ChunkId::from("ck-a"))
            .await?
            .expect("chunk exists");
        let candidates = repo
            .fetch_candidates(CandidateQuery {
                agent_id: AgentId::from("agent-1"),
                chunk_type: Some("fact".to_string()),
                cue_slots: vec![Slot::new("topic", SlotValue::Symbol("rust".to_string()))],
                context_chunk_ids: Vec::new(),
                candidate_limit: 10,
            })
            .await?;

        assert_eq!(fetched.retrieval_count, 1);
        assert_eq!(candidates[0].practice_events.len(), 2);
        assert!(candidates[0].base_level_cache_stale);
        Ok(())
    }

    #[tokio::test]
    async fn consolidation_creates_summary_and_marks_sources() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        for id in ["episode-a", "episode-b"] {
            repo.create_chunk(CreateChunk {
                chunk: Chunk::new(
                    AgentId::from("agent-1"),
                    ChunkId::from(id),
                    ChunkType::from("episode"),
                    1_000,
                )
                .with_slot("topic", SlotValue::Symbol("act-r".to_string()))
                .with_slot("subject", SlotValue::Symbol("memory".to_string())),
                initial_practice_event_id: format!("event-{id}"),
            })
            .await?;
        }

        let report = repo
            .consolidate(ConsolidateRequest {
                agent_id: AgentId::from("agent-1"),
                chunk_type: Some(ChunkType::from("episode")),
                summary_chunk_type: ChunkType::from("semantic"),
                group_slot_keys: vec!["topic".to_string(), "subject".to_string()],
                min_group_size: 2,
                now_ms: 5_000,
            })
            .await?;

        assert_eq!(report.groups_consolidated, 1);
        assert_eq!(report.summaries[0].source_chunk_ids.len(), 2);
        let source = repo
            .get_chunk(&AgentId::from("agent-1"), &ChunkId::from("episode-a"))
            .await?
            .expect("source exists");
        let summary = repo
            .get_chunk(
                &AgentId::from("agent-1"),
                &report.summaries[0].summary_chunk_id,
            )
            .await?
            .expect("summary exists");
        assert_eq!(source.slot("consolidated"), Some(&SlotValue::Bool(true)));
        assert_eq!(summary.chunk_type, ChunkType::from("semantic"));
        assert_eq!(summary.slot("source_count"), Some(&SlotValue::Number(2.0)));
        Ok(())
    }

    #[tokio::test]
    async fn forget_soft_deletes_stale_chunks_and_skips_protected() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        repo.create_chunk(CreateChunk {
            chunk: Chunk::new(
                AgentId::from("agent-1"),
                ChunkId::from("old"),
                ChunkType::from("stale"),
                100,
            )
            .with_slot("topic", SlotValue::Symbol("old".to_string())),
            initial_practice_event_id: "event-old".to_string(),
        })
        .await?;
        repo.create_chunk(CreateChunk {
            chunk: Chunk::new(
                AgentId::from("agent-1"),
                ChunkId::from("protected"),
                ChunkType::from("stale"),
                100,
            )
            .with_slot("topic", SlotValue::Symbol("old".to_string()))
            .with_slot("protected", SlotValue::Bool(true)),
            initial_practice_event_id: "event-protected".to_string(),
        })
        .await?;

        let report = repo
            .forget(ForgetRequest {
                agent_id: AgentId::from("agent-1"),
                chunk_type: Some(ChunkType::from("stale")),
                now_ms: 1_000_000,
                recency_cutoff_ms: 500,
                base_level_cutoff: 0.0,
                allow_linked_forget: false,
            })
            .await?;

        assert_eq!(report.forgotten_chunk_ids, vec![ChunkId::from("old")]);
        assert_eq!(report.protected_chunk_ids, vec![ChunkId::from("protected")]);
        assert!(
            repo.get_chunk(&AgentId::from("agent-1"), &ChunkId::from("old"))
                .await?
                .is_none()
        );
        assert!(
            repo.get_chunk(&AgentId::from("agent-1"), &ChunkId::from("protected"))
                .await?
                .is_some()
        );
        Ok(())
    }

    #[tokio::test]
    async fn fetch_candidates_rejects_unbounded_requests() {
        let repo = InMemoryRepository::default();
        let result = repo
            .fetch_candidates(CandidateQuery {
                agent_id: AgentId::from("agent-1"),
                chunk_type: None,
                cue_slots: Vec::new(),
                context_chunk_ids: Vec::new(),
                candidate_limit: MAX_CANDIDATE_LIMIT + 1,
            })
            .await;

        assert!(matches!(result, Err(MemoryError::Validation(_))));
    }

    #[test]
    fn chunk_slot_hash_is_order_independent() {
        let first = BTreeMap::from([
            ("a".to_string(), SlotValue::Number(1.0)),
            ("b".to_string(), SlotValue::Bool(true)),
        ]);
        let second = BTreeMap::from([
            ("b".to_string(), SlotValue::Bool(true)),
            ("a".to_string(), SlotValue::Number(1.0)),
        ]);
        let unique = BTreeSet::from([chunk_slot_hash(&first), chunk_slot_hash(&second)]);

        assert_eq!(unique.len(), 1);
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
}
