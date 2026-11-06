use super::*;
use serde_json::json;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn search_parses_flags_into_newznab_search() {
    let cmd = parse(
        ServiceKind::Newznab,
        "search",
        &args(&[
            "--query",
            "amelie",
            "--search-type",
            "movie",
            "--limit",
            "5",
        ]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        cmd,
        Command::Curated {
            action: "newznab_search",
            params: json!({
                "service": "newznab",
                "query": "amelie",
                "search_type": "movie",
                "limit": 5
            }),
        }
    );
}

#[test]
fn search_requires_query_and_rejects_unknown_flags() {
    assert!(parse(ServiceKind::Newznab, "search", &[]).is_err());
    assert!(parse(ServiceKind::Newznab, "search", &args(&["--nope", "x"])).is_err());
    assert!(
        parse(
            ServiceKind::Newznab,
            "search",
            &args(&["--limit", "x", "--query", "q"])
        )
        .is_err()
    );
}

#[test]
fn other_verbs_fall_through() {
    assert!(
        parse(ServiceKind::Newznab, "status", &[])
            .unwrap()
            .is_none()
    );
}
