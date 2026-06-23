use std::collections::BTreeMap;

use actr_core::{
    AgentId, Chunk, ChunkId, MemoryError, MemoryResult, PracticeEvent, Slot, SlotValue,
};
use actr_rules::{ProductionRule, RuleId};
use actr_session::BufferName;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSlot {
    pub key: String,
    pub value_type: &'static str,
    pub value_norm: String,
    pub value_hash: String,
}

impl StoredSlot {
    pub fn from_slot(slot: &Slot) -> Self {
        Self {
            key: slot.key.clone(),
            value_type: slot.value.value_type(),
            value_norm: slot.value.normalized(),
            value_hash: slot_value_hash(&slot.value),
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

pub trait MemoryRepository {
    fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk>;

    fn get_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<Option<Chunk>>;

    fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk>;

    fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()>;

    fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()>;

    fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>>;

    fn upsert_association(&self, req: AssociationWrite) -> MemoryResult<()>;

    fn set_buffer_current(&self, req: BufferSetCurrent) -> MemoryResult<()>;

    fn upsert_production_rule(
        &self,
        req: ProductionRuleRecord,
    ) -> MemoryResult<ProductionRuleRecord>;

    fn get_production_rule(
        &self,
        agent_id: &AgentId,
        rule_id: &RuleId,
    ) -> MemoryResult<Option<ProductionRuleRecord>>;
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
        cell::RefCell,
        cmp::Ordering,
        collections::{BTreeMap, BTreeSet},
    };

    use actr_core::{ChunkType, SlotValue};
    use actr_rules::{BufferCondition, RuleId};

    use super::*;

    #[derive(Debug, Clone)]
    struct StoredChunkState {
        chunk: Chunk,
        version: u64,
        active: bool,
    }

    #[derive(Debug, Default)]
    struct InMemoryRepository {
        chunks: RefCell<BTreeMap<(AgentId, ChunkId), StoredChunkState>>,
        practice_events: RefCell<BTreeMap<String, PracticeEventWrite>>,
        associations: RefCell<BTreeMap<(AgentId, ChunkId, ChunkId, String), AssociationWrite>>,
        buffers: RefCell<BTreeMap<(AgentId, BufferName), ChunkId>>,
        rules: RefCell<BTreeMap<(AgentId, RuleId), ProductionRuleRecord>>,
    }

    impl InMemoryRepository {
        fn active_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<Chunk> {
            self.chunks
                .borrow()
                .get(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .map(|stored| stored.chunk.clone())
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))
        }
    }

    impl MemoryRepository for InMemoryRepository {
        fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
            req.validate()?;
            let key = (req.chunk.agent_id.clone(), req.chunk.chunk_id.clone());
            let mut chunks = self.chunks.borrow_mut();
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
            self.practice_events
                .borrow_mut()
                .insert(practice.event_id.clone(), practice);
            chunks.insert(
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
            Ok(self
                .chunks
                .borrow()
                .get(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .map(|stored| stored.chunk.clone()))
        }

        fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk> {
            req.validate()?;
            let key = (req.agent_id.clone(), req.chunk_id.clone());
            let mut chunks = self.chunks.borrow_mut();
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

        fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()> {
            let mut chunks = self.chunks.borrow_mut();
            let stored = chunks
                .get_mut(&(agent_id.clone(), chunk_id.clone()))
                .filter(|stored| stored.active)
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
            stored.active = false;
            Ok(())
        }

        fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
            req.validate()?;
            let chunk_key = (req.agent_id.clone(), req.chunk_id.clone());
            if !self
                .chunks
                .borrow()
                .get(&chunk_key)
                .is_some_and(|stored| stored.active)
            {
                return Err(MemoryError::NotFound(format!(
                    "chunk {}",
                    req.chunk_id.as_str()
                )));
            }
            let mut events = self.practice_events.borrow_mut();
            if events.contains_key(&req.event_id) {
                return Err(MemoryError::Conflict(format!(
                    "practice event {} already exists",
                    req.event_id
                )));
            }
            events.insert(req.event_id.clone(), req);
            Ok(())
        }

        fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
            req.validate()?;
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
            self.active_chunk(&req.agent_id, &req.src_chunk_id)?;
            self.active_chunk(&req.agent_id, &req.dst_chunk_id)?;
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
            self.active_chunk(&req.agent_id, &req.chunk_id)?;
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
            req.validate()?;
            self.rules.borrow_mut().insert(
                (req.agent_id.clone(), req.rule.rule_id.clone()),
                req.clone(),
            );
            Ok(req)
        }

        fn get_production_rule(
            &self,
            agent_id: &AgentId,
            rule_id: &RuleId,
        ) -> MemoryResult<Option<ProductionRuleRecord>> {
            Ok(self
                .rules
                .borrow()
                .get(&(agent_id.clone(), rule_id.clone()))
                .cloned())
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
    fn candidate_limit_is_bounded() {
        assert_eq!(bounded_candidate_limit(1), Some(1));
        assert_eq!(bounded_candidate_limit(DEFAULT_CANDIDATE_LIMIT), Some(200));
        assert_eq!(bounded_candidate_limit(0), None);
        assert_eq!(bounded_candidate_limit(MAX_CANDIDATE_LIMIT + 1), None);
    }

    #[test]
    fn repository_contract_persists_core_memory_graph_shapes() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let chunk_a = repo.create_chunk(CreateChunk {
            chunk: chunk("ck-a", "rust"),
            initial_practice_event_id: "event-a".to_string(),
        })?;
        let chunk_b = repo.create_chunk(CreateChunk {
            chunk: chunk("ck-b", "memgraph"),
            initial_practice_event_id: "event-b".to_string(),
        })?;

        repo.append_practice_event(PracticeEventWrite {
            event_id: "event-c".to_string(),
            agent_id: AgentId::from("agent-1"),
            chunk_id: chunk_a.chunk_id.clone(),
            occurred_at_ms: 2_000,
            kind: "retrieve".to_string(),
            weight: 1.0,
        })?;
        repo.upsert_association(AssociationWrite {
            agent_id: AgentId::from("agent-1"),
            src_chunk_id: chunk_a.chunk_id.clone(),
            dst_chunk_id: chunk_b.chunk_id.clone(),
            source: "goal".to_string(),
            strength: 0.75,
            fan: 1,
            updated_at_ms: 2_000,
        })?;
        repo.set_buffer_current(BufferSetCurrent {
            agent_id: AgentId::from("agent-1"),
            buffer_name: BufferName::Goal,
            chunk_id: chunk_a.chunk_id.clone(),
            set_at_ms: 2_000,
        })?;

        let fetched = repo.get_chunk(&AgentId::from("agent-1"), &ChunkId::from("ck-a"))?;
        let candidates = repo.fetch_candidates(CandidateQuery {
            agent_id: AgentId::from("agent-1"),
            chunk_type: Some("fact".to_string()),
            cue_slots: vec![Slot::new(
                "topic",
                SlotValue::Symbol("memgraph".to_string()),
            )],
            context_chunk_ids: vec![chunk_a.chunk_id],
            candidate_limit: 10,
        })?;

        assert_eq!(fetched, Some(chunk("ck-a", "rust")));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].chunk.chunk_id, ChunkId::from("ck-b"));
        assert_eq!(candidates[0].practice_events.len(), 1);
        assert_eq!(candidates[0].spread_score, 0.75);
        Ok(())
    }

    #[test]
    fn repository_contract_enforces_uniqueness_and_versions() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let create = CreateChunk {
            chunk: chunk("ck-a", "rust"),
            initial_practice_event_id: "event-a".to_string(),
        };

        repo.create_chunk(create.clone())?;
        assert!(matches!(
            repo.create_chunk(create),
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
            }),
            Err(MemoryError::Conflict(_))
        ));
        assert!(matches!(
            repo.update_chunk(UpdateChunk {
                agent_id: AgentId::from("agent-1"),
                chunk_id: ChunkId::from("ck-a"),
                expected_version: 99,
                slots: Vec::new(),
            }),
            Err(MemoryError::Conflict(_))
        ));
        Ok(())
    }

    #[test]
    fn production_rules_are_part_of_repository_contract() -> MemoryResult<()> {
        let repo = InMemoryRepository::default();
        let record = ProductionRuleRecord {
            agent_id: AgentId::from("agent-1"),
            rule: ProductionRule {
                rule_id: RuleId("rule-1".to_string()),
                name: "retrieve fact".to_string(),
                enabled: true,
                utility: 1.25,
                version: 1,
                conditions: vec![BufferCondition::buffer_present(BufferName::Goal)],
            },
            success_count: 3,
            failure_count: 1,
            avg_reward: 0.5,
        };

        repo.upsert_production_rule(record.clone())?;
        let fetched =
            repo.get_production_rule(&AgentId::from("agent-1"), &RuleId("rule-1".to_string()))?;

        assert_eq!(fetched, Some(record));
        Ok(())
    }

    #[test]
    fn fetch_candidates_rejects_unbounded_requests() {
        let repo = InMemoryRepository::default();
        let result = repo.fetch_candidates(CandidateQuery {
            agent_id: AgentId::from("agent-1"),
            chunk_type: None,
            cue_slots: Vec::new(),
            context_chunk_ids: Vec::new(),
            candidate_limit: MAX_CANDIDATE_LIMIT + 1,
        });

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
}
