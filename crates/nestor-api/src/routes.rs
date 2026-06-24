use std::time::Instant;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
    routing::{get, post, put},
};
use nestor_core::{
    ActivationParams, AgentId, Chunk, ChunkId, ChunkType, MemoryError, MemoryResult, Slot,
};
use nestor_ops::{HealthCheck, HealthStatus, RuntimeConfig, render_prometheus_metrics};
use nestor_rules::RuleId;
use nestor_store::{
    AssociationWrite, BufferSetCurrent, CreateChunk, MemoryRepository, MismatchPolicy,
    PracticeEventWrite, RetrievalRequest as StoreRetrievalRequest, UpdateChunk, retrieve_chunk,
};

use crate::{
    dto::{
        AgentQuery, AssociateRequest, AssociateResponse, BufferResponse, BufferSetRequest,
        ChunkPatchRequest, ChunkResponse, ChunkUpsertRequest, DeleteResponse, HealthCheckDto,
        HealthResponse, PracticeRequest, PracticeResponse, RetrieveRequest, RetrieveResponse,
        RuleEvaluateRequest, RuleEvaluateResponse, evaluation_context, parse_buffer_name,
    },
    problem::ApiProblem,
    service::ApiState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteSpec {
    pub method: &'static str,
    pub path: &'static str,
    pub purpose: &'static str,
}

pub fn route_manifest() -> Vec<RouteSpec> {
    vec![
        RouteSpec {
            method: "POST",
            path: "/v1/memory/chunks",
            purpose: "create or upsert chunk",
        },
        RouteSpec {
            method: "GET",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "inspect chunk",
        },
        RouteSpec {
            method: "PATCH",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "update chunk slots with optimistic versioning",
        },
        RouteSpec {
            method: "DELETE",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "soft-delete chunk",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/retrieve",
            purpose: "ACT-R retrieval with score breakdown",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/retrieve/stream",
            purpose: "retrieval diagnostics response",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/practice",
            purpose: "record encoding, retrieval, or rehearsal",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/associate",
            purpose: "add or update spreading-activation association",
        },
        RouteSpec {
            method: "PUT",
            path: "/v1/memory/buffers/{buffer_name}",
            purpose: "set current buffer chunk",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/rules/evaluate",
            purpose: "evaluate production candidates",
        },
        RouteSpec {
            method: "GET",
            path: "/healthz",
            purpose: "liveness",
        },
        RouteSpec {
            method: "GET",
            path: "/readyz",
            purpose: "readiness with dependency status",
        },
        RouteSpec {
            method: "GET",
            path: "/metrics",
            purpose: "Prometheus metrics scrape",
        },
    ]
}

pub fn app() -> Router {
    app_with_state(ApiState::new())
}

pub fn app_with_state(state: ApiState) -> Router {
    Router::new()
        .route("/v1/memory/chunks", post(upsert_chunk))
        .route(
            "/v1/memory/chunks/{chunk_id}",
            get(get_chunk).patch(patch_chunk).delete(delete_chunk),
        )
        .route("/v1/memory/retrieve", post(retrieve_memory))
        .route("/v1/memory/retrieve/stream", post(retrieve_memory))
        .route("/v1/memory/practice", post(record_practice))
        .route("/v1/memory/associate", post(upsert_association))
        .route("/v1/memory/buffers/{buffer_name}", put(set_buffer))
        .route("/v1/rules/evaluate", post(evaluate_rules))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .with_state(state)
}

pub async fn serve(config: RuntimeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    config.validate().map_err(MemoryError::Validation)?;
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(listener, app()).await?;
    Ok(())
}

pub fn route_manifest_text() -> String {
    route_manifest()
        .into_iter()
        .map(|route| format!("{} {} - {}", route.method, route.path, route.purpose))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn upsert_chunk(
    State(state): State<ApiState>,
    Json(req): Json<ChunkUpsertRequest>,
) -> Result<Json<ChunkResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    validate_non_empty("chunk_id", &req.chunk_id)?;
    validate_non_empty("chunk_type", &req.chunk_type)?;

    let mut chunk = Chunk::new(
        AgentId::from(req.agent_id.clone()),
        ChunkId::from(req.chunk_id.clone()),
        ChunkType::from(req.chunk_type),
        req.now_ms,
    );
    for (key, value) in req.slots {
        validate_non_empty("slot key", &key)?;
        chunk.upsert_slot(key, value.into());
    }
    let event_id = format!("encode-{}-{}-{}", req.agent_id, req.chunk_id, req.now_ms);
    let response = track_memory_result(
        &state,
        state
            .repository
            .upsert_chunk(CreateChunk {
                chunk,
                initial_practice_event_id: event_id,
            })
            .map(ChunkResponse::from),
    )?;
    Ok(Json(response))
}

async fn get_chunk(
    State(state): State<ApiState>,
    Path(chunk_id): Path<String>,
    Query(query): Query<AgentQuery>,
) -> Result<Json<ChunkResponse>, ApiProblem> {
    validate_non_empty("agent_id", &query.agent_id)?;
    validate_non_empty("chunk_id", &chunk_id)?;
    let chunk = state
        .repository
        .get_chunk(
            &AgentId::from(query.agent_id),
            &ChunkId::from(chunk_id.clone()),
        )?
        .ok_or_else(|| MemoryError::NotFound(format!("chunk {chunk_id}")))?;
    Ok(Json(ChunkResponse::from(chunk)))
}

async fn patch_chunk(
    State(state): State<ApiState>,
    Path(chunk_id): Path<String>,
    Json(req): Json<ChunkPatchRequest>,
) -> Result<Json<ChunkResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    validate_non_empty("chunk_id", &chunk_id)?;
    let slots = req
        .slots
        .into_iter()
        .map(|(key, value)| {
            validate_non_empty("slot key", &key)?;
            Ok(Slot {
                key,
                value: value.into(),
            })
        })
        .collect::<Result<Vec<_>, ApiProblem>>()?;
    let chunk = track_memory_result(
        &state,
        state.repository.update_chunk(UpdateChunk {
            agent_id: AgentId::from(req.agent_id),
            chunk_id: ChunkId::from(chunk_id),
            expected_version: req.expected_version,
            slots,
        }),
    )?;
    Ok(Json(ChunkResponse::from(chunk)))
}

async fn delete_chunk(
    State(state): State<ApiState>,
    Path(chunk_id): Path<String>,
    Query(query): Query<AgentQuery>,
) -> Result<Json<DeleteResponse>, ApiProblem> {
    validate_non_empty("agent_id", &query.agent_id)?;
    validate_non_empty("chunk_id", &chunk_id)?;
    track_memory_result(
        &state,
        state.repository.soft_delete_chunk(
            &AgentId::from(query.agent_id.clone()),
            &ChunkId::from(chunk_id.clone()),
        ),
    )?;
    Ok(Json(DeleteResponse {
        deleted: true,
        agent_id: query.agent_id,
        chunk_id,
    }))
}

async fn retrieve_memory(
    State(state): State<ApiState>,
    Json(req): Json<RetrieveRequest>,
) -> Result<Json<RetrieveResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    if req.result_limit == 0 {
        return Err(
            MemoryError::Validation("result_limit must be greater than zero".to_string()).into(),
        );
    }

    let mut retrieval = StoreRetrievalRequest::new(AgentId::from(req.agent_id.clone()), req.now_ms);
    retrieval.chunk_type = req.chunk_type;
    retrieval.cue_slots = req.cue_slots.into_iter().map(Slot::from).collect();
    retrieval.context_chunk_ids = req
        .context_chunk_ids
        .into_iter()
        .map(ChunkId::from)
        .collect();
    retrieval.candidate_limit = req.candidate_limit;
    retrieval.activation_params = ActivationParams {
        retrieval_threshold: req.activation_threshold,
        noise_s: req.noise_s,
        ..ActivationParams::deterministic()
    };
    retrieval.mismatch_policy = if req.partial_matching {
        MismatchPolicy::default()
    } else {
        MismatchPolicy::Disabled
    };
    retrieval.deterministic_seed = req.deterministic_seed;
    retrieval.commit_on_hit = req.commit_on_hit;

    let retrieval_started = Instant::now();
    let observed = track_memory_result(
        &state,
        state
            .sessions
            .with_session_observed(AgentId::from(req.agent_id), |session| {
                retrieve_chunk(&state.repository, session, retrieval)
            }),
    )?;
    if observed.contended {
        state.counters.record_session_lock_contention();
    }
    let outcome = observed.output;
    state.counters.record_retrieval_observation(
        retrieval_started.elapsed().as_secs_f64() * 1_000.0,
        outcome.diagnostics.candidates_examined,
        outcome.diagnostics.activation_compute_ms,
    );
    match outcome.status {
        nestor_store::RetrievalStatus::Hit => state.counters.record_retrieval_hit(),
        nestor_store::RetrievalStatus::Miss => state.counters.record_retrieval_miss(),
    }
    Ok(Json(RetrieveResponse::from_outcome(
        outcome,
        req.result_limit,
        req.return_diagnostics,
    )))
}

async fn record_practice(
    State(state): State<ApiState>,
    Json(req): Json<PracticeRequest>,
) -> Result<Json<PracticeResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    validate_non_empty("chunk_id", &req.chunk_id)?;
    validate_non_empty("kind", &req.kind)?;
    let event_id = req.event_id.unwrap_or_else(|| {
        format!(
            "practice-{}-{}-{}-{}",
            req.agent_id, req.chunk_id, req.occurred_at_ms, req.kind
        )
    });
    track_memory_result(
        &state,
        state.repository.append_practice_event(PracticeEventWrite {
            event_id: event_id.clone(),
            agent_id: AgentId::from(req.agent_id.clone()),
            chunk_id: ChunkId::from(req.chunk_id.clone()),
            occurred_at_ms: req.occurred_at_ms,
            kind: req.kind.clone(),
            weight: req.weight,
        }),
    )?;
    Ok(Json(PracticeResponse {
        event_id,
        agent_id: req.agent_id,
        chunk_id: req.chunk_id,
        kind: req.kind,
        weight: req.weight,
    }))
}

async fn upsert_association(
    State(state): State<ApiState>,
    Json(req): Json<AssociateRequest>,
) -> Result<Json<AssociateResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    validate_non_empty("src_chunk_id", &req.src_chunk_id)?;
    validate_non_empty("dst_chunk_id", &req.dst_chunk_id)?;
    validate_non_empty("source", &req.source)?;
    track_memory_result(
        &state,
        state.repository.upsert_association(AssociationWrite {
            agent_id: AgentId::from(req.agent_id.clone()),
            src_chunk_id: ChunkId::from(req.src_chunk_id.clone()),
            dst_chunk_id: ChunkId::from(req.dst_chunk_id.clone()),
            source: req.source.clone(),
            strength: req.strength,
            fan: req.fan,
            updated_at_ms: req.updated_at_ms,
        }),
    )?;
    Ok(Json(AssociateResponse {
        agent_id: req.agent_id,
        src_chunk_id: req.src_chunk_id,
        dst_chunk_id: req.dst_chunk_id,
        source: req.source,
        strength: req.strength,
    }))
}

async fn set_buffer(
    State(state): State<ApiState>,
    Path(buffer_name): Path<String>,
    Json(req): Json<BufferSetRequest>,
) -> Result<Json<BufferResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    validate_non_empty("chunk_id", &req.chunk_id)?;
    let buffer_name = parse_buffer_name(&buffer_name)?;
    let agent_id = AgentId::from(req.agent_id.clone());
    let chunk_id = ChunkId::from(req.chunk_id.clone());
    let chunk = state
        .repository
        .get_chunk(&agent_id, &chunk_id)?
        .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk_id)))?;
    track_memory_result(
        &state,
        state.repository.set_buffer_current(BufferSetCurrent {
            agent_id: agent_id.clone(),
            buffer_name: buffer_name.clone(),
            chunk_id: chunk_id.clone(),
            set_at_ms: req.set_at_ms,
        }),
    )?;
    let snapshot = state.sessions.with_session(agent_id, |session| {
        Ok(session.set_buffer(
            buffer_name,
            chunk_id,
            chunk.chunk_type.clone(),
            req.set_at_ms,
        ))
    })?;
    Ok(Json(BufferResponse::from_snapshot(req.agent_id, snapshot)))
}

async fn evaluate_rules(
    State(state): State<ApiState>,
    Json(req): Json<RuleEvaluateRequest>,
) -> Result<Json<RuleEvaluateResponse>, ApiProblem> {
    validate_non_empty("agent_id", &req.agent_id)?;
    let agent_id = AgentId::from(req.agent_id.clone());
    let mut rules = req
        .rules
        .into_iter()
        .map(|rule| rule.into_rule())
        .collect::<MemoryResult<Vec<_>>>()?;

    if rules.is_empty() {
        for rule_id in &req.candidate_rule_ids {
            if let Some(record) = state
                .repository
                .get_production_rule(&agent_id, &RuleId::from(rule_id.clone()))?
            {
                rules.push(record.rule);
            }
        }
    } else if !req.candidate_rule_ids.is_empty() {
        rules.retain(|rule| {
            req.candidate_rule_ids
                .iter()
                .any(|rule_id| rule.rule_id.as_str() == rule_id)
        });
    }

    let retrieved_chunk = req
        .retrieved_chunk_id
        .map(|chunk_id| {
            state
                .repository
                .get_chunk(&agent_id, &ChunkId::from(chunk_id.clone()))?
                .ok_or_else(|| MemoryError::NotFound(format!("chunk {chunk_id}")))
        })
        .transpose()?;
    let buffers = state.session_snapshot(agent_id)?;
    let context = evaluation_context(&buffers, retrieved_chunk.as_ref());
    let conflict = state.rule_engine(rules).conflict_resolution(context);
    Ok(Json(RuleEvaluateResponse::from_conflict(
        req.agent_id,
        conflict,
    )))
}

async fn healthz() -> Json<HealthResponse> {
    Json(health_response("pass", vec![HealthCheck::liveness()]))
}

async fn readyz() -> (StatusCode, Json<HealthResponse>) {
    let response = health_response(
        "warn",
        vec![HealthCheck::liveness(), HealthCheck::memgraph_unchecked()],
    );
    (StatusCode::OK, Json(response))
}

async fn metrics(State(state): State<ApiState>) -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        render_prometheus_metrics(&state.counters.metric_samples()),
    )
}

fn health_response(status: &str, checks: Vec<HealthCheck>) -> HealthResponse {
    HealthResponse {
        status: status.to_string(),
        checks: checks
            .into_iter()
            .map(|check| HealthCheckDto {
                name: check.name.to_string(),
                status: health_status_label(check.status).to_string(),
                detail: check.detail.to_string(),
            })
            .collect(),
    }
}

fn health_status_label(status: HealthStatus) -> &'static str {
    match status {
        HealthStatus::Pass => "pass",
        HealthStatus::Warn => "warn",
        HealthStatus::Fail => "fail",
    }
}

fn track_memory_result<T>(state: &ApiState, result: MemoryResult<T>) -> Result<T, ApiProblem> {
    if matches!(result, Err(MemoryError::Conflict(_))) {
        state.counters.record_write_conflict();
    }
    result.map_err(ApiProblem::from)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ApiProblem> {
    if value.trim().is_empty() {
        Err(MemoryError::Validation(format!("{field} must not be empty")).into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header::CONTENT_TYPE},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    async fn request_json(
        app: Router,
        method: &str,
        uri: &str,
        body: Value,
    ) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body)?))?;
        let response = app.oneshot(request).await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value = serde_json::from_slice(&bytes)?;
        Ok((status, value))
    }

    async fn get_json(
        app: Router,
        uri: &str,
    ) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value = serde_json::from_slice(&bytes)?;
        Ok((status, value))
    }

    async fn get_text(
        app: Router,
        uri: &str,
    ) -> Result<(StatusCode, Option<String>, String), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = String::from_utf8(bytes.to_vec())?;
        Ok((status, content_type, body))
    }

    async fn seed_chunk(
        app: Router,
        chunk_id: &str,
        topic: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (status, _) = request_json(
            app,
            "POST",
            "/v1/memory/chunks",
            json!({
                "agent_id": "agent-1",
                "chunk_id": chunk_id,
                "chunk_type": "fact",
                "now_ms": 1000,
                "slots": {
                    "topic": { "type": "symbol", "value": topic }
                }
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn route_manifest_contains_reported_endpoints() {
        let routes = route_manifest();

        assert!(
            routes
                .iter()
                .any(|route| route.path == "/v1/memory/retrieve")
        );
        assert!(routes.iter().any(|route| route.path == "/metrics"));
        assert!(routes.iter().any(|route| route.path == "/readyz"));
    }

    #[tokio::test]
    async fn chunk_retrieve_and_metrics_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let app = app();
        seed_chunk(app.clone(), "ck-nestor", "act-r").await?;

        let (retrieve_status, retrieve_body) = request_json(
            app.clone(),
            "POST",
            "/v1/memory/retrieve",
            json!({
                "agent_id": "agent-1",
                "chunk_type": "fact",
                "now_ms": 2000,
                "activation_threshold": -5.0,
                "cue_slots": [
                    { "key": "topic", "value": { "type": "symbol", "value": "ACT-R" } }
                ]
            }),
        )
        .await?;
        assert_eq!(retrieve_status, StatusCode::OK);
        assert_eq!(retrieve_body["status"], "hit");
        assert_eq!(retrieve_body["results"][0]["chunk_id"], "ck-nestor");
        assert!(retrieve_body["diagnostics"]["candidates_examined"].as_u64() == Some(1));

        let (metrics_status, content_type, metrics_body) = get_text(app, "/metrics").await?;
        assert_eq!(metrics_status, StatusCode::OK);
        assert!(content_type.is_some_and(|value| value.starts_with("text/plain")));
        assert!(metrics_body.contains("# TYPE nestor_memory_retrieval_hits_total counter"));
        assert!(metrics_body.contains("nestor_memory_retrieval_hits_total 1"));
        assert!(metrics_body.contains("nestor_memory_candidates_examined 1"));
        assert!(metrics_body.contains("nestor_memory_activation_compute_ms"));
        assert!(metrics_body.contains("nestor_memory_session_lock_contention_total"));
        Ok(())
    }

    #[tokio::test]
    async fn practice_association_buffer_and_rule_endpoints_work()
    -> Result<(), Box<dyn std::error::Error>> {
        let app = app();
        seed_chunk(app.clone(), "ctx", "goal").await?;
        seed_chunk(app.clone(), "fact", "act-r").await?;

        let (practice_status, practice_body) = request_json(
            app.clone(),
            "POST",
            "/v1/memory/practice",
            json!({
                "agent_id": "agent-1",
                "chunk_id": "fact",
                "event_id": "practice-1",
                "kind": "retrieve",
                "weight": 1.0,
                "occurred_at_ms": 2000
            }),
        )
        .await?;
        assert_eq!(practice_status, StatusCode::OK);
        assert_eq!(practice_body["event_id"], "practice-1");

        let (assoc_status, assoc_body) = request_json(
            app.clone(),
            "POST",
            "/v1/memory/associate",
            json!({
                "agent_id": "agent-1",
                "src_chunk_id": "ctx",
                "dst_chunk_id": "fact",
                "source": "goal",
                "strength": 1.5
            }),
        )
        .await?;
        assert_eq!(assoc_status, StatusCode::OK);
        assert_eq!(assoc_body["strength"], 1.5);

        let (buffer_status, buffer_body) = request_json(
            app.clone(),
            "PUT",
            "/v1/memory/buffers/goal",
            json!({
                "agent_id": "agent-1",
                "chunk_id": "ctx",
                "set_at_ms": 2500
            }),
        )
        .await?;
        assert_eq!(buffer_status, StatusCode::OK);
        assert_eq!(buffer_body["chunk_id"], "ctx");

        let (rule_status, rule_body) = request_json(
            app,
            "POST",
            "/v1/rules/evaluate",
            json!({
                "agent_id": "agent-1",
                "rules": [{
                    "rule_id": "rule-1",
                    "name": "goal present",
                    "utility": 2.0,
                    "conditions": [{ "buffer": "goal", "chunk_type": "fact" }]
                }]
            }),
        )
        .await?;
        assert_eq!(rule_status, StatusCode::OK);
        assert_eq!(rule_body["selected"]["rule_id"], "rule-1");
        Ok(())
    }

    #[tokio::test]
    async fn validation_errors_return_problem_json() -> Result<(), Box<dyn std::error::Error>> {
        let (status, body) = request_json(
            app(),
            "POST",
            "/v1/memory/chunks",
            json!({
                "agent_id": "",
                "chunk_id": "ck",
                "chunk_type": "fact",
                "slots": {}
            }),
        )
        .await?;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["kind"], "bad_request");
        assert_eq!(body["status"], 400);
        Ok(())
    }

    #[tokio::test]
    async fn health_and_ready_are_json() -> Result<(), Box<dyn std::error::Error>> {
        let app = app();

        let (health_status, health_body) = get_json(app.clone(), "/healthz").await?;
        let (ready_status, ready_body) = get_json(app, "/readyz").await?;

        assert_eq!(health_status, StatusCode::OK);
        assert_eq!(health_body["status"], "pass");
        assert_eq!(ready_status, StatusCode::OK);
        assert_eq!(ready_body["status"], "warn");
        Ok(())
    }
}
