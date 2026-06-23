use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use actr_core::{AgentId, ChunkId, ChunkType, MemoryError, MemoryResult};

use crate::buffers::{BufferName, BufferSnapshot, BufferState};

#[derive(Debug, Clone)]
pub struct SessionState {
    pub agent_id: AgentId,
    buffers: BTreeMap<BufferName, BufferState>,
    cognitive_step: u64,
}

impl SessionState {
    pub fn new(agent_id: AgentId) -> Self {
        let mut buffers = BTreeMap::new();
        for name in [
            BufferName::Goal,
            BufferName::Retrieval,
            BufferName::Imaginal,
            BufferName::Task,
        ] {
            buffers.insert(name.clone(), BufferState::empty(name));
        }

        Self {
            agent_id,
            buffers,
            cognitive_step: 0,
        }
    }

    pub fn set_buffer(
        &mut self,
        name: BufferName,
        chunk_id: ChunkId,
        chunk_type: ChunkType,
        now_ms: u64,
    ) {
        self.cognitive_step += 1;
        self.buffers
            .entry(name.clone())
            .or_insert_with(|| BufferState::empty(name))
            .set(chunk_id, chunk_type, now_ms);
    }

    pub fn clear_buffer(&mut self, name: &BufferName, now_ms: u64) -> MemoryResult<()> {
        let buffer = self
            .buffers
            .get_mut(name)
            .ok_or_else(|| MemoryError::NotFound(format!("buffer {}", name.as_str())))?;
        self.cognitive_step += 1;
        buffer.clear(now_ms);
        Ok(())
    }

    pub fn commit_retrieval(&mut self, chunk_id: ChunkId, chunk_type: ChunkType, now_ms: u64) {
        self.set_buffer(BufferName::Retrieval, chunk_id, chunk_type, now_ms);
    }

    pub fn snapshot(&self) -> Vec<BufferSnapshot> {
        self.buffers.values().map(BufferSnapshot::from).collect()
    }

    pub fn cognitive_step(&self) -> u64 {
        self.cognitive_step
    }
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: Mutex<BTreeMap<AgentId, Arc<Mutex<SessionState>>>>,
}

impl SessionRegistry {
    pub fn get_or_create(&self, agent_id: AgentId) -> MemoryResult<Arc<Mutex<SessionState>>> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| MemoryError::Conflict("session registry lock poisoned".to_string()))?;
        Ok(sessions
            .entry(agent_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(SessionState::new(agent_id))))
            .clone())
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn retrieval_commit_replaces_single_buffer_chunk() {
        let mut session = SessionState::new(AgentId("agent".to_string()));

        session.commit_retrieval(
            ChunkId("ck-1".to_string()),
            ChunkType("episodic".to_string()),
            100,
        );
        session.commit_retrieval(
            ChunkId("ck-2".to_string()),
            ChunkType("episodic".to_string()),
            200,
        );

        let retrieval = session
            .snapshot()
            .into_iter()
            .find(|buffer| buffer.name == BufferName::Retrieval)
            .expect("retrieval buffer should exist");
        assert_eq!(retrieval.chunk_id, Some(ChunkId("ck-2".to_string())));
        assert_eq!(session.cognitive_step(), 2);
    }

    #[test]
    fn registry_serializes_agent_session_mutation() {
        let registry = Arc::new(SessionRegistry::default());
        let mut handles = Vec::new();

        for i in 0..8 {
            let registry = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                let session = registry
                    .get_or_create(AgentId("agent".to_string()))
                    .expect("session should be created");
                let mut session = session.lock().expect("session lock should be held");
                session.set_buffer(
                    BufferName::Goal,
                    ChunkId(format!("ck-{i}")),
                    ChunkType("goal".to_string()),
                    i,
                );
            }));
        }

        for handle in handles {
            handle.join().expect("thread should complete");
        }

        let session = registry
            .get_or_create(AgentId("agent".to_string()))
            .expect("session should exist");
        let session = session.lock().expect("session lock should be held");

        assert_eq!(session.cognitive_step(), 8);
    }
}
