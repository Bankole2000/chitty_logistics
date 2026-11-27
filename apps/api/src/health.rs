use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

/// Whether the api and its database answer.
#[derive(Debug, Serialize, PartialEq, Eq, ToSchema)]
pub struct Health {
    /// `ok` when everything answers, `degraded` when the database does not.
    #[schema(example = "ok")]
    pub status: &'static str,
    /// `ok`, or `unavailable` when the database does not answer.
    #[schema(example = "ok")]
    pub database: &'static str,
}

/// `GET /health`: 200 when the api and its database answer, 503 when the database does not.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    operation_id = "health",
    summary = "Check the api and its database",
    description = "Returns 200 when the api and its database answer, and 503 when the database does not. Needs no authentication.",
    responses(
        (status = 200, description = "The api and its database answer.", body = Health,
            example = json!({"status": "ok", "database": "ok"})),
        (status = 503, description = "The api answers but its database does not.", body = Health,
            example = json!({"status": "degraded", "database": "unavailable"})),
    )
)]
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(Health {
                status: "ok",
                database: "ok",
            }),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "health check: database unavailable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: "degraded",
                    database: "unavailable",
                }),
            )
        }
    }
}
