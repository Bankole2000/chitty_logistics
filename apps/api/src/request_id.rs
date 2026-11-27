//! Request IDs: an incoming `x-request-id` is kept, otherwise one is generated. It is echoed in
//! the response, recorded on the request's tracing span (so every log line carries it) and
//! included in error bodies.

use axum::http::HeaderName;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn header_name() -> HeaderName {
    HeaderName::from_static(REQUEST_ID_HEADER)
}
