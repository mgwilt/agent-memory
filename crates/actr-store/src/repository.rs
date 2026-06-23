use actr_core::{AgentId, Chunk, ChunkId, MemoryResult, PracticeEvent, Slot};
use actr_rules::ProductionRule;
use actr_session::BufferName;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateChunk {
    pub chunk: Chunk,
    pub initial_practice_event_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateChunk {
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub expected_version: u64,
    pub slots: Vec<Slot>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateQuery {
    pub agent_id: AgentId,
    pub chunk_type: Option<String>,
    pub cue_slots: Vec<Slot>,
    pub context_chunk_ids: Vec<ChunkId>,
    pub candidate_limit: usize,
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

#[derive(Debug, Clone, PartialEq)]
pub struct BufferSetCurrent {
    pub agent_id: AgentId,
    pub buffer_name: BufferName,
    pub chunk_id: ChunkId,
    pub set_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionRuleRecord {
    pub agent_id: AgentId,
    pub rule: ProductionRule,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_reward: f64,
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
}
