//! Staff sign-in with password and TOTP, sessions, sign-out and rate limits (TASK-005;
//! FT-044, FT-062; TC-007, TC-010).
//! database: every test runs in its own throwaway database with the migrations applied
//! (#[sqlx::test]); Postgres from docker-compose.yml (DATABASE_URL via .cargo/config.toml).

use std::sync::Arc;

use api::{
    auth::{Permission, admin, secrets},
    config::Config,
    state::AppState,
};
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const IP: &str = "203.0.113.7";

fn config() -> Config {
    Config::from_lookup(|name| match name {
        "API_CLIENT_IP_HEADER" => Some("x-test-client-ip".into()),
        _ => None,
    })
    .unwrap()
}

fn app(db: &PgPool) -> Router {
    api::router(AppState {
        db: db.clone(),
        config: Arc::new(config()),
    })
}

struct Res {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

async fn call(
    db: &PgPool,
    method: Method,
    path: &str,
    body: Option<Value>,
    bearer: Option<&str>,
    ip: &str,
) -> Res {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("x-test-client-ip", ip)
        .header("x-request-id", "req-test");
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
    let res = app(db).oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    Res {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

async fn post(db: &PgPool, path: &str, body: Value) -> Res {
    call(db, Method::POST, path, Some(body), None, IP).await
}

async fn me(db: &PgPool, token: &str) -> Res {
    call(db, Method::GET, "/staff/me", None, Some(token), IP).await
}

/// The code an authenticator app would show `offset` seconds from now.
fn code(secret_base32: &str, email: &str, offset: i64) -> String {
    let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
    let totp = secrets::totp(secret.as_ref().to_vec(), email).unwrap();
    let t = (secrets::unix_now() as i64 + offset) as u64;
    totp.generate(t).to_string()
}

struct Enrolled {
    staff_id: Uuid,
    email: String,
    secret: String,
    /// The session from completing enrolment.
    token: String,
}

/// `api create-admin`, then enrolment with a password and a first code.
async fn enrolled_admin(db: &PgPool, email: &str) -> Enrolled {
    let created = admin::create_admin(db, &config().staff_auth, email, "First Admin")
        .await
        .unwrap();
    let res = post(
        db,
        "/staff/enrolment",
        json!({"token": created.enrolment_token, "password": PASSWORD}),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let secret = res.body["totp"]["secret"].as_str().unwrap().to_string();
    let challenge = res.body["challenge"].as_str().unwrap().to_string();
    let res = post(
        db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": code(&secret, email, 0)}),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    Enrolled {
        staff_id: created.staff_id,
        email: email.to_string(),
        secret,
        token: res.body["token"].as_str().unwrap().to_string(),
    }
}

/// Lets the next code be accepted again (each 30-second code works once).
async fn forget_last_code(db: &PgPool, staff_id: Uuid) {
    sqlx::query("UPDATE staff SET totp_last_used_step = NULL WHERE id = $1")
        .bind(staff_id)
        .execute(db)
        .await
        .unwrap();
}

async fn password_step(db: &PgPool, email: &str, password: &str) -> Res {
    post(
        db,
        "/staff/sign-in",
        json!({"email": email, "password": password}),
    )
    .await
}

/// Both steps; returns the session token.
async fn sign_in(db: &PgPool, a: &Enrolled) -> String {
    forget_last_code(db, a.staff_id).await;
    let res = password_step(db, &a.email, PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let res = post(
        db,
        "/staff/sign-in/verify",
        json!({"challenge": res.body["challenge"], "code": code(&a.secret, &a.email, 0)}),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    res.body["token"].as_str().unwrap().to_string()
}

fn error_code(res: &Res) -> &str {
    res.body["error"]["code"].as_str().unwrap_or("")
}

async fn audit_actions(db: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_entries ORDER BY created_at, action")
        .fetch_all(db)
        .await
        .unwrap()
}

// --- The full flow ---

#[sqlx::test]
async fn enrolment_then_sign_in_give_a_session_that_works(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;

    let res = me(&db, &a.token).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["id"], a.staff_id.to_string());
    assert_eq!(res.body["email"], "admin@example.test");
    assert_eq!(res.body["status"], "active");
    assert_eq!(res.body["roles"], json!(["admin"]));
    assert_eq!(
        res.body["permissions"].as_array().unwrap().len(),
        Permission::ALL.len()
    );

    // A later sign-in with password and code.
    let token = sign_in(&db, &a).await;
    assert_ne!(token, a.token);
    assert_eq!(me(&db, &token).await.status, StatusCode::OK);

    let actions = audit_actions(&db).await;
    for expected in [
        "staff.create",
        "staff.enrolment_start",
        "staff.enrolment_complete",
        "staff.sign_in",
    ] {
        assert!(actions.contains(&expected.to_string()), "{actions:?}");
    }
    assert_eq!(actions.iter().filter(|a| *a == "staff.sign_in").count(), 2);
}

#[sqlx::test]
async fn the_verify_response_has_the_documented_shape(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let step = password_step(&db, &a.email, PASSWORD).await;
    assert!(step.body["expires_at"].is_string());
    let res = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": step.body["challenge"], "code": code(&a.secret, &a.email, 0)}),
    )
    .await;
    assert_eq!(res.body["token_type"], "Bearer");
    assert!(res.body["expires_at"].is_string());
    assert_eq!(res.body["staff"]["email"], "admin@example.test");
}

// --- Refusals ---

#[sqlx::test]
async fn wrong_password_and_unknown_email_give_the_same_error(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;

    let wrong = password_step(&db, &a.email, "not the password at all").await;
    let unknown = password_step(&db, "nobody@example.test", PASSWORD).await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.body, unknown.body);
    assert_eq!(error_code(&wrong), "invalid_credentials");

    // The failure is audited for the account that exists, without the password.
    let failures: Vec<(Option<Uuid>, String, Value)> = sqlx::query_as(
        "SELECT staff_id, record_id, after FROM audit_entries WHERE action = 'staff.sign_in_failed'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, None);
    assert_eq!(failures[0].1, a.staff_id.to_string());
    assert_eq!(
        failures[0].2,
        json!({"failure": "wrong_password", "ip": IP})
    );
}

#[sqlx::test]
async fn email_matching_ignores_case_and_spaces(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    let res = password_step(&db, "  Admin@Example.TEST ", PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let _ = a;
}

#[sqlx::test]
async fn wrong_and_expired_codes_are_refused(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let challenge = password_step(&db, &a.email, PASSWORD).await.body["challenge"].clone();

    let correct = code(&a.secret, &a.email, 0);
    let wrong = if correct == "000000" {
        "111111"
    } else {
        "000000"
    };
    for bad in [
        wrong.to_string(),
        code(&a.secret, &a.email, -120),
        code(&a.secret, &a.email, 120),
        "12345".into(),
    ] {
        if bad == correct {
            continue;
        }
        let res = post(
            &db,
            "/staff/sign-in/verify",
            json!({"challenge": challenge, "code": bad}),
        )
        .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error_code(&res), "invalid_code");
    }
    // The challenge is still open for the right code.
    let res = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": correct}),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
}

#[sqlx::test]
async fn a_used_challenge_cannot_be_used_again(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let challenge = password_step(&db, &a.email, PASSWORD).await.body["challenge"].clone();
    let first = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": code(&a.secret, &a.email, 0)}),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);

    forget_last_code(&db, a.staff_id).await;
    let again = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": code(&a.secret, &a.email, 0)}),
    )
    .await;
    assert_eq!(again.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&again), "challenge_expired");
}

#[sqlx::test]
async fn a_code_works_only_once(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let c = code(&a.secret, &a.email, 0);
    for expected in [StatusCode::OK, StatusCode::UNAUTHORIZED] {
        let challenge = password_step(&db, &a.email, PASSWORD).await.body["challenge"].clone();
        let res = post(
            &db,
            "/staff/sign-in/verify",
            json!({"challenge": challenge, "code": c}),
        )
        .await;
        assert_eq!(res.status, expected, "{}", res.body);
    }
}

#[sqlx::test]
async fn an_expired_challenge_is_refused(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let challenge = password_step(&db, &a.email, PASSWORD).await.body["challenge"].clone();
    sqlx::query("UPDATE staff_sign_in_challenges SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    let res = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": code(&a.secret, &a.email, 0)}),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "challenge_expired");
    let res = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": "made-up", "code": "123456"}),
    )
    .await;
    assert_eq!(error_code(&res), "challenge_expired");
}

#[sqlx::test]
async fn archived_staff_cannot_sign_in_and_their_sessions_stop_working(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    sqlx::query("UPDATE staff SET status = 'archived', archived_at = now() WHERE id = $1")
        .bind(a.staff_id)
        .execute(&db)
        .await
        .unwrap();
    let res = password_step(&db, &a.email, PASSWORD).await;
    assert_eq!(error_code(&res), "invalid_credentials");
    assert_eq!(me(&db, &a.token).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn a_password_without_an_enrolled_authenticator_does_not_sign_in(db: PgPool) {
    let created = admin::create_admin(&db, &config().staff_auth, "new@example.test", "New")
        .await
        .unwrap();
    let res = post(
        &db,
        "/staff/enrolment",
        json!({"token": created.enrolment_token, "password": PASSWORD}),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    // Enrolment was not completed with a code.
    let res = password_step(&db, "new@example.test", PASSWORD).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&res), "totp_enrolment_required");
}

// --- Sessions ---

#[sqlx::test]
async fn after_sign_out_the_token_is_refused(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    let other = sign_in(&db, &a).await;

    let res = call(
        &db,
        Method::POST,
        "/staff/sign-out",
        None,
        Some(&a.token),
        IP,
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let res = me(&db, &a.token).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "unauthenticated");
    let res = call(
        &db,
        Method::POST,
        "/staff/sign-out",
        None,
        Some(&a.token),
        IP,
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);

    // Only that session ended.
    assert_eq!(me(&db, &other).await.status, StatusCode::OK);
    assert!(
        audit_actions(&db)
            .await
            .contains(&"staff.sign_out".to_string())
    );
}

#[sqlx::test]
async fn an_idle_session_is_refused_and_use_keeps_it_alive(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    sqlx::query(
        "UPDATE staff_sessions SET last_used_at = now() - interval '11 hours',
                                   expires_at = now() + interval '1 hour'",
    )
    .execute(&db)
    .await
    .unwrap();
    assert_eq!(me(&db, &a.token).await.status, StatusCode::OK);
    let extended: bool = sqlx::query_scalar(
        "SELECT expires_at > now() + interval '11 hours' AND last_used_at > now() - interval '1 minute'
         FROM staff_sessions",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(extended, "using the session moves its idle expiry forward");

    sqlx::query("UPDATE staff_sessions SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    let res = me(&db, &a.token).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "unauthenticated");
}

#[sqlx::test]
async fn missing_or_made_up_tokens_are_unauthenticated(db: PgPool) {
    let res = call(&db, Method::GET, "/staff/me", None, None, IP).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "unauthenticated");
    let (fake, _) = secrets::new_token();
    for token in [fake.as_str(), "garbage"] {
        let res = me(&db, token).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
}

// --- Rate limits ---

#[sqlx::test]
async fn too_many_failed_passwords_give_429_until_the_window_passes(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    for _ in 0..5 {
        let res = password_step(&db, &a.email, "wrong password here").await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    // Even the right password is refused now, from another address too.
    for ip in [IP, "198.51.100.1"] {
        let res = call(
            &db,
            Method::POST,
            "/staff/sign-in",
            Some(json!({"email": a.email, "password": PASSWORD})),
            None,
            ip,
        )
        .await;
        assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error_code(&res), "rate_limited");
        let retry: u64 = res.headers[header::RETRY_AFTER]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=15 * 60).contains(&retry), "{retry}");
    }

    // The window passes.
    sqlx::query("UPDATE staff_sign_in_failures SET created_at = now() - interval '16 minutes'")
        .execute(&db)
        .await
        .unwrap();
    let res = password_step(&db, &a.email, PASSWORD).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
}

#[sqlx::test]
async fn one_address_is_limited_across_accounts(db: PgPool) {
    for i in 0..5 {
        let res = password_step(&db, &format!("guess{i}@example.test"), PASSWORD).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    let res = password_step(&db, "guess9@example.test", PASSWORD).await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    // Another address is not affected.
    let res = call(
        &db,
        Method::POST,
        "/staff/sign-in",
        Some(json!({"email": "guess9@example.test", "password": PASSWORD})),
        None,
        "198.51.100.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn wrong_codes_count_towards_the_limit(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    forget_last_code(&db, a.staff_id).await;
    let challenge = password_step(&db, &a.email, PASSWORD).await.body["challenge"].clone();
    let correct = code(&a.secret, &a.email, 0);
    let wrong = if correct == "000000" {
        "111111"
    } else {
        "000000"
    };
    for _ in 0..5 {
        let res = post(
            &db,
            "/staff/sign-in/verify",
            json!({"challenge": challenge, "code": wrong}),
        )
        .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    let res = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": challenge, "code": correct}),
    )
    .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test]
async fn successful_sign_in_does_not_count_against_the_limit(db: PgPool) {
    let a = enrolled_admin(&db, "admin@example.test").await;
    for _ in 0..6 {
        sign_in(&db, &a).await;
    }
}

// --- create-admin and enrolment ---

#[sqlx::test]
async fn create_admin_makes_an_active_admin_without_a_password(db: PgPool) {
    let created = admin::create_admin(&db, &config().staff_auth, "boss@example.test", "Boss")
        .await
        .unwrap();
    let (status, password_hash): (String, Option<String>) =
        sqlx::query_as("SELECT status, password_hash FROM staff WHERE id = $1")
            .bind(created.staff_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(status, "active");
    assert_eq!(password_hash, None);
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role_key FROM staff_roles WHERE staff_id = $1")
            .bind(created.staff_id)
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(roles, vec!["admin"]);
    assert!(created.expires_at > chrono::Utc::now() + chrono::Duration::hours(23));

    // Without a password nobody can sign in as them.
    let res = password_step(&db, "boss@example.test", "").await;
    assert_eq!(error_code(&res), "invalid_credentials");

    // The same email again, or bad input, is refused.
    let dup = admin::create_admin(&db, &config().staff_auth, "BOSS@example.test", "Boss").await;
    assert!(matches!(dup, Err(admin::CreateAdminError::EmailTaken)));
    for (email, name) in [("not-an-email", "X"), ("a@example.test", " ")] {
        let bad = admin::create_admin(&db, &config().staff_auth, email, name).await;
        assert!(matches!(bad, Err(admin::CreateAdminError::Invalid(_))));
    }
}

#[sqlx::test]
async fn an_enrolment_token_works_until_enrolment_completes(db: PgPool) {
    let created = admin::create_admin(&db, &config().staff_auth, "boss@example.test", "Boss")
        .await
        .unwrap();
    let token = created.enrolment_token;

    let short = post(
        &db,
        "/staff/enrolment",
        json!({"token": token, "password": "too short"}),
    )
    .await;
    assert_eq!(short.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(&short), "invalid_request");
    assert_eq!(short.body["error"]["details"]["min_length"], 12);

    // Starting twice is fine (e.g. a lost secret); only the latest secret counts.
    let first = post(
        &db,
        "/staff/enrolment",
        json!({"token": token, "password": PASSWORD}),
    )
    .await;
    let second = post(
        &db,
        "/staff/enrolment",
        json!({"token": token, "password": PASSWORD}),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK);
    assert_ne!(first.body["totp"]["secret"], second.body["totp"]["secret"]);
    let uri = second.body["totp"]["otpauth_uri"].as_str().unwrap();
    assert!(uri.starts_with("otpauth://totp/"), "{uri}");
    assert!(uri.contains("issuer=Chitty"), "{uri}");
    let secret = second.body["totp"]["secret"].as_str().unwrap();
    let old = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": first.body["challenge"], "code": code(secret, "boss@example.test", 0)}),
    )
    .await;
    assert_eq!(error_code(&old), "challenge_expired");

    let done = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": second.body["challenge"], "code": code(secret, "boss@example.test", 0)}),
    )
    .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);

    let reuse = post(
        &db,
        "/staff/enrolment",
        json!({"token": token, "password": PASSWORD}),
    )
    .await;
    assert_eq!(reuse.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&reuse), "invalid_enrolment_token");
}

#[sqlx::test]
async fn an_expired_enrolment_token_is_refused(db: PgPool) {
    let created = admin::create_admin(&db, &config().staff_auth, "boss@example.test", "Boss")
        .await
        .unwrap();
    sqlx::query("UPDATE staff_enrolment_tokens SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    let res = post(
        &db,
        "/staff/enrolment",
        json!({"token": created.enrolment_token, "password": PASSWORD}),
    )
    .await;
    assert_eq!(error_code(&res), "invalid_enrolment_token");
}

// --- Nothing secret is stored ---

#[sqlx::test]
async fn the_database_holds_no_raw_token_password_or_totp_secret(db: PgPool) {
    let created = admin::create_admin(&db, &config().staff_auth, "boss@example.test", "Boss")
        .await
        .unwrap();
    let start = post(
        &db,
        "/staff/enrolment",
        json!({"token": created.enrolment_token, "password": PASSWORD}),
    )
    .await;
    let secret = start.body["totp"]["secret"].as_str().unwrap().to_string();
    let enrol_challenge = start.body["challenge"].as_str().unwrap().to_string();
    let first_code = code(&secret, "boss@example.test", 0);
    let done = post(
        &db,
        "/staff/sign-in/verify",
        json!({"challenge": enrol_challenge, "code": first_code}),
    )
    .await;
    let session = done.body["token"].as_str().unwrap().to_string();
    let _ = password_step(&db, "boss@example.test", "a wrong password guess").await;
    let challenge = password_step(&db, "boss@example.test", PASSWORD).await.body["challenge"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &db,
        Method::POST,
        "/staff/sign-out",
        None,
        Some(&session),
        IP,
    )
    .await;

    let raw_secret = totp_rs::Secret::try_from_base32(&secret).unwrap();
    let raw_secret_hex: String = raw_secret
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut dump = String::new();
    for sql in [
        "SELECT row_to_json(t)::text FROM staff t",
        "SELECT row_to_json(t)::text FROM staff_enrolment_tokens t",
        "SELECT row_to_json(t)::text FROM staff_sign_in_challenges t",
        "SELECT row_to_json(t)::text FROM staff_sessions t",
        "SELECT row_to_json(t)::text FROM staff_sign_in_failures t",
        "SELECT row_to_json(t)::text FROM audit_entries t",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(&db).await.unwrap();
        assert!(!rows.is_empty(), "rows to check: {sql}");
        dump.push_str(&rows.join("\n"));
    }
    for (what, value) in [
        ("password", PASSWORD.to_string()),
        ("wrong password", "a wrong password guess".to_string()),
        ("enrolment token", created.enrolment_token.clone()),
        ("session token", session),
        ("challenge", challenge),
        ("enrolment challenge", enrol_challenge),
        ("TOTP secret (base32)", secret.clone()),
        ("TOTP secret (hex)", raw_secret_hex),
    ] {
        assert!(!dump.contains(&value), "the database contains the {what}");
    }
    assert!(
        !dump.contains(&format!("\"{first_code}\"")),
        "a code was stored"
    );
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM staff")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
}
