//! Staff accounts (FT-063, FT-064; FR-019, FR-022): admins invite staff with a role, change
//! roles, archive people who leave and restore them.
//!
//! - `GET /staff` (`staff.view`), `GET /staff/{id}` (`staff.view`)
//! - `POST /staff` (`staff.create`): an `invited` account and a single-use invitation, valid
//!   for `STAFF_INVITE_TTL_HOURS`, delivered by the [`invitation`] sender. The invitation is
//!   an enrolment token: accepting it is `POST /staff/enrolment` then
//!   `POST /staff/sign-in/verify`, which makes the account `active`.
//! - `POST /staff/{id}/invitation` (`staff.create`): a new invitation for an invited account;
//!   earlier ones stop working.
//! - `PATCH /staff/{id}` (`staff.edit_role`): replaces the account's role.
//! - `POST /staff/{id}/archive` (`staff.archive`), `/restore` (`staff.restore`), with a reason.
//!   Archiving revokes every session at once; sign-in refuses archived accounts.
//!
//! Every change writes its audit entry in the same transaction. The last active staff member
//! who can manage roles (holds `staff.edit_role`, i.e. an admin with the drafted roles) cannot
//! be archived or lose that permission (409 `last_admin`). The check is by permission, not by
//! role name (ADR-003), and runs under a transaction-level advisory lock so two concurrent
//! changes cannot both pass it.

pub mod invitation;

use axum::{
    Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgExecutor, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEntry},
    auth::{
        Permission, StaffIdentity,
        admin::{is_email, issue_enrolment_token},
        sign_in::request_id,
    },
    error::{ApiError, ErrorBody, code_for_status},
    state::AppState,
};
use invitation::{Invitation, SendError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/staff", get(list).post(create))
        .route("/staff/{id}", get(show).patch(edit_role))
        .route("/staff/{id}/invitation", post(reissue_invitation))
        .route("/staff/{id}/archive", post(archive))
        .route("/staff/{id}/restore", post(restore))
}

/// Longest email, name and reason accepted.
const EMAIL_MAX_LENGTH: usize = 254;
const NAME_MAX_LENGTH: usize = 200;
const REASON_MAX_LENGTH: usize = 1000;

/// The permission whose last active holder may not be archived or demoted: whoever holds it
/// can give any role, so without one nobody could manage staff any more.
const ROOT_PERMISSION: Permission = Permission::StaffEditRole;

/// Key of the transaction-level advisory lock taken by every change that can remove the
/// last holder of [`ROOT_PERMISSION`] (archive, role change).
const STAFF_ADMIN_LOCK: i64 = 0x5354_4146_4600; // "STAFF\0"

// --- Requests and responses ---

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateStaffRequest {
    #[schema(example = "agent@example.com")]
    pub email: String,
    #[schema(example = "Ada Obi")]
    pub name: String,
    /// Role key, e.g. `operations`, `admin`, `finance`, `hr`.
    #[schema(example = "operations")]
    pub role: String,
    /// Why, recorded in the audit entry. Optional.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EditRoleRequest {
    /// The new role key; replaces the account's current roles.
    #[schema(example = "finance")]
    pub role: String,
    /// Why, recorded in the audit entry. Optional.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReasonRequest {
    /// Why, recorded in the audit entry. Required, at most 1000 characters.
    #[schema(example = "Left the company on 2026-11-30")]
    pub reason: String,
}

/// A staff account as admins see it. Never contains a password, secret or token.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct StaffMember {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// `invited`, `active` or `archived`.
    #[schema(example = "active")]
    pub status: String,
    /// Role keys, e.g. `operations`.
    #[schema(example = json!(["operations"]))]
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    /// When the account was archived; null unless `archived`.
    pub archived_at: Option<DateTime<Utc>>,
    /// When the newest unused invitation of an `invited` account expires; null otherwise or
    /// when every invitation has expired (reissue one with `POST /staff/{id}/invitation`).
    pub invitation_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StaffList {
    /// Every staff account, archived ones included, by name.
    pub staff: Vec<StaffMember>,
}

// --- Errors ---

fn invalid(field: &'static str, problem: &'static str, message: &str) -> ApiError {
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request", message)
        .with_details(json!({"fields": {field: problem}}))
}

fn conflict(code: &'static str, message: &str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, code, message)
}

fn email_taken() -> ApiError {
    conflict(
        "email_taken",
        "A staff member with this email already exists.",
    )
}

fn delivery_error(e: SendError) -> ApiError {
    match e {
        SendError::Unavailable => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "invitation_delivery_unavailable",
            "Invitations can't be sent yet: no email provider is configured. Nothing was changed.",
        ),
        SendError::Failed(cause) => {
            tracing::error!(error = %cause, "invitation delivery failed");
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "invitation_delivery_failed",
                "The invitation could not be sent. Nothing was changed; please try again.",
            )
        }
    }
}

// Handlers take their body and path as `Result`s so the permission check comes first and a
// forbidden caller learns nothing from validation errors; these turn a rejection into the
// standard error afterwards.

fn body<T>(r: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    r.map(|Json(v)| v).map_err(|e| {
        let status = e.status();
        ApiError::new(status, code_for_status(status), e.body_text())
    })
}

fn path_id(r: Result<Path<Uuid>, PathRejection>) -> Result<Uuid, ApiError> {
    r.map(|Path(id)| id).map_err(|e| {
        let status = e.status();
        ApiError::new(status, code_for_status(status), e.body_text())
    })
}

fn required_reason(reason: &str) -> Result<String, ApiError> {
    match optional_reason(Some(reason))? {
        Some(r) => Ok(r),
        None => Err(invalid(
            "reason",
            "required",
            "Give a reason; it is recorded in the audit log.",
        )),
    }
}

fn optional_reason(reason: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(reason) = reason.map(str::trim).filter(|r| !r.is_empty()) else {
        return Ok(None);
    };
    if reason.chars().count() > REASON_MAX_LENGTH {
        return Err(invalid(
            "reason",
            "length",
            "The reason must have at most 1000 characters.",
        ));
    }
    Ok(Some(reason.to_string()))
}

// --- Queries ---

/// The columns of [`StaffMember`]; a literal, so queries are built with `concat!`.
macro_rules! member_select {
    () => {
        "SELECT s.id, s.email, s.name, s.status, s.created_at, s.archived_at,
            COALESCE(array_agg(sr.role_key ORDER BY sr.role_key)
                     FILTER (WHERE sr.role_key IS NOT NULL), '{}') AS roles,
            CASE WHEN s.status = 'invited' THEN
                (SELECT max(t.expires_at) FROM staff_enrolment_tokens t
                 WHERE t.staff_id = s.id AND t.used_at IS NULL AND t.expires_at > now())
            END AS invitation_expires_at
         FROM staff s LEFT JOIN staff_roles sr ON sr.staff_id = s.id"
    };
}

async fn load_member<'e>(
    db: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<StaffMember>, sqlx::Error> {
    sqlx::query_as(concat!(member_select!(), " WHERE s.id = $1 GROUP BY s.id"))
        .bind(id)
        .fetch_optional(db)
        .await
}

async fn role_exists(tx: &mut Transaction<'_, Postgres>, role: &str) -> Result<bool, ApiError> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roles WHERE key = $1)")
            .bind(role)
            .fetch_one(&mut **tx)
            .await?,
    )
}

fn unknown_role() -> ApiError {
    invalid(
        "role",
        "unknown",
        "No such role. Use a role key such as operations, admin, finance or hr.",
    )
}

/// Takes the advisory lock that serialises every change able to remove the last holder of
/// [`ROOT_PERMISSION`]; released at commit or rollback.
async fn lock_root_holders(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(STAFF_ADMIN_LOCK)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Whether active staff other than `except` hold `permission` through their roles.
async fn others_hold(
    tx: &mut Transaction<'_, Postgres>,
    except: Uuid,
    permission: Permission,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM staff s
             JOIN staff_roles sr ON sr.staff_id = s.id
             JOIN role_permissions rp ON rp.role_key = sr.role_key
             WHERE rp.permission_key = $2 AND s.status = 'active' AND s.id <> $1)",
    )
    .bind(except)
    .bind(permission.key())
    .fetch_one(&mut **tx)
    .await
}

/// Whether `staff_id`'s current roles give `permission` (whatever their status).
async fn roles_give(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    permission: Permission,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM staff_roles sr JOIN role_permissions rp ON rp.role_key = sr.role_key
             WHERE sr.staff_id = $1 AND rp.permission_key = $2)",
    )
    .bind(staff_id)
    .bind(permission.key())
    .fetch_one(&mut **tx)
    .await
}

fn last_admin() -> ApiError {
    conflict(
        "last_admin",
        "This is the last active staff member who can manage staff roles. Give another active staff member that role first.",
    )
}

/// Locks the staff row; its status, email and name. 404 when there is none.
async fn lock_staff(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(String, String, String), ApiError> {
    sqlx::query_as("SELECT status, email, name FROM staff WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// Ends every unused enrolment token (invitation) of `staff_id`.
async fn expire_invitations(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE staff_enrolment_tokens SET expires_at = now()
         WHERE staff_id = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(staff_id)
    .execute(&mut **tx)
    .await?
    .rows_affected())
}

fn member_or_internal(m: Option<StaffMember>) -> Result<StaffMember, ApiError> {
    m.ok_or_else(|| ApiError::internal("staff member vanished inside its own transaction"))
}

fn audit_entry<'a>(
    actor: &StaffIdentity,
    action: &'a str,
    target: Uuid,
    before: Option<Value>,
    after: Value,
    reason: Option<String>,
    headers: &HeaderMap,
) -> AuditEntry<'a> {
    AuditEntry {
        staff_id: Some(actor.staff_id),
        action,
        record_type: "staff",
        record_id: target.to_string(),
        before,
        after: Some(after),
        reason,
        request_id: request_id(headers),
    }
}

// --- Handlers ---

/// `GET /staff`
#[utoipa::path(
    get,
    path = "/staff",
    tag = "staff",
    operation_id = "staff_list",
    summary = "List staff accounts with their roles and status",
    description = "Every staff account, archived ones included, ordered by name. Not paginated: a staff list stays in the tens. Needs `staff.view`.",
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "The staff accounts.", body = StaffList),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.view`.", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    staff: StaffIdentity,
) -> Result<Json<StaffList>, ApiError> {
    staff.require(Permission::StaffView)?;
    let members: Vec<StaffMember> = sqlx::query_as(concat!(
        member_select!(),
        " GROUP BY s.id ORDER BY lower(s.name), s.id"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(StaffList { staff: members }))
}

/// `GET /staff/{id}`
#[utoipa::path(
    get,
    path = "/staff/{id}",
    tag = "staff",
    operation_id = "staff_get",
    summary = "One staff account with its roles and status",
    description = "Needs `staff.view`.",
    params(("id" = Uuid, Path, description = "The staff account's id.")),
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "The staff account.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.view`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no such staff account.", body = ErrorBody),
    )
)]
pub async fn show(
    State(state): State<AppState>,
    staff: StaffIdentity,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<StaffMember>, ApiError> {
    staff.require(Permission::StaffView)?;
    let id = path_id(id)?;
    load_member(&state.db, id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

/// `POST /staff`
#[utoipa::path(
    post,
    path = "/staff",
    tag = "staff",
    operation_id = "staff_create",
    summary = "Invite a staff member with a role",
    description = "Creates an `invited` account with the role and a single-use invitation valid for `STAFF_INVITE_TTL_HOURS` (default 168, 7 days), delivered by the configured sender. The response never contains the invitation token. The invited person accepts with `POST /staff/enrolment` (the token and a new password) and `POST /staff/sign-in/verify` (the first code), which makes the account `active`. Where no email provider is configured (every environment but development, for now) nothing is created and the answer is 503 `invitation_delivery_unavailable`. Needs `staff.create`.",
    request_body = CreateStaffRequest,
    security(("staff_session" = [])),
    responses(
        (status = 201, description = "Invited.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.create`.", body = ErrorBody),
        (status = 409, description = "`email_taken`: a staff member with this email exists (archived ones included).", body = ErrorBody),
        (status = 422, description = "`invalid_request`: bad email, name, role or reason (`details.fields`).", body = ErrorBody),
        (status = 503, description = "`invitation_delivery_unavailable` or `invitation_delivery_failed`: the invitation could not be sent; nothing was created.", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    staff: StaffIdentity,
    headers: HeaderMap,
    req: Result<Json<CreateStaffRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<StaffMember>), ApiError> {
    staff.require(Permission::StaffCreate)?;
    let req = body(req)?;
    let email = req.email.trim();
    let name = req.name.trim();
    let role = req.role.trim();
    if !is_email(email) || email.len() > EMAIL_MAX_LENGTH {
        return Err(invalid("email", "format", "Enter a valid email address."));
    }
    if name.is_empty() || name.chars().count() > NAME_MAX_LENGTH {
        return Err(invalid(
            "name",
            "length",
            "The name must have between 1 and 200 characters.",
        ));
    }
    let reason = optional_reason(req.reason.as_deref())?;

    let mut tx = state.db.begin().await?;
    if !role_exists(&mut tx, role).await? {
        return Err(unknown_role());
    }
    let id: Uuid = match sqlx::query_scalar(
        "INSERT INTO staff (email, name, status) VALUES ($1, $2, 'invited') RETURNING id",
    )
    .bind(email)
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => return Err(email_taken()),
        Err(e) => return Err(e.into()),
    };
    sqlx::query("INSERT INTO staff_roles (staff_id, role_key) VALUES ($1, $2)")
        .bind(id)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    let (token, expires_at) =
        issue_enrolment_token(&mut tx, id, state.config.staff_auth.invite_ttl).await?;
    audit::record(
        &mut tx,
        &audit_entry(
            &staff,
            "staff.create",
            id,
            None,
            json!({"email": email, "name": name, "status": "invited", "roles": [role],
                   "invitation_expires_at": expires_at}),
            reason,
            &headers,
        ),
    )
    .await?;
    let member = member_or_internal(load_member(&mut *tx, id).await?)?;
    let invitation = Invitation {
        staff_id: id,
        email: email.to_string(),
        name: name.to_string(),
        token,
        expires_at,
    };
    // Before commit: when delivery fails, the account is not created.
    state
        .invitations
        .send(&invitation)
        .await
        .map_err(delivery_error)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(member)))
}

/// `POST /staff/{id}/invitation`
#[utoipa::path(
    post,
    path = "/staff/{id}/invitation",
    tag = "staff",
    operation_id = "staff_reissue_invitation",
    summary = "Send a new invitation to an invited staff member",
    description = "For an `invited` account whose invitation expired or was lost: sends a new one, valid for `STAFF_INVITE_TTL_HOURS`; earlier invitations stop working. The response never contains the token. Needs `staff.create`.",
    params(("id" = Uuid, Path, description = "The staff account's id.")),
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "A new invitation was sent.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.create`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no such staff account.", body = ErrorBody),
        (status = 409, description = "`not_invited`: the account is active or archived.", body = ErrorBody),
        (status = 503, description = "`invitation_delivery_unavailable` or `invitation_delivery_failed`: nothing was changed.", body = ErrorBody),
    )
)]
pub async fn reissue_invitation(
    State(state): State<AppState>,
    staff: StaffIdentity,
    headers: HeaderMap,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<StaffMember>, ApiError> {
    staff.require(Permission::StaffCreate)?;
    let id = path_id(id)?;
    let mut tx = state.db.begin().await?;
    let (status, email, name) = lock_staff(&mut tx, id).await?;
    if status != "invited" {
        return Err(conflict(
            "not_invited",
            "Only invited staff can get a new invitation.",
        ));
    }
    let expired = expire_invitations(&mut tx, id).await?;
    let (token, expires_at) =
        issue_enrolment_token(&mut tx, id, state.config.staff_auth.invite_ttl).await?;
    audit::record(
        &mut tx,
        &audit_entry(
            &staff,
            "staff.invitation_reissue",
            id,
            None,
            json!({"invitation_expires_at": expires_at, "earlier_invitations_ended": expired}),
            None,
            &headers,
        ),
    )
    .await?;
    let member = member_or_internal(load_member(&mut *tx, id).await?)?;
    let invitation = Invitation {
        staff_id: id,
        email,
        name,
        token,
        expires_at,
    };
    state
        .invitations
        .send(&invitation)
        .await
        .map_err(delivery_error)?;
    tx.commit().await?;
    Ok(Json(member))
}

/// `PATCH /staff/{id}`
#[utoipa::path(
    patch,
    path = "/staff/{id}",
    tag = "staff",
    operation_id = "staff_edit_role",
    summary = "Change a staff member's role",
    description = "Replaces the account's roles with `role`. Takes effect on the person's next request (permissions are read per request); their sessions stay open. Setting the role they already have changes nothing and writes no audit entry. Refused with `last_admin` when it would leave no active staff member able to manage roles (`staff.edit_role`). Needs `staff.edit_role`.",
    params(("id" = Uuid, Path, description = "The staff account's id.")),
    request_body = EditRoleRequest,
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "The account with its new role.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.edit_role`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no such staff account.", body = ErrorBody),
        (status = 409, description = "`last_admin`: the last active staff member able to manage roles would lose that permission.", body = ErrorBody),
        (status = 422, description = "`invalid_request`: unknown role or bad reason (`details.fields`).", body = ErrorBody),
    )
)]
pub async fn edit_role(
    State(state): State<AppState>,
    staff: StaffIdentity,
    headers: HeaderMap,
    id: Result<Path<Uuid>, PathRejection>,
    req: Result<Json<EditRoleRequest>, JsonRejection>,
) -> Result<Json<StaffMember>, ApiError> {
    staff.require(Permission::StaffEditRole)?;
    let id = path_id(id)?;
    let req = body(req)?;
    let role = req.role.trim();
    let reason = optional_reason(req.reason.as_deref())?;

    let mut tx = state.db.begin().await?;
    lock_root_holders(&mut tx).await?;
    let (status, _, _) = lock_staff(&mut tx, id).await?;
    if !role_exists(&mut tx, role).await? {
        return Err(unknown_role());
    }
    let before: Vec<String> = sqlx::query_scalar(
        "SELECT role_key FROM staff_roles WHERE staff_id = $1 ORDER BY role_key",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    if before == [role] {
        let member = member_or_internal(load_member(&mut *tx, id).await?)?;
        return Ok(Json(member));
    }
    if status == "active" && roles_give(&mut tx, id, ROOT_PERMISSION).await? {
        let new_role_gives: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM role_permissions WHERE role_key = $1 AND permission_key = $2)",
        )
        .bind(role)
        .bind(ROOT_PERMISSION.key())
        .fetch_one(&mut *tx)
        .await?;
        if !new_role_gives && !others_hold(&mut tx, id, ROOT_PERMISSION).await? {
            return Err(last_admin());
        }
    }
    sqlx::query("DELETE FROM staff_roles WHERE staff_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO staff_roles (staff_id, role_key) VALUES ($1, $2)")
        .bind(id)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        &audit_entry(
            &staff,
            "staff.edit_role",
            id,
            Some(json!({"roles": before})),
            json!({"roles": [role]}),
            reason,
            &headers,
        ),
    )
    .await?;
    let member = member_or_internal(load_member(&mut *tx, id).await?)?;
    tx.commit().await?;
    Ok(Json(member))
}

/// `POST /staff/{id}/archive`
#[utoipa::path(
    post,
    path = "/staff/{id}/archive",
    tag = "staff",
    operation_id = "staff_archive",
    summary = "Archive a staff member who has left",
    description = "Revokes every session of the person at once and ends any unused invitation; they cannot sign in or enrol until restored. Their past audit entries stay. Staff cannot archive themselves (`cannot_archive_self`), and the last active staff member able to manage roles cannot be archived (`last_admin`). Needs `staff.archive`.",
    params(("id" = Uuid, Path, description = "The staff account's id.")),
    request_body = ReasonRequest,
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "Archived.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.archive`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no such staff account.", body = ErrorBody),
        (status = 409, description = "`cannot_archive_self`, `last_admin`, or `already_archived`.", body = ErrorBody),
        (status = 422, description = "`invalid_request`: the reason is missing or too long.", body = ErrorBody),
    )
)]
pub async fn archive(
    State(state): State<AppState>,
    staff: StaffIdentity,
    headers: HeaderMap,
    id: Result<Path<Uuid>, PathRejection>,
    req: Result<Json<ReasonRequest>, JsonRejection>,
) -> Result<Json<StaffMember>, ApiError> {
    staff.require(Permission::StaffArchive)?;
    let id = path_id(id)?;
    let req = body(req)?;
    let reason = required_reason(&req.reason)?;
    if id == staff.staff_id {
        return Err(conflict(
            "cannot_archive_self",
            "You can't archive your own account. Ask another admin.",
        ));
    }

    let mut tx = state.db.begin().await?;
    lock_root_holders(&mut tx).await?;
    let (status, _, _) = lock_staff(&mut tx, id).await?;
    if status == "archived" {
        return Err(conflict(
            "already_archived",
            "This staff member is already archived.",
        ));
    }
    if status == "active"
        && roles_give(&mut tx, id, ROOT_PERMISSION).await?
        && !others_hold(&mut tx, id, ROOT_PERMISSION).await?
    {
        return Err(last_admin());
    }
    sqlx::query("UPDATE staff SET status = 'archived', archived_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let sessions_revoked = sqlx::query(
        "UPDATE staff_sessions SET revoked_at = now() WHERE staff_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let invitations_ended = expire_invitations(&mut tx, id).await?;
    sqlx::query(
        "UPDATE staff_sign_in_challenges SET used_at = now() WHERE staff_id = $1 AND used_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &audit_entry(
            &staff,
            "staff.archive",
            id,
            Some(json!({"status": status})),
            json!({"status": "archived", "sessions_revoked": sessions_revoked,
                   "invitations_ended": invitations_ended}),
            Some(reason),
            &headers,
        ),
    )
    .await?;
    let member = member_or_internal(load_member(&mut *tx, id).await?)?;
    tx.commit().await?;
    Ok(Json(member))
}

/// `POST /staff/{id}/restore`
#[utoipa::path(
    post,
    path = "/staff/{id}/restore",
    tag = "staff",
    operation_id = "staff_restore",
    summary = "Restore an archived staff member",
    description = "A person who had finished enrolment becomes `active` again and signs in with their existing password and authenticator; sessions revoked by archiving stay revoked. A person who had not finished enrolment becomes `invited` and needs a new invitation (`POST /staff/{id}/invitation`). Needs `staff.restore`.",
    params(("id" = Uuid, Path, description = "The staff account's id.")),
    request_body = ReasonRequest,
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "Restored.", body = StaffMember),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
        (status = 403, description = "`forbidden`: lacks `staff.restore`.", body = ErrorBody),
        (status = 404, description = "`not_found`: no such staff account.", body = ErrorBody),
        (status = 409, description = "`not_archived`: the account is not archived.", body = ErrorBody),
        (status = 422, description = "`invalid_request`: the reason is missing or too long.", body = ErrorBody),
    )
)]
pub async fn restore(
    State(state): State<AppState>,
    staff: StaffIdentity,
    headers: HeaderMap,
    id: Result<Path<Uuid>, PathRejection>,
    req: Result<Json<ReasonRequest>, JsonRejection>,
) -> Result<Json<StaffMember>, ApiError> {
    staff.require(Permission::StaffRestore)?;
    let id = path_id(id)?;
    let req = body(req)?;
    let reason = required_reason(&req.reason)?;

    let mut tx = state.db.begin().await?;
    let (status, _, _) = lock_staff(&mut tx, id).await?;
    if status != "archived" {
        return Err(conflict(
            "not_archived",
            "This staff member is not archived.",
        ));
    }
    let new_status: String = sqlx::query_scalar(
        "UPDATE staff SET archived_at = NULL,
                status = CASE WHEN password_hash IS NOT NULL AND totp_confirmed_at IS NOT NULL
                              THEN 'active' ELSE 'invited' END
         WHERE id = $1 RETURNING status",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &audit_entry(
            &staff,
            "staff.restore",
            id,
            Some(json!({"status": "archived"})),
            json!({"status": new_status}),
            Some(reason),
            &headers,
        ),
    )
    .await?;
    let member = member_or_internal(load_member(&mut *tx, id).await?)?;
    tx.commit().await?;
    Ok(Json(member))
}
