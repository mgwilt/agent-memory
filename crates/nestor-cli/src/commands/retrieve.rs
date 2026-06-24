use clap::ArgMatches;
use nestor_api::RetrieveRequest;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_retrieve},
    values::{parse_bool_option, parse_cues, parse_json_file, repeated_strings},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<RetrieveRequest>(matches)? {
        request
    } else {
        let mut request = RetrieveRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("retrieve"))?,
            ..RetrieveRequest::default()
        };
        request.chunk_type = matches.get_one::<String>("type").cloned();
        request.cue_slots = parse_cues(matches)?;
        request.context_chunk_ids = repeated_strings(matches, "context");
        if let Some(value) = matches.get_one::<usize>("candidate-limit") {
            request.candidate_limit = *value;
        }
        if let Some(value) = matches.get_one::<usize>("result-limit") {
            request.result_limit = *value;
        }
        if let Some(value) = matches.get_one::<f64>("threshold") {
            request.activation_threshold = *value;
        }
        if let Some(value) = matches.get_one::<f64>("noise-s") {
            request.noise_s = *value;
        }
        if let Some(value) = parse_bool_option(matches, "partial-matching")? {
            request.partial_matching = value;
        }
        if let Some(value) = parse_bool_option(matches, "diagnostics")? {
            request.return_diagnostics = value;
        }
        if let Some(value) = matches.get_one::<u64>("seed") {
            request.deterministic_seed = Some(*value);
        }
        if let Some(value) = parse_bool_option(matches, "commit")? {
            request.commit_on_hit = value;
        }
        if let Some(value) = matches.get_one::<u64>("now-ms") {
            request.now_ms = *value;
        }
        request
    };
    let endpoint = matches
        .get_one::<String>("endpoint")
        .map(String::as_str)
        .unwrap_or("normal");
    if options.verbose {
        let path = if endpoint == "stream" {
            "/v1/memory/retrieve/stream"
        } else {
            "/v1/memory/retrieve"
        };
        eprintln!("POST {}{}", client.config().api_url, path);
    }
    let response = if endpoint == "stream" {
        client
            .retrieve_memory_stream_endpoint(&request)
            .await
            .map_err(map_client_error)?
    } else {
        client
            .retrieve_memory(&request)
            .await
            .map_err(map_client_error)?
    };
    print_json_or_text(options.format, &response, || render_retrieve(&response))
}
