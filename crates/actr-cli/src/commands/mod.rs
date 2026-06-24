pub mod associate;
pub mod buffer;
pub mod chunk;
pub mod guide;
pub mod ops;
pub mod practice;
pub mod retrieve;
pub mod rule;
pub mod serve;

use actr_client::ActrClient;
use clap::ArgMatches;

use crate::{errors::CliError, output::GlobalOptions};

pub async fn dispatch(
    client: &ActrClient,
    options: &GlobalOptions,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    match matches.subcommand() {
        Some(("guide", matches)) => guide::run(matches),
        Some(("serve", matches)) => serve::run(matches).await,
        Some(("manifest", _)) => ops::manifest(client, options),
        Some(("doctor", matches)) => ops::doctor(client, options, matches).await,
        Some(("health", _)) => ops::health(client, options).await,
        Some(("ready", _)) => ops::ready(client, options).await,
        Some(("metrics", matches)) => ops::metrics(client, options, matches).await,
        Some(("chunk", matches)) => chunk::run(client, options, matches).await,
        Some(("retrieve", matches)) => retrieve::run(client, options, matches).await,
        Some(("practice", matches)) => practice::run(client, options, matches).await,
        Some(("associate", matches)) => associate::run(client, options, matches).await,
        Some(("buffer", matches)) => buffer::run(client, options, matches).await,
        Some(("rule", matches)) => rule::run(client, options, matches).await,
        _ => Err(CliError::usage(
            "missing command",
            "Use: actr-memory guide commands\nExplore: actr-memory --help",
        )),
    }
}
