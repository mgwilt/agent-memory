use actr_core::{ChunkId, ChunkType};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BufferName {
    Goal,
    Retrieval,
    Imaginal,
    Task,
    Custom(String),
}

impl BufferName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Goal => "goal",
            Self::Retrieval => "retrieval",
            Self::Imaginal => "imaginal",
            Self::Task => "task",
            Self::Custom(name) => name.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BufferState {
    pub name: BufferName,
    pub chunk_id: Option<ChunkId>,
    pub chunk_type: Option<ChunkType>,
    pub updated_at_ms: u64,
}

impl BufferState {
    pub fn empty(name: BufferName) -> Self {
        Self {
            name,
            chunk_id: None,
            chunk_type: None,
            updated_at_ms: 0,
        }
    }

    pub fn set(&mut self, chunk_id: ChunkId, chunk_type: ChunkType, now_ms: u64) {
        self.chunk_id = Some(chunk_id);
        self.chunk_type = Some(chunk_type);
        self.updated_at_ms = now_ms;
    }

    pub fn clear(&mut self, now_ms: u64) {
        self.chunk_id = None;
        self.chunk_type = None;
        self.updated_at_ms = now_ms;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BufferSnapshot {
    pub name: BufferName,
    pub chunk_id: Option<ChunkId>,
    pub chunk_type: Option<ChunkType>,
    pub updated_at_ms: u64,
}

impl From<&BufferState> for BufferSnapshot {
    fn from(value: &BufferState) -> Self {
        Self {
            name: value.name.clone(),
            chunk_id: value.chunk_id.clone(),
            chunk_type: value.chunk_type.clone(),
            updated_at_ms: value.updated_at_ms,
        }
    }
}
