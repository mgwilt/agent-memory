use actr_core::{ChunkId, ChunkType};
use actr_session::{BufferName, BufferSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct BufferCondition {
    pub buffer: BufferName,
    pub chunk_id: Option<ChunkId>,
    pub chunk_type: Option<ChunkType>,
}

impl BufferCondition {
    pub fn buffer_present(buffer: BufferName) -> Self {
        Self {
            buffer,
            chunk_id: None,
            chunk_type: None,
        }
    }

    pub fn matches(&self, buffers: &[BufferSnapshot]) -> bool {
        buffers.iter().any(|snapshot| {
            snapshot.name == self.buffer
                && snapshot.chunk_id.is_some()
                && self
                    .chunk_id
                    .as_ref()
                    .is_none_or(|chunk_id| Some(chunk_id) == snapshot.chunk_id.as_ref())
                && self
                    .chunk_type
                    .as_ref()
                    .is_none_or(|chunk_type| Some(chunk_type) == snapshot.chunk_type.as_ref())
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionRule {
    pub rule_id: RuleId,
    pub name: String,
    pub enabled: bool,
    pub utility: f64,
    pub version: u64,
    pub conditions: Vec<BufferCondition>,
}

impl ProductionRule {
    pub fn specificity(&self) -> usize {
        self.conditions.len()
            + self
                .conditions
                .iter()
                .filter(|condition| condition.chunk_id.is_some() || condition.chunk_type.is_some())
                .count()
    }
}
