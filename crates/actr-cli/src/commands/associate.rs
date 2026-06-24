use actr_api::AssociateRequest;
use actr_client::ActrClient;
use clap::ArgMatches;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_association},
    values::{parse_json_file, required_string},
};

pub async fn run(
    client: &ActrClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<AssociateRequest>(matches)? {
        request
    } else {
        AssociateRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("associate"))?,
            src_chunk_id: required_string(matches, "src-chunk-id")?,
            dst_chunk_id: required_string(matches, "dst-chunk-id")?,
            source: required_string(matches, "source")?,
            strength: matches.get_one::<f64>("strength").copied().ok_or_else(|| {
                CliError::usage("associate: missing strength", "Use: --strength <FLOAT>")
            })?,
            fan: matches.get_one::<u64>("fan").copied().unwrap_or(1),
            updated_at_ms: matches.get_one::<u64>("at-ms").copied().unwrap_or(1_000),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/memory/associate", client.config().api_url);
    }
    let response = client
        .upsert_association(&request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_association(&response))
}
