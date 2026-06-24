use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use nestor_api::{ApiState, app_with_state};
use nestor_ops::{MemgraphSecurityConfig, RepositoryBackend, RuntimeConfig, SecretSource};
use serde_json::{Value, json};
use tower::ServiceExt;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn live_memgraph_api_state_persists_memory_across_rebuild() -> TestResult<()> {
    if std::env::var("NESTOR_STORE_MEMGRAPH_TESTS").as_deref() != Ok("1") {
        return Ok(());
    }

    let config = live_runtime_config();
    let agent_id = format!("agent-api-live-{}", now_ms());
    let chunk_id = format!("fact-{agent_id}");

    let app = app_with_state(ApiState::from_config(&config).await?);
    let (ready_status, ready_body) = get_json(app.clone(), "/readyz").await?;
    assert_eq!(ready_status, StatusCode::OK);
    assert_eq!(ready_body["status"], "pass");

    let (chunk_status, chunk_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/chunks",
        json!({
            "agent_id": agent_id,
            "chunk_id": chunk_id,
            "chunk_type": "fact",
            "now_ms": 1_000,
            "slots": {
                "topic": { "type": "symbol", "value": " ACT-R " },
                "detail": { "type": "text", "value": "Durable payload" },
                "confidence": { "type": "number", "value": 0.875 },
                "protected": { "type": "bool", "value": false }
            }
        }),
    )
    .await?;
    assert_eq!(chunk_status, StatusCode::OK);
    assert_eq!(chunk_body["slots"]["detail"]["value"], "Durable payload");

    let (retrieve_status, retrieve_body) = request_json(
        app,
        "POST",
        "/v1/memory/retrieve",
        json!({
            "agent_id": agent_id,
            "chunk_type": "fact",
            "cue_slots": [
                { "key": "topic", "value": { "type": "symbol", "value": "act-r" } }
            ],
            "candidate_limit": 10,
            "result_limit": 1,
            "activation_threshold": -10.0,
            "noise_s": 0.0,
            "partial_matching": true,
            "return_diagnostics": true,
            "deterministic_seed": 42,
            "commit_on_hit": true,
            "now_ms": 2_000
        }),
    )
    .await?;
    assert_eq!(retrieve_status, StatusCode::OK);
    assert_eq!(retrieve_body["status"], "hit");
    assert_eq!(retrieve_body["results"][0]["chunk_id"], chunk_id);

    let rebuilt_app = app_with_state(ApiState::from_config(&config).await?);
    let (get_status, get_body) = get_json(
        rebuilt_app.clone(),
        &format!("/v1/memory/chunks/{chunk_id}?agent_id={agent_id}"),
    )
    .await?;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(get_body["retrieval_count"], 1);
    assert_eq!(get_body["slots"]["topic"]["value"], " ACT-R ");
    assert_eq!(get_body["slots"]["confidence"]["value"], 0.875);

    let (second_retrieve_status, second_retrieve_body) = request_json(
        rebuilt_app,
        "POST",
        "/v1/memory/retrieve",
        json!({
            "agent_id": agent_id,
            "chunk_type": "fact",
            "cue_slots": [
                { "key": "topic", "value": { "type": "symbol", "value": "ACT-R" } }
            ],
            "candidate_limit": 10,
            "result_limit": 1,
            "activation_threshold": -10.0,
            "noise_s": 0.0,
            "partial_matching": true,
            "return_diagnostics": true,
            "deterministic_seed": 42,
            "commit_on_hit": false,
            "now_ms": 3_000
        }),
    )
    .await?;
    assert_eq!(second_retrieve_status, StatusCode::OK);
    assert_eq!(second_retrieve_body["status"], "hit");
    assert_eq!(
        second_retrieve_body["results"][0]["practice_input"]["exact_practice_event_count"],
        2
    );

    Ok(())
}

fn live_runtime_config() -> RuntimeConfig {
    let default = RuntimeConfig::default();
    let security = default.memgraph_security.clone();
    let credentials = if std::env::var("NESTOR_MEMGRAPH_PASSWORD").is_ok() {
        Some(SecretSource::EnvVar("NESTOR_MEMGRAPH_PASSWORD".to_string()))
    } else {
        security.credentials.clone()
    };
    let tls_ca_file = std::env::var("NESTOR_MEMGRAPH_TLS_CA_FILE")
        .ok()
        .or_else(|| security.tls_ca_file.clone());
    let memgraph_max_connections = std::env::var("NESTOR_MEMGRAPH_MAX_CONNECTIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default.memgraph_max_connections);

    RuntimeConfig {
        repository_backend: RepositoryBackend::Memgraph,
        memgraph_uri: std::env::var("NESTOR_MEMGRAPH_URI")
            .unwrap_or_else(|_| default.memgraph_uri.clone()),
        memgraph_user: std::env::var("NESTOR_MEMGRAPH_USER")
            .unwrap_or_else(|_| default.memgraph_user.clone()),
        memgraph_max_connections,
        memgraph_security: MemgraphSecurityConfig {
            credentials,
            tls_ca_file,
            ..security
        },
        ..default
    }
}

async fn request_json(
    app: Router,
    method: &str,
    uri: &str,
    body: Value,
) -> TestResult<(StatusCode, Value)> {
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

async fn get_json(app: Router, uri: &str) -> TestResult<(StatusCode, Value)> {
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

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}
