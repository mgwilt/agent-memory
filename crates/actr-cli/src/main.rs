mod cli;
mod commands;
mod docs;
mod errors;
mod examples;
mod output;
mod values;

use std::{process, time::Instant};

use actr_client::{ActrClient, ApiClientConfig, ErrorCategory};
use clap::ArgMatches;

use crate::{
    cli::build_cli,
    errors::CliError,
    output::{GlobalOptions, OutputFormat},
};

#[tokio::main]
async fn main() {
    let started = Instant::now();
    let matches = build_cli().get_matches();
    let options = match global_options(&matches) {
        Ok(options) => options,
        Err(error) => {
            error.print(false, started.elapsed());
            process::exit(error.exit_code().0);
        }
    };
    let config = ApiClientConfig::new(options.api_url.clone(), options.timeout);
    let client = ActrClient::new(config);
    let result = commands::dispatch(&client, &options, &matches).await;
    match result {
        Ok(()) => {
            if options.agent_footer {
                println!("[exit:0 | {}]", output::format_duration(started.elapsed()));
            }
        }
        Err(error) => {
            error.print(options.agent_footer, started.elapsed());
            process::exit(error.exit_code().0);
        }
    }
}

fn global_options(matches: &ArgMatches) -> Result<GlobalOptions, CliError> {
    let api_url = matches
        .get_one::<String>("api-url")
        .cloned()
        .or_else(|| std::env::var("ACTR_API_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
    let agent_id = matches
        .get_one::<String>("agent")
        .cloned()
        .or_else(|| std::env::var("ACTR_AGENT_ID").ok());
    let timeout_ms = *matches.get_one::<u64>("timeout-ms").unwrap_or(&5_000);
    let format = matches
        .get_one::<String>("format")
        .map(|value| OutputFormat::parse(value))
        .transpose()
        .map_err(|detail| CliError::usage(detail, "Use: actr-memory --help"))?
        .unwrap_or(OutputFormat::Text);
    Ok(GlobalOptions {
        api_url,
        agent_id,
        format,
        timeout: std::time::Duration::from_millis(timeout_ms),
        agent_footer: matches.get_flag("agent-footer"),
        verbose: matches.get_flag("verbose"),
    })
}

pub fn missing_agent_error(command: &str) -> CliError {
    CliError::usage(
        format!("{command}: missing agent id"),
        format!("Use: actr-memory --agent <agent-id> {command}"),
    )
}

pub fn map_client_error(error: actr_client::ClientError) -> CliError {
    let hint = match error.category() {
        ErrorCategory::Unavailable => {
            "Start API: actr-memory serve\nCheck config: actr-memory doctor".to_string()
        }
        ErrorCategory::BadRequest | ErrorCategory::NotFound | ErrorCategory::Conflict => {
            "Explore: actr-memory guide errors".to_string()
        }
        ErrorCategory::Usage | ErrorCategory::Internal => {
            "Explore: actr-memory guide commands".to_string()
        }
    };
    CliError::client(error, hint)
}
