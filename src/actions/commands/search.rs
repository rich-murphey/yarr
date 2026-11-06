//! Search (Newznab) curated command descriptors.
//!
//! Thin adapters only — the request and result shaping live in
//! `crate::app::search`.

use serde_json::Value;

use crate::actions::model::READ_SCOPE;
use crate::actions::parse::{optional_i64, optional_string, string_arg};
use crate::actions::registry::{
    CommandDescriptor, CommandFuture,
    ParamType::{Integer, String as StringParam},
};
use crate::app::YarrService;
use crate::capability::Capability;

/// The Search (Newznab) curated commands.
pub const SEARCH_COMMANDS: &[CommandDescriptor] = &[CommandDescriptor {
    name: "newznab_search",
    capability: Capability::Search,
    description: "search releases by --query; --search-type search|tvsearch|movie \
         (default search), --category a Newznab category list, --limit N. Returns \
         title, guid, link (the NZB URL for download_add), pubDate, category, size.",
    required_scope: READ_SCOPE,
    required_params: &["service", "query"],
    optional_params: &["search_type", "category", "limit"],
    destructive: false,
    mutates: false,
    typed_params: &[
        ("query", StringParam),
        ("search_type", StringParam),
        ("category", StringParam),
        ("limit", Integer),
    ],
    handler: handle_search,
}];

fn handle_search<'a>(svc: &'a YarrService, args: &'a Value) -> CommandFuture<'a> {
    Box::pin(async move {
        let service = string_arg(args, "service")?;
        let query = string_arg(args, "query")?;
        let search_type = optional_string(args, "search_type")?;
        let category = optional_string(args, "category")?;
        let limit = optional_i64(args, "limit")?;
        svc.newznab_search(
            &service,
            &query,
            search_type.as_deref(),
            category.as_deref(),
            limit,
        )
        .await
    })
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
