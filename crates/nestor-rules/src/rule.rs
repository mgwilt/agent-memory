use nestor_core::{Chunk, ChunkId, ChunkType, Slot, SlotValue};
use nestor_session::{BufferName, BufferSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(pub String);

impl RuleId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for RuleId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for RuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

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

    pub fn chunk_type(buffer: BufferName, chunk_type: ChunkType) -> Self {
        Self {
            buffer,
            chunk_id: None,
            chunk_type: Some(chunk_type),
        }
    }

    pub fn chunk_id(buffer: BufferName, chunk_id: ChunkId) -> Self {
        Self {
            buffer,
            chunk_id: Some(chunk_id),
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

    pub fn specificity(&self) -> usize {
        1 + usize::from(self.chunk_id.is_some()) + usize::from(self.chunk_type.is_some())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievedChunkCondition {
    pub chunk_id: Option<ChunkId>,
    pub chunk_type: Option<ChunkType>,
    pub slots: Vec<Slot>,
}

impl RetrievedChunkCondition {
    pub fn any() -> Self {
        Self {
            chunk_id: None,
            chunk_type: None,
            slots: Vec::new(),
        }
    }

    pub fn chunk_type(chunk_type: ChunkType) -> Self {
        Self {
            chunk_id: None,
            chunk_type: Some(chunk_type),
            slots: Vec::new(),
        }
    }

    pub fn with_slot(mut self, key: impl Into<String>, value: SlotValue) -> Self {
        self.slots.push(Slot::new(key, value));
        self
    }

    pub fn matches(&self, retrieved_chunk: Option<&Chunk>) -> bool {
        let Some(chunk) = retrieved_chunk else {
            return false;
        };

        self.chunk_id
            .as_ref()
            .is_none_or(|chunk_id| chunk_id == &chunk.chunk_id)
            && self
                .chunk_type
                .as_ref()
                .is_none_or(|chunk_type| chunk_type == &chunk.chunk_type)
            && self.slots.iter().all(|slot| {
                chunk
                    .slot(&slot.key)
                    .is_some_and(|value| slot_values_match(value, &slot.value))
            })
    }

    pub fn specificity(&self) -> usize {
        1 + usize::from(self.chunk_id.is_some())
            + usize::from(self.chunk_type.is_some())
            + self.slots.len()
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
    pub retrieved_chunk: Option<RetrievedChunkCondition>,
}

impl ProductionRule {
    pub fn new(rule_id: RuleId, name: impl Into<String>, conditions: Vec<BufferCondition>) -> Self {
        Self {
            rule_id,
            name: name.into(),
            enabled: true,
            utility: 0.0,
            version: 1,
            conditions,
            retrieved_chunk: None,
        }
    }

    pub fn with_utility(mut self, utility: f64) -> Self {
        self.utility = utility;
        self
    }

    pub fn with_version(mut self, version: u64) -> Self {
        self.version = version;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_retrieved_chunk(mut self, condition: RetrievedChunkCondition) -> Self {
        self.retrieved_chunk = Some(condition);
        self
    }

    pub fn specificity(&self) -> usize {
        self.conditions
            .iter()
            .map(BufferCondition::specificity)
            .sum::<usize>()
            + self
                .retrieved_chunk
                .as_ref()
                .map_or(0, RetrievedChunkCondition::specificity)
    }

    pub fn metadata(&self) -> ProductionRuleMetadata {
        ProductionRuleMetadata {
            rule_id: self.rule_id.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            utility: self.utility,
            version: self.version,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionRuleMetadata {
    pub rule_id: RuleId,
    pub name: String,
    pub enabled: bool,
    pub utility: f64,
    pub version: u64,
}

fn slot_values_match(left: &SlotValue, right: &SlotValue) -> bool {
    left.value_type() == right.value_type() && left.normalized() == right.normalized()
}
