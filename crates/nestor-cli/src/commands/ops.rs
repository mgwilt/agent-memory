use clap::ArgMatches;
use nestor_client::NestorClient;

use crate::{
    errors::CliError,
    map_client_error,
    output::{
        GlobalOptions, OutputFormat, json_manifest, print_json_or_text, render_doctor,
        render_health, render_manifest,
    },
};

pub fn manifest(client: &NestorClient, options: &GlobalOptions) -> Result<(), CliError> {
    let routes = client.manifest();
    if options.format == OutputFormat::Text {
        println!("{}", render_manifest(&routes));
        Ok(())
    } else {
        print_json_or_text(options.format, &json_manifest(&routes), || {
            render_manifest(&routes)
        })
    }
}

pub async fn doctor(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    if options.verbose {
        eprintln!("GET {}/healthz", client.config().api_url);
    }
    let health = client.health().await.map_err(map_client_error)?;
    if options.verbose {
        eprintln!("GET {}/readyz", client.config().api_url);
    }
    let ready = client.ready().await.map_err(map_client_error)?;
    if matches.get_flag("require-ready-pass") && ready.status != "pass" {
        return Err(CliError::usage(
            format!("ready status is {}", ready.status),
            "Use without --require-ready-pass to allow warn readiness",
        ));
    }
    if options.verbose {
        eprintln!("GET {}/metrics", client.config().api_url);
    }
    let metrics = client.metrics().await.map_err(map_client_error)?;
    let value = serde_json::json!({
        "health": health,
        "ready": ready,
        "metrics_lines": metrics.lines().filter(|line| !line.is_empty()).count(),
    });
    print_json_or_text(options.format, &value, || {
        render_doctor(&health, &ready, &metrics)
    })
}

pub async fn health(client: &NestorClient, options: &GlobalOptions) -> Result<(), CliError> {
    if options.verbose {
        eprintln!("GET {}/healthz", client.config().api_url);
    }
    let response = client.health().await.map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || {
        render_health("health", &response)
    })
}

pub async fn ready(client: &NestorClient, options: &GlobalOptions) -> Result<(), CliError> {
    if options.verbose {
        eprintln!("GET {}/readyz", client.config().api_url);
    }
    let response = client.ready().await.map_err(map_client_error)?;
    print_json_or_text(options.format, &response, || {
        render_health("ready", &response)
    })
}

pub async fn metrics(
    client: &NestorClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    if options.verbose {
        eprintln!("GET {}/metrics", client.config().api_url);
    }
    let mut response = client.metrics().await.map_err(map_client_error)?;
    if let Some(pattern) = matches.get_one::<String>("grep") {
        response = response
            .lines()
            .filter(|line| line.contains(pattern))
            .collect::<Vec<_>>()
            .join("\n");
    }
    println!("{response}");
    Ok(())
}
