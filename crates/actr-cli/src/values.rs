use std::{collections::BTreeMap, fs, io::Read};

use actr_api::{SlotDto, SlotValueDto};
use clap::ArgMatches;
use serde::de::DeserializeOwned;

use crate::errors::CliError;

pub fn required_string(matches: &ArgMatches, name: &str) -> Result<String, CliError> {
    matches.get_one::<String>(name).cloned().ok_or_else(|| {
        CliError::usage(
            format!("missing required argument {name}"),
            "Explore: actr-memory guide commands",
        )
    })
}

pub fn optional_string(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.get_one::<String>(name).cloned()
}

pub fn repeated_strings(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .get_many::<String>(name)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

pub fn parse_slots(
    matches: &ArgMatches,
    name: &str,
) -> Result<BTreeMap<String, SlotValueDto>, CliError> {
    let mut slots = BTreeMap::new();
    for raw in repeated_strings(matches, name) {
        let (key, value) = parse_slot_assignment(&raw)?;
        slots.insert(key, value);
    }
    Ok(slots)
}

pub fn parse_cues(matches: &ArgMatches) -> Result<Vec<SlotDto>, CliError> {
    repeated_strings(matches, "cue")
        .into_iter()
        .map(|raw| {
            let (key, value) = parse_slot_assignment(&raw)?;
            Ok(SlotDto { key, value })
        })
        .collect()
}

pub fn parse_bool_option(matches: &ArgMatches, name: &str) -> Result<Option<bool>, CliError> {
    matches
        .get_one::<String>(name)
        .map(|value| parse_bool(value, name))
        .transpose()
}

pub fn parse_json_file<T>(matches: &ArgMatches) -> Result<Option<T>, CliError>
where
    T: DeserializeOwned,
{
    match matches.get_one::<String>("json-file") {
        Some(path) => {
            let body = if path == "-" {
                let mut body = String::new();
                std::io::stdin()
                    .read_to_string(&mut body)
                    .map_err(|err| CliError::usage(err.to_string(), "Use: --json-file <path>"))?;
                body
            } else {
                fs::read_to_string(path)
                    .map_err(|err| CliError::usage(err.to_string(), "Use: --json-file <path>"))?
            };
            serde_json::from_str(&body)
                .map(Some)
                .map_err(|err| CliError::usage(err.to_string(), "Explore: actr-memory guide slots"))
        }
        None => Ok(None),
    }
}

pub fn parse_rules_file<T>(matches: &ArgMatches) -> Result<Option<T>, CliError>
where
    T: DeserializeOwned,
{
    match matches.get_one::<String>("rules-file") {
        Some(path) => {
            let body = if path == "-" {
                let mut body = String::new();
                std::io::stdin()
                    .read_to_string(&mut body)
                    .map_err(|err| CliError::usage(err.to_string(), "Use: --rules-file <path>"))?;
                body
            } else {
                fs::read_to_string(path)
                    .map_err(|err| CliError::usage(err.to_string(), "Use: --rules-file <path>"))?
            };
            serde_json::from_str(&body).map(Some).map_err(|err| {
                CliError::usage(err.to_string(), "Explore: actr-memory rule eval --help")
            })
        }
        None => Ok(None),
    }
}

fn parse_slot_assignment(raw: &str) -> Result<(String, SlotValueDto), CliError> {
    let (key, rest) = raw.split_once('=').ok_or_else(|| slot_error(raw))?;
    if key.trim().is_empty() {
        return Err(slot_error(raw));
    }
    let (kind, value) = rest.split_once(':').ok_or_else(|| slot_error(raw))?;
    let parsed = match kind {
        "symbol" => SlotValueDto::Symbol(value.to_string()),
        "text" => SlotValueDto::Text(value.to_string()),
        "number" => SlotValueDto::Number(value.parse().map_err(|_| slot_error(raw))?),
        "bool" => SlotValueDto::Bool(parse_bool(value, "slot bool")?),
        _ => return Err(slot_error(raw)),
    };
    Ok((key.to_string(), parsed))
}

fn parse_bool(value: &str, field: &str) -> Result<bool, CliError> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(CliError::usage(
            format!("{field}: expected true or false"),
            "Explore: actr-memory guide slots",
        )),
    }
}

fn slot_error(raw: &str) -> CliError {
    CliError::usage(
        format!("invalid slot value \"{raw}\""),
        "Use typed values: --slot key=symbol:value\nExplore: actr-memory guide slots",
    )
}

#[cfg(test)]
mod tests {
    use clap::{Arg, ArgAction, Command};

    use super::*;

    #[test]
    fn parses_all_slot_value_types() {
        let matches = Command::new("test")
            .arg(
                Arg::new("slot")
                    .long("slot")
                    .action(ArgAction::Append)
                    .value_name("KEY=TYPE:VALUE"),
            )
            .try_get_matches_from([
                "test",
                "--slot",
                "a=symbol:one",
                "--slot",
                "b=text:two",
                "--slot",
                "c=number:3.5",
                "--slot",
                "d=bool:true",
            ]);
        assert!(matches.is_ok());
        let slots = parse_slots(&matches.unwrap_or_else(|err| panic!("{err}")), "slot");
        assert!(slots.is_ok());
        let slots = slots.unwrap_or_else(|err| panic!("{err:?}"));
        assert_eq!(slots.len(), 4);
    }

    #[test]
    fn malformed_slot_points_to_guide() {
        let error = parse_slot_assignment("topic=preference");
        assert!(error.is_err());
        let error = error.expect_err("invalid slot should fail");
        match error {
            CliError::Usage { hint, .. } => assert!(hint.contains("guide slots")),
            _ => panic!("wrong error kind"),
        }
    }
}
