//! Admins invite, change the role of, archive and restore staff (TASK-006; FT-063, FT-064,
//! FT-049; FR-019, FR-022; TC-010).
//! database: every test runs in its own throwaway database with the migrations applied
//! (#[sqlx::test]); Postgres from docker-compose.yml (DATABASE_URL via .cargo/config.toml).

use std::sync::{Arc, Mutex};

use api::{
    auth::{admin, secrets},
    config::Config,
    staff::invitation::{Invitation, InvitationSender, SendFuture, UnavailableSender},
    state::AppState,
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";

fn config() -> Config {
    Config::from_lookup(|name| match name {
        "API_ENV" => Some("development".into()),
        _ => None,
    })
    .unwrap()
}

/// Collects the invitations "sent", so tests can accept them.
#[derive(Default)]
struct Outbox(Mutex<Vec<(Uuid, String)>>);

impl InvitationSender for Outbox {
    fn send<'a>(&'a self, invitation: &'a Invitation) -> SendFuture<'a> {
        self.0
            .lock()
            .unwrap()
            .push((invitation.staff_id, invitation.token.clone()));
        Box::pin(async { Ok(()) })
    }
}

impl Outbox {
    /// The newest invitation token sent to `staff_id`.
    fn token_for(&self, staff_id: Uuid) -> String {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(id, _)| *id == staff_id)
            .map(|(_, t)| t.clone())
            .expect("an invitation was sent")
    }
}

struct Ctx {
    db: PgPool,
    outbox: Arc<Outbox>,
    sender: Arc<dyn InvitationSender>,
}

struct Res {
    status: StatusCode,
    body: Value,
    raw: String,
}

impl Ctx {
    fn new(db: &PgPool) -> Self {
        let outbox = Arc::new(Outbox::default());
        Self {
            db: db.clone(),
            sender: outbox.clone(),
            outbox,
        }
    }

    fn without_email_provider(db: &PgPool) -> Self {
        Self {
            db: db.clone(),
            outbox: Arc::new(Outbox::default()),
            sender: Arc::new(UnavailableSender),
        }
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        bearer: Option<&str>,
    ) -> Res {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("x-request-id", "req-staff");
        if let Some(t) = bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let mut state = AppState::new(self.db.clone(), config());
        state.invitations = self.sender.clone();
        let res = api::router(state).oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        Res {
            status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            raw,
        }
    }

    async fn post(&self, path: &str, body: Value, bearer: Option<&str>) -> Res {
        self.call(Method::POST, path, Some(body), bearer).await
    }

    async fn get(&self, path: &str, bearer: &str) -> Res {
        self.call(Method::GET, path, None, Some(bearer)).await
    }
}

fn code(secret_base32: &str, email: &str) -> String {
    let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
    let totp = secrets::totp(secret.as_ref().to_vec(), email).unwrap();
    totp.generate(secrets::unix_now()).to_string()
}

fn error_code(res: &Res) -> &str {
    res.body["error"]["code"].as_str().unwrap_or("")
}

/// A signed-in staff member.
struct Person {
    id: Uuid,
    email: String,
    secret: String,
    token: String,
}

/// `POST /staff/enrolment` then `POST /staff/sign-in/verify` with the token.
async fn accept(ctx: &Ctx, id: Uuid, email: &str, token: &str) -> Person {
    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": token, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let secret = res.body["totp"]["secret"].as_str().unwrap().to_string();
    let res = ctx
        .post(
            "/staff/sign-in/verify",
            json!({"challenge": res.body["challenge"], "code": code(&secret, email)}),
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    Person {
        id,
        email: email.into(),
        secret,
        token: res.body["token"].as_str().unwrap().into(),
    }
}

/// `api create-admin`, enrolled and signed in.
async fn first_admin(ctx: &Ctx, email: &str) -> Person {
    let created = admin::create_admin(&ctx.db, &config().staff_auth, email, "Admin")
        .await
        .unwrap();
    accept(ctx, created.staff_id, email, &created.enrolment_token).await
}

async fn invite(ctx: &Ctx, by: &Person, email: &str, role: &str) -> Uuid {
    let res = ctx
        .post(
            "/staff",
            json!({"email": email, "name": "New Staff", "role": role, "reason": "Hired"}),
            Some(&by.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.body);
    res.body["id"].as_str().unwrap().parse().unwrap()
}

/// Invited by `by` and accepted.
async fn staff_member(ctx: &Ctx, by: &Person, email: &str, role: &str) -> Person {
    let id = invite(ctx, by, email, role).await;
    let token = ctx.outbox.token_for(id);
    accept(ctx, id, email, &token).await
}

/// Signs in again with password and code (the last code is forgotten first, since each
/// 30-second code is accepted once).
async fn sign_in(ctx: &Ctx, p: &Person) -> Res {
    sqlx::query("UPDATE staff SET totp_last_used_step = NULL WHERE id = $1")
        .bind(p.id)
        .execute(&ctx.db)
        .await
        .unwrap();
    let res = ctx
        .post(
            "/staff/sign-in",
            json!({"email": p.email, "password": PASSWORD}),
            None,
        )
        .await;
    if res.status != StatusCode::OK {
        return res;
    }
    ctx.post(
        "/staff/sign-in/verify",
        json!({"challenge": res.body["challenge"], "code": code(&p.secret, &p.email)}),
        None,
    )
    .await
}

async fn archive(ctx: &Ctx, by: &Person, id: Uuid) -> Res {
    ctx.post(
        &format!("/staff/{id}/archive"),
        json!({"reason": "Left the company"}),
        Some(&by.token),
    )
    .await
}

/// (action, actor, reason, before, after) of the audit entries about staff record `id`.
async fn audit_of(
    db: &PgPool,
    id: Uuid,
) -> Vec<(
    String,
    Option<Uuid>,
    Option<String>,
    Option<Value>,
    Option<Value>,
)> {
    sqlx::query_as(
        "SELECT action, staff_id, reason, before, after FROM audit_entries
         WHERE record_type = 'staff' AND record_id = $1 ORDER BY created_at, action",
    )
    .bind(id.to_string())
    .fetch_all(db)
    .await
    .unwrap()
}

// --- Invite and accept ---

#[sqlx::test]
async fn an_invited_operations_agent_accepts_signs_in_and_has_only_operations_permissions(
    db: PgPool,
) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;

    let res = ctx
        .post(
            "/staff",
            json!({"email": "agent@example.test", "name": "Ada Agent", "role": "operations"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.body);
    assert_eq!(res.body["status"], "invited");
    assert_eq!(res.body["roles"], json!(["operations"]));
    assert!(res.body["invitation_expires_at"].is_string());
    let id: Uuid = res.body["id"].as_str().unwrap().parse().unwrap();
    let token = ctx.outbox.token_for(id);
    // The response never carries the invitation token.
    assert!(!res.raw.contains(&token));

    // Invited staff cannot sign in before accepting.
    let res = ctx
        .post(
            "/staff/sign-in",
            json!({"email": "agent@example.test", "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "invalid_credentials");

    let agent = accept(&ctx, id, "agent@example.test", &token).await;
    let me = ctx.get("/staff/me", &agent.token).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert_eq!(me.body["status"], "active");
    assert_eq!(me.body["roles"], json!(["operations"]));
    let mut expected: Vec<String> = sqlx::query_scalar(
        "SELECT permission_key FROM role_permissions WHERE role_key = 'operations'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    expected.sort();
    let mut got: Vec<String> = me.body["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect();
    got.sort();
    assert_eq!(got, expected);
    assert!(!got.iter().any(|p| p.starts_with("staff.")));

    // A staff endpoint the agent lacks the permission for.
    let res = ctx.get("/staff", &agent.token).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&res), "forbidden");

    // A later sign-in works as for anyone else.
    assert_eq!(sign_in(&ctx, &agent).await.status, StatusCode::OK);

    // The listing shows the agent as active with their role.
    let res = ctx.get("/staff", &admin.token).await;
    assert_eq!(res.status, StatusCode::OK);
    let listed = res.body["staff"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id.to_string())
        .unwrap()
        .clone();
    assert_eq!(listed["status"], "active");
    assert_eq!(listed["roles"], json!(["operations"]));
    assert_eq!(listed["invitation_expires_at"], Value::Null);
    assert!(!res.raw.contains("password") && !res.raw.contains("totp"));

    let res = ctx.get(&format!("/staff/{id}"), &admin.token).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["email"], "agent@example.test");
    let res = ctx
        .get(&format!("/staff/{}", Uuid::new_v4()), &admin.token)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn a_used_invitation_is_refused(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let id = invite(&ctx, &admin, "agent@example.test", "operations").await;
    let token = ctx.outbox.token_for(id);
    accept(&ctx, id, "agent@example.test", &token).await;

    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": token, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "invalid_enrolment_token");
}

#[sqlx::test]
async fn an_expired_invitation_is_refused_and_a_reissued_one_replaces_it(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let id = invite(&ctx, &admin, "agent@example.test", "operations").await;
    let expires: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT expires_at FROM staff_enrolment_tokens WHERE staff_id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    let ttl = expires - chrono::Utc::now();
    assert!(ttl > chrono::Duration::days(7) - chrono::Duration::minutes(1));
    assert!(ttl <= chrono::Duration::days(7));

    let first = ctx.outbox.token_for(id);
    sqlx::query("UPDATE staff_enrolment_tokens SET expires_at = now() - interval '1 second' WHERE staff_id = $1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": first, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "invalid_enrolment_token");
    let res = ctx.get(&format!("/staff/{id}"), &admin.token).await;
    assert_eq!(res.body["invitation_expires_at"], Value::Null);

    // A new invitation; an earlier unexpired one stops working.
    let res = ctx
        .post(
            &format!("/staff/{id}/invitation"),
            json!({}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert!(res.body["invitation_expires_at"].is_string());
    let second = ctx.outbox.token_for(id);
    assert!(!res.raw.contains(&second));
    let res = ctx
        .post(
            &format!("/staff/{id}/invitation"),
            json!({}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let third = ctx.outbox.token_for(id);
    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": second, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(error_code(&res), "invalid_enrolment_token");
    accept(&ctx, id, "agent@example.test", &third).await;

    // Only invited staff get new invitations.
    let res = ctx
        .post(
            &format!("/staff/{id}/invitation"),
            json!({}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(error_code(&res), "not_invited");
}

#[sqlx::test]
async fn invalid_invitations_are_refused_and_create_nothing(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    for (body, field) in [
        (
            json!({"email": "not-an-email", "name": "X", "role": "operations"}),
            "email",
        ),
        (
            json!({"email": "x@example.test", "name": "  ", "role": "operations"}),
            "name",
        ),
        (
            json!({"email": "x@example.test", "name": "X", "role": "superuser"}),
            "role",
        ),
    ] {
        let res = ctx.post("/staff", body, Some(&admin.token)).await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.body);
        assert_eq!(error_code(&res), "invalid_request");
        assert!(res.body["error"]["details"]["fields"][field].is_string());
    }
    let res = ctx
        .post(
            "/staff",
            json!({"email": "x@example.test"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    let res = ctx
        .post(
            "/staff",
            json!({"email": "ADMIN@example.test", "name": "X", "role": "operations"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(error_code(&res), "email_taken");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM staff")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn without_an_email_provider_inviting_is_refused_and_creates_nothing(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let no_email = Ctx::without_email_provider(&db);
    let res = no_email
        .post(
            "/staff",
            json!({"email": "agent@example.test", "name": "A", "role": "operations"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error_code(&res), "invitation_delivery_unavailable");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM staff")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let creates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_entries WHERE action = 'staff.create' AND staff_id IS NOT NULL",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(creates, 0);
}

// --- Role change ---

#[sqlx::test]
async fn an_admin_changes_a_role_and_it_applies_at_once(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let agent = staff_member(&ctx, &admin, "agent@example.test", "operations").await;
    let path = format!("/staff/{}", agent.id);

    let res = ctx
        .call(
            Method::PATCH,
            &path,
            Some(json!({"role": "finance", "reason": "Moved to finance"})),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["roles"], json!(["finance"]));

    // The agent's open session has the new permissions on its next request.
    let me = ctx.get("/staff/me", &agent.token).await;
    assert_eq!(me.body["roles"], json!(["finance"]));
    let perms = me.body["permissions"].as_array().unwrap();
    assert!(perms.contains(&json!("payout.approve")));
    assert!(!perms.contains(&json!("driver.approve")));

    // The same role again changes nothing and writes no entry.
    let res = ctx
        .call(
            Method::PATCH,
            &path,
            Some(json!({"role": "finance"})),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let res = ctx
        .call(
            Method::PATCH,
            &path,
            Some(json!({"role": "nope"})),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    let entries: Vec<_> = audit_of(&db, agent.id)
        .await
        .into_iter()
        .filter(|e| e.0 == "staff.edit_role")
        .collect();
    assert_eq!(entries.len(), 1);
    let (_, actor, reason, before, after) = &entries[0];
    assert_eq!(*actor, Some(admin.id));
    assert_eq!(reason.as_deref(), Some("Moved to finance"));
    assert_eq!(before.as_ref().unwrap()["roles"], json!(["operations"]));
    assert_eq!(after.as_ref().unwrap()["roles"], json!(["finance"]));
}

#[sqlx::test]
async fn the_last_admin_cannot_give_up_managing_roles(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let res = ctx
        .call(
            Method::PATCH,
            &format!("/staff/{}", admin.id),
            Some(json!({"role": "operations"})),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(error_code(&res), "last_admin");

    // With a second admin it is allowed.
    staff_member(&ctx, &admin, "admin2@example.test", "admin").await;
    let res = ctx
        .call(
            Method::PATCH,
            &format!("/staff/{}", admin.id),
            Some(json!({"role": "operations"})),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
}

// --- Archive and restore ---

#[sqlx::test]
async fn archiving_signs_the_person_out_at_once_and_restoring_lets_them_back_in(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let agent = staff_member(&ctx, &admin, "agent@example.test", "operations").await;
    let second = sign_in(&ctx, &agent).await;
    assert_eq!(second.status, StatusCode::OK);
    let second_token = second.body["token"].as_str().unwrap().to_string();
    let entries_by_agent_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_entries WHERE staff_id = $1")
            .bind(agent.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(entries_by_agent_before > 0);

    let res = archive(&ctx, &admin, agent.id).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["status"], "archived");
    assert!(res.body["archived_at"].is_string());

    // Every session is refused at once, and sign-in is refused.
    for token in [&agent.token, &second_token] {
        let res = ctx.get("/staff/me", token).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_sessions WHERE staff_id = $1 AND revoked_at IS NULL",
    )
    .bind(agent.id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(open, 0);
    let res = sign_in(&ctx, &agent).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "invalid_credentials");

    // Their past audit entries stay.
    let entries_by_agent_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_entries WHERE staff_id = $1")
            .bind(agent.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(entries_by_agent_after, entries_by_agent_before);

    let res = archive(&ctx, &admin, agent.id).await;
    assert_eq!(error_code(&res), "already_archived");

    let res = ctx
        .post(
            &format!("/staff/{}/restore", agent.id),
            json!({"reason": "Rehired"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["status"], "active");
    assert_eq!(res.body["archived_at"], Value::Null);

    // Old sessions stay revoked; signing in again works with the existing credentials.
    assert_eq!(
        ctx.get("/staff/me", &agent.token).await.status,
        StatusCode::UNAUTHORIZED
    );
    let res = sign_in(&ctx, &agent).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let token = res.body["token"].as_str().unwrap();
    assert_eq!(ctx.get("/staff/me", token).await.status, StatusCode::OK);

    let res = ctx
        .post(
            &format!("/staff/{}/restore", agent.id),
            json!({"reason": "Again"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(error_code(&res), "not_archived");
}

#[sqlx::test]
async fn an_archived_invitee_cannot_enrol_and_is_restored_as_invited(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let id = invite(&ctx, &admin, "agent@example.test", "operations").await;
    let token = ctx.outbox.token_for(id);
    let res = archive(&ctx, &admin, id).await;
    assert_eq!(res.status, StatusCode::OK);
    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": token, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(error_code(&res), "invalid_enrolment_token");

    let res = ctx
        .post(
            &format!("/staff/{id}/restore"),
            json!({"reason": "Archived by mistake"}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.body["status"], "invited");
    // The old invitation stays ended; a new one works.
    let res = ctx
        .post(
            "/staff/enrolment",
            json!({"token": token, "password": PASSWORD}),
            None,
        )
        .await;
    assert_eq!(error_code(&res), "invalid_enrolment_token");
    let res = ctx
        .post(
            &format!("/staff/{id}/invitation"),
            json!({}),
            Some(&admin.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    accept(&ctx, id, "agent@example.test", &ctx.outbox.token_for(id)).await;
}

#[sqlx::test]
async fn archiving_needs_a_reason(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let agent = staff_member(&ctx, &admin, "agent@example.test", "operations").await;
    for body in [json!({"reason": "   "}), json!({})] {
        let res = ctx
            .post(
                &format!("/staff/{}/archive", agent.id),
                body,
                Some(&admin.token),
            )
            .await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.body);
        assert_eq!(error_code(&res), "invalid_request");
    }
    assert_eq!(
        ctx.get("/staff/me", &agent.token).await.status,
        StatusCode::OK
    );
}

#[sqlx::test]
async fn self_archive_and_archiving_the_last_admin_are_refused(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;

    let res = archive(&ctx, &admin, admin.id).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(error_code(&res), "cannot_archive_self");

    // Someone who may archive staff but not manage roles cannot archive the only admin.
    sqlx::query("INSERT INTO roles (key, name) VALUES ('archiver', 'Archiver')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_key, permission_key) VALUES ('archiver', 'staff.archive')",
    )
    .execute(&db)
    .await
    .unwrap();
    let archiver = staff_member(&ctx, &admin, "archiver@example.test", "archiver").await;
    let res = archive(&ctx, &archiver, admin.id).await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.body);
    assert_eq!(error_code(&res), "last_admin");
    assert_eq!(
        ctx.get("/staff/me", &admin.token).await.status,
        StatusCode::OK
    );

    // With a second admin, one admin may archive the other.
    let admin2 = staff_member(&ctx, &admin, "admin2@example.test", "admin").await;
    let res = archive(&ctx, &admin2, admin.id).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
}

#[sqlx::test]
async fn two_admins_archiving_each_other_at_once_leave_one_admin(db: PgPool) {
    let ctx = Ctx::new(&db);
    let a = first_admin(&ctx, "a@example.test").await;
    let b = staff_member(&ctx, &a, "b@example.test", "admin").await;

    let (x, y) = tokio::join!(archive(&ctx, &a, b.id), archive(&ctx, &b, a.id));
    let ok = [&x, &y]
        .iter()
        .filter(|r| r.status == StatusCode::OK)
        .count();
    assert_eq!(ok, 1, "{} / {}", x.body, y.body);
    let active_admins: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff s JOIN staff_roles sr ON sr.staff_id = s.id
         WHERE sr.role_key = 'admin' AND s.status = 'active'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(active_admins, 1);
}

// --- Audit ---

#[sqlx::test]
async fn every_change_has_an_audit_entry_naming_who_what_when_and_why(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let id = invite(&ctx, &admin, "agent@example.test", "operations").await;
    ctx.post(
        &format!("/staff/{id}/invitation"),
        json!({}),
        Some(&admin.token),
    )
    .await;
    accept(&ctx, id, "agent@example.test", &ctx.outbox.token_for(id)).await;
    ctx.call(
        Method::PATCH,
        &format!("/staff/{id}"),
        Some(json!({"role": "finance", "reason": "Moved"})),
        Some(&admin.token),
    )
    .await;
    archive(&ctx, &admin, id).await;
    ctx.post(
        &format!("/staff/{id}/restore"),
        json!({"reason": "Came back"}),
        Some(&admin.token),
    )
    .await;

    let entries = audit_of(&db, id).await;
    let by_admin: Vec<_> = entries
        .iter()
        .filter(|e| e.1 == Some(admin.id))
        .map(|e| (e.0.as_str(), e.2.as_deref()))
        .collect();
    assert_eq!(
        by_admin,
        [
            ("staff.create", Some("Hired")),
            ("staff.invitation_reissue", None),
            ("staff.edit_role", Some("Moved")),
            ("staff.archive", Some("Left the company")),
            ("staff.restore", Some("Came back")),
        ]
    );
    let created = entries.iter().find(|e| e.0 == "staff.create").unwrap();
    assert_eq!(created.4.as_ref().unwrap()["status"], "invited");
    assert_eq!(created.4.as_ref().unwrap()["roles"], json!(["operations"]));
    let completed = entries
        .iter()
        .find(|e| e.0 == "staff.enrolment_complete")
        .unwrap();
    assert_eq!(completed.3.as_ref().unwrap()["status"], "invited");
    assert_eq!(completed.4.as_ref().unwrap()["status"], "active");
    let archived = entries.iter().find(|e| e.0 == "staff.archive").unwrap();
    assert_eq!(archived.4.as_ref().unwrap()["sessions_revoked"], 1);

    // Entries carry the request id and a time; none holds a token.
    let rows: Vec<(Option<String>, String)> = sqlx::query_as(
        "SELECT request_id, coalesce(before::text, '') || coalesce(after::text, '')
         FROM audit_entries WHERE record_id = $1 AND created_at IS NOT NULL",
    )
    .bind(id.to_string())
    .fetch_all(&db)
    .await
    .unwrap();
    assert!(
        rows.iter()
            .all(|(rid, _)| rid.as_deref() == Some("req-staff"))
    );
    let tokens: Vec<String> = ctx
        .outbox
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|t| t.1.clone())
        .collect();
    for (_, text) in &rows {
        for t in &tokens {
            assert!(!text.contains(t.as_str()));
        }
    }
}

// --- Permissions ---

#[sqlx::test]
async fn every_staff_endpoint_needs_its_permission_and_a_session(db: PgPool) {
    let ctx = Ctx::new(&db);
    let admin = first_admin(&ctx, "admin@example.test").await;
    let agent = staff_member(&ctx, &admin, "agent@example.test", "operations").await;
    let target = invite(&ctx, &admin, "other@example.test", "operations").await;
    let create = json!({"email": "new@example.test", "name": "N", "role": "operations"});
    let reason = json!({"reason": "r"});
    let endpoints = [
        (Method::GET, "/staff".to_string(), None),
        (Method::GET, format!("/staff/{target}"), None),
        (Method::POST, "/staff".to_string(), Some(create)),
        (Method::POST, format!("/staff/{target}/invitation"), None),
        (
            Method::PATCH,
            format!("/staff/{target}"),
            Some(json!({"role": "admin"})),
        ),
        (
            Method::POST,
            format!("/staff/{target}/archive"),
            Some(reason.clone()),
        ),
        (
            Method::POST,
            format!("/staff/{target}/restore"),
            Some(reason),
        ),
    ];
    for (method, path, body) in &endpoints {
        let res = ctx
            .call(method.clone(), path, body.clone(), Some(&agent.token))
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {path}");
        assert_eq!(error_code(&res), "forbidden");
        let res = ctx.call(method.clone(), path, body.clone(), None).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {path}");
        assert_eq!(error_code(&res), "unauthenticated");
    }
    // A forbidden caller gets 403 even with a bad body or id.
    let res = ctx
        .call(
            Method::PATCH,
            "/staff/not-a-uuid",
            Some(json!({})),
            Some(&agent.token),
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Nothing changed.
    let (status, roles): (String, Vec<String>) = sqlx::query_as(
        "SELECT s.status, array_agg(sr.role_key) FROM staff s
         JOIN staff_roles sr ON sr.staff_id = s.id WHERE s.id = $1 GROUP BY s.status",
    )
    .bind(target)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), roles),
        ("invited", vec!["operations".to_string()])
    );
}
