//! Audit entries (R-16, FR-022): who changed which record, how, when and why.
//!
//! [`record`] takes the open transaction of the change it records, so the change and its
//! entry commit together or not at all:
//!
//! ```ignore
//! let mut tx = state.db.begin().await?;
//! sqlx::query("UPDATE staff SET status = 'archived', archived_at = now() WHERE id = $1")
//!     .bind(id).execute(&mut *tx).await?;
//! audit::record(&mut tx, &AuditEntry {
//!     staff_id: Some(staff.staff_id),
//!     action: "staff.archive",
//!     record_type: "staff",
//!     record_id: id.to_string(),
//!     before: Some(before), after: Some(after),
//!     reason: Some(reason), request_id,
//! }).await?;
//! tx.commit().await?;
//! ```
//!
//! Entries are append-only: the database rejects updates and deletes.

use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// One change to record.
#[derive(Debug, Clone)]
pub struct AuditEntry<'a> {
    /// The staff member who made the change; `None` for the system.
    pub staff_id: Option<Uuid>,
    /// What was done, e.g. `staff.archive` (usually the permission it needed).
    pub action: &'a str,
    /// The kind of record changed, e.g. `staff`, `driver`, `fare_rule`.
    pub record_type: &'a str,
    pub record_id: String,
    /// The record before the change; `None` when it was created.
    pub before: Option<Value>,
    /// The record after the change; `None` when it was removed.
    pub after: Option<Value>,
    /// Why, as given by the staff member.
    pub reason: Option<String>,
    /// The request's `x-request-id`, linking the entry to the logs.
    pub request_id: Option<String>,
}

/// Writes `entry` inside `tx` and returns its id. Nothing is stored unless `tx` commits.
pub async fn record(
    tx: &mut Transaction<'_, Postgres>,
    entry: &AuditEntry<'_>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO audit_entries
             (staff_id, action, record_type, record_id, before, after, reason, request_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(entry.staff_id)
    .bind(entry.action)
    .bind(entry.record_type)
    .bind(&entry.record_id)
    .bind(&entry.before)
    .bind(&entry.after)
    .bind(&entry.reason)
    .bind(&entry.request_id)
    .fetch_one(&mut **tx)
    .await
}
