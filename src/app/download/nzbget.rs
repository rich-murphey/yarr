//! NZBGet DownloadClient impl.
//!
//! NZBGet speaks JSON-RPC: every call is a POST of `{"method", "params"}` to
//! `/jsonrpc`, authed by HTTP basic auth from the service's username/password
//! (NZBGet's `ControlUsername`/`ControlPassword`, applied in
//! [`crate::yarr::auth`]). Params are positional JSON values, so caller text is
//! never spliced into a path or query string (S6).
//!
//! `remove` is not implemented: this fork runs no delete (rich-murphey/yarr#1),
//! and the shared dispatch refuses `download_remove` before reaching here.

use anyhow::{Result, anyhow, bail};
use reqwest::Method;
use serde_json::{Value, json};

use crate::app::YarrService;
use crate::config::ServiceConfig;
use crate::yarr::slim;

/// NZBGet's JSON-RPC endpoint, relative to the service base URL.
pub(super) const RPC_PATH: &str = "/jsonrpc";

/// Fields kept for a slimmed `listgroups` row.
pub(super) const GROUP_FIELDS: &[&str] = &[
    "NZBID",
    "NZBName",
    "Status",
    "Category",
    "FileSizeMB",
    "RemainingSizeMB",
    "DownloadedSizeMB",
    "MaxPriority",
];

/// Fields kept for a slimmed `history` row.
pub(super) const HISTORY_FIELDS: &[&str] = &[
    "NZBID",
    "Name",
    "Status",
    "Category",
    "FileSizeMB",
    "HistoryTime",
    "Kind",
];

/// The JSON-RPC request body for `method` with positional `params`. Pure for
/// testability.
pub(super) fn rpc_body(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}

/// POST one JSON-RPC call and unwrap its `result`, turning an `error` member into
/// an error.
async fn call(
    svc: &YarrService,
    config: &ServiceConfig,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = svc
        .client_ref()
        .request_json(
            Method::POST,
            config,
            RPC_PATH,
            Some(rpc_body(method, params)),
            None,
        )
        .await?;
    rpc_result(&config.name, method, response)
}

/// Unwrap a JSON-RPC response envelope. Pure for testability.
pub(super) fn rpc_result(service: &str, method: &str, response: Value) -> Result<Value> {
    if let Some(error) = response.get("error").filter(|e| !e.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("JSON-RPC error");
        bail!("{service} {method} failed: {message}");
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow!("{service} {method} returned no result"))
}

/// NZBGet queue ids are integer `NZBID`s.
fn nzbid(id: &str) -> Result<i64> {
    id.trim()
        .parse::<i64>()
        .map_err(|_| anyhow!("NZBGet ids are numeric NZBIDs, got `{id}`"))
}

/// `listgroups(0)` → the download queue, slimmed.
pub(super) async fn queue(svc: &YarrService, config: &ServiceConfig) -> Result<Value> {
    let groups = call(svc, config, "listgroups", json!([0])).await?;
    Ok(json!({ "groups": slim(groups, GROUP_FIELDS) }))
}

/// `history(false)` → finished, failed and deleted downloads (hidden records
/// excluded), slimmed.
pub(super) async fn history(svc: &YarrService, config: &ServiceConfig) -> Result<Value> {
    let rows = call(svc, config, "history", json!([false])).await?;
    Ok(json!({ "history": slim(rows, HISTORY_FIELDS) }))
}

/// `append` with the URL as `NZBContent`: NZBGet fetches the NZB itself. Returns
/// the new `NZBID`; a result of 0 or less means NZBGet rejected it.
pub(super) async fn add(svc: &YarrService, config: &ServiceConfig, url: &str) -> Result<Value> {
    // append(NZBFilename, NZBContent, Category, Priority, AddToTop, AddPaused,
    //        DupeKey, DupeScore, DupeMode, PPParameters)
    let params = json!(["", url, "", 0, false, false, "", 0, "SCORE", []]);
    let id = call(svc, config, "append", params).await?;
    match id.as_i64() {
        Some(nzbid) if nzbid > 0 => Ok(json!({ "nzbid": nzbid })),
        _ => bail!("{} rejected the URL (append returned {id})", config.name),
    }
}

/// Pause one group (`editqueue GroupPause`) or the whole queue (`pausedownload`).
pub(super) async fn pause(
    svc: &YarrService,
    config: &ServiceConfig,
    id: Option<&str>,
) -> Result<Value> {
    let ok = match id {
        Some(id) => {
            call(
                svc,
                config,
                "editqueue",
                json!(["GroupPause", "", [nzbid(id)?]]),
            )
            .await?
        }
        None => call(svc, config, "pausedownload", json!([])).await?,
    };
    Ok(json!({ "ok": ok }))
}

/// Resume one group (`editqueue GroupResume`) or the whole queue
/// (`resumedownload`).
pub(super) async fn resume(
    svc: &YarrService,
    config: &ServiceConfig,
    id: Option<&str>,
) -> Result<Value> {
    let ok = match id {
        Some(id) => {
            call(
                svc,
                config,
                "editqueue",
                json!(["GroupResume", "", [nzbid(id)?]]),
            )
            .await?
        }
        None => call(svc, config, "resumedownload", json!([])).await?,
    };
    Ok(json!({ "ok": ok }))
}

/// Not implemented — see the module docs.
pub(super) fn remove(config: &ServiceConfig) -> Result<Value> {
    bail!(
        "{}: remove is not implemented for NZBGet; this fork runs no delete (rich-murphey/yarr#1)",
        config.name
    )
}

#[cfg(test)]
#[path = "nzbget_tests.rs"]
mod tests;
