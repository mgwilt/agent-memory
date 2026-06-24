use clap::ArgMatches;
use nestor_api::RehearseRequest;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_practice},
    values::{parse_json_file, required_string},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<RehearseRequest>(matches)? {
        request
    } else {
        RehearseRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("rehearse"))?,
            chunk_id: required_string(matches, "chunk-id")?,
            weight: matches.get_one::<f64>("weight").copied().unwrap_or(1.0),
            occurred_at_ms: matches.get_one::<u64>("at-ms").copied().unwrap_or(1_000),
            event_id: matches.get_one::<String>("event-id").cloned(),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/memory/rehearse", client.config().api_url);
    }
    let response = client
        .rehearse_memory(&request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_practice(&response))
}
