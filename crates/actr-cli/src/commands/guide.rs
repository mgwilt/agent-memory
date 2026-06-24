use clap::ArgMatches;

use crate::{
    docs::DOC_INDEX,
    errors::CliError,
    examples::{ERRORS, SLOT_GUIDE, WORKFLOW},
};

pub fn run(matches: &ArgMatches) -> Result<(), CliError> {
    let topic = matches
        .get_one::<String>("topic")
        .map(String::as_str)
        .unwrap_or("commands");
    match topic {
        "commands" => println!("{}", commands()),
        "slots" => println!("{SLOT_GUIDE}"),
        "workflow" => println!("{WORKFLOW}"),
        "errors" => println!("{ERRORS}"),
        "docs" => println!("{DOC_INDEX}"),
        _ => {
            println!("{}", commands());
        }
    }
    Ok(())
}

fn commands() -> &'static str {
    "Available commands:
  guide      Agent-oriented command map and examples
  serve      Start the ACT-R API server
  manifest   Print route manifest
  doctor     Check API connectivity
  health     GET /healthz
  ready      GET /readyz
  metrics    GET /metrics
  chunk      Create, inspect, patch, delete chunks
  retrieve   Retrieve memory with activation diagnostics
  practice   Record practice events
  associate  Upsert spreading-activation associations
  buffer     Set ACT-R buffers
  rule       Evaluate production rules

Drill down:
  actr-memory chunk --help
  actr-memory retrieve --help
  actr-memory guide workflow
  actr-memory guide docs"
}
