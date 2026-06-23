use std::collections::BTreeMap;

/// Stable tenant or agent identifier for declarative memory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(pub String);

/// Stable chunk identifier within an agent's declarative memory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId(pub String);

/// Symbolic ACT-R chunk type, such as `goal`, `fact`, or `episode`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkType(pub String);

macro_rules! impl_string_id {
    ($type_name:ident) => {
        impl $type_name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl From<String> for $type_name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $type_name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl AsRef<str> for $type_name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

impl_string_id!(AgentId);
impl_string_id!(ChunkId);
impl_string_id!(ChunkType);

/// Typed slot value used by chunks and cue requests.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue {
    Symbol(String),
    Text(String),
    Number(f64),
    Bool(bool),
}

impl SlotValue {
    /// Returns a stable type label for storage and diagnostics.
    pub fn value_type(&self) -> &'static str {
        match self {
            Self::Symbol(_) => "symbol",
            Self::Text(_) => "text",
            Self::Number(_) => "number",
            Self::Bool(_) => "bool",
        }
    }

    /// Returns a deterministic normalized representation for indexing.
    pub fn normalized(&self) -> String {
        match self {
            Self::Symbol(value) | Self::Text(value) => value.trim().to_lowercase(),
            Self::Number(value) => format!("{value:.12}"),
            Self::Bool(value) => value.to_string(),
        }
    }
}

/// A named chunk slot.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub key: String,
    pub value: SlotValue,
}

impl Slot {
    pub fn new(key: impl Into<String>, value: SlotValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

/// Declarative memory chunk with deterministic slot ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub chunk_type: ChunkType,
    pub slots: BTreeMap<String, SlotValue>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub retrieval_count: u64,
    pub base_bias: f64,
}

impl Chunk {
    /// Creates an empty chunk stamped with caller-provided logical time.
    pub fn new(agent_id: AgentId, chunk_id: ChunkId, chunk_type: ChunkType, now_ms: u64) -> Self {
        Self {
            agent_id,
            chunk_id,
            chunk_type,
            slots: BTreeMap::new(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            retrieval_count: 0,
            base_bias: 0.0,
        }
    }

    /// Returns a copy of this chunk with a slot inserted or replaced.
    pub fn with_slot(mut self, key: impl Into<String>, value: SlotValue) -> Self {
        self.upsert_slot(key, value);
        self
    }

    /// Inserts or replaces a slot value.
    pub fn upsert_slot(&mut self, key: impl Into<String>, value: SlotValue) {
        self.slots.insert(key.into(), value);
    }

    /// Returns a slot by key.
    pub fn slot(&self, key: &str) -> Option<&SlotValue> {
        self.slots.get(key)
    }

    /// Records a retrieval in pure domain state using caller-provided time.
    pub fn record_retrieval(&mut self, now_ms: u64) {
        self.retrieval_count = self.retrieval_count.saturating_add(1);
        self.updated_at_ms = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_symbolic_slot_values() {
        let value = SlotValue::Symbol("  ACT-R  ".to_string());

        assert_eq!(value.normalized(), "act-r");
        assert_eq!(value.value_type(), "symbol");
    }

    #[test]
    fn chunks_store_slots_by_key() {
        let chunk = Chunk::new(
            AgentId("agent-1".to_string()),
            ChunkId("ck-1".to_string()),
            ChunkType("episodic".to_string()),
            1_000,
        )
        .with_slot("topic", SlotValue::Symbol("memgraph".to_string()));

        assert_eq!(
            chunk.slot("topic"),
            Some(&SlotValue::Symbol("memgraph".to_string()))
        );
    }

    #[test]
    fn ids_can_be_constructed_from_strs() {
        let agent = AgentId::from("agent-1");

        assert_eq!(agent.as_str(), "agent-1");
    }

    #[test]
    fn retrieval_record_updates_pure_chunk_state() {
        let mut chunk = Chunk::new(
            AgentId::from("agent-1"),
            ChunkId::from("ck-1"),
            ChunkType::from("episodic"),
            1_000,
        );

        chunk.record_retrieval(1_500);

        assert_eq!(chunk.retrieval_count, 1);
        assert_eq!(chunk.updated_at_ms, 1_500);
    }
}
