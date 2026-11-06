use super::*;
use crate::actions::YarrAction;
use crate::testing::loopback_state;

#[tokio::test]
async fn help_action_dispatches_to_generated_help() {
    // The shared dispatch returns the MCP help shape `{ "help": <markdown> }`
    // (the CLI renders the structured rest_help payload directly, not through here).
    let state = loopback_state();
    let result = execute_service_action(&state.service, &YarrAction::Help)
        .await
        .unwrap();
    let text = result
        .get("help")
        .and_then(|v| v.as_str())
        .expect("help payload carries a `help` markdown string");
    assert!(text.contains("# yarr MCP Tool"));
}

#[tokio::test]
async fn help_action_dispatches() {
    let state = loopback_state();
    let result = execute_service_action(&state.service, &YarrAction::Help)
        .await
        .unwrap();
    assert!(result.get("help").is_some());
}

#[test]
fn shared_guard_allows_infra_actions_for_configured_kind() {
    // loopback_state configures a sonarr (ArrManager) service. Every infra action
    // is allowed for it via the shared guard.
    let state = loopback_state();
    for action in ["service_status", "api_get", "api_post"] {
        validate_action_for_service(&state.service, action, "sonarr")
            .unwrap_or_else(|e| panic!("{action} should be allowed for sonarr: {e}"));
    }
}

#[test]
fn shared_guard_rejects_action_invalid_for_kind_with_valid_actions() {
    // A non-infra, non-curated (unknown) action fails closed and the error carries
    // the valid-action list so its Display teaches the agent (AN-2). `set_quality`
    // is now a real arr command (valid for sonarr), so use an unknown name here.
    let state = loopback_state();
    let err = validate_action_for_service(&state.service, "totally_unknown", "sonarr")
        .expect_err("totally_unknown is not valid for sonarr");
    let msg = err.to_string();
    assert!(msg.contains("not valid for kind=sonarr"), "msg: {msg}");
    assert!(msg.contains("valid actions for sonarr"), "msg: {msg}");
    // The valid-action list includes the infra actions for that kind.
    assert!(msg.contains("service_status"), "msg: {msg}");
    assert!(crate::actions::is_validation_error(&err));
}

#[test]
fn shared_guard_allows_curated_write_command_for_matching_kind() {
    // A curated download command is allowed for a DownloadClient kind and rejected
    // for a mismatched kind — verified via the registry guard directly (no
    // configured service needed).
    use crate::actions::action_allowed_for_kind;
    use crate::config::ServiceKind;
    assert!(action_allowed_for_kind(
        "download_add",
        ServiceKind::Qbittorrent
    ));
    assert!(!action_allowed_for_kind(
        "download_add",
        ServiceKind::Sonarr
    ));
}

#[test]
fn shared_guard_skips_unknown_service_name() {
    // An unconfigured service name resolves to no kind; the guard defers to the
    // downstream service lookup rather than producing a kind error.
    let state = loopback_state();
    validate_action_for_service(&state.service, "set_quality", "not-configured")
        .expect("guard is a no-op for unknown service names");
}

/// Sonarr + Radarr on an unreachable stub URL: a call that is not refused fails
/// at the network layer instead.
fn arr_service() -> YarrService {
    use crate::config::{ServiceConfig, ServiceKind, YarrConfig};
    let service = |name: &str, kind| ServiceConfig {
        name: name.into(),
        kind,
        base_url: "http://localhost:1".into(),
        api_key: Some("test".into()),
        ..ServiceConfig::default()
    };
    let config = YarrConfig {
        services: vec![
            service("sonarr", ServiceKind::Sonarr),
            service("radarr", ServiceKind::Radarr),
        ],
    };
    let client = crate::yarr::YarrClient::new(&config).unwrap();
    YarrService::new(client, config)
}

#[tokio::test]
async fn deletes_are_refused_for_sonarr_and_radarr() {
    let svc = arr_service();
    let refused = [
        YarrAction::ApiDelete {
            service: "sonarr".into(),
            path: "/api/v3/series/1".into(),
            body: None,
        },
        YarrAction::ApiDelete {
            service: "radarr".into(),
            path: "/api/v3/movie/1".into(),
            body: None,
        },
        YarrAction::Op {
            service: "sonarr".into(),
            op: "delete_series_by_id".into(),
            args: serde_json::json!({ "id": 1, "deleteFiles": true }),
        },
        YarrAction::Op {
            service: "radarr".into(),
            op: "delete_movie_by_id".into(),
            args: serde_json::json!({ "id": 1, "deleteFiles": true }),
        },
        YarrAction::Op {
            service: "radarr".into(),
            op: "delete_moviefile_bulk".into(),
            args: serde_json::json!({ "body": { "movieFileIds": [1] } }),
        },
    ];
    for action in refused {
        let err = execute_service_action(&svc, &action)
            .await
            .expect_err("delete must be refused");
        assert!(err.to_string().contains("refused"), "{action:?}: {err}");
        assert!(crate::actions::is_validation_error(&err));
    }
}

#[tokio::test]
async fn non_delete_ops_are_not_refused() {
    // A GET op dispatches and fails only at the unreachable stub.
    let svc = arr_service();
    for (service, op) in [("sonarr", "get_series"), ("radarr", "get_movie")] {
        let action = YarrAction::Op {
            service: service.into(),
            op: op.into(),
            args: serde_json::json!({}),
        };
        let err = execute_service_action(&svc, &action)
            .await
            .expect_err("stub upstream is unreachable");
        assert!(!err.to_string().contains("refused"), "{op}: {err}");
    }
}
