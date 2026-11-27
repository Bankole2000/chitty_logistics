//! Staff permissions and audit entries (TASK-004; TC-007, TC-010).
//! database: every test runs in its own throwaway database with the migrations applied
//! (#[sqlx::test]); Postgres from docker-compose.yml (DATABASE_URL via .cargo/config.toml).

use api::{
    audit::{self, AuditEntry},
    auth::{Permission, StaffIdentity, permissions},
    error::{self, ApiError},
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::post,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn add_staff(db: &PgPool, email: &str, roles: &[&str]) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO staff (email, name, status) VALUES ($1, 'Test Staff', 'active') RETURNING id",
    )
    .bind(email)
    .fetch_one(db)
    .await
    .unwrap();
    for role in roles {
        sqlx::query("INSERT INTO staff_roles (staff_id, role_key) VALUES ($1, $2)")
            .bind(id)
            .bind(role)
            .execute(db)
            .await
            .unwrap();
    }
    id
}

async fn identity(db: &PgPool, id: Uuid) -> StaffIdentity {
    StaffIdentity::load(db, id)
        .await
        .unwrap()
        .expect("staff exists")
}

/// A stand-in staff endpoint guarded by `driver.approve`.
async fn approve_driver(staff: StaffIdentity) -> Result<&'static str, ApiError> {
    staff.require(Permission::DriverApprove)?;
    Ok("approved")
}

fn app() -> Router {
    Router::new()
        .route("/drivers/approve", post(approve_driver))
        .layer(middleware::from_fn(error::render_errors))
}

/// Calls the endpoint as `who` (as the sign-in layer will, by putting the identity in the
/// request extensions), or as nobody.
async fn call(who: Option<StaffIdentity>) -> (StatusCode, Value) {
    let mut req = Request::post("/drivers/approve")
        .header("x-request-id", "req-1")
        .body(Body::empty())
        .unwrap();
    if let Some(who) = who {
        req.extensions_mut().insert(who);
    }
    let res = app().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[sqlx::test]
async fn role_with_the_permission_passes_and_without_gets_403(db: PgPool) {
    let ops = add_staff(&db, "ops@example.test", &["operations"]).await;
    let hr = add_staff(&db, "hr@example.test", &["hr"]).await;

    let (status, _) = call(Some(identity(&db, ops).await)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(Some(identity(&db, hr).await)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body,
        json!({"error": {"code": "forbidden", "message": "You don't have permission to do this.", "request_id": "req-1", "details": null}})
    );
}

#[sqlx::test]
async fn no_signed_in_staff_gets_401(db: PgPool) {
    let _ = db;
    let (status, body) = call(None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[sqlx::test]
async fn changing_role_permissions_in_data_changes_the_result(db: PgPool) {
    let hr = add_staff(&db, "hr@example.test", &["hr"]).await;
    assert_eq!(
        call(Some(identity(&db, hr).await)).await.0,
        StatusCode::FORBIDDEN
    );

    sqlx::query(
        "INSERT INTO role_permissions (role_key, permission_key) VALUES ('hr', 'driver.approve')",
    )
    .execute(&db)
    .await
    .unwrap();
    assert_eq!(call(Some(identity(&db, hr).await)).await.0, StatusCode::OK);

    sqlx::query(
        "DELETE FROM role_permissions WHERE role_key = 'hr' AND permission_key = 'driver.approve'",
    )
    .execute(&db)
    .await
    .unwrap();
    assert_eq!(
        call(Some(identity(&db, hr).await)).await.0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test]
async fn a_new_role_works_without_code_changes(db: PgPool) {
    sqlx::query("INSERT INTO roles (key, name) VALUES ('approver', 'Approver')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions (role_key, permission_key) VALUES ('approver', 'driver.approve')")
        .execute(&db)
        .await
        .unwrap();
    let id = add_staff(&db, "a@example.test", &["approver"]).await;
    assert_eq!(call(Some(identity(&db, id).await)).await.0, StatusCode::OK);
}

#[sqlx::test]
async fn archived_and_invited_staff_have_no_permissions(db: PgPool) {
    let id = add_staff(&db, "admin@example.test", &["admin"]).await;
    assert!(identity(&db, id).await.has(Permission::DriverApprove));

    sqlx::query("UPDATE staff SET status = 'archived', archived_at = now() WHERE id = $1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let archived = identity(&db, id).await;
    assert_eq!(archived.permissions().count(), 0);
    assert_eq!(call(Some(archived)).await.0, StatusCode::FORBIDDEN);

    let invited: Uuid = sqlx::query_scalar(
        "INSERT INTO staff (email, name) VALUES ('new@example.test', 'New') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO staff_roles (staff_id, role_key) VALUES ($1, 'admin')")
        .bind(invited)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(identity(&db, invited).await.permissions().count(), 0);
}

#[sqlx::test]
async fn unknown_staff_loads_as_none(db: PgPool) {
    assert!(
        StaffIdentity::load(&db, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn staff_emails_are_unique_ignoring_case(db: PgPool) {
    add_staff(&db, "Same@Example.test", &[]).await;
    let dup = sqlx::query("INSERT INTO staff (email, name) VALUES ('same@example.TEST', 'Other')")
        .execute(&db)
        .await;
    assert!(dup.is_err());
}

#[sqlx::test]
async fn seeded_roles_hold_the_drafted_permissions(db: PgPool) {
    let keys = |role: &'static str| {
        let db = db.clone();
        async move {
            let mut k: Vec<String> = sqlx::query_scalar(
                "SELECT permission_key FROM role_permissions WHERE role_key = $1 ORDER BY 1",
            )
            .bind(role)
            .fetch_all(&db)
            .await
            .unwrap();
            k.sort();
            k
        }
    };
    assert_eq!(keys("admin").await.len(), Permission::ALL.len());
    assert_eq!(keys("hr").await, vec!["staff.view"]);
    assert!(
        !keys("operations")
            .await
            .contains(&"payout.approve".to_string())
    );
    assert!(
        keys("finance")
            .await
            .contains(&"payout.approve".to_string())
    );
}

// --- Audit entries ---

async fn audit_count(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(db)
        .await
        .unwrap()
}

fn rename_entry(actor: Uuid, target: Uuid) -> AuditEntry<'static> {
    AuditEntry {
        staff_id: Some(actor),
        action: "staff.edit",
        record_type: "staff",
        record_id: target.to_string(),
        before: Some(json!({"name": "Test Staff"})),
        after: Some(json!({"name": "Renamed"})),
        reason: Some("Name was misspelled".into()),
        request_id: Some("req-42".into()),
    }
}

async fn rename(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, id: Uuid) {
    sqlx::query("UPDATE staff SET name = 'Renamed' WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .unwrap();
}

#[sqlx::test]
async fn committed_change_writes_exactly_one_entry(db: PgPool) {
    let actor = add_staff(&db, "admin@example.test", &["admin"]).await;
    let target = add_staff(&db, "t@example.test", &[]).await;

    let mut tx = db.begin().await.unwrap();
    rename(&mut tx, target).await;
    let entry_id = audit::record(&mut tx, &rename_entry(actor, target))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(audit_count(&db).await, 1);
    let row: (
        Uuid,
        Option<Uuid>,
        String,
        String,
        String,
        Value,
        Value,
        String,
        String,
        bool,
    ) = sqlx::query_as(
        "SELECT id, staff_id, action, record_type, record_id, before, after, reason, request_id,
                    created_at > now() - interval '1 minute'
             FROM audit_entries",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(row.0, entry_id);
    assert_eq!(row.1, Some(actor));
    assert_eq!((row.2.as_str(), row.3.as_str()), ("staff.edit", "staff"));
    assert_eq!(row.4, target.to_string());
    assert_eq!(row.5, json!({"name": "Test Staff"}));
    assert_eq!(row.6, json!({"name": "Renamed"}));
    assert_eq!(row.7, "Name was misspelled");
    assert_eq!(row.8, "req-42");
    assert!(row.9, "created_at is set");
}

#[sqlx::test]
async fn rolled_back_change_leaves_no_entry(db: PgPool) {
    let actor = add_staff(&db, "admin@example.test", &["admin"]).await;
    let target = add_staff(&db, "t@example.test", &[]).await;

    let mut tx = db.begin().await.unwrap();
    rename(&mut tx, target).await;
    audit::record(&mut tx, &rename_entry(actor, target))
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(audit_count(&db).await, 0);
    let name: String = sqlx::query_scalar("SELECT name FROM staff WHERE id = $1")
        .bind(target)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(name, "Test Staff");
}

#[sqlx::test]
async fn failed_change_leaves_no_entry(db: PgPool) {
    let actor = add_staff(&db, "admin@example.test", &["admin"]).await;
    let target = add_staff(&db, "t@example.test", &[]).await;

    let mut tx = db.begin().await.unwrap();
    audit::record(&mut tx, &rename_entry(actor, target))
        .await
        .unwrap();
    // The change breaks a constraint: the transaction is aborted and nothing commits.
    let change = sqlx::query("UPDATE staff SET status = 'gone' WHERE id = $1")
        .bind(target)
        .execute(&mut *tx)
        .await;
    assert!(change.is_err());
    // Committing an aborted transaction rolls it back (Postgres answers ROLLBACK).
    let _ = tx.commit().await;
    assert_eq!(audit_count(&db).await, 0);
}

#[sqlx::test]
async fn failed_entry_leaves_no_change(db: PgPool) {
    let target = add_staff(&db, "t@example.test", &[]).await;

    let mut tx = db.begin().await.unwrap();
    rename(&mut tx, target).await;
    // No such staff member: the entry is refused, so the change must not commit either.
    let written = audit::record(&mut tx, &rename_entry(Uuid::new_v4(), target)).await;
    assert!(written.is_err());
    drop(tx);

    let name: String = sqlx::query_scalar("SELECT name FROM staff WHERE id = $1")
        .bind(target)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(name, "Test Staff");
    assert_eq!(audit_count(&db).await, 0);
}

#[sqlx::test]
async fn audit_entries_cannot_be_changed_or_deleted(db: PgPool) {
    let mut tx = db.begin().await.unwrap();
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: None,
            action: "system.test",
            record_type: "staff",
            record_id: "x".into(),
            before: None,
            after: None,
            reason: None,
            request_id: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert!(
        sqlx::query("UPDATE audit_entries SET reason = 'edited'")
            .execute(&db)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_entries")
            .execute(&db)
            .await
            .is_err()
    );
    assert_eq!(audit_count(&db).await, 1);
}

// --- Permission sync ---

#[sqlx::test]
async fn migrations_and_code_define_the_same_permissions(db: PgPool) {
    let in_db: Vec<String> = sqlx::query_scalar("SELECT key FROM permissions ORDER BY key")
        .fetch_all(&db)
        .await
        .unwrap();
    let in_code: Vec<String> = permissions::all_keys()
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(in_db, in_code);
    assert_eq!(permissions::sync(&db).await.unwrap(), Default::default());
}

#[sqlx::test]
async fn sync_restores_missing_and_removes_unused_stale_keys(db: PgPool) {
    // A key the code defines has gone missing; two keys the code no longer defines exist,
    // one of them still held by a role.
    sqlx::query("DELETE FROM role_permissions WHERE permission_key = 'report.view'")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM permissions WHERE key = 'report.view'")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO permissions (key, description) VALUES ('old.unused', 'x'), ('old.held', 'x')",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_key, permission_key) VALUES ('admin', 'old.held')",
    )
    .execute(&db)
    .await
    .unwrap();

    let report = permissions::sync(&db).await.unwrap();
    assert_eq!(report.inserted, vec!["report.view"]);
    assert_eq!(report.removed, vec!["old.unused"]);
    assert_eq!(report.stale_in_use, vec!["old.held"]);

    let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM permissions ORDER BY key")
        .fetch_all(&db)
        .await
        .unwrap();
    assert!(keys.contains(&"report.view".to_string()));
    assert!(keys.contains(&"old.held".to_string()));
    assert!(!keys.contains(&"old.unused".to_string()));

    // The held stale key gives nothing: the code never checks it.
    let admin = add_staff(&db, "admin@example.test", &["admin"]).await;
    assert_eq!(
        identity(&db, admin).await.permissions().count(),
        Permission::ALL.len() - 1
    );
}
