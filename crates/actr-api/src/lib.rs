#![forbid(unsafe_code)]

pub mod dto;
pub mod problem;
pub mod routes;
pub mod service;

pub use dto::{
    AgentQuery, ApiRetrievalMissReason, ApiRetrievalStatus, AssociateRequest, AssociateResponse,
    BufferResponse, BufferSetRequest, ChunkPatchRequest, ChunkResponse, ChunkUpsertRequest,
    DeleteResponse, HealthCheckDto, HealthResponse, MetricDto, MetricsResponse, PracticeRequest,
    PracticeResponse, ProductionRuleDto, RetrievalDiagnostics, RetrievalResult, RetrieveRequest,
    RetrieveResponse, RuleCandidateDiagnosticDto, RuleEvaluateRequest, RuleEvaluateResponse,
    RuleMatchDto, ScoreComponents, SlotDto, SlotValueDto,
};
pub use problem::{ApiProblem, ProblemKind};
pub use routes::{RouteSpec, app, app_with_state, route_manifest, route_manifest_text, serve};
pub use service::{ApiCounters, ApiRepository, ApiState};
