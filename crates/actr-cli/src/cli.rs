use clap::{Arg, ArgAction, Command, value_parser};

use crate::examples::{ROOT_AFTER_HELP, SLOT_GUIDE};

pub fn build_cli() -> Command {
    Command::new("actr-memory")
        .about("Agent-friendly CLI for the ACT-R memory API.")
        .version(env!("CARGO_PKG_VERSION"))
        .after_help(ROOT_AFTER_HELP)
        .arg(
            Arg::new("api-url")
                .long("api-url")
                .value_name("URL")
                .help("API base URL [env: ACTR_API_URL]")
                .global(true),
        )
        .arg(
            Arg::new("agent")
                .long("agent")
                .value_name("AGENT_ID")
                .help("Default agent id [env: ACTR_AGENT_ID]")
                .global(true),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .default_value("text")
                .value_parser(["text", "json", "pretty-json"])
                .help("Output format: text, json, pretty-json")
                .global(true),
        )
        .arg(
            Arg::new("timeout-ms")
                .long("timeout-ms")
                .value_name("MS")
                .default_value("5000")
                .value_parser(value_parser!(u64))
                .help("HTTP timeout in milliseconds")
                .global(true),
        )
        .arg(
            Arg::new("agent-footer")
                .long("agent-footer")
                .action(ArgAction::SetTrue)
                .help("Append [exit:N | duration] footer for LLM run tools")
                .global(true),
        )
        .arg(
            Arg::new("no-color")
                .long("no-color")
                .action(ArgAction::SetTrue)
                .help("Disable ANSI color")
                .global(true),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("Print request/response metadata to stderr")
                .global(true),
        )
        .subcommand(guide_command())
        .subcommand(serve_command())
        .subcommand(simple_command("manifest", "Print route manifest"))
        .subcommand(
            simple_command("doctor", "Check API connectivity").arg(
                Arg::new("require-ready-pass")
                    .long("require-ready-pass")
                    .action(ArgAction::SetTrue)
                    .help("Fail unless readiness status is pass"),
            ),
        )
        .subcommand(simple_command("health", "GET /healthz"))
        .subcommand(simple_command("ready", "GET /readyz"))
        .subcommand(
            simple_command("metrics", "GET /metrics").arg(
                Arg::new("grep")
                    .long("grep")
                    .value_name("TEXT")
                    .help("Print only metric lines containing text"),
            ),
        )
        .subcommand(chunk_command())
        .subcommand(retrieve_command())
        .subcommand(practice_command())
        .subcommand(associate_command())
        .subcommand(buffer_command())
        .subcommand(rule_command())
}

fn simple_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .after_help("Examples:\n  actr-memory guide commands\n\nDocs:\n  docs/cli/commands.md")
}

fn guide_command() -> Command {
    Command::new("guide")
        .about("Agent-oriented command map and examples")
        .arg(
            Arg::new("topic")
                .value_parser(["commands", "slots", "workflow", "errors", "docs"])
                .help("Guide topic"),
        )
        .after_help("Examples:\n  actr-memory guide\n  actr-memory guide slots\n  actr-memory guide workflow\n\nDocs:\n  docs/cli/progressive-disclosure.md")
}

fn serve_command() -> Command {
    Command::new("serve")
        .about("Start the ACT-R API server")
        .arg(Arg::new("bind").long("bind").value_name("ADDR").help("Bind address"))
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_name("NAME")
                .value_parser(["development", "staging", "production"])
                .help("Runtime profile"),
        )
        .after_help("Examples:\n  actr-memory serve\n  actr-memory serve --bind 127.0.0.1:8090\n\nEndpoint: local API server\nDocs:\n  docs/cli/commands.md#serve")
}

fn chunk_command() -> Command {
    Command::new("chunk")
        .about("Create, inspect, patch, delete chunks")
        .subcommand(
            Command::new("put")
                .about("Create or upsert a chunk")
                .arg(Arg::new("chunk-id").required(true))
                .arg(Arg::new("type").long("type").required(true).value_name("TYPE"))
                .arg(slot_arg("slot", "Slot value; repeatable"))
                .arg(Arg::new("now-ms").long("now-ms").value_parser(value_parser!(u64)))
                .arg(json_file_arg())
                .after_help(format!("Endpoint: POST /v1/memory/chunks\n\nExamples:\n  actr-memory --agent agent-1 chunk put mem-preference --type fact --slot topic=symbol:preference\n\n{SLOT_GUIDE}\nDocs:\n  docs/cli/commands.md#chunk-put")),
        )
        .subcommand(
            Command::new("get")
                .about("Fetch one active chunk")
                .arg(Arg::new("chunk-id").required(true))
                .after_help("Endpoint: GET /v1/memory/chunks/{chunk_id}\n\nExamples:\n  actr-memory --agent agent-1 chunk get mem-preference\n\nDocs:\n  docs/cli/commands.md#chunk-get"),
        )
        .subcommand(
            Command::new("patch")
                .about("Replace chunk slots using optimistic versioning")
                .arg(Arg::new("chunk-id").required(true))
                .arg(
                    Arg::new("expected-version")
                        .long("expected-version")
                        .required(true)
                        .value_parser(value_parser!(u64)),
                )
                .arg(slot_arg("slot", "Replacement slot value; repeatable"))
                .arg(json_file_arg())
                .after_help(format!("Endpoint: PATCH /v1/memory/chunks/{{chunk_id}}\n\nExamples:\n  actr-memory --agent agent-1 chunk patch mem-preference --expected-version 1 --slot verified=bool:true\n\n{SLOT_GUIDE}\nDocs:\n  docs/cli/commands.md#chunk-patch")),
        )
        .subcommand(
            Command::new("delete")
                .about("Soft-delete a chunk")
                .arg(Arg::new("chunk-id").required(true))
                .arg(Arg::new("yes").long("yes").action(ArgAction::SetTrue).help("Confirm deletion"))
                .after_help("Endpoint: DELETE /v1/memory/chunks/{chunk_id}\n\nExamples:\n  actr-memory --agent agent-1 chunk delete old-fact --yes\n\nDocs:\n  docs/cli/commands.md#chunk-delete"),
        )
        .after_help("Subcommands:\n  put, get, patch, delete\n\nNext:\n  actr-memory chunk put --help\n  actr-memory guide workflow\nDocs:\n  docs/cli/commands.md#chunk")
}

fn retrieve_command() -> Command {
    Command::new("retrieve")
        .about("Retrieve memory with activation diagnostics")
        .arg(Arg::new("type").long("type").value_name("TYPE"))
        .arg(slot_arg("cue", "Retrieval cue slot; repeatable"))
        .arg(Arg::new("context").long("context").action(ArgAction::Append).value_name("CHUNK_ID"))
        .arg(Arg::new("candidate-limit").long("candidate-limit").value_parser(value_parser!(usize)))
        .arg(Arg::new("result-limit").long("result-limit").value_parser(value_parser!(usize)))
        .arg(
            Arg::new("threshold")
                .long("threshold")
                .allow_hyphen_values(true)
                .value_parser(value_parser!(f64)),
        )
        .arg(
            Arg::new("noise-s")
                .long("noise-s")
                .allow_hyphen_values(true)
                .value_parser(value_parser!(f64)),
        )
        .arg(Arg::new("partial-matching").long("partial-matching").value_parser(["true", "false"]))
        .arg(Arg::new("diagnostics").long("diagnostics").value_parser(["true", "false"]))
        .arg(Arg::new("seed").long("seed").value_parser(value_parser!(u64)))
        .arg(Arg::new("commit").long("commit").value_parser(["true", "false"]))
        .arg(Arg::new("now-ms").long("now-ms").value_parser(value_parser!(u64)))
        .arg(Arg::new("endpoint").long("endpoint").default_value("normal").value_parser(["normal", "stream"]).help("Endpoint variant; stream currently returns the same JSON shape"))
        .arg(json_file_arg())
        .after_help(format!("Endpoint: POST /v1/memory/retrieve or /v1/memory/retrieve/stream\n\nExamples:\n  actr-memory --agent agent-1 retrieve --type fact --cue topic=symbol:preference --context ctx-goal --threshold -10\n\n{SLOT_GUIDE}\nDocs:\n  docs/cli/commands.md#retrieve"))
}

fn practice_command() -> Command {
    Command::new("practice")
        .about("Record practice events")
        .arg(Arg::new("chunk-id").required(true))
        .arg(Arg::new("kind").long("kind").required(true).value_name("KIND"))
        .arg(Arg::new("weight").long("weight").value_parser(value_parser!(f64)))
        .arg(Arg::new("at-ms").long("at-ms").value_parser(value_parser!(u64)))
        .arg(Arg::new("event-id").long("event-id").value_name("ID"))
        .arg(json_file_arg())
        .after_help("Endpoint: POST /v1/memory/practice\n\nExamples:\n  actr-memory --agent agent-1 practice mem-preference --kind retrieve --weight 2\n\nDocs:\n  docs/cli/commands.md#practice")
}

fn associate_command() -> Command {
    Command::new("associate")
        .about("Upsert spreading-activation associations")
        .arg(Arg::new("src-chunk-id").required(true))
        .arg(Arg::new("dst-chunk-id").required(true))
        .arg(Arg::new("source").long("source").required(true).value_name("SOURCE"))
        .arg(Arg::new("strength").long("strength").required(true).value_parser(value_parser!(f64)))
        .arg(Arg::new("fan").long("fan").value_parser(value_parser!(u64)))
        .arg(Arg::new("at-ms").long("at-ms").value_parser(value_parser!(u64)))
        .arg(json_file_arg())
        .after_help("Endpoint: POST /v1/memory/associate\n\nExamples:\n  actr-memory --agent agent-1 associate ctx-goal mem-preference --source goal --strength 1.25\n\nDocs:\n  docs/cli/commands.md#associate")
}

fn buffer_command() -> Command {
    Command::new("buffer")
        .about("Set ACT-R buffers")
        .subcommand(
            Command::new("set")
                .about("Set the current chunk for a buffer")
                .arg(Arg::new("buffer-name").required(true))
                .arg(Arg::new("chunk-id").required(true))
                .arg(Arg::new("at-ms").long("at-ms").value_parser(value_parser!(u64)))
                .arg(json_file_arg())
                .after_help("Endpoint: PUT /v1/memory/buffers/{buffer_name}\n\nExamples:\n  actr-memory --agent agent-1 buffer set goal ctx-goal\n\nDocs:\n  docs/cli/commands.md#buffer-set"),
        )
        .after_help("Subcommands:\n  set\n\nNext:\n  actr-memory buffer set --help\nDocs:\n  docs/cli/commands.md#buffer")
}

fn rule_command() -> Command {
    Command::new("rule")
        .about("Evaluate production rules")
        .subcommand(
            Command::new("eval")
                .about("Evaluate candidate production rules")
                .arg(Arg::new("candidate-rule").long("candidate-rule").action(ArgAction::Append).value_name("RULE_ID"))
                .arg(Arg::new("rules-file").long("rules-file").value_name("PATH|-"))
                .arg(Arg::new("retrieved").long("retrieved").value_name("CHUNK_ID"))
                .arg(json_file_arg())
                .after_help("Endpoint: POST /v1/rules/evaluate\n\nExamples:\n  actr-memory --agent agent-1 rule eval --retrieved mem-preference --rules-file rules.json\n\nDocs:\n  docs/cli/commands.md#rule-eval"),
        )
        .after_help("Subcommands:\n  eval\n\nNext:\n  actr-memory rule eval --help\nDocs:\n  docs/cli/commands.md#rule")
}

fn slot_arg(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Append)
        .value_name("KEY=TYPE:VALUE")
        .help(help)
}

fn json_file_arg() -> Arg {
    Arg::new("json-file")
        .long("json-file")
        .value_name("PATH|-")
        .help("Read exact request JSON from file or stdin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_help_lists_expected_commands() {
        let mut command = build_cli();
        let mut output = Vec::new();
        let result = command.write_help(&mut output);
        assert!(result.is_ok());
        let help = String::from_utf8(output).unwrap_or_default();
        for expected in [
            "guide",
            "serve",
            "manifest",
            "doctor",
            "chunk",
            "retrieve",
            "practice",
            "associate",
            "buffer",
            "rule",
        ] {
            assert!(help.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn retrieve_help_contains_endpoint_and_examples() {
        let matches = build_cli().try_get_matches_from(["actr-memory", "retrieve", "--help"]);
        assert!(matches.is_err());
        let error = matches.expect_err("help exits through clap error");
        let rendered = error.to_string();
        assert!(rendered.contains("Examples:"));
        assert!(rendered.contains("POST /v1/memory/retrieve"));
    }
}
