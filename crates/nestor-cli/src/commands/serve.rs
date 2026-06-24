use clap::ArgMatches;
use nestor_ops::{RuntimeConfig, RuntimeProfile};

use crate::{errors::CliError, values::optional_string};

pub async fn run(matches: &ArgMatches) -> Result<(), CliError> {
    let mut config = if let Some(profile) = optional_string(matches, "profile") {
        RuntimeConfig::for_profile(
            RuntimeProfile::parse(&profile).map_err(|detail| {
                CliError::usage(detail, "Use: nestor serve --profile development")
            })?,
        )
    } else {
        RuntimeConfig::from_env()
            .map_err(|detail| CliError::usage(detail, "Use: nestor serve --profile development"))?
    };
    if let Some(bind) = optional_string(matches, "bind") {
        config.bind_addr = bind;
    }
    println!("serving Nestor API on {}", config.bind_addr);
    nestor_api::serve(config)
        .await
        .map_err(|err| CliError::internal(err.to_string(), "Check: nestor doctor"))
}
