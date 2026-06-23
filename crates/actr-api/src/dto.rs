use std::collections::BTreeMap;

use actr_core::{AgentId, ChunkId, Slot, SlotValue};
use actr_rules::RuleId;

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkUpsertRequest {
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub chunk_type: String,
    pub slots: BTreeMap<String, SlotValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrieveRequest {
    pub agent_id: AgentId,
    pub chunk_type: Option<String>,
    pub cue_slots: Vec<Slot>,
    pub context_chunk_ids: Vec<ChunkId>,
    pub candidate_limit: usize,
    pub result_limit: usize,
    pub activation_threshold: f64,
    pub noise_s: f64,
    pub partial_matching: bool,
    pub return_diagnostics: bool,
}

impl Default for RetrieveRequest {
    fn default() -> Self {
        Self {
            agent_id: AgentId("agent-123".to_string()),
            chunk_type: Some("episodic".to_string()),
            cue_slots: Vec::new(),
            context_chunk_ids: Vec::new(),
            candidate_limit: 200,
            result_limit: 12,
            activation_threshold: 0.25,
            noise_s: 0.0,
            partial_matching: true,
            return_diagnostics: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PracticeRequest {
    pub agent_id: AgentId,
    pub chunk_id: ChunkId,
    pub kind: String,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssociateRequest {
    pub agent_id: AgentId,
    pub src_chunk_id: ChunkId,
    pub dst_chunk_id: ChunkId,
    pub source: String,
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleEvaluateRequest {
    pub agent_id: AgentId,
    pub candidate_rule_ids: Vec<RuleId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalStatus {
    Hit,
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreComponents {
    pub base_level: f64,
    pub spreading: f64,
    pub partial_match: f64,
    pub noise: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalResult {
    pub chunk_id: ChunkId,
    pub chunk_type: String,
    pub activation: f64,
    pub predicted_latency_ms: f64,
    pub components: ScoreComponents,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalDiagnostics {
    pub candidates_examined: usize,
    pub threshold: f64,
    pub noise_s: f64,
    pub context_window_tokens: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrieveResponse {
    pub agent_id: AgentId,
    pub status: RetrievalStatus,
    pub results: Vec<RetrievalResult>,
    pub diagnostics: Option<RetrievalDiagnostics>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: &'static str,
    pub memgraph: &'static str,
}
