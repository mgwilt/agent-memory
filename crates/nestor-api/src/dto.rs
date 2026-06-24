use std::collections::BTreeMap;

use nestor_core::{Chunk, ChunkId, ChunkType, MemoryError, MemoryResult, Slot, SlotValue};
use nestor_rules::{
    ConflictResolution, ProductionRule, RetrievedChunkCondition, RuleEvaluationContext,
    RuleRejectionReason,
};
use nestor_session::{BufferName, BufferSnapshot};
use nestor_store::{
    ConsolidationReport as StoreConsolidationReport, ForgetReport as StoreForgetReport,
    RetrievalMissReason, RetrievalOutcome, RetrievalStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkUpsertRequest {
    pub agent_id: String,
    pub chunk_id: String,
    pub chunk_type: String,
    #[serde(default)]
    pub slots: BTreeMap<String, SlotValueDto>,
    #[serde(default)]
    pub now_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkPatchRequest {
    pub agent_id: String,
    pub expected_version: u64,
    pub slots: BTreeMap<String, SlotValueDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentQuery {
    pub agent_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkResponse {
    pub agent_id: String,
    pub chunk_id: String,
    pub chunk_type: String,
    pub slots: BTreeMap<String, SlotValueDto>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub retrieval_count: u64,
    pub base_bias: f64,
}

impl From<Chunk> for ChunkResponse {
    fn from(chunk: Chunk) -> Self {
        Self {
            agent_id: chunk.agent_id.0,
            chunk_id: chunk.chunk_id.0,
            chunk_type: chunk.chunk_type.0,
            slots: chunk
                .slots
                .into_iter()
                .map(|(key, value)| (key, SlotValueDto::from(value)))
                .collect(),
            created_at_ms: chunk.created_at_ms,
            updated_at_ms: chunk.updated_at_ms,
            retrieval_count: chunk.retrieval_count,
            base_bias: chunk.base_bias,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum SlotValueDto {
    Symbol(String),
    Text(String),
    Number(f64),
    Bool(bool),
}

impl From<SlotValueDto> for SlotValue {
    fn from(value: SlotValueDto) -> Self {
        match value {
            SlotValueDto::Symbol(value) => Self::Symbol(value),
            SlotValueDto::Text(value) => Self::Text(value),
            SlotValueDto::Number(value) => Self::Number(value),
            SlotValueDto::Bool(value) => Self::Bool(value),
        }
    }
}

impl From<SlotValue> for SlotValueDto {
    fn from(value: SlotValue) -> Self {
        match value {
            SlotValue::Symbol(value) => Self::Symbol(value),
            SlotValue::Text(value) => Self::Text(value),
            SlotValue::Number(value) => Self::Number(value),
            SlotValue::Bool(value) => Self::Bool(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotDto {
    pub key: String,
    pub value: SlotValueDto,
}

impl From<SlotDto> for Slot {
    fn from(value: SlotDto) -> Self {
        Self {
            key: value.key,
            value: value.value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrieveRequest {
    pub agent_id: String,
    #[serde(default)]
    pub chunk_type: Option<String>,
    #[serde(default)]
    pub cue_slots: Vec<SlotDto>,
    #[serde(default)]
    pub context_chunk_ids: Vec<String>,
    #[serde(default = "default_candidate_limit")]
    pub candidate_limit: usize,
    #[serde(default = "default_result_limit")]
    pub result_limit: usize,
    #[serde(default)]
    pub activation_threshold: f64,
    #[serde(default)]
    pub noise_s: f64,
    #[serde(default = "default_true")]
    pub partial_matching: bool,
    #[serde(default = "default_true")]
    pub return_diagnostics: bool,
    #[serde(default)]
    pub deterministic_seed: Option<u64>,
    #[serde(default = "default_true")]
    pub commit_on_hit: bool,
    #[serde(default = "default_now_ms")]
    pub now_ms: u64,
}

impl Default for RetrieveRequest {
    fn default() -> Self {
        Self {
            agent_id: "agent-123".to_string(),
            chunk_type: Some("episodic".to_string()),
            cue_slots: Vec::new(),
            context_chunk_ids: Vec::new(),
            candidate_limit: default_candidate_limit(),
            result_limit: default_result_limit(),
            activation_threshold: 0.0,
            noise_s: 0.0,
            partial_matching: true,
            return_diagnostics: true,
            deterministic_seed: Some(42),
            commit_on_hit: true,
            now_ms: default_now_ms(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticeRequest {
    pub agent_id: String,
    pub chunk_id: String,
    pub kind: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default = "default_now_ms")]
    pub occurred_at_ms: u64,
    #[serde(default)]
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticeResponse {
    pub event_id: String,
    pub agent_id: String,
    pub chunk_id: String,
    pub kind: String,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RehearseRequest {
    pub agent_id: String,
    pub chunk_id: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default = "default_now_ms")]
    pub occurred_at_ms: u64,
    #[serde(default)]
    pub event_id: Option<String>,
}

pub type RehearseResponse = PracticeResponse;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidateRequest {
    pub agent_id: String,
    #[serde(default)]
    pub chunk_type: Option<String>,
    #[serde(default = "default_summary_chunk_type")]
    pub summary_chunk_type: String,
    #[serde(default)]
    pub group_slot_keys: Vec<String>,
    #[serde(default = "default_min_group_size")]
    pub min_group_size: usize,
    #[serde(default = "default_now_ms")]
    pub now_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsolidationGroupResponse {
    pub summary_chunk_id: String,
    pub source_chunk_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsolidateResponse {
    pub agent_id: String,
    pub groups_considered: usize,
    pub groups_consolidated: usize,
    pub summaries: Vec<ConsolidationGroupResponse>,
}

impl From<StoreConsolidationReport> for ConsolidateResponse {
    fn from(report: StoreConsolidationReport) -> Self {
        Self {
            agent_id: report.agent_id.0,
            groups_considered: report.groups_considered,
            groups_consolidated: report.groups_consolidated,
            summaries: report
                .summaries
                .into_iter()
                .map(|summary| ConsolidationGroupResponse {
                    summary_chunk_id: summary.summary_chunk_id.0,
                    source_chunk_ids: summary
                        .source_chunk_ids
                        .into_iter()
                        .map(|chunk_id| chunk_id.0)
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgetRequest {
    pub agent_id: String,
    #[serde(default)]
    pub chunk_type: Option<String>,
    #[serde(default = "default_now_ms")]
    pub now_ms: u64,
    #[serde(default)]
    pub recency_cutoff_ms: u64,
    #[serde(default = "default_forget_base_level_cutoff")]
    pub base_level_cutoff: f64,
    #[serde(default)]
    pub allow_linked_forget: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgetResponse {
    pub agent_id: String,
    pub examined: usize,
    pub forgotten_chunk_ids: Vec<String>,
    pub archived_chunk_ids: Vec<String>,
    pub protected_chunk_ids: Vec<String>,
}

impl From<StoreForgetReport> for ForgetResponse {
    fn from(report: StoreForgetReport) -> Self {
        Self {
            agent_id: report.agent_id.0,
            examined: report.examined,
            forgotten_chunk_ids: report
                .forgotten_chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.0)
                .collect(),
            archived_chunk_ids: report
                .archived_chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.0)
                .collect(),
            protected_chunk_ids: report
                .protected_chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.0)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssociateRequest {
    pub agent_id: String,
    pub src_chunk_id: String,
    pub dst_chunk_id: String,
    pub source: String,
    pub strength: f64,
    #[serde(default = "default_fan")]
    pub fan: u64,
    #[serde(default = "default_now_ms")]
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssociateResponse {
    pub agent_id: String,
    pub src_chunk_id: String,
    pub dst_chunk_id: String,
    pub source: String,
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferSetRequest {
    pub agent_id: String,
    pub chunk_id: String,
    #[serde(default = "default_now_ms")]
    pub set_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferResponse {
    pub agent_id: String,
    pub buffer_name: String,
    pub chunk_id: Option<String>,
    pub chunk_type: Option<String>,
    pub updated_at_ms: u64,
}

impl BufferResponse {
    pub fn from_snapshot(agent_id: String, snapshot: BufferSnapshot) -> Self {
        Self {
            agent_id,
            buffer_name: snapshot.name.as_str().to_string(),
            chunk_id: snapshot.chunk_id.map(|id| id.0),
            chunk_type: snapshot.chunk_type.map(|chunk_type| chunk_type.0),
            updated_at_ms: snapshot.updated_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleEvaluateRequest {
    pub agent_id: String,
    #[serde(default)]
    pub candidate_rule_ids: Vec<String>,
    #[serde(default)]
    pub rules: Vec<ProductionRuleDto>,
    #[serde(default)]
    pub retrieved_chunk_id: Option<String>,
    #[serde(default = "default_rule_selection_policy")]
    pub selection_policy: String,
    #[serde(default = "default_utility_temperature")]
    pub utility_temperature: f64,
    #[serde(default)]
    pub deterministic_seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductionRuleDto {
    pub rule_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub utility: f64,
    #[serde(default = "default_version")]
    pub version: u64,
    #[serde(default)]
    pub conditions: Vec<BufferConditionDto>,
    #[serde(default)]
    pub retrieved_chunk: Option<RetrievedChunkConditionDto>,
}

impl ProductionRuleDto {
    pub fn into_rule(self) -> MemoryResult<ProductionRule> {
        if self.rule_id.trim().is_empty() {
            return Err(MemoryError::Validation(
                "rule_id must not be empty".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(MemoryError::Validation(
                "name must not be empty".to_string(),
            ));
        }
        if !self.utility.is_finite() {
            return Err(MemoryError::Validation(
                "rule utility must be finite".to_string(),
            ));
        }

        let mut rule = ProductionRule::new(
            nestor_rules::RuleId::from(self.rule_id),
            self.name,
            self.conditions
                .into_iter()
                .map(BufferConditionDto::try_into_condition)
                .collect::<MemoryResult<Vec<_>>>()?,
        )
        .with_utility(self.utility)
        .with_version(self.version);
        if !self.enabled {
            rule.enabled = false;
        }
        if let Some(condition) = self.retrieved_chunk {
            rule = rule.with_retrieved_chunk(condition.into_condition());
        }
        Ok(rule)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferConditionDto {
    pub buffer: String,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub chunk_type: Option<String>,
}

impl BufferConditionDto {
    pub fn try_into_condition(self) -> MemoryResult<nestor_rules::BufferCondition> {
        Ok(nestor_rules::BufferCondition {
            buffer: parse_buffer_name(&self.buffer)?,
            chunk_id: self.chunk_id.map(ChunkId::from),
            chunk_type: self.chunk_type.map(ChunkType::from),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievedChunkConditionDto {
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub chunk_type: Option<String>,
    #[serde(default)]
    pub slots: Vec<SlotDto>,
}

impl RetrievedChunkConditionDto {
    pub fn into_condition(self) -> RetrievedChunkCondition {
        RetrievedChunkCondition {
            chunk_id: self.chunk_id.map(ChunkId::from),
            chunk_type: self.chunk_type.map(ChunkType::from),
            slots: self.slots.into_iter().map(Slot::from).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleEvaluateResponse {
    pub agent_id: String,
    pub selected: Option<RuleMatchDto>,
    pub matches: Vec<RuleMatchDto>,
    pub candidates: Vec<RuleCandidateDiagnosticDto>,
}

impl RuleEvaluateResponse {
    pub fn from_conflict(agent_id: String, conflict: ConflictResolution) -> Self {
        Self {
            agent_id,
            selected: conflict.selected.map(RuleMatchDto::from),
            matches: conflict
                .matches
                .into_iter()
                .map(RuleMatchDto::from)
                .collect(),
            candidates: conflict
                .candidates
                .into_iter()
                .map(RuleCandidateDiagnosticDto::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleMatchDto {
    pub rule_id: String,
    pub name: String,
    pub utility: f64,
    pub specificity: usize,
    pub version: u64,
    pub rank: usize,
}

impl From<nestor_rules::RuleMatch> for RuleMatchDto {
    fn from(value: nestor_rules::RuleMatch) -> Self {
        Self {
            rule_id: value.rule_id.0,
            name: value.name,
            utility: value.utility,
            specificity: value.specificity,
            version: value.version,
            rank: value.rank,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleCandidateDiagnosticDto {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub utility: f64,
    pub specificity: usize,
    pub version: u64,
    pub matched: bool,
    pub rank: Option<usize>,
    pub rejection_reason: Option<String>,
}

impl From<nestor_rules::RuleCandidateDiagnostic> for RuleCandidateDiagnosticDto {
    fn from(value: nestor_rules::RuleCandidateDiagnostic) -> Self {
        Self {
            rule_id: value.rule_id.0,
            name: value.name,
            enabled: value.enabled,
            utility: value.utility,
            specificity: value.specificity,
            version: value.version,
            matched: value.matched,
            rank: value.rank,
            rejection_reason: value.rejection_reason.map(rejection_reason_label),
        }
    }
}

fn rejection_reason_label(reason: RuleRejectionReason) -> String {
    match reason {
        RuleRejectionReason::Disabled => "disabled",
        RuleRejectionReason::NonFiniteUtility => "non_finite_utility",
        RuleRejectionReason::BufferConditionsNotMet => "buffer_conditions_not_met",
        RuleRejectionReason::RetrievedChunkConditionNotMet => "retrieved_chunk_condition_not_met",
    }
    .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiRetrievalStatus {
    Hit,
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiRetrievalMissReason {
    NoCandidates,
    Threshold,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoreComponents {
    pub base_level: f64,
    pub spreading: f64,
    pub partial_match: f64,
    pub noise: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub chunk_id: String,
    pub chunk_type: String,
    pub activation: f64,
    pub retrieval_probability: f64,
    pub predicted_latency_ms: f64,
    pub passes_threshold: bool,
    pub components: ScoreComponents,
    pub practice_input: RetrievalPracticeInputDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalPracticeInputDiagnostics {
    pub total_practice_event_count: usize,
    pub exact_practice_event_count: usize,
    pub compressed_practice_bin_count: usize,
    pub base_level_cache_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalDiagnostics {
    pub candidates_examined: usize,
    pub candidate_limit: usize,
    pub threshold: f64,
    pub deterministic_seed: Option<u64>,
    pub context_chunk_count: usize,
    pub activation_compute_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrieveResponse {
    pub agent_id: String,
    pub status: ApiRetrievalStatus,
    pub miss_reason: Option<ApiRetrievalMissReason>,
    pub results: Vec<RetrievalResult>,
    pub diagnostics: Option<RetrievalDiagnostics>,
}

impl RetrieveResponse {
    pub fn from_outcome(
        outcome: RetrievalOutcome,
        result_limit: usize,
        include_diagnostics: bool,
    ) -> Self {
        let miss_reason = outcome.miss.as_ref().map(|miss| match miss.reason {
            RetrievalMissReason::NoCandidates => ApiRetrievalMissReason::NoCandidates,
            RetrievalMissReason::Threshold => ApiRetrievalMissReason::Threshold,
        });
        Self {
            agent_id: outcome.agent_id.0,
            status: match outcome.status {
                RetrievalStatus::Hit => ApiRetrievalStatus::Hit,
                RetrievalStatus::Miss => ApiRetrievalStatus::Miss,
            },
            miss_reason,
            results: outcome
                .ranked_candidates
                .into_iter()
                .take(result_limit)
                .map(|candidate| RetrievalResult {
                    chunk_id: candidate.chunk.chunk_id.0,
                    chunk_type: candidate.chunk.chunk_type.0,
                    activation: candidate.score.activation,
                    retrieval_probability: candidate.score.retrieval_probability,
                    predicted_latency_ms: candidate.score.predicted_latency_ms,
                    passes_threshold: candidate.score.passes_threshold,
                    components: ScoreComponents {
                        base_level: candidate.score.base_level,
                        spreading: candidate.score.spreading,
                        partial_match: candidate.score.partial_match,
                        noise: candidate.score.noise,
                    },
                    practice_input: RetrievalPracticeInputDiagnostics {
                        total_practice_event_count: candidate
                            .practice_input
                            .total_practice_event_count,
                        exact_practice_event_count: candidate
                            .practice_input
                            .exact_practice_event_count,
                        compressed_practice_bin_count: candidate
                            .practice_input
                            .compressed_practice_bin_count,
                        base_level_cache_stale: candidate.practice_input.base_level_cache_stale,
                    },
                })
                .collect(),
            diagnostics: include_diagnostics.then_some(RetrievalDiagnostics {
                candidates_examined: outcome.diagnostics.candidates_examined,
                candidate_limit: outcome.diagnostics.candidate_limit,
                threshold: outcome.diagnostics.threshold,
                deterministic_seed: outcome.diagnostics.deterministic_seed,
                context_chunk_count: outcome.diagnostics.context_chunk_count,
                activation_compute_ms: outcome.diagnostics.activation_compute_ms,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub checks: Vec<HealthCheckDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckDto {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricDto {
    pub name: String,
    pub help: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub deleted: bool,
    pub agent_id: String,
    pub chunk_id: String,
}

pub fn parse_buffer_name(value: &str) -> MemoryResult<BufferName> {
    match value {
        "goal" => Ok(BufferName::Goal),
        "retrieval" => Ok(BufferName::Retrieval),
        "imaginal" => Ok(BufferName::Imaginal),
        "task" => Ok(BufferName::Task),
        custom if !custom.trim().is_empty() => Ok(BufferName::Custom(custom.to_string())),
        _ => Err(MemoryError::Validation(
            "buffer_name must not be empty".to_string(),
        )),
    }
}

pub fn evaluation_context<'a>(
    buffers: &'a [BufferSnapshot],
    retrieved_chunk: Option<&'a Chunk>,
) -> RuleEvaluationContext<'a> {
    let context = RuleEvaluationContext::from_buffers(buffers);
    if let Some(chunk) = retrieved_chunk {
        context.with_retrieved_chunk(chunk)
    } else {
        context
    }
}

fn default_candidate_limit() -> usize {
    200
}

fn default_result_limit() -> usize {
    12
}

fn default_true() -> bool {
    true
}

fn default_weight() -> f64 {
    1.0
}

fn default_fan() -> u64 {
    1
}

fn default_summary_chunk_type() -> String {
    "semantic".to_string()
}

fn default_min_group_size() -> usize {
    2
}

fn default_forget_base_level_cutoff() -> f64 {
    -4.0
}

fn default_version() -> u64 {
    1
}

fn default_rule_selection_policy() -> String {
    "specificity".to_string()
}

fn default_utility_temperature() -> f64 {
    1.0
}

fn default_now_ms() -> u64 {
    1_000
}
