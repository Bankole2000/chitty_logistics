//! Failed sign-in attempts per account and per client IP address
//! (`STAFF_SIGNIN_MAX_ATTEMPTS` per `STAFF_SIGNIN_WINDOW_MINUTES`).
//!
//! Each attempt is recorded as a failure *before* the password or code is checked, then
//! counted; the row is removed again when the check succeeds. Concurrent guesses therefore
//! see each other, so a burst cannot get more than the allowed number of checks.

use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use crate::{config::StaffAuthConfig, error::ApiError};

/// An attempt in progress; [`Attempt::succeeded`] removes it from the count.
#[must_use]
pub struct Attempt {
    id: i64,
}

/// 429 `rate_limited`, with `Retry-After` in seconds.
pub struct RateLimited {
    pub retry_after: Duration,
}

impl IntoResponse for RateLimited {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        let secs = self.retry_after.as_secs().max(1);
        headers.insert(header::RETRY_AFTER, HeaderValue::from(secs));
        let error = ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many sign-in attempts. Please wait and try again.",
        )
        .with_details(serde_json::json!({ "retry_after_seconds": secs }));
        (headers, error).into_response()
    }
}

pub enum Outcome {
    Allowed(Attempt),
    Limited(RateLimited),
}

/// Records an attempt for `account_key` (lower-cased email) from `ip` and decides whether
/// it may go ahead.
pub async fn begin(
    db: &PgPool,
    config: &StaffAuthConfig,
    account_key: &str,
    ip: &str,
) -> Result<Outcome, sqlx::Error> {
    let window = config.sign_in_window.as_secs_f64();
    sqlx::query(
        "DELETE FROM staff_sign_in_failures WHERE created_at < now() - make_interval(secs => $1)",
    )
    .bind(window)
    .execute(db)
    .await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO staff_sign_in_failures (account_key, ip) VALUES ($1, $2) RETURNING id",
    )
    .bind(account_key)
    .bind(ip)
    .fetch_one(db)
    .await?;
    // Attempts in the window for this account and this IP, and the seconds until the
    // oldest of each leaves the window.
    let (by_account, by_ip, account_wait, ip_wait): (i64, i64, Option<f64>, Option<f64>) =
        sqlx::query_as(
            "WITH recent AS (
                 SELECT account_key, ip, created_at FROM staff_sign_in_failures
                 WHERE created_at >= now() - make_interval(secs => $3)
                   AND (account_key = $1 OR ip = $2)
             )
             SELECT
                 count(*) FILTER (WHERE account_key = $1),
                 count(*) FILTER (WHERE ip = $2),
                 extract(epoch FROM min(created_at) FILTER (WHERE account_key = $1)
                     + make_interval(secs => $3) - now())::float8,
                 extract(epoch FROM min(created_at) FILTER (WHERE ip = $2)
                     + make_interval(secs => $3) - now())::float8
             FROM recent",
        )
        .bind(account_key)
        .bind(ip)
        .bind(window)
        .fetch_one(db)
        .await?;
    let max = i64::from(config.sign_in_max_attempts);
    if by_account > max || by_ip > max {
        // A refused attempt is not counted, so waiting out the window always works.
        delete(db, id).await?;
        let mut wait: f64 = 0.0;
        if by_account > max {
            wait = wait.max(account_wait.unwrap_or(0.0));
        }
        if by_ip > max {
            wait = wait.max(ip_wait.unwrap_or(0.0));
        }
        tracing::warn!(%ip, "staff sign-in rate limited");
        return Ok(Outcome::Limited(RateLimited {
            retry_after: Duration::from_secs_f64(wait.max(1.0).ceil()),
        }));
    }
    Ok(Outcome::Allowed(Attempt { id }))
}

impl Attempt {
    /// The password or code was right: the attempt does not count.
    pub async fn succeeded(self, db: &PgPool) -> Result<(), sqlx::Error> {
        delete(db, self.id).await
    }

    /// The attempt failed and stays counted.
    pub fn failed(self) {}
}

async fn delete(db: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM staff_sign_in_failures WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// After a completed sign-in, the account's earlier failures no longer count (failures from
/// the IP address still do).
pub async fn clear_account(db: &PgPool, account_key: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM staff_sign_in_failures WHERE account_key = $1")
        .bind(account_key)
        .execute(db)
        .await?;
    Ok(())
}
