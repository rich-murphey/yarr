//! Search capability: release search over the Newznab API (NZBHydra2, or a
//! single Newznab indexer).
//!
//! Newznab is a `?t=` query API on `/api`. `apikey` and `o=json` are injected by
//! [`query_get`](crate::yarr::query_get), and every caller value goes through
//! `append_pair`, so a query like `"x&t=caps"` cannot inject a parameter (S6).
//!
//! The JSON is NZBHydra2's XML-to-JSON rendering of the Newznab RSS feed. Item
//! `attr` rows can carry upstream indexer api keys (the `info` attr), so results
//! are reduced to a fixed field set rather than returned raw.

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::app::YarrService;
use crate::capability::Capability;

/// Base path for the Newznab API.
const NEWZNAB_API: &str = "/api";

/// Newznab search functions this command exposes.
const SEARCH_TYPES: &[&str] = &["search", "tvsearch", "movie"];

impl YarrService {
    /// Search releases. READ. `search_type` is the Newznab `t` (default
    /// `search`); `category` is a comma-separated Newznab category list.
    pub async fn newznab_search(
        &self,
        service: &str,
        query: &str,
        search_type: Option<&str>,
        category: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Value> {
        let config = self.service_of_capability(service, Capability::Search)?;
        let search_type = search_type.unwrap_or("search");
        if !SEARCH_TYPES.contains(&search_type) {
            bail!(
                "search_type must be one of {}, got `{search_type}`",
                SEARCH_TYPES.join(", ")
            );
        }
        if limit.is_some_and(|n| n < 1) {
            bail!("limit must be at least 1");
        }
        let limit = limit.map(|n| n.to_string());
        let mut params = vec![("t", search_type), ("q", query)];
        if let Some(category) = category {
            params.push(("cat", category));
        }
        if let Some(limit) = limit.as_deref() {
            params.push(("limit", limit));
        }
        let url = crate::yarr::query_get(config, NEWZNAB_API, &params)?;
        let raw = self.client_ref().send_get(config, url, None).await?;
        search_results(&config.name, &raw)
    }
}

/// Reduce a Newznab JSON response to `{total, items: [{title, guid, link,
/// pubDate, category, size}]}`. A Newznab error (`{code, description}`, sent
/// with HTTP 200) becomes an error. Pure for testability.
pub(crate) fn search_results(service: &str, raw: &Value) -> Result<Value> {
    let Some(channel) = raw.get("channel") else {
        let description = raw
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("response has no channel");
        let code = raw.get("code").and_then(Value::as_str).unwrap_or("?");
        bail!("{service} search failed: Newznab error {code}: {description}");
    };
    // A single result may be rendered as an object rather than a one-element array.
    let items = match channel.get("item") {
        Some(Value::Array(items)) => items.iter().collect::<Vec<_>>(),
        Some(item @ Value::Object(_)) => vec![item],
        _ => Vec::new(),
    };
    let items = items
        .into_iter()
        .map(|item| {
            let size = item
                .pointer("/enclosure/attributes/length")
                .and_then(|v| match v {
                    Value::String(s) => s.parse::<u64>().ok(),
                    other => other.as_u64(),
                });
            json!({
                "title": item.get("title"),
                "guid": item.get("guid"),
                "link": item.get("link"),
                "pubDate": item.get("pubDate"),
                "category": item.get("category"),
                "size": size,
            })
        })
        .collect::<Vec<_>>();
    let total = channel
        .pointer("/response/attributes/total")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<u64>().ok());
    Ok(json!({ "total": total, "items": items }))
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
