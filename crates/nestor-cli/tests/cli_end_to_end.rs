use std::{
    collections::BTreeSet,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use nestor_api::route_manifest;
use serde_json::{Value, json};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_covers_all_api_routes_and_validates_retrieval_math() -> TestResult<()> {
    let api_url = start_api().await?;
    let agent_id = "agent-cli-e2e";
    let mut covered = BTreeSet::new();

    run_ok(&api_url, &["health"])?;
    covered.insert("GET /healthz");

    run_ok(&api_url, &["ready"])?;
    covered.insert("GET /readyz");
    let ready_body = run_json(&api_url, &["--format", "json", "ready"])?;
    assert_eq!(ready_body["status"], "pass");
    assert_eq!(ready_body["checks"][1]["name"], "memgraph");
    assert_eq!(ready_body["checks"][1]["status"], "pass");

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "ctx-goal",
            "--type",
            "goal",
            "--slot",
            "task=symbol:answer-memory-question",
            "--slot",
            "owner=symbol:eli",
            "--now-ms",
            "1000",
        ],
    )?;
    covered.insert("POST /v1/memory/chunks");

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "mem-preference",
            "--type",
            "fact",
            "--slot",
            "topic=symbol:preference",
            "--slot",
            "subject=symbol:eli",
            "--slot",
            "detail=symbol:strong-black-coffee",
            "--now-ms",
            "1000",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "episode-a",
            "--type",
            "episode",
            "--slot",
            "topic=symbol:preference",
            "--slot",
            "subject=symbol:eli",
            "--slot",
            "detail=symbol:coffee-a",
            "--now-ms",
            "1000",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "episode-b",
            "--type",
            "episode",
            "--slot",
            "topic=symbol:preference",
            "--slot",
            "subject=symbol:eli",
            "--slot",
            "detail=symbol:coffee-b",
            "--now-ms",
            "1100",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "forget-old",
            "--type",
            "stale",
            "--slot",
            "topic=symbol:old",
            "--now-ms",
            "100",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "forget-protected",
            "--type",
            "stale",
            "--slot",
            "topic=symbol:old",
            "--slot",
            "protected=bool:true",
            "--now-ms",
            "100",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "mem-project",
            "--type",
            "fact",
            "--slot",
            "topic=symbol:project",
            "--slot",
            "subject=symbol:eli",
            "--slot",
            "detail=symbol:agent-memory-cli",
            "--now-ms",
            "1000",
        ],
    )?;

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "chunk",
            "put",
            "delete-me",
            "--type",
            "fact",
            "--slot",
            "topic=symbol:temporary",
            "--now-ms",
            "1000",
        ],
    )?;

    let get_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "chunk",
            "get",
            "mem-preference",
        ],
    )?;
    covered.insert("GET /v1/memory/chunks/{chunk_id}");
    assert_eq!(get_body["slots"]["detail"], symbol("strong-black-coffee"));

    let patch_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "chunk",
            "patch",
            "mem-project",
            "--expected-version",
            "1",
            "--slot",
            "topic=symbol:project",
            "--slot",
            "subject=symbol:eli",
            "--slot",
            "detail=symbol:agent-memory-cli",
            "--slot",
            "verified=bool:true",
        ],
    )?;
    covered.insert("PATCH /v1/memory/chunks/{chunk_id}");
    assert_eq!(
        patch_body["slots"]["verified"],
        json!({"type": "bool", "value": true})
    );

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "practice",
            "mem-preference",
            "--kind",
            "retrieve",
            "--weight",
            "2",
            "--at-ms",
            "1500",
            "--event-id",
            "practice-cli-e2e-preference-1",
        ],
    )?;
    covered.insert("POST /v1/memory/practice");

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "rehearse",
            "mem-project",
            "--weight",
            "1",
            "--at-ms",
            "1600",
            "--event-id",
            "rehearse-cli-e2e-project-1",
        ],
    )?;
    covered.insert("POST /v1/memory/rehearse");

    run_ok(
        &api_url,
        &[
            "--agent",
            agent_id,
            "associate",
            "ctx-goal",
            "mem-preference",
            "--source",
            "goal",
            "--strength",
            "1.25",
            "--fan",
            "1",
            "--at-ms",
            "2000",
        ],
    )?;
    covered.insert("POST /v1/memory/associate");

    run_ok(
        &api_url,
        &[
            "--agent", agent_id, "buffer", "set", "goal", "ctx-goal", "--at-ms", "2500",
        ],
    )?;
    covered.insert("PUT /v1/memory/buffers/{buffer_name}");

    let stream_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "retrieve",
            "--endpoint",
            "stream",
            "--type",
            "fact",
            "--cue",
            "topic=symbol:preference",
            "--context",
            "ctx-goal",
            "--threshold",
            "-10",
            "--result-limit",
            "3",
            "--seed",
            "42",
            "--commit",
            "false",
            "--now-ms",
            "11000",
        ],
    )?;
    covered.insert("POST /v1/memory/retrieve/stream");
    assert_eq!(stream_body["status"], "hit");

    let retrieve_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "retrieve",
            "--type",
            "fact",
            "--cue",
            "topic=symbol:preference",
            "--context",
            "ctx-goal",
            "--threshold",
            "-10",
            "--result-limit",
            "3",
            "--seed",
            "42",
            "--now-ms",
            "11000",
        ],
    )?;
    covered.insert("POST /v1/memory/retrieve");
    assert_eq!(retrieve_body["status"], "hit");
    assert_eq!(retrieve_body["results"][0]["chunk_id"], "mem-preference");
    assert_retrieval_formula(&retrieve_body)?;

    let retrieved_get_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "chunk",
            "get",
            "mem-preference",
        ],
    )?;
    assert_eq!(retrieved_get_body["retrieval_count"], 1);

    let consolidate_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "consolidate",
            "--type",
            "episode",
            "--summary-type",
            "semantic",
            "--group-slot",
            "topic",
            "--group-slot",
            "subject",
            "--min-group-size",
            "2",
            "--now-ms",
            "12000",
        ],
    )?;
    covered.insert("POST /v1/memory/consolidate");
    assert_eq!(consolidate_body["groups_consolidated"], 1);

    let rules_file = write_rules_file()?;
    let rule_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "rule",
            "eval",
            "--retrieved",
            "mem-preference",
            "--rules-file",
            path_str(&rules_file)?,
        ],
    )?;
    covered.insert("POST /v1/rules/evaluate");
    assert_eq!(
        rule_body["selected"]["rule_id"],
        "answer-with-retrieved-preference"
    );

    let delete_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "chunk",
            "delete",
            "delete-me",
            "--yes",
        ],
    )?;
    covered.insert("DELETE /v1/memory/chunks/{chunk_id}");
    assert_eq!(delete_body["deleted"], true);

    let forget_body = run_json(
        &api_url,
        &[
            "--agent",
            agent_id,
            "--format",
            "json",
            "forget",
            "--type",
            "stale",
            "--now-ms",
            "1000000",
            "--recency-cutoff-ms",
            "500",
            "--base-level-cutoff",
            "0",
            "--allow-linked",
            "false",
        ],
    )?;
    covered.insert("POST /v1/memory/forget");
    assert_eq!(forget_body["forgotten_chunk_ids"], json!(["forget-old"]));
    assert_eq!(
        forget_body["protected_chunk_ids"],
        json!(["forget-protected"])
    );

    let metrics = run_stdout(&api_url, &["metrics"])?;
    covered.insert("GET /metrics");
    assert!(metrics.contains("nestor_memory_retrieval_hits_total 2"));
    assert!(metrics.contains("nestor_memory_candidates_examined 1"));

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

fn run_json(api_url: &str, args: &[&str]) -> TestResult<Value> {
    let stdout = run_stdout(api_url, args)?;
    Ok(serde_json::from_str(&stdout)?)
}

fn run_stdout(api_url: &str, args: &[&str]) -> TestResult<String> {
    let output = run(api_url, args)?;
    if !output.status.success() {
        return Err(format!(
            "command failed: status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn run_ok(api_url: &str, args: &[&str]) -> TestResult<()> {
    let _stdout = run_stdout(api_url, args)?;
    Ok(())
}

fn run(api_url: &str, args: &[&str]) -> TestResult<Output> {
    let output = Command::new(env!("CARGO_BIN_EXE_nestor"))
        .arg("--api-url")
        .arg(api_url)
        .args(args)
        .output()?;
    Ok(output)
}

async fn start_api() -> TestResult<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    tokio::spawn(async move {
        let result = axum::serve(listener, nestor_api::app()).await;
        if let Err(error) = result {
            eprintln!("test API server failed: {error}");
        }
    });
    Ok(format!("http://{address}"))
}

fn write_rules_file() -> TestResult<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "nestor-cli-rules-{}-{}.json",
        std::process::id(),
        monotonic_id()
    ));
    fs::write(
        &path,
        serde_json::to_vec(&json!([
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
        ]))?,
    )?;
    Ok(path)
}

fn monotonic_id() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn path_str(path: &Path) -> TestResult<&str> {
    path.to_str()
        .ok_or_else(|| "temporary rules path was not UTF-8".into())
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
