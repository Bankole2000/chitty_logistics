//! Chitty Logistics api: one server for every client (ADR-001).

pub mod audit;
pub mod auth;
pub mod config;
pub mod error;
pub mod health;
pub mod openapi;
pub mod request_id;
pub mod state;

use std::sync::Arc;

use axum::{
    Router,
    http::{HeaderValue, Method, Request, header},
    middleware,
    routing::get,
};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::{
    config::{Config, LogFormat},
    request_id::header_name,
    state::AppState,
};

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// A pool that connects on first use, so the server starts (and reports itself unhealthy)
/// while the database is down.
pub fn pool(config: &Config) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(config.database_acquire_timeout)
        .connect_lazy(&config.database_url)
}

pub fn cors(config: &Config) -> CorsLayer {
    let origins: Vec<HeaderValue> = config
        .cors_origins
        .iter()
        .filter_map(|o| match HeaderValue::from_str(o) {
            Ok(v) => Some(v),
            Err(_) => {
                tracing::warn!(origin = %o, "ignoring invalid CORS origin");
                None
            }
        })
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header_name()])
        .expose_headers([header_name()])
}

pub fn router(state: AppState) -> Router {
    let config = state.config.clone();
    Router::new()
        .route("/health", get(health::health))
        .fallback(error::not_found)
        .with_state(state)
        .layer(
            // Outermost first: the request ID is set before the trace span opens, so every
            // log line and error body carries it.
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(header_name(), MakeRequestUuid))
                .layer(TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
                    let id = req
                        .headers()
                        .get(request_id::REQUEST_ID_HEADER)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!("request", method = %req.method(), path = %req.uri().path(), request_id = %id)
                }))
                .layer(PropagateRequestIdLayer::new(header_name()))
                .layer(cors(&config))
                .layer(middleware::from_fn(error::render_errors)),
        )
}

pub fn init_tracing(format: LogFormat) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "api=info,tower_http=info".into());
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

/// Starts the server and runs until Ctrl+C or SIGTERM.
pub async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = pool(&config)?;
    if config.migrate_on_start {
        MIGRATOR.run(&db).await?;
        tracing::info!("migrations up to date");
    }
    // The server still starts if the database is down; permissions are then synced on the
    // next start. A key missing from the database is held by no role, so checks fail closed.
    if let Err(e) = auth::permissions::sync(&db).await {
        tracing::error!(error = %e, "could not sync permissions with the database");
    }
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "api listening");
    let state = AppState {
        db,
        config: Arc::new(config),
    };
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
