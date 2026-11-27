//! HTTP behaviour of the router: request IDs, the standard error body, CORS and health.
//! Tests marked "database" need Postgres from docker-compose.yml (or DATABASE_URL).

use std::sync::Arc;

use api::{config::Config, state::AppState};
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn config(database_url: Option<&str>) -> Config {
    let url = database_url.map(str::to_owned);
    Config::from_lookup(move |name| match name {
        "DATABASE_URL" => url.clone().or_else(|| std::env::var("DATABASE_URL").ok()),
        "API_CORS_ORIGINS" => Some("http://allowed.test".into()),
        _ => None,
    })
    .unwrap()
}

fn app_with(config: Config) -> Router {
    let db = api::pool(&config).unwrap();
    api::router(AppState {
        db,
        config: Arc::new(config),
    })
}

/// A router whose database can never be reached (nothing listens on port 1).
fn app_without_database() -> Router {
    let mut c = config(Some("postgres://postgres@127.0.0.1:1/none"));
    c.database_acquire_timeout = std::time::Duration::from_millis(300);
    app_with(c)
}

async fn json_body(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn generates_a_request_id_when_none_is_sent() {
    let res = app_without_database()
        .oneshot(Request::get("/nowhere").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let id = res
        .headers()
        .get("x-request-id")
        .expect("x-request-id header");
    assert!(!id.is_empty());
}

#[tokio::test]
async fn echoes_a_supplied_request_id() {
    let res = app_without_database()
        .oneshot(
            Request::get("/nowhere")
                .header("x-request-id", "test-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.headers()["x-request-id"], "test-123");
    assert_eq!(json_body(res).await["error"]["request_id"], "test-123");
}

#[tokio::test]
async fn unknown_route_returns_the_standard_error_body() {
    let res = app_without_database()
        .oneshot(
            Request::get("/nowhere")
                .header("x-request-id", "r-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        json_body(res).await,
        json!({"error": {"code": "not_found", "message": "The requested resource was not found.", "request_id": "r-1", "details": null}})
    );
}

#[tokio::test]
async fn wrong_method_returns_the_standard_error_body() {
    let res = app_without_database()
        .oneshot(Request::post("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(json_body(res).await["error"]["code"], "method_not_allowed");
}

#[tokio::test]
async fn cors_allows_configured_origins_only() {
    let preflight = |origin: &'static str| {
        Request::builder()
            .method(Method::OPTIONS)
            .uri("/health")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap()
    };
    let allowed = app_without_database()
        .oneshot(preflight("http://allowed.test"))
        .await
        .unwrap();
    assert_eq!(
        allowed.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://allowed.test"
    );

    let denied = app_without_database()
        .oneshot(preflight("http://evil.test"))
        .await
        .unwrap();
    assert!(
        denied
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );
}

#[tokio::test]
async fn health_is_503_when_the_database_is_unavailable() {
    let res = app_without_database()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        json_body(res).await,
        json!({"status": "degraded", "database": "unavailable"})
    );
}

/// database
#[tokio::test]
async fn health_is_ok_with_the_database() {
    let res = app_with(config(None))
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "is Postgres running? (docker compose up -d)"
    );
    assert_eq!(
        json_body(res).await,
        json!({"status": "ok", "database": "ok"})
    );
}

/// database
#[tokio::test]
async fn migrations_apply_and_enable_postgis() {
    let c = config(None);
    let db = api::pool(&c).unwrap();
    api::MIGRATOR
        .run(&db)
        .await
        .expect("migrations run (is Postgres running?)");
    let version: String = sqlx::query_scalar("SELECT postgis_version()")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!version.is_empty());
}
