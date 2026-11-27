//! Staff sessions (ADR-003): opaque 256-bit tokens sent as `Authorization: Bearer <token>`.
//! Only the SHA-256 hash of a token is stored. A session is refused once revoked, once it
//! has gone unused for `STAFF_SESSION_IDLE_TIMEOUT_MINUTES`, or once its staff member is no
//! longer active.
//!
//! [`authenticate`] is the sign-in layer: the only code that puts a [`StaffIdentity`] on a
//! request.

use std::time::Duration;

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{StaffIdentity, secrets};
use crate::{error::ApiError, state::AppState};

/// The session the current request was authenticated with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentSession {
    pub id: Uuid,
}

/// A session just created; its token is shown to the client once and never stored.
pub struct NewSession {
    pub id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

/// Creates a session for `staff_id` inside `tx`.
pub async fn create(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    idle_timeout: Duration,
) -> Result<NewSession, sqlx::Error> {
    let (token, hash) = secrets::new_token();
    let (id, expires_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO staff_sessions (token_hash, staff_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))
         RETURNING id, expires_at",
    )
    .bind(&hash[..])
    .bind(staff_id)
    .bind(idle_timeout.as_secs_f64())
    .fetch_one(&mut **tx)
    .await?;
    Ok(NewSession {
        id,
        token,
        expires_at,
    })
}

/// The bearer token of a request, if it sent one.
fn bearer(req: &Request) -> Option<&str> {
    let value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Middleware: when the request carries a valid session token, records the use (moving the
/// idle expiry forward) and puts the staff member's [`StaffIdentity`] and the
/// [`CurrentSession`] on the request. Otherwise the request goes on without them, and
/// endpoints that need a staff member answer 401 `unauthenticated`.
pub async fn authenticate(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    // Only this layer may set these; drop anything already there.
    req.extensions_mut().remove::<StaffIdentity>();
    req.extensions_mut().remove::<CurrentSession>();

    if let Some(hash) = bearer(&req).and_then(secrets::token_hash) {
        match load(&state, &hash).await {
            Ok(Some((session, identity))) => {
                req.extensions_mut().insert(session);
                req.extensions_mut().insert(identity);
            }
            Ok(None) => {}
            Err(e) => return ApiError::from(e).into_response(),
        }
    }
    next.run(req).await
}

async fn load(
    state: &AppState,
    hash: &[u8; 32],
) -> Result<Option<(CurrentSession, StaffIdentity)>, sqlx::Error> {
    let idle = state.config.staff_auth.session_idle_timeout;
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "UPDATE staff_sessions s
         SET last_used_at = now(), expires_at = now() + make_interval(secs => $2)
         FROM staff
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now()
           AND staff.id = s.staff_id AND staff.status = 'active'
         RETURNING s.id, s.staff_id",
    )
    .bind(&hash[..])
    .bind(idle.as_secs_f64())
    .fetch_optional(&state.db)
    .await?;
    let Some((id, staff_id)) = row else {
        return Ok(None);
    };
    Ok(StaffIdentity::load(&state.db, staff_id)
        .await?
        .map(|identity| (CurrentSession { id }, identity)))
}
