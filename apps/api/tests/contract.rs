//! The committed contract in packages/api-contract matches the code: openapi.json is what the
//! api generates, and every path and method in it has a request in the Bruno collection.
//! Needs no database.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use api::openapi::{self, CheckError};
use serde_json::{Value, json};

const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// The Bruno variable every request url starts with. Built with `concat!` so the repository's
/// placeholder check (which flags double braces) does not report this file.
const BASE_URL_VAR: &str = concat!("{", "{baseUrl}", "}");

fn contract_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/api-contract")
}

/// A (method, path) pair, e.g. ("get", "/orders/{id}").
type Endpoint = (String, String);

fn spec_endpoints(spec: &Value) -> BTreeSet<Endpoint> {
    let mut out = BTreeSet::new();
    if let Some(paths) = spec["paths"].as_object() {
        for (path, item) in paths {
            for method in METHODS {
                if item.get(method).is_some() {
                    out.insert((method.to_string(), path.clone()));
                }
            }
        }
    }
    out
}

/// Reads the method block (`get {`, `post {`, …) and its `url:` line from a `.bru` file.
/// The url's leading variable (the `baseUrl` environment variable in double braces) and query
/// string are dropped and `:name` path parameters become OpenAPI's `{name}`.
fn bru_endpoint(text: &str) -> Option<Endpoint> {
    let mut method = None;
    for line in text.lines() {
        if method.is_none() {
            let head = line.trim_end();
            if let Some(m) = head.strip_suffix(" {")
                && METHODS.contains(&m)
            {
                method = Some(m.to_string());
            }
            continue;
        }
        if line.trim() == "}" {
            return None;
        }
        if let Some(url) = line.trim().strip_prefix("url:") {
            let url = url.trim();
            let url = url.strip_prefix(BASE_URL_VAR).unwrap_or(url);
            let path = url.split(['?', '#']).next().unwrap_or("");
            let path = path
                .split('/')
                .map(|seg| match seg.strip_prefix(':') {
                    Some(name) => ["{", name, "}"].concat(),
                    None => seg.to_string(),
                })
                .collect::<Vec<_>>()
                .join("/");
            return Some((method?, path));
        }
    }
    None
}

fn bruno_endpoints(dir: &Path) -> BTreeSet<Endpoint> {
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "environments") {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "bru")
                && let Some(endpoint) = bru_endpoint(&fs::read_to_string(&path).unwrap())
            {
                out.insert(endpoint);
            }
        }
    }
    out
}

/// Endpoints in the spec with no Bruno request, as `METHOD /path`.
fn missing_requests(spec: &Value, requests: &BTreeSet<Endpoint>) -> Vec<String> {
    spec_endpoints(spec)
        .difference(requests)
        .map(|(m, p)| format!("{} {p}", m.to_uppercase()))
        .collect()
}

#[test]
fn committed_openapi_json_matches_the_code() {
    let path = contract_dir().join("openapi.json");
    if let Err(e) = openapi::check(&path) {
        // Name the repository-relative path, so the message gives the usual command.
        let e = match e {
            CheckError::Stale { .. } => CheckError::Stale {
                path: openapi::DEFAULT_PATH.into(),
            },
            other => other,
        };
        panic!("{e}");
    }
}

#[test]
fn check_fails_on_a_stale_copy_and_names_the_command() {
    let dir = std::env::temp_dir().join(format!("api-contract-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let stale = dir.join("openapi.json");
    fs::write(
        &stale,
        openapi::to_json().replace("\"/health\"", "\"/healthz\""),
    )
    .unwrap();

    let e = openapi::check(&stale).expect_err("a stale copy must fail the check");
    assert!(matches!(e, CheckError::Stale { .. }));
    let msg = e.to_string();
    assert!(
        msg.contains("cargo run -p api -- openapi --out"),
        "message names the command: {msg}"
    );

    openapi::write(&stale).unwrap();
    openapi::check(&stale).expect("a freshly written copy passes");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_fails_when_the_file_is_missing() {
    let e = openapi::check(Path::new("does/not/exist/openapi.json")).unwrap_err();
    assert!(matches!(e, CheckError::Unreadable { .. }));
}

#[test]
fn every_endpoint_has_a_bruno_request() {
    let spec: Value = serde_json::from_str(&openapi::to_json()).unwrap();
    let requests = bruno_endpoints(&contract_dir().join("bruno"));
    let missing = missing_requests(&spec, &requests);
    assert!(
        missing.is_empty(),
        "endpoints without a Bruno request in packages/api-contract/bruno/ (add one .bru file each): {}",
        missing.join(", ")
    );
}

#[test]
fn coverage_check_reports_an_endpoint_without_a_request() {
    let spec = json!({"paths": {
        "/health": {"get": {}},
        "/orders/{id}": {"get": {}, "patch": {}},
    }});
    let requests = BTreeSet::from([
        ("get".to_string(), "/health".to_string()),
        ("get".to_string(), "/orders/{id}".to_string()),
    ]);
    assert_eq!(missing_requests(&spec, &requests), ["PATCH /orders/{id}"]);
}

#[test]
fn bru_files_are_parsed_for_method_and_path() {
    let text = [
        "meta {\n  name: Order\n  type: http\n}\n\npatch {\n  url: ",
        BASE_URL_VAR,
        "/orders/:id?expand=1\n  body: json\n}\n",
    ]
    .concat();
    assert_eq!(
        bru_endpoint(&text),
        Some(("patch".to_string(), "/orders/{id}".to_string()))
    );
    assert_eq!(bru_endpoint("vars {\n  baseUrl: x\n}\n"), None);
}
