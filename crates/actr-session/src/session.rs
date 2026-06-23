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
        for name in BufferName::core() {
            buffers.insert(name.clone(), BufferState::empty(name));
        }

        Self {
            agent_id,
            buffers,
            cognitive_step: 0,
        }
    }

    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    pub fn set_buffer(
        &mut self,
        name: BufferName,
        chunk_id: ChunkId,
        chunk_type: ChunkType,
        now_ms: u64,
    ) -> BufferSnapshot {
        self.cognitive_step = self.cognitive_step.saturating_add(1);
        let buffer = self
            .buffers
            .entry(name.clone())
            .or_insert_with(|| BufferState::empty(name));
        buffer.set(chunk_id, chunk_type, now_ms);
        buffer.snapshot()
    }

    pub fn clear_buffer(&mut self, name: &BufferName, now_ms: u64) -> MemoryResult<BufferSnapshot> {
        let buffer = self
            .buffers
            .get_mut(name)
            .ok_or_else(|| MemoryError::NotFound(format!("buffer {}", name.as_str())))?;
        self.cognitive_step = self.cognitive_step.saturating_add(1);
        buffer.clear(now_ms);
        Ok(buffer.snapshot())
    }

    pub fn commit_retrieval(
        &mut self,
        chunk_id: ChunkId,
        chunk_type: ChunkType,
        now_ms: u64,
    ) -> BufferSnapshot {
        self.set_buffer(BufferName::Retrieval, chunk_id, chunk_type, now_ms)
    }

    pub fn buffer_snapshot(&self, name: &BufferName) -> MemoryResult<BufferSnapshot> {
        self.buffers
            .get(name)
            .map(BufferState::snapshot)
            .ok_or_else(|| MemoryError::NotFound(format!("buffer {}", name.as_str())))
    }

    pub fn snapshot(&self) -> Vec<BufferSnapshot> {
        self.buffers.values().map(BufferState::snapshot).collect()
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

    pub fn with_session<T>(
        &self,
        agent_id: AgentId,
        mutate: impl FnOnce(&mut SessionState) -> MemoryResult<T>,
    ) -> MemoryResult<T> {
        let session = self.get_or_create(agent_id)?;
        let mut session = session
            .lock()
            .map_err(|_| MemoryError::Conflict("session lock poisoned".to_string()))?;
        mutate(&mut session)
    }

    pub fn snapshot(&self, agent_id: AgentId) -> MemoryResult<Vec<BufferSnapshot>> {
        self.with_session(agent_id, |session| Ok(session.snapshot()))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;

    #[test]
    fn core_buffers_start_empty_in_deterministic_order() -> MemoryResult<()> {
        let session = SessionState::new(AgentId::from("agent"));

        let snapshot = session.snapshot();

        assert_eq!(
            snapshot
                .iter()
                .map(|buffer| buffer.name.clone())
                .collect::<Vec<_>>(),
            vec![
                BufferName::Goal,
                BufferName::Retrieval,
                BufferName::Imaginal,
                BufferName::Task,
            ]
        );
        assert!(snapshot.iter().all(|buffer| buffer.chunk_id.is_none()));
        assert_eq!(
            session
                .buffer_snapshot(&BufferName::Goal)?
                .chunk_type
                .as_ref(),
            None
        );
        assert_eq!(session.cognitive_step(), 0);
        Ok(())
    }

    #[test]
    fn buffer_replacement_and_clearing_are_single_steps() -> MemoryResult<()> {
        let mut session = SessionState::new(AgentId::from("agent"));

        let first = session.set_buffer(
            BufferName::Goal,
            ChunkId::from("ck-1"),
            ChunkType::from("goal"),
            100,
        );
        let second = session.set_buffer(
            BufferName::Goal,
            ChunkId::from("ck-2"),
            ChunkType::from("goal"),
            200,
        );
        let cleared = session.clear_buffer(&BufferName::Goal, 300)?;

        assert_eq!(first.chunk_id, Some(ChunkId::from("ck-1")));
        assert_eq!(second.chunk_id, Some(ChunkId::from("ck-2")));
        assert_eq!(cleared.chunk_id, None);
        assert_eq!(cleared.chunk_type, None);
        assert_eq!(cleared.updated_at_ms, 300);
        assert_eq!(session.cognitive_step(), 3);
        Ok(())
    }

    #[test]
    fn clearing_unknown_buffer_returns_error_without_step() {
        let mut session = SessionState::new(AgentId::from("agent"));

        let result = session.clear_buffer(&BufferName::Custom("missing".to_string()), 100);

        assert!(matches!(result, Err(MemoryError::NotFound(_))));
        assert_eq!(session.cognitive_step(), 0);
    }

    #[test]
    fn retrieval_commit_replaces_single_buffer_chunk() -> MemoryResult<()> {
        let mut session = SessionState::new(AgentId::from("agent"));

        let first =
            session.commit_retrieval(ChunkId::from("ck-1"), ChunkType::from("episodic"), 100);
        let second =
            session.commit_retrieval(ChunkId::from("ck-2"), ChunkType::from("episodic"), 200);

        assert_eq!(first.chunk_id, Some(ChunkId::from("ck-1")));
        assert_eq!(second.chunk_id, Some(ChunkId::from("ck-2")));
        assert_eq!(
            session
                .buffer_snapshot(&BufferName::Retrieval)?
                .chunk_id
                .as_ref(),
            Some(&ChunkId::from("ck-2"))
        );
        assert_eq!(session.cognitive_step(), 2);
        Ok(())
    }

    #[test]
    fn registry_serializes_agent_session_mutation() {
        let registry = Arc::new(SessionRegistry::default());
        let mut handles = Vec::new();

        for i in 0..8 {
            let registry = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                registry.with_session(AgentId::from("agent"), |session| {
                    let snapshot = session.set_buffer(
                        BufferName::Goal,
                        ChunkId::from(format!("ck-{i}")),
                        ChunkType::from("goal"),
                        i,
                    );
                    Ok((session.cognitive_step(), snapshot.chunk_id))
                })
            }));
        }

        let mut steps = BTreeSet::new();
        for handle in handles {
            let (step, chunk_id) = handle
                .join()
                .expect("thread should complete")
                .expect("session mutation should succeed");
            assert!(chunk_id.is_some());
            steps.insert(step);
        }

        assert_eq!(steps, BTreeSet::from_iter(1..=8));
        assert_eq!(
            registry
                .snapshot(AgentId::from("agent"))
                .expect("snapshot should succeed")
                .len(),
            4
        );
    }

    #[test]
    fn registry_keeps_independent_agent_sessions() -> MemoryResult<()> {
        let registry = SessionRegistry::default();
        let held_agent = registry.get_or_create(AgentId::from("agent-a"))?;
        let _held_guard = held_agent
            .lock()
            .map_err(|_| MemoryError::Conflict("session lock poisoned".to_string()))?;

        let step = registry.with_session(AgentId::from("agent-b"), |session| {
            session.set_buffer(
                BufferName::Task,
                ChunkId::from("task-b"),
                ChunkType::from("task"),
                100,
            );
            Ok(session.cognitive_step())
        })?;

        assert_eq!(step, 1);
        assert_eq!(
            registry
                .snapshot(AgentId::from("agent-b"))?
                .into_iter()
                .find(|buffer| buffer.name == BufferName::Task)
                .and_then(|buffer| buffer.chunk_id),
            Some(ChunkId::from("task-b"))
        );
        Ok(())
    }

    #[test]
    fn concurrent_retrieval_commits_are_serialized_and_last_step_wins() -> MemoryResult<()> {
        let registry = Arc::new(SessionRegistry::default());
        let barrier = Arc::new(Barrier::new(12));
        let mut handles = Vec::new();

        for i in 0..12 {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let chunk_id = ChunkId::from(format!("retrieved-{i}"));
                barrier.wait();
                registry.with_session(AgentId::from("agent"), |session| {
                    let snapshot =
                        session.commit_retrieval(chunk_id.clone(), ChunkType::from("fact"), i);
                    Ok((session.cognitive_step(), snapshot.chunk_id))
                })
            }));
        }

        let mut records = Vec::new();
        for handle in handles {
            records.push(
                handle
                    .join()
                    .expect("thread should complete")
                    .expect("retrieval commit should succeed"),
            );
        }
        let steps = records
            .iter()
            .map(|(step, _)| *step)
            .collect::<BTreeSet<_>>();
        let latest = records
            .iter()
            .max_by_key(|(step, _)| *step)
            .expect("records should not be empty");
        let final_state = registry.with_session(AgentId::from("agent"), |session| {
            Ok((
                session.cognitive_step(),
                session.buffer_snapshot(&BufferName::Retrieval)?.chunk_id,
            ))
        })?;

        assert_eq!(steps, BTreeSet::from_iter(1..=12));
        assert_eq!(final_state.0, 12);
        assert_eq!(final_state.1, latest.1.clone());
        Ok(())
    }
}
