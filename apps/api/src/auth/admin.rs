//! `api create-admin`: creates an active admin with no password and a one-time enrolment
//! token. The admin uses the token with `POST /staff/enrolment` to choose a password and
//! enrol an authenticator app. No default credentials exist anywhere.

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::secrets;
use crate::{
    audit::{self, AuditEntry},
    config::StaffAuthConfig,
};

/// The new admin and their enrolment token, shown once. No Debug: holds the token.
pub struct CreatedAdmin {
    pub staff_id: Uuid,
    pub enrolment_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateAdminError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("a staff member with this email already exists")]
    EmailTaken,
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn create_admin(
    db: &PgPool,
    config: &StaffAuthConfig,
    email: &str,
    name: &str,
) -> Result<CreatedAdmin, CreateAdminError> {
    let email = email.trim();
    let name = name.trim();
    let valid_email = email
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
        && !email.contains(char::is_whitespace);
    if !valid_email {
        return Err(CreateAdminError::Invalid(
            "--email must be an email address",
        ));
    }
    if name.is_empty() {
        return Err(CreateAdminError::Invalid("--name must not be empty"));
    }

    let mut tx = db.begin().await?;
    let staff_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO staff (email, name, status) VALUES ($1, $2, 'active') RETURNING id",
    )
    .bind(email)
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(CreateAdminError::EmailTaken);
        }
        Err(e) => return Err(e.into()),
    };
    sqlx::query("INSERT INTO staff_roles (staff_id, role_key) VALUES ($1, 'admin')")
        .bind(staff_id)
        .execute(&mut *tx)
        .await?;
    let (enrolment_token, hash) = secrets::new_token();
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO staff_enrolment_tokens (token_hash, staff_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))
         RETURNING expires_at",
    )
    .bind(&hash[..])
    .bind(staff_id)
    .bind(config.enrolment_token_ttl.as_secs_f64())
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: None,
            action: "staff.create",
            record_type: "staff",
            record_id: staff_id.to_string(),
            before: None,
            after: Some(
                json!({"email": email, "name": name, "status": "active", "roles": ["admin"]}),
            ),
            reason: Some("api create-admin".into()),
            request_id: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(CreatedAdmin {
        staff_id,
        enrolment_token,
        expires_at,
    })
}
