//! The standard error body (docs/TECHNICAL.md, API):
//!
//! ```json
//! {"error": {"code": "not_found", "message": "…", "request_id": "…", "details": null}}
//! ```
//!
//! Handlers return [`ApiError`]. Its response carries the error in an extension, and the
//! [`render_errors`] middleware writes the body, adding the request ID. The same middleware
//! turns any other error response without a JSON body (axum's own 405, a panic caught by a
//! layer) into the standard body, so clients only ever see one error shape.

use axum::{
    Json,
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;

use crate::request_id::REQUEST_ID_HEADER;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested resource was not found.",
        )
    }

    /// An unexpected failure. The cause is logged; the client sees only a generic message.
    pub fn internal(cause: impl std::fmt::Display) -> Self {
        tracing::error!(error = %cause, "internal error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Something went wrong. Please try again.",
        )
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut res = self.status.into_response();
        res.extensions_mut().insert(self);
        res
    }
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
    pub details: Option<Value>,
}

/// The machine-readable code for a status, used when an error response did not come from [`ApiError`].
pub fn code_for_status(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "bad_request",
        401 => "unauthenticated",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        409 => "conflict",
        413 => "payload_too_large",
        415 => "unsupported_media_type",
        422 => "invalid_request",
        429 => "rate_limited",
        503 => "unavailable",
        500..=599 => "internal",
        _ => "error",
    }
}

pub async fn render_errors(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let res = next.run(req).await;

    let error = if let Some(e) = res.extensions().get::<ApiError>() {
        e.clone()
    } else if res.status().is_client_error() || res.status().is_server_error() {
        let is_json = res
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.as_bytes().starts_with(b"application/json"));
        if is_json {
            return res;
        }
        let status = res.status();
        let message = status
            .canonical_reason()
            .unwrap_or("Request failed")
            .to_string();
        ApiError::new(status, code_for_status(status), message)
    } else {
        return res;
    };

    let (mut parts, _) = res.into_parts();
    parts.extensions.remove::<ApiError>();
    let body = ErrorBody {
        error: ErrorDetail {
            code: error.code.to_string(),
            message: error.message,
            request_id,
            details: error.details,
        },
    };
    let mut out = (error.status, Json(body)).into_response();
    for (name, value) in parts.headers.iter() {
        if name != header::CONTENT_TYPE && name != header::CONTENT_LENGTH {
            out.headers_mut().append(name.clone(), value.clone());
        }
    }
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    out
}

/// Fallback for unknown routes.
pub async fn not_found() -> ApiError {
    ApiError::not_found()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_has_the_standard_shape() {
        let body = ErrorBody {
            error: ErrorDetail {
                code: "not_found".into(),
                message: "Missing".into(),
                request_id: Some("abc".into()),
                details: None,
            },
        };
        assert_eq!(
            serde_json::to_value(body).unwrap(),
            serde_json::json!({"error": {"code": "not_found", "message": "Missing", "request_id": "abc", "details": null}})
        );
    }

    #[test]
    fn statuses_map_to_codes() {
        assert_eq!(
            code_for_status(StatusCode::METHOD_NOT_ALLOWED),
            "method_not_allowed"
        );
        assert_eq!(code_for_status(StatusCode::BAD_GATEWAY), "internal");
    }
}
