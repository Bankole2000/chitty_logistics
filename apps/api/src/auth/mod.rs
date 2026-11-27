//! Staff authorization (ADR-003): the code checks permissions; roles are data that map to them.
//!
//! A staff endpoint takes [`StaffIdentity`] as an extractor and calls
//! [`StaffIdentity::require`] with the permission it needs:
//!
//! ```ignore
//! async fn approve(staff: StaffIdentity, /* … */) -> Result<Json<…>, ApiError> {
//!     staff.require(Permission::DriverApprove)?;
//!     // …change the record and write its audit entry in one transaction (crate::audit)
//! }
//! ```
//!
//! The identity is read from the request extensions. Only the sign-in layer,
//! [`session::authenticate`], puts it there, after checking the request's
//! `Authorization: Bearer` session token; without a valid session every staff endpoint
//! answers 401 `unauthenticated`. Tests of permission checks alone may insert an identity
//! loaded with [`StaffIdentity::load`] into a test router's requests.

pub mod admin;
pub mod permissions;
pub mod rate_limit;
pub mod secrets;
pub mod session;
pub mod sign_in;

use std::collections::BTreeSet;

use axum::{extract::FromRequestParts, http::request::Parts};
use sqlx::PgExecutor;
use uuid::Uuid;

pub use permissions::Permission;

use crate::error::ApiError;

/// The signed-in staff member and the permissions their roles give them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaffIdentity {
    pub staff_id: Uuid,
    permissions: BTreeSet<Permission>,
}

impl StaffIdentity {
    /// Loads a staff member's permissions from their roles. Only active staff hold
    /// permissions: invited and archived staff get an empty set. Keys in the database that
    /// the code no longer defines are ignored. `None` if there is no such staff member.
    pub async fn load<'e>(
        db: impl PgExecutor<'e> + Copy,
        staff_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM staff WHERE id = $1")
            .bind(staff_id)
            .fetch_optional(db)
            .await?;
        let Some(status) = status else {
            return Ok(None);
        };
        if status != "active" {
            return Ok(Some(Self {
                staff_id,
                permissions: BTreeSet::new(),
            }));
        }
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT rp.permission_key
             FROM staff_roles sr JOIN role_permissions rp ON rp.role_key = sr.role_key
             WHERE sr.staff_id = $1",
        )
        .bind(staff_id)
        .fetch_all(db)
        .await?;
        Ok(Some(Self {
            staff_id,
            permissions: keys
                .iter()
                .filter_map(|k| Permission::from_key(k))
                .collect(),
        }))
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// `Ok` when the staff member holds `permission`, otherwise 403 `forbidden`.
    pub fn require(&self, permission: Permission) -> Result<(), ApiError> {
        if self.has(permission) {
            Ok(())
        } else {
            tracing::info!(staff_id = %self.staff_id, %permission, "permission denied");
            Err(ApiError::forbidden())
        }
    }

    pub fn permissions(&self) -> impl Iterator<Item = Permission> + '_ {
        self.permissions.iter().copied()
    }
}

/// 401 `unauthenticated` when no staff member is signed in.
impl<S: Send + Sync> FromRequestParts<S> for StaffIdentity {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<StaffIdentity>()
            .cloned()
            .ok_or_else(ApiError::unauthenticated)
    }
}
