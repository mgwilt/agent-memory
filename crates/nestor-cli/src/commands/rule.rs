use clap::ArgMatches;
use nestor_api::{ProductionRuleDto, RuleEvaluateRequest};
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error, missing_agent_error,
    output::{GlobalOptions, print_json_or_text, render_rule},
    values::{parse_json_file, parse_rules_file, repeated_strings},
};

pub async fn run(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    match matches.subcommand() {
        Some(("eval", matches)) => eval(client, options, matches).await,
        _ => Err(CliError::usage(
            "rule: missing subcommand",
            "Use: nestor rule eval --help",
        )),
    }
}

async fn eval(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    let request = if let Some(request) = parse_json_file::<RuleEvaluateRequest>(matches)? {
        request
    } else {
        RuleEvaluateRequest {
            agent_id: options
                .agent_id
                .clone()
                .ok_or_else(|| missing_agent_error("rule eval"))?,
            candidate_rule_ids: repeated_strings(matches, "candidate-rule"),
            rules: parse_rules_file::<Vec<ProductionRuleDto>>(matches)?.unwrap_or_default(),
            retrieved_chunk_id: matches.get_one::<String>("retrieved").cloned(),
        }
    };
    if options.verbose {
        eprintln!("POST {}/v1/rules/evaluate", client.config().api_url);
    }
    let response = client
        .evaluate_rules(&request)
        .await
        .map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || render_rule(&response))
}
