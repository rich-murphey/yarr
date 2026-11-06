//! CLI parse module for Search (Newznab) curated commands.
//!
//! Thin shim: `yarr <service> search --query Q [--search-type T] [--category C]
//! [--limit N]` → `newznab_search`. No business logic.

use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

use crate::actions::curated_command;
use crate::capability::Capability;
use crate::cli::command::Command;
use crate::config::ServiceKind;

/// Friendly CLI verb → registry action name. SSOT for USAGE and parity.
pub const VERBS: &[(&str, &str)] = &[("search", "newznab_search")];

pub fn parse(kind: ServiceKind, verb: &str, rest: &[String]) -> Result<Option<Command>> {
    let Some(action) = resolve(verb)? else {
        return Ok(None);
    };
    let mut params = Map::new();
    params.insert("service".into(), json!(kind.as_str()));
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let key = match flag {
            "--query" => "query",
            "--search-type" => "search_type",
            "--category" => "category",
            "--limit" => "limit",
            other => return Err(anyhow!("{verb} does not accept argument `{other}`")),
        };
        i += 1;
        let value = rest
            .get(i)
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| anyhow!("{flag} requires a value"))?;
        let value = if key == "limit" {
            json!(
                value
                    .parse::<i64>()
                    .map_err(|_| anyhow!("--limit must be an integer"))?
            )
        } else {
            json!(value)
        };
        if params.insert(key.into(), value).is_some() {
            return Err(anyhow!("{verb} received duplicate {flag}"));
        }
        i += 1;
    }
    if !params.contains_key("query") {
        return Err(anyhow!("{verb} requires --query"));
    }
    Ok(Some(Command::Curated {
        action,
        params: Value::Object(params),
    }))
}

fn resolve(verb: &str) -> Result<Option<&'static str>> {
    let Some((_, action)) = VERBS.iter().find(|(cli_verb, _)| *cli_verb == verb) else {
        return Ok(None);
    };
    curated_command(action)
        .filter(|cmd| cmd.capability == Capability::Search)
        .map(|cmd| Some(cmd.name))
        .ok_or_else(|| anyhow!("internal: verb `{verb}` has no Search descriptor"))
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
