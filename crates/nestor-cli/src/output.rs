use std::time::Duration;

use nestor_api::{
    AssociateResponse, BufferResponse, ChunkResponse, ConsolidateResponse, DeleteResponse,
    ForgetResponse, HealthResponse, PracticeResponse, RetrieveResponse, RuleEvaluateResponse,
};
use serde_json::json;

use crate::errors::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    PrettyJson,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "pretty-json" => Ok(Self::PrettyJson),
            _ => Err("format must be text, json, or pretty-json".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub api_url: String,
    pub agent_id: Option<String>,
    pub format: OutputFormat,
    pub timeout: Duration,
    pub agent_footer: bool,
    pub verbose: bool,
}

pub fn print_json_or_text<T>(
    format: OutputFormat,
    value: &T,
    render_text: impl FnOnce() -> String,
) -> Result<(), CliError>
where
    T: serde::Serialize,
{
    match format {
        OutputFormat::Text => {
            println!("{}", render_text());
            Ok(())
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(value)
                    .map_err(|err| CliError::internal(err.to_string(), "Try --format text"))?
            );
            Ok(())
        }
        OutputFormat::PrettyJson => {
            println!(
                "{}",
                serde_json::to_string_pretty(value)
                    .map_err(|err| CliError::internal(err.to_string(), "Try --format text"))?
            );
            Ok(())
        }
    }
}

pub fn render_chunk(prefix: &str, chunk: &ChunkResponse) -> String {
    format!(
        "{prefix} {}\nagent: {}\ntype: {}\nslots: {}\ncreated_at_ms: {}\nupdated_at_ms: {}\nretrieval_count: {}\nbase_bias: {}",
        chunk.chunk_id,
        chunk.agent_id,
        chunk.chunk_type,
        render_slots(&chunk.slots),
        chunk.created_at_ms,
        chunk.updated_at_ms,
        chunk.retrieval_count,
        chunk.base_bias
    )
}

pub fn render_delete(response: &DeleteResponse) -> String {
    format!(
        "chunk {} deleted\nagent: {}\ndeleted: {}",
        response.chunk_id, response.agent_id, response.deleted
    )
}

pub fn render_practice(response: &PracticeResponse) -> String {
    format!(
        "practice recorded\nevent_id: {}\nagent: {}\nchunk: {}\nkind: {}\nweight: {}",
        response.event_id, response.agent_id, response.chunk_id, response.kind, response.weight
    )
}

pub fn render_consolidate(response: &ConsolidateResponse) -> String {
    let mut lines = vec![format!(
        "consolidate: {} groups\nagent: {}\nconsidered: {}",
        response.groups_consolidated, response.agent_id, response.groups_considered
    )];
    for summary in &response.summaries {
        lines.push(format!(
            "- {} from {}",
            summary.summary_chunk_id,
            summary.source_chunk_ids.join(", ")
        ));
    }
    lines.join("\n")
}

pub fn render_forget(response: &ForgetResponse) -> String {
    format!(
        "forget: examined {}\nagent: {}\nforgotten: {}\narchived: {}\nprotected: {}",
        response.examined,
        response.agent_id,
        render_list(&response.forgotten_chunk_ids),
        render_list(&response.archived_chunk_ids),
        render_list(&response.protected_chunk_ids)
    )
}

pub fn render_association(response: &AssociateResponse) -> String {
    format!(
        "association upserted\nagent: {}\nsource: {}\nfrom: {}\nto: {}\nstrength: {}",
        response.agent_id,
        response.source,
        response.src_chunk_id,
        response.dst_chunk_id,
        response.strength
    )
}

pub fn render_buffer(response: &BufferResponse) -> String {
    format!(
        "buffer {} set\nagent: {}\nchunk: {}\ntype: {}\nupdated_at_ms: {}",
        response.buffer_name,
        response.agent_id,
        response.chunk_id.as_deref().unwrap_or("<empty>"),
        response.chunk_type.as_deref().unwrap_or("<unknown>"),
        response.updated_at_ms
    )
}

pub fn render_retrieve(response: &RetrieveResponse) -> String {
    let mut lines = vec![format!(
        "retrieve: {}{}",
        serde_json::to_value(response.status)
            .ok()
            .and_then(|value| value.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "unknown".to_string()),
        response
            .miss_reason
            .map(|reason| {
                let label = serde_json::to_value(reason)
                    .ok()
                    .and_then(|value| value.as_str().map(ToString::to_string))
                    .unwrap_or_else(|| "unknown".to_string());
                format!(" reason={label}")
            })
            .unwrap_or_default()
    )];
    for (index, result) in response.results.iter().enumerate() {
        lines.push(format!(
            "{}. {} type={} activation={} probability={} latency_ms={} threshold={}",
            index + 1,
            result.chunk_id,
            result.chunk_type,
            result.activation,
            result.retrieval_probability,
            result.predicted_latency_ms,
            if result.passes_threshold {
                "pass"
            } else {
                "fail"
            }
        ));
        lines.push(format!(
            "   components: base={} spreading={} partial={} noise={}",
            result.components.base_level,
            result.components.spreading,
            result.components.partial_match,
            result.components.noise
        ));
    }
    if let Some(diagnostics) = &response.diagnostics {
        lines.push(format!(
            "diagnostics: candidates={}/{} context={} threshold={} seed={} compute_ms={}",
            diagnostics.candidates_examined,
            diagnostics.candidate_limit,
            diagnostics.context_chunk_count,
            diagnostics.threshold,
            diagnostics
                .deterministic_seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            diagnostics.activation_compute_ms
        ));
    }
    if response.results.is_empty() {
        lines.push(
            "Next: lower --threshold, add --cue values, or inspect candidate chunk_type"
                .to_string(),
        );
    }
    lines.join("\n")
}

pub fn render_rule(response: &RuleEvaluateResponse) -> String {
    if let Some(selected) = &response.selected {
        format!(
            "rule: selected {}\nname: {}\nutility: {}\nspecificity: {}\nrank: {}\nmatches: {}\ncandidates: {}",
            selected.rule_id,
            selected.name,
            selected.utility,
            selected.specificity,
            selected.rank,
            response.matches.len(),
            response.candidates.len()
        )
    } else {
        let mut lines = vec!["rule: no match".to_string(), "candidates:".to_string()];
        for candidate in &response.candidates {
            lines.push(format!(
                "- {} rejected: {}",
                candidate.rule_id,
                candidate.rejection_reason.as_deref().unwrap_or("unknown")
            ));
        }
        lines.push("Next: inspect buffers or pass --retrieved <chunk-id>".to_string());
        lines.join("\n")
    }
}

pub fn render_health(label: &str, response: &HealthResponse) -> String {
    let mut lines = vec![format!("{label}: {}", response.status)];
    for check in &response.checks {
        lines.push(format!(
            "- {}: {} - {}",
            check.name, check.status, check.detail
        ));
    }
    lines.join("\n")
}

pub fn render_manifest(routes: &[nestor_client::HttpRoute]) -> String {
    routes
        .iter()
        .map(|route| format!("{} {} - {}", route.method, route.path, route.purpose))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_doctor(health: &HealthResponse, ready: &HealthResponse, metrics: &str) -> String {
    let metrics_lines = metrics.lines().filter(|line| !line.is_empty()).count();
    format!(
        "doctor: api reachable\nhealth: {}\nready: {}\nmetrics_lines: {}",
        health.status, ready.status, metrics_lines
    )
}

pub fn json_manifest(routes: &[nestor_client::HttpRoute]) -> serde_json::Value {
    json!(
        routes
            .iter()
            .map(|route| json!({
                "method": route.method,
                "path": route.path,
                "purpose": route.purpose,
            }))
            .collect::<Vec<_>>()
    )
}

pub fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn render_slots(slots: &std::collections::BTreeMap<String, nestor_api::SlotValueDto>) -> String {
    if slots.is_empty() {
        return "<none>".to_string();
    }
    slots
        .iter()
        .map(|(key, value)| format!("{key}={}", render_slot_value(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_slot_value(value: &nestor_api::SlotValueDto) -> String {
    match value {
        nestor_api::SlotValueDto::Symbol(value) => format!("symbol:{value}"),
        nestor_api::SlotValueDto::Text(value) => format!("text:{value}"),
        nestor_api::SlotValueDto::Number(value) => format!("number:{value}"),
        nestor_api::SlotValueDto::Bool(value) => format!("bool:{value}"),
    }
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_string()
    } else {
        values.join(", ")
    }
}
