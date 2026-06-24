use std::collections::BTreeSet;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use nestor_api::{app, route_manifest};
use serde_json::{Value, json};
use tower::ServiceExt;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn all_http_endpoints_and_activation_formulas_work() -> TestResult<()> {
    let app = app();
    let agent_id = "agent-http-e2e";
    let mut covered = BTreeSet::new();

    let (health_status, health_body) = get_json(app.clone(), "/healthz").await?;
    covered.insert("GET /healthz");
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(health_body["status"], "pass");

    let (ready_status, ready_body) = get_json(app.clone(), "/readyz").await?;
    covered.insert("GET /readyz");
    assert_eq!(ready_status, StatusCode::OK);
    assert_eq!(ready_body["status"], "pass");
    assert_eq!(ready_body["checks"][1]["name"], "memgraph");
    assert_eq!(ready_body["checks"][1]["status"], "pass");

    create_chunk(
        app.clone(),
        agent_id,
        "ctx-goal",
        "goal",
        1_000,
        json!({
            "task": symbol("answer-memory-question"),
            "owner": symbol("eli")
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/chunks");

    create_chunk(
        app.clone(),
        agent_id,
        "mem-preference",
        "fact",
        1_000,
        json!({
            "topic": symbol("preference"),
            "subject": symbol("eli"),
            "detail": symbol("strong-black-coffee")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "mem-project",
        "fact",
        1_000,
        json!({
            "topic": symbol("project"),
            "subject": symbol("eli"),
            "detail": symbol("agent-memory-cli")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "delete-me",
        "fact",
        1_000,
        json!({
            "topic": symbol("temporary")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "episode-a",
        "episode",
        1_000,
        json!({
            "topic": symbol("preference"),
            "subject": symbol("eli"),
            "detail": symbol("coffee-a")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "episode-b",
        "episode",
        1_100,
        json!({
            "topic": symbol("preference"),
            "subject": symbol("eli"),
            "detail": symbol("coffee-b")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "forget-old",
        "stale",
        100,
        json!({
            "topic": symbol("old")
        }),
    )
    .await?;

    create_chunk(
        app.clone(),
        agent_id,
        "forget-protected",
        "stale",
        100,
        json!({
            "topic": symbol("old"),
            "protected": { "type": "bool", "value": true }
        }),
    )
    .await?;

    let (get_status, get_body) = get_json(
        app.clone(),
        &format!("/v1/memory/chunks/mem-preference?agent_id={agent_id}"),
    )
    .await?;
    covered.insert("GET /v1/memory/chunks/{chunk_id}");
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(get_body["slots"]["detail"], symbol("strong-black-coffee"));

    let (patch_status, patch_body) = request_json(
        app.clone(),
        "PATCH",
        "/v1/memory/chunks/mem-project",
        json!({
            "agent_id": agent_id,
            "expected_version": 1,
            "slots": {
                "topic": symbol("project"),
                "subject": symbol("eli"),
                "detail": symbol("agent-memory-cli"),
                "verified": { "type": "bool", "value": true }
            }
        }),
    )
    .await?;
    covered.insert("PATCH /v1/memory/chunks/{chunk_id}");
    assert_eq!(patch_status, StatusCode::OK);
    assert_eq!(
        patch_body["slots"]["verified"],
        json!({ "type": "bool", "value": true })
    );

    let (practice_status, practice_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/practice",
        json!({
            "agent_id": agent_id,
            "chunk_id": "mem-preference",
            "event_id": "practice-http-e2e-preference-1",
            "kind": "retrieve",
            "weight": 2.0,
            "occurred_at_ms": 1_500
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/practice");
    assert_eq!(practice_status, StatusCode::OK);
    assert_eq!(practice_body["weight"], 2.0);

    let (rehearse_status, rehearse_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/rehearse",
        json!({
            "agent_id": agent_id,
            "chunk_id": "mem-project",
            "event_id": "rehearse-http-e2e-project-1",
            "weight": 1.0,
            "occurred_at_ms": 1_600
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/rehearse");
    assert_eq!(rehearse_status, StatusCode::OK);
    assert_eq!(rehearse_body["kind"], "rehearse");

    let (associate_status, associate_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/associate",
        json!({
            "agent_id": agent_id,
            "src_chunk_id": "ctx-goal",
            "dst_chunk_id": "mem-preference",
            "source": "goal",
            "strength": 1.25,
            "fan": 1,
            "updated_at_ms": 2_000
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/associate");
    assert_eq!(associate_status, StatusCode::OK);
    assert_eq!(associate_body["strength"], 1.25);

    let (buffer_status, buffer_body) = request_json(
        app.clone(),
        "PUT",
        "/v1/memory/buffers/goal",
        json!({
            "agent_id": agent_id,
            "chunk_id": "ctx-goal",
            "set_at_ms": 2_500
        }),
    )
    .await?;
    covered.insert("PUT /v1/memory/buffers/{buffer_name}");
    assert_eq!(buffer_status, StatusCode::OK);
    assert_eq!(buffer_body["chunk_id"], "ctx-goal");
    assert_eq!(buffer_body["chunk_type"], "goal");

    let retrieval_request = json!({
        "agent_id": agent_id,
        "chunk_type": "fact",
        "cue_slots": [
            { "key": "topic", "value": symbol("preference") }
        ],
        "context_chunk_ids": ["ctx-goal"],
        "candidate_limit": 10,
        "result_limit": 3,
        "activation_threshold": -10.0,
        "noise_s": 0.0,
        "partial_matching": true,
        "return_diagnostics": true,
        "deterministic_seed": 42,
        "commit_on_hit": true,
        "now_ms": 11_000
    });

    let mut stream_request = retrieval_request.clone();
    stream_request["commit_on_hit"] = json!(false);
    let (stream_status, stream_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/retrieve/stream",
        stream_request,
    )
    .await?;
    covered.insert("POST /v1/memory/retrieve/stream");
    assert_eq!(stream_status, StatusCode::OK);
    assert_eq!(stream_body["status"], "hit");
    assert_eq!(stream_body["results"][0]["chunk_id"], "mem-preference");

    let (retrieve_status, retrieve_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/retrieve",
        retrieval_request,
    )
    .await?;
    covered.insert("POST /v1/memory/retrieve");
    assert_eq!(retrieve_status, StatusCode::OK);
    assert_eq!(retrieve_body["status"], "hit");
    assert_eq!(retrieve_body["diagnostics"]["candidates_examined"], 1);
    assert_eq!(
        retrieve_body["results"][0]["practice_input"]["exact_practice_event_count"],
        2
    );
    assert_retrieval_formula(&retrieve_body)?;

    let (retrieved_get_status, retrieved_get_body) = get_json(
        app.clone(),
        &format!("/v1/memory/chunks/mem-preference?agent_id={agent_id}"),
    )
    .await?;
    assert_eq!(retrieved_get_status, StatusCode::OK);
    assert_eq!(retrieved_get_body["retrieval_count"], 1);

    let (consolidate_status, consolidate_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/consolidate",
        json!({
            "agent_id": agent_id,
            "chunk_type": "episode",
            "summary_chunk_type": "semantic",
            "group_slot_keys": ["topic", "subject"],
            "min_group_size": 2,
            "now_ms": 12_000
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/consolidate");
    assert_eq!(consolidate_status, StatusCode::OK);
    assert_eq!(consolidate_body["groups_consolidated"], 1);
    assert_eq!(
        consolidate_body["summaries"][0]["source_chunk_ids"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let (rule_status, rule_body) = request_json(
        app.clone(),
        "POST",
        "/v1/rules/evaluate",
        json!({
            "agent_id": agent_id,
            "retrieved_chunk_id": "mem-preference",
            "rules": [
                {
                    "rule_id": "answer-with-retrieved-preference",
                    "name": "answer with retrieved preference",
                    "utility": 2.0,
                    "conditions": [
                        { "buffer": "goal", "chunk_type": "goal" }
                    ],
                    "retrieved_chunk": {
                        "chunk_type": "fact",
                        "slots": [
                            { "key": "topic", "value": symbol("preference") }
                        ]
                    }
                }
            ]
        }),
    )
    .await?;
    covered.insert("POST /v1/rules/evaluate");
    assert_eq!(rule_status, StatusCode::OK);
    assert_eq!(
        rule_body["selected"]["rule_id"],
        "answer-with-retrieved-preference"
    );

    let (delete_status, delete_body) = request_empty(
        app.clone(),
        "DELETE",
        &format!("/v1/memory/chunks/delete-me?agent_id={agent_id}"),
    )
    .await?;
    covered.insert("DELETE /v1/memory/chunks/{chunk_id}");
    assert_eq!(delete_status, StatusCode::OK);
    assert_eq!(delete_body["deleted"], true);

    let (forget_status, forget_body) = request_json(
        app.clone(),
        "POST",
        "/v1/memory/forget",
        json!({
            "agent_id": agent_id,
            "chunk_type": "stale",
            "now_ms": 1_000_000,
            "recency_cutoff_ms": 500,
            "base_level_cutoff": 0.0,
            "allow_linked_forget": false
        }),
    )
    .await?;
    covered.insert("POST /v1/memory/forget");
    assert_eq!(forget_status, StatusCode::OK);
    assert_eq!(forget_body["forgotten_chunk_ids"], json!(["forget-old"]));
    assert_eq!(
        forget_body["protected_chunk_ids"],
        json!(["forget-protected"])
    );

    let (metrics_status, metrics_content_type, metrics_body) =
        get_text(app.clone(), "/metrics").await?;
    covered.insert("GET /metrics");
    assert_eq!(metrics_status, StatusCode::OK);
    assert!(metrics_content_type.is_some_and(|value| value.starts_with("text/plain")));
    assert!(metrics_body.contains("nestor_memory_retrieval_hits_total 2"));
    assert!(metrics_body.contains("nestor_memory_candidates_examined 1"));

    let uncovered = route_manifest()
        .into_iter()
        .filter(|route| !covered.contains(format!("{} {}", route.method, route.path).as_str()))
        .collect::<Vec<_>>();
    assert!(
        uncovered.is_empty(),
        "route manifest entries were not covered: {uncovered:?}"
    );

    Ok(())
}

async fn create_chunk(
    app: Router,
    agent_id: &str,
    chunk_id: &str,
    chunk_type: &str,
    now_ms: u64,
    slots: Value,
) -> TestResult<Value> {
    let (status, body) = request_json(
        app,
        "POST",
        "/v1/memory/chunks",
        json!({
            "agent_id": agent_id,
            "chunk_id": chunk_id,
            "chunk_type": chunk_type,
            "now_ms": now_ms,
            "slots": slots
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["chunk_id"], chunk_id);
    Ok(body)
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
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body)?))?;
    json_response(app, request).await
}

async fn request_empty(app: Router, method: &str, uri: &str) -> TestResult<(StatusCode, Value)> {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())?;
    json_response(app, request).await
}

async fn get_json(app: Router, uri: &str) -> TestResult<(StatusCode, Value)> {
    request_empty(app, "GET", uri).await
}

async fn json_response(app: Router, request: Request<Body>) -> TestResult<(StatusCode, Value)> {
    let response = app.oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

async fn get_text(app: Router, uri: &str) -> TestResult<(StatusCode, Option<String>, String)> {
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
    Ok((status, content_type, String::from_utf8(bytes.to_vec())?))
}

fn symbol(value: &str) -> Value {
    json!({ "type": "symbol", "value": value })
}

fn assert_retrieval_formula(body: &Value) -> TestResult<()> {
    let result = &body["results"][0];
    let base_level = numeric(&result["components"]["base_level"], "base_level")?;
    let spreading = numeric(&result["components"]["spreading"], "spreading")?;
    let partial_match = numeric(&result["components"]["partial_match"], "partial_match")?;
    let noise = numeric(&result["components"]["noise"], "noise")?;
    let activation = numeric(&result["activation"], "activation")?;
    let probability = numeric(&result["retrieval_probability"], "retrieval_probability")?;
    let latency = numeric(&result["predicted_latency_ms"], "predicted_latency_ms")?;

    let expected_base = (10.0_f64.powf(-0.5) + 2.0 * 9.5_f64.powf(-0.5)).ln();
    let expected_activation = expected_base + 1.25;
    let expected_latency = 350.0 * (-expected_activation).exp();

    assert_close(base_level, expected_base, 1e-12, "base_level");
    assert_close(spreading, 1.25, 1e-12, "spreading");
    assert_close(partial_match, 0.0, 1e-12, "partial_match");
    assert_close(noise, 0.0, 1e-12, "noise");
    assert_close(activation, expected_activation, 1e-12, "activation");
    assert_close(probability, 1.0, 1e-12, "retrieval_probability");
    assert_close(latency, expected_latency, 1e-9, "predicted_latency_ms");
    assert_eq!(result["passes_threshold"], true);
    Ok(())
}

fn numeric(value: &Value, label: &str) -> TestResult<f64> {
    value
        .as_f64()
        .ok_or_else(|| format!("{label} was not numeric").into())
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label} mismatch: actual={actual}, expected={expected}, tolerance={tolerance}"
    );
}
