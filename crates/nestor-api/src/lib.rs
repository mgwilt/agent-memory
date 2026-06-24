#![forbid(unsafe_code)]

pub mod dto;
pub mod problem;
pub mod routes;
pub mod service;

pub use dto::{
    AgentQuery, ApiRetrievalMissReason, ApiRetrievalStatus, AssociateRequest, AssociateResponse,
    BufferResponse, BufferSetRequest, ChunkPatchRequest, ChunkResponse, ChunkUpsertRequest,
    ConsolidateRequest, ConsolidateResponse, ConsolidationGroupResponse, DeleteResponse,
    ForgetRequest, ForgetResponse, HealthCheckDto, HealthResponse, MetricDto, MetricsResponse,
    PracticeRequest, PracticeResponse, ProductionRuleDto, RehearseRequest, RehearseResponse,
    RetrievalDiagnostics, RetrievalPracticeInputDiagnostics, RetrievalResult, RetrieveRequest,
    RetrieveResponse, RuleCandidateDiagnosticDto, RuleEvaluateRequest, RuleEvaluateResponse,
    RuleMatchDto, ScoreComponents, SlotDto, SlotValueDto,
};
pub use problem::{ApiProblem, ProblemKind};
pub use routes::{RouteSpec, app, app_with_state, route_manifest, route_manifest_text, serve};
pub use service::{ApiCounters, ApiRepository, ApiState};
