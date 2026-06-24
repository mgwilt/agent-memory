use clap::ArgMatches;
use nestor_api::BufferSetRequest;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_buffer},
    values::{parse_json_file, required_string},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    match matches.subcommand() {
        Some(("set", matches)) => set(client, options, matches).await,
        _ => Err(CliError::usage(
            "buffer: missing subcommand",
            "Use: nestor buffer set <buffer-name> <chunk-id>",
        )),
    }
}

async fn set(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let buffer_name = required_string(matches, "buffer-name")?;
    let request = if let Some(request) = parse_json_file::<BufferSetRequest>(matches)? {
        request
    } else {
        BufferSetRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("buffer set"))?,
            chunk_id: required_string(matches, "chunk-id")?,
            set_at_ms: matches.get_one::<u64>("at-ms").copied().unwrap_or(1_000),
        }
    };
    if options.verbose {
        eprintln!(
            "PUT {}/v1/memory/buffers/{}",
            client.config().api_url,
            buffer_name
        );
    }
    let response = client
        .set_buffer(&buffer_name, &request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_buffer(&response))
}
