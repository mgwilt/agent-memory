use clap::ArgMatches;
use nestor_api::ConsolidateRequest;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_consolidate},
    values::{parse_json_file, repeated_strings},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<ConsolidateRequest>(matches)? {
        request
    } else {
        ConsolidateRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("consolidate"))?,
            chunk_type: matches.get_one::<String>("type").cloned(),
            summary_chunk_type: matches
                .get_one::<String>("summary-type")
                .cloned()
                .unwrap_or_else(|| "semantic".to_string()),
            group_slot_keys: repeated_strings(matches, "group-slot"),
            min_group_size: matches
                .get_one::<usize>("min-group-size")
                .copied()
                .unwrap_or(2),
            now_ms: matches.get_one::<u64>("now-ms").copied().unwrap_or(1_000),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/memory/consolidate", client.config().api_url);
    }
    let response = client
        .consolidate_memory(&request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_consolidate(&response))
}
