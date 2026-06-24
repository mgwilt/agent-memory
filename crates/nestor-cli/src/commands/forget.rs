use clap::ArgMatches;
use nestor_api::ForgetRequest;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_forget},
    values::{parse_bool_option, parse_json_file},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<ForgetRequest>(matches)? {
        request
    } else {
        ForgetRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("forget"))?,
            chunk_type: matches.get_one::<String>("type").cloned(),
            now_ms: matches.get_one::<u64>("now-ms").copied().unwrap_or(1_000),
            recency_cutoff_ms: matches
                .get_one::<u64>("recency-cutoff-ms")
                .copied()
                .unwrap_or(0),
            base_level_cutoff: matches
                .get_one::<f64>("base-level-cutoff")
                .copied()
                .unwrap_or(-4.0),
            allow_linked_forget: parse_bool_option(matches, "allow-linked")?.unwrap_or(false),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/memory/forget", client.config().api_url);
    }
    let response = client
        .forget_memory(&request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_forget(&response))
}
