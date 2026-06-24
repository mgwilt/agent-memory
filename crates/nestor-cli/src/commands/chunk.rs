use clap::ArgMatches;
use nestor_api::{ChunkPatchRequest, ChunkUpsertRequest};
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_chunk, render_delete},
    values::{parse_json_file, parse_slots, required_string},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    match matches.subcommand() {
        Some(("put", matches)) => put(client, options, matches).await,
        Some(("get", matches)) => get(client, options, matches).await,
        Some(("patch", matches)) => patch(client, options, matches).await,
        Some(("delete", matches)) => delete(client, options, matches).await,
        _ => Err(CliError::usage(
            "chunk: missing subcommand",
            "Use: nestor chunk put|get|patch|delete\nExplore: nestor chunk --help",
        )),
    }
}

async fn put(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<ChunkUpsertRequest>(matches)? {
        request
    } else {
        ChunkUpsertRequest {
            agent_id: agent(options, "chunk put")?,
            chunk_id: required_string(matches, "chunk-id")?,
            chunk_type: required_string(matches, "type")?,
            slots: parse_slots(matches, "slot")?,
            now_ms: matches.get_one::<u64>("now-ms").copied().unwrap_or(1_000),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/memory/chunks", client.config().api_url);
    }
    let response = client.put_chunk(&request).await.map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || {
        render_chunk("chunk upserted", &response)
    })
}

async fn get(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let agent_id = agent(options, "chunk get")?;
    let chunk_id = required_string(matches, "chunk-id")?;
    if options.verbose {
        eprintln!(
            "GET {}/v1/memory/chunks/{}",
            client.config().api_url,
            chunk_id
        );
    }
    let response = client
        .get_chunk(&agent_id, &chunk_id)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || {
        render_chunk("chunk", &response)
    })
}

async fn patch(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let chunk_id = required_string(matches, "chunk-id")?;
    let request = if let Some(request) = parse_json_file::<ChunkPatchRequest>(matches)? {
        request
    } else {
        ChunkPatchRequest {
            agent_id: agent(options, "chunk patch")?,
            expected_version: matches
                .get_one::<u64>("expected-version")
                .copied()
                .ok_or_else(|| {
                    CliError::usage(
                        "chunk patch: missing expected version",
                        "Use: --expected-version <N>",
                    )
                })?,
            slots: parse_slots(matches, "slot")?,
        }
    };
    if options.verbose {
        eprintln!(
            "PATCH {}/v1/memory/chunks/{}",
            client.config().api_url,
            chunk_id
        );
    }
    let response = client
        .patch_chunk(&chunk_id, &request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || {
        render_chunk("chunk patched", &response)
    })
}

async fn delete(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    if !matches.get_flag("yes") {
        return Err(CliError::usage(
            "chunk delete: --yes is required",
            "Use: nestor --agent <agent-id> chunk delete <chunk-id> --yes",
        ));
    }
    let agent_id = agent(options, "chunk delete")?;
    let chunk_id = required_string(matches, "chunk-id")?;
    if options.verbose {
        eprintln!(
            "DELETE {}/v1/memory/chunks/{}",
            client.config().api_url,
            chunk_id
        );
    }
    let response = client
        .delete_chunk(&agent_id, &chunk_id)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_delete(&response))
}

fn agent(options: &GlobalOptions, command: &str) -> Result<String, CliError> {
    options
        .agent_id
        .clone()
        .ok_or_else(|| missing_agent_error(command))
}
