use crate::app::YarrService;
use crate::config::{ServiceConfig, ServiceKind, YarrConfig};
use crate::yarr::YarrClient;
use serde_json::{Value, json};

use super::{rpc_body, rpc_result};

fn nzbget_service(base_url: &str) -> YarrService {
    let config = YarrConfig {
        services: vec![ServiceConfig {
            name: "nzbget".into(),
            kind: ServiceKind::Nzbget,
            base_url: base_url.into(),
            username: Some("user".into()),
            password: Some("pass".into()),
            ..ServiceConfig::default()
        }],
    };
    let client = YarrClient::new(&config).unwrap();
    YarrService::new(client, config)
}

/// A captured request: request line, lowercased header lines, and body.
struct Captured {
    line: String,
    headers: Vec<String>,
    body: Value,
}

/// Single-request TCP stub answering with `body`; hands back what was sent.
fn stub(body: &'static str) -> (String, std::sync::mpsc::Receiver<Captured>) {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut headers = Vec::new();
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            let header = header.trim_end().to_ascii_lowercase();
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            headers.push(header);
        }
        let mut raw = vec![0; length];
        reader.read_exact(&mut raw).unwrap();
        tx.send(Captured {
            line: line.trim_end().to_string(),
            headers,
            body: serde_json::from_slice(&raw).unwrap_or(Value::Null),
        })
        .unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (format!("http://{addr}/user/nzbget"), rx)
}

#[test]
fn rpc_body_is_method_plus_positional_params() {
    assert_eq!(
        rpc_body("listgroups", json!([0])),
        json!({ "method": "listgroups", "params": [0] })
    );
}

#[test]
fn rpc_result_unwraps_result_and_surfaces_errors() {
    let ok = rpc_result(
        "nzbget",
        "version",
        json!({ "version": "1.1", "result": "26.3" }),
    );
    assert_eq!(ok.unwrap(), json!("26.3"));
    let err = rpc_result(
        "nzbget",
        "nosuch",
        json!({ "version": "1.1", "error": { "code": 1, "message": "Invalid procedure" } }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Invalid procedure"), "{err}");
}

#[tokio::test]
async fn queue_posts_listgroups_with_basic_auth_and_slims() {
    let (url, rx) = stub(
        r#"{"version":"1.1","result":[{"NZBID":7,"NZBName":"a","Status":"QUEUED","Category":"movies","FileSizeMB":10,"RemainingSizeMB":5,"DownloadedSizeMB":5,"MaxPriority":0,"ServerStats":[]}]}"#,
    );
    let out = nzbget_service(&url).download_queue("nzbget").await.unwrap();
    let sent = rx.recv().unwrap();
    assert_eq!(sent.line, "POST /user/nzbget/jsonrpc HTTP/1.1");
    // base64("user:pass")
    assert!(
        sent.headers
            .contains(&"authorization: basic dxnlcjpwyxnz".to_string()),
        "{:?}",
        sent.headers
    );
    assert_eq!(sent.body, json!({ "method": "listgroups", "params": [0] }));
    assert_eq!(out["groups"][0]["NZBID"], 7);
    assert!(out["groups"][0].get("ServerStats").is_none());
}

#[tokio::test]
async fn history_posts_history_false() {
    let (url, rx) = stub(
        r#"{"version":"1.1","result":[{"NZBID":3,"Name":"b","Status":"SUCCESS/ALL","Log":[]}]}"#,
    );
    let out = nzbget_service(&url)
        .download_history("nzbget")
        .await
        .unwrap();
    assert_eq!(rx.recv().unwrap().body["params"], json!([false]));
    assert_eq!(out["history"][0]["Status"], "SUCCESS/ALL");
    assert!(out["history"][0].get("Log").is_none());
}

#[tokio::test]
async fn add_appends_url_as_content_and_returns_nzbid() {
    let (url, rx) = stub(r#"{"version":"1.1","result":42}"#);
    let out = nzbget_service(&url)
        .download_add("nzbget", "https://hydra/getnzb/1?x=y&t=z")
        .await
        .unwrap();
    let sent = rx.recv().unwrap();
    assert_eq!(sent.body["method"], "append");
    assert_eq!(sent.body["params"][1], "https://hydra/getnzb/1?x=y&t=z");
    assert_eq!(sent.body["params"].as_array().unwrap().len(), 10);
    assert_eq!(out, json!({ "nzbid": 42 }));
}

#[tokio::test]
async fn add_rejected_by_nzbget_is_an_error() {
    let (url, _rx) = stub(r#"{"version":"1.1","result":0}"#);
    let err = nzbget_service(&url)
        .download_add("nzbget", "https://x/a.nzb")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("rejected"), "{err}");
}

#[tokio::test]
async fn pause_one_group_uses_editqueue() {
    let (url, rx) = stub(r#"{"version":"1.1","result":true}"#);
    let out = nzbget_service(&url)
        .download_pause("nzbget", Some("7"))
        .await
        .unwrap();
    assert_eq!(
        rx.recv().unwrap().body,
        json!({ "method": "editqueue", "params": ["GroupPause", "", [7]] })
    );
    assert_eq!(out, json!({ "ok": true }));
}

#[tokio::test]
async fn resume_all_uses_resumedownload() {
    let (url, rx) = stub(r#"{"version":"1.1","result":true}"#);
    nzbget_service(&url)
        .download_resume("nzbget", None)
        .await
        .unwrap();
    assert_eq!(
        rx.recv().unwrap().body,
        json!({ "method": "resumedownload", "params": [] })
    );
}

#[tokio::test]
async fn non_numeric_id_is_rejected_before_any_request() {
    let svc = nzbget_service("http://localhost:1");
    let err = svc.download_pause("nzbget", Some("abc")).await.unwrap_err();
    assert!(err.to_string().contains("numeric"), "{err}");
}

#[tokio::test]
async fn remove_is_not_implemented() {
    let svc = nzbget_service("http://localhost:1");
    let err = svc.download_remove("nzbget", "7", false).await.unwrap_err();
    assert!(err.to_string().contains("no delete"), "{err}");
}

#[tokio::test]
async fn history_is_nzbget_only() {
    let config = YarrConfig {
        services: vec![ServiceConfig {
            name: "sabnzbd".into(),
            kind: ServiceKind::Sabnzbd,
            base_url: "http://localhost:1".into(),
            ..ServiceConfig::default()
        }],
    };
    let client = YarrClient::new(&config).unwrap();
    let err = YarrService::new(client, config)
        .download_history("sabnzbd")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("nzbget only"), "{err}");
}
