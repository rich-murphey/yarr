use crate::app::YarrService;
use crate::config::{ServiceConfig, ServiceKind, YarrConfig};
use crate::yarr::{YarrClient, query_get};
use serde_json::json;

use super::{NEWZNAB_API, search_results};

fn newznab_config(base_url: &str) -> ServiceConfig {
    ServiceConfig {
        name: "hydra".into(),
        kind: ServiceKind::Newznab,
        base_url: base_url.into(),
        api_key: Some("secret".into()),
        ..ServiceConfig::default()
    }
}

fn service(kind: ServiceKind) -> YarrService {
    let config = YarrConfig {
        services: vec![ServiceConfig {
            kind,
            ..newznab_config("http://localhost:1")
        }],
    };
    let client = YarrClient::new(&config).unwrap();
    YarrService::new(client, config)
}

#[test]
fn query_injects_json_output_and_apikey_and_encodes_user_text() {
    let url = query_get(
        &newznab_config("https://host/user/nzbhydra2"),
        NEWZNAB_API,
        &[("t", "search"), ("q", "a&t=caps")],
    )
    .unwrap();
    assert_eq!(url.path(), "/user/nzbhydra2/api");
    let query = url.query().unwrap();
    assert!(query.contains("t=search"), "{query}");
    assert!(query.contains("q=a%26t%3Dcaps"), "S6 encoding: {query}");
    assert!(query.contains("o=json"), "{query}");
    assert!(query.contains("apikey=secret"), "{query}");
}

#[test]
fn results_keep_a_fixed_field_set() {
    let raw = json!({
        "channel": {
            "response": { "attributes": { "offset": "0", "total": "720" } },
            "item": [{
                "title": "Some.Release",
                "guid": "-174",
                "link": "https://hydra/getnzb/api/-174?apikey=k",
                "pubDate": 1793508594.0,
                "category": "PC",
                "enclosure": { "attributes": { "length": "786479169" } },
                "attr": [{ "attributes": { "name": "info", "value": "https://x/?apikey=upstream" } }]
            }]
        }
    });
    let out = search_results("hydra", &raw).unwrap();
    assert_eq!(out["total"], 720);
    let item = &out["items"][0];
    assert_eq!(item["title"], "Some.Release");
    assert_eq!(item["size"], 786479169u64);
    assert!(item.get("attr").is_none(), "attr rows carry upstream keys");
}

#[test]
fn single_item_object_is_one_result() {
    let raw = json!({ "channel": { "item": { "title": "Only" } } });
    let out = search_results("hydra", &raw).unwrap();
    assert_eq!(out["items"].as_array().unwrap().len(), 1);
}

#[test]
fn newznab_error_is_an_error() {
    let raw = json!({ "code": "100", "description": "Wrong api key" });
    let err = search_results("hydra", &raw).unwrap_err();
    assert!(err.to_string().contains("Wrong api key"), "{err}");
}

#[tokio::test]
async fn bad_search_type_and_limit_are_rejected_before_any_request() {
    let svc = service(ServiceKind::Newznab);
    let err = svc
        .newznab_search("hydra", "x", Some("music"), None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("search_type"), "{err}");
    let err = svc
        .newznab_search("hydra", "x", None, None, Some(0))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("limit"), "{err}");
}

#[tokio::test]
async fn search_rejects_non_search_kind() {
    let err = service(ServiceKind::Sonarr)
        .newznab_search("hydra", "x", None, None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Search"), "{err}");
}
