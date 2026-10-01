use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use totp_rs::{Algorithm, Builder, Secret};

use super::identity::AuthIdentity;
use super::jwt::{Claims, JwtConfig, Role};
use super::lab_guards::{lab_mode, refuse_known_defaults};
use super::oidc::{
    build_authorize_url, discover, display_username, exchange_code, pkce_challenge_s256,
    random_urlsafe, verify_id_token, OidcConfig, PendingOidc, ProviderInfo,
};
use super::user_db::{ApiTokenRecord, UserDb};
use crate::api::pam_auth::{authenticate_pam, validate_username};

const OIDC_PENDING_TTL_SECS: u64 = 600;

pub struct AuthState {
    pub jwt: JwtConfig,
    pub db: UserDb,
    /// Lab-only shared API key (honored only when ZORVIA_LAB_MODE=1).
    pub lab_api_key: Option<String>,
    pub oidc: Option<OidcConfig>,
    pub oidc_pending: Mutex<HashMap<String, PendingOidc>>,
}

impl AuthState {
    pub fn from_env() -> anyhow::Result<Self> {
        let db = UserDb::from_env()?;
        let admin_user = std::env::var("ZORVIA_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());

        let admin_password = if lab_mode() {
            std::env::var("ZORVIA_ADMIN_PASSWORD").unwrap_or_else(|_| "Admin@321".to_string())
        } else if let Ok(pw) = std::env::var("ZORVIA_ADMIN_PASSWORD") {
            pw
        } else if db.count_users()? == 0 {
            // First boot outside lab: generate a one-time password.
            let generated = format!("Zv-{}", uuid::Uuid::new_v4());
            log::warn!(
                "ZORVIA_ADMIN_PASSWORD unset; generated bootstrap password for '{}': {} \
                 (shown once — store it and rotate)",
                admin_user,
                generated
            );
            generated
        } else {
            // DB already seeded; password only needed for seed.
            String::new()
        };

        let jwt = JwtConfig::from_env();
        refuse_known_defaults(
            jwt.secret(),
            if admin_password.is_empty() {
                None
            } else {
                Some(admin_password.as_str())
            },
        )?;

        if !admin_password.is_empty() {
            db.seed_admin(&admin_user, &admin_password)?;
        }

        let lab_api_key = match std::env::var("ZORVIA_API_KEY") {
            Ok(k) if !k.is_empty() => {
                if lab_mode() {
                    log::warn!(
                        "ZORVIA_API_KEY is active in lab mode only; prefer scoped /v1/api-tokens"
                    );
                    Some(k)
                } else {
                    log::warn!(
                        "ZORVIA_API_KEY is set but ignored outside ZORVIA_LAB_MODE=1; \
                         create scoped tokens via POST /api/v1/api-tokens"
                    );
                    None
                }
            }
            _ => None,
        };

        Ok(Self {
            jwt,
            db,
            lab_api_key,
            oidc: OidcConfig::from_env(),
            oidc_pending: Mutex::new(HashMap::new()),
        })
    }

    /// Validate JWT and ensure local users are still enabled with matching token_version.
    pub fn validate_bearer(&self, token: &str) -> Option<Claims> {
        let claims = self.jwt.validate(token).ok()?;
        // Non-local identities (PAM) have no DB row / token_version.
        // Their `tv` is the issue time, so logout can revoke by timestamp.
        if let Some(name) = claims.sub.strip_prefix("pam:") {
            if claims.tv < self.db.pam_not_before(name).unwrap_or(u32::MAX) {
                return None;
            }
            return Some(claims);
        }
        let user = self.db.get_by_id(&claims.sub).ok().flatten()?;
        if !user.enabled {
            return None;
        }
        if user.token_version != claims.tv {
            return None;
        }
        Some(claims)
    }

    pub fn lab_api_key_ok(&self, provided: &str) -> bool {
        match &self.lab_api_key {
            Some(expected) => {
                use subtle::ConstantTimeEq;
                bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
            }
            None => false,
        }
    }

    /// Resolve a credential string (Bearer or x-api-key) into an identity.
    pub fn resolve_credential(&self, credential: &str) -> Option<AuthIdentity> {
        if self.lab_api_key_ok(credential) {
            return Some(AuthIdentity::lab_api_key());
        }
        if let Ok(Some(tok)) = self.db.lookup_api_token(credential) {
            let _ = self.db.touch_api_token(&tok.id);
            return Some(AuthIdentity::from_api_token(
                tok.id,
                tok.name,
                tok.role,
                &tok.scopes,
            ));
        }
        let claims = self.validate_bearer(credential)?;
        let mut id = AuthIdentity::from_jwt(claims.sub.clone(), claims.username, claims.role);
        if !claims.sub.starts_with("pam:") && !matches!(id.role, Role::Admin) {
            // Fail closed: if the allow-list cannot be read, confine to nothing.
            id.namespaces = match self.db.get_namespaces(&claims.sub) {
                Ok(n) => n,
                Err(_) => Some(Vec::new()),
            };
        }
        Some(id)
    }
}

pub type SharedAuth = Arc<AuthState>;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub role: String,
    pub username: String,
}

fn role_str(r: &Role) -> &'static str {
    match r {
        Role::Admin => "admin",
        Role::User => "user",
        Role::Viewer => "viewer",
    }
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "success": false,
            "status": status.as_u16(),
            "error": { "code": status.as_str(), "message": msg },
            "data": null
        })),
    )
}

/// Failed-login throttle: after `MAX_FAILURES` 401s for one username inside
/// `LOCKOUT`, further attempts get 429 until the window ends. This also caps
/// TOTP guessing, because a wrong code is a 401. In-memory and per process,
/// keyed by lower-cased username (unknown names are tracked too, so the
/// response does not reveal whether an account exists).
mod login_throttle {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    pub const MAX_FAILURES: u32 = 8;
    pub const LOCKOUT: Duration = Duration::from_secs(15 * 60);
    const MAX_TRACKED: usize = 10_000;

    static FAILS: Mutex<Option<HashMap<String, (u32, Instant)>>> = Mutex::new(None);

    pub fn blocked(user: &str) -> bool {
        let mut g = FAILS.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(HashMap::new);
        match m.get(&user.to_lowercase()) {
            Some((n, at)) if at.elapsed() < LOCKOUT => *n >= MAX_FAILURES,
            Some(_) => {
                m.remove(&user.to_lowercase());
                false
            }
            None => false,
        }
    }

    pub fn record_failure(user: &str) {
        let mut g = FAILS.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(HashMap::new);
        if m.len() >= MAX_TRACKED {
            m.retain(|_, (_, at)| at.elapsed() < LOCKOUT);
        }
        let e = m.entry(user.to_lowercase()).or_insert((0, Instant::now()));
        if e.1.elapsed() >= LOCKOUT {
            *e = (0, Instant::now());
        }
        e.0 += 1;
        // Every further failure extends the lockout.
        e.1 = Instant::now();
    }

    pub fn clear(user: &str) {
        let mut g = FAILS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = g.as_mut() {
            m.remove(&user.to_lowercase());
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn locks_after_repeated_failures_and_clears() {
            let u = "throttle-test-user";
            assert!(!blocked(u));
            for _ in 0..MAX_FAILURES {
                record_failure(u);
            }
            assert!(blocked(u));
            assert!(blocked("THROTTLE-TEST-USER"));
            clear(u);
            assert!(!blocked(u));
        }
    }
}

pub async fn login_handler(
    State(auth): State<SharedAuth>,
    Json(req): Json<LoginRequest>,
) -> axum::response::Response {
    let username = req.username.clone();
    if login_throttle::blocked(&username) {
        return err(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many failed sign-in attempts. Try again later.",
        )
        .into_response();
    }
    let resp = login_inner(auth, req).await;
    match resp.status() {
        StatusCode::UNAUTHORIZED => login_throttle::record_failure(&username),
        s if s.is_success() => login_throttle::clear(&username),
        _ => {}
    }
    resp
}

async fn login_inner(auth: SharedAuth, req: LoginRequest) -> axum::response::Response {
    if !validate_username(&req.username) {
        return err(StatusCode::BAD_REQUEST, "Invalid username format").into_response();
    }

    if let Ok(Some(user)) = auth.db.get_by_username(&req.username) {
        if !user.enabled {
            return err(StatusCode::FORBIDDEN, "This account has been disabled").into_response();
        }
        match user.verify_password(&req.password) {
            Ok(true) => {
                if user.totp_enabled {
                    let Some(code) = req.totp_code.as_deref() else {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(serde_json::json!({
                                "success": false,
                                "status": 403,
                                "error": {
                                    "code": "requires_2fa",
                                    "message": "2FA code required",
                                    "requires_2fa": true
                                },
                                "data": null
                            })),
                        )
                            .into_response();
                    };
                    if !verify_totp(user.totp_secret.as_deref(), code) {
                        return err(StatusCode::UNAUTHORIZED, "Invalid 2FA code").into_response();
                    }
                }
                return match auth.jwt.generate(
                    &user.id,
                    &user.username,
                    user.role.clone(),
                    user.token_version,
                ) {
                    Ok(token) => {
                        let _ = auth.db.update_last_login(&user.id);
                        Json(LoginResponse {
                            token,
                            user_id: user.id,
                            role: role_str(&user.role).into(),
                            username: user.username,
                        })
                        .into_response()
                    }
                    Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
                };
            }
            Ok(false) => {
                return err(StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
            }
            Err(_) => {
                return err(StatusCode::INTERNAL_SERVER_ERROR, "Auth error").into_response();
            }
        }
    }

    let username = req.username.clone();
    let password = req.password.clone();
    let pam_ok = tokio::task::spawn_blocking(move || authenticate_pam(&username, &password))
        .await
        .unwrap_or(Ok(false))
        .unwrap_or(false);

    if pam_ok {
        let uid = format!("pam:{}", req.username);
        return match auth
            .jwt
            .generate(&uid, &req.username, Role::User, pam_now())
        {
            Ok(token) => Json(LoginResponse {
                token,
                user_id: uid,
                role: "user".into(),
                username: req.username,
            })
            .into_response(),
            Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
        };
    }

    err(StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
}

fn verify_totp(secret: Option<&str>, code: &str) -> bool {
    let Some(secret) = secret else {
        return false;
    };
    let Ok(secret) = Secret::try_from_base32(secret) else {
        return false;
    };
    let Ok(totp) = Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .with_account_name("")
        .build()
    else {
        return false;
    };
    totp.check_current(code).is_some()
}

pub async fn me_handler(State(auth): State<SharedAuth>, headers: HeaderMap) -> impl IntoResponse {
    let Some(token) = bearer_token(&headers) else {
        return err(StatusCode::UNAUTHORIZED, "Missing bearer token").into_response();
    };
    match auth.validate_bearer(&token) {
        Some(c) => Json(serde_json::json!({
            "id": c.sub,
            "username": c.username,
            "role": role_str(&c.role),
        }))
        .into_response(),
        None => err(StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.to_string()))
}

fn parse_role(s: &str) -> Option<Role> {
    match s.to_ascii_lowercase().as_str() {
        "admin" => Some(Role::Admin),
        "user" => Some(Role::User),
        "viewer" => Some(Role::Viewer),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub created: String,
    pub last_login: Option<String>,
}

impl From<&crate::api::auth::user_db::User> for UserSummary {
    fn from(u: &crate::api::auth::user_db::User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            role: role_str(&u.role).to_string(),
            enabled: u.enabled,
            created: u.created.clone(),
            last_login: u.last_login.clone(),
        }
    }
}

pub async fn list_users_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
) -> impl IntoResponse {
    match auth.db.list_users() {
        Ok(users) => Json(users.iter().map(UserSummary::from).collect::<Vec<_>>()).into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list users").into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

pub async fn create_user_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if !validate_username(&body.username) {
        return err(StatusCode::BAD_REQUEST, "Invalid username format").into_response();
    }
    if body.password.len() < 8 {
        return err(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters",
        )
        .into_response();
    }
    let Some(role) = parse_role(&body.role) else {
        return err(
            StatusCode::BAD_REQUEST,
            "Invalid role (expected admin, user, or viewer)",
        )
        .into_response();
    };
    if auth
        .db
        .get_by_username(&body.username)
        .ok()
        .flatten()
        .is_some()
    {
        return err(StatusCode::CONFLICT, "Username already exists").into_response();
    }
    match auth.db.create_user(&body.username, &body.password, role) {
        Ok(user) => (StatusCode::CREATED, Json(UserSummary::from(&user))).into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create user").into_response(),
    }
}

fn reject_if_last_admin(
    auth: &SharedAuth,
    target: &crate::api::auth::user_db::User,
    target_will_remain_enabled_admin: bool,
) -> Option<axum::response::Response> {
    if target.role != Role::Admin || !target.enabled || target_will_remain_enabled_admin {
        return None;
    }
    match auth.db.enabled_admin_count() {
        Ok(n) if n <= 1 => Some(
            err(
                StatusCode::CONFLICT,
                "This is the last enabled admin account",
            )
            .into_response(),
        ),
        Ok(_) => None,
        Err(_) => Some(
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to check admin count",
            )
            .into_response(),
        ),
    }
}

pub async fn delete_user_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(Some(target)) = auth.db.get_by_id(&id) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    if let Some(resp) = reject_if_last_admin(&auth, &target, false) {
        return resp;
    }
    match auth.db.delete_user(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete user").into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

pub async fn update_role_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateRoleRequest>,
) -> impl IntoResponse {
    let Some(new_role) = parse_role(&body.role) else {
        return err(StatusCode::BAD_REQUEST, "Invalid role").into_response();
    };
    let Ok(Some(target)) = auth.db.get_by_id(&id) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    if let Some(resp) = reject_if_last_admin(&auth, &target, new_role == Role::Admin) {
        return resp;
    }
    match auth.db.update_role(&id, new_role) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update role").into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetEnabledRequest {
    pub enabled: bool,
}

pub async fn set_enabled_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SetEnabledRequest>,
) -> impl IntoResponse {
    let Ok(Some(target)) = auth.db.get_by_id(&id) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    if let Some(resp) = reject_if_last_admin(&auth, &target, body.enabled) {
        return resp;
    }
    match auth.db.set_enabled(&id, body.enabled) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update user").into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct TotpDisableRequest {
    pub password: String,
    pub totp_code: String,
}

/// Body for `POST /v1/auth/totp/setup`. Only needed when 2FA is already on.
#[derive(Debug, Default, Deserialize)]
pub struct TotpSetupRequest {
    pub password: Option<String>,
    pub totp_code: Option<String>,
}

pub async fn totp_setup_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    // Re-enrolling replaces the secret. While 2FA is on, a bearer token alone
    // must not be able to do that, or a stolen session could swap in its own
    // authenticator and then drop the second factor.
    let mut was_enabled = false;
    if let Ok(Some(user)) = auth.db.get_by_id(&claims.sub) {
        if user.totp_enabled {
            was_enabled = true;
            let req: TotpSetupRequest = serde_json::from_slice(&body).unwrap_or_default();
            let password_ok = req
                .password
                .as_deref()
                .is_some_and(|p| user.verify_password(p).unwrap_or(false));
            let code_ok = req
                .totp_code
                .as_deref()
                .is_some_and(|c| verify_totp(user.totp_secret.as_deref(), c));
            if !(password_ok && code_ok) {
                return err(
                    StatusCode::FORBIDDEN,
                    "2FA is already enabled: supply password and a current totp_code to re-enrol",
                )
                .into_response();
            }
        }
    }
    let secret = Secret::generate();
    let encoded = secret.to_base32();
    let Ok(totp) = Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .with_issuer(Some("Zorvia"))
        .with_account_name(claims.username.clone())
        .build()
    else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "TOTP error").into_response();
    };
    // While 2FA is on, the new secret waits as "pending" and the current one
    // keeps protecting the account until the new one produces a valid code.
    // Otherwise (2FA not yet on) enrolment just stores the secret as before.
    if was_enabled {
        let _ = auth.db.set_totp_pending(&claims.sub, Some(&encoded));
    } else {
        let _ = auth.db.set_totp(&claims.sub, &encoded, false);
    }
    let otpauth_url = match totp.to_url() {
        Ok(u) => u,
        Err(_) => {
            return err(StatusCode::INTERNAL_SERVER_ERROR, "TOTP error").into_response();
        }
    };
    Json(serde_json::json!({
        "secret": encoded,
        "otpauth_url": otpauth_url,
    }))
    .into_response()
}

pub async fn totp_verify_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    Json(body): Json<TotpVerifyRequest>,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let Ok(Some(user)) = auth.db.get_by_id(&claims.sub) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    if let Ok(Some(pending)) = auth.db.get_totp_pending(&user.id) {
        if !verify_totp(Some(&pending), &body.code) {
            return err(StatusCode::UNAUTHORIZED, "Invalid 2FA code").into_response();
        }
        let _ = auth.db.commit_totp_pending(&user.id, &pending);
        return Json(serde_json::json!({
            "success": true, "totp_enabled": true, "sessions_revoked": true
        }))
        .into_response();
    }
    let secret = user.totp_secret.as_deref().unwrap_or("");
    if !verify_totp(Some(secret), &body.code) {
        return err(StatusCode::UNAUTHORIZED, "Invalid 2FA code").into_response();
    }
    let _ = auth.db.set_totp(&user.id, secret, true);
    Json(serde_json::json!({ "success": true, "totp_enabled": true })).into_response()
}

pub async fn totp_disable_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    Json(body): Json<TotpDisableRequest>,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let Ok(Some(user)) = auth.db.get_by_id(&claims.sub) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    match user.verify_password(&body.password) {
        Ok(true) => {}
        Ok(false) => {
            return err(StatusCode::UNAUTHORIZED, "Invalid password").into_response();
        }
        Err(_) => {
            return err(StatusCode::INTERNAL_SERVER_ERROR, "Auth error").into_response();
        }
    }
    if !user.totp_enabled {
        return err(StatusCode::BAD_REQUEST, "TOTP is not enabled").into_response();
    }
    if !verify_totp(user.totp_secret.as_deref(), &body.totp_code) {
        return err(StatusCode::UNAUTHORIZED, "Invalid 2FA code").into_response();
    }
    if auth.db.disable_totp(&claims.sub).is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to disable TOTP").into_response();
    }
    log::info!(
        "TOTP disabled for user '{}' ({}); sessions revoked via token_version bump",
        user.username,
        user.id
    );
    Json(serde_json::json!({
        "success": true,
        "totp_enabled": false,
        "sessions_revoked": true
    }))
    .into_response()
}

pub async fn providers_handler(State(auth): State<SharedAuth>) -> impl IntoResponse {
    let list: Vec<ProviderInfo> = match &auth.oidc {
        Some(cfg) => vec![ProviderInfo {
            id: cfg.id.clone(),
            name: cfg.name.clone(),
        }],
        None => Vec::new(),
    };
    Json(list).into_response()
}

pub async fn oidc_login_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(cfg) = auth.oidc.as_ref() else {
        return err(
            StatusCode::NOT_FOUND,
            "OIDC is not configured (set ZORVIA_OIDC_ENABLED=1 and issuer/client/secret/redirect)",
        )
        .into_response();
    };
    if id != cfg.id && id != "default" {
        return err(StatusCode::NOT_FOUND, "Unknown OIDC provider").into_response();
    }

    let endpoints = match discover(cfg).await {
        Ok(e) => e,
        Err(e) => {
            log::error!("OIDC discovery failed: {e:#}");
            return err(StatusCode::BAD_GATEWAY, "OIDC discovery failed").into_response();
        }
    };

    let state = random_urlsafe(32);
    let nonce = random_urlsafe(32);
    let verifier = random_urlsafe(48);
    let challenge = pkce_challenge_s256(&verifier);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    {
        let mut pending = match auth.oidc_pending.lock() {
            Ok(p) => p,
            Err(_) => {
                return err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OIDC state store poisoned",
                )
                .into_response()
            }
        };
        // Drop expired entries.
        pending.retain(|_, v| now.saturating_sub(v.created_unix) < OIDC_PENDING_TTL_SECS);
        pending.insert(
            state.clone(),
            PendingOidc {
                code_verifier: verifier,
                nonce: nonce.clone(),
                created_unix: now,
                token_endpoint: endpoints.token_endpoint.clone(),
                jwks_uri: endpoints.jwks_uri.clone(),
                issuer: endpoints.issuer.clone(),
            },
        );
    }

    let url = build_authorize_url(
        &endpoints.authorization_endpoint,
        cfg,
        &state,
        &nonce,
        &challenge,
    );
    // Bind the flow to this browser: the callback must present the same state
    // in a cookie, so a victim cannot be lured into finishing a flow an
    // attacker started (login CSRF). Lax so it rides the top-level redirect
    // back from the IdP.
    let mut resp = Json(serde_json::json!({ "url": url })).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&oidc_state_cookie(
        &state,
        OIDC_PENDING_TTL_SECS,
        &cfg.redirect_uri,
    )) {
        resp.headers_mut().insert(axum::http::header::SET_COOKIE, v);
    }
    resp
}

const OIDC_STATE_COOKIE: &str = "zorvia_oidc_state";

fn oidc_state_cookie(value: &str, max_age: u64, redirect_uri: &str) -> String {
    let secure = if redirect_uri.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{OIDC_STATE_COOKIE}={value}; Path=/api/v1/auth/oidc; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    )
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn oidc_callback_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    Query(q): Query<OidcCallbackQuery>,
) -> impl IntoResponse {
    let Some(cfg) = auth.oidc.as_ref() else {
        return err(StatusCode::NOT_FOUND, "OIDC is not configured").into_response();
    };
    if let Some(e) = q.error.as_deref() {
        return err(
            StatusCode::BAD_REQUEST,
            &format!("OIDC provider error: {e}"),
        )
        .into_response();
    }
    let (Some(code), Some(state)) = (q.code.as_deref(), q.state.as_deref()) else {
        return err(StatusCode::BAD_REQUEST, "Missing code or state").into_response();
    };
    if cookie_value(&headers, OIDC_STATE_COOKIE) != Some(state) {
        return err(
            StatusCode::BAD_REQUEST,
            "OIDC sign-in was not started in this browser",
        )
        .into_response();
    }

    let pending = {
        let mut map = match auth.oidc_pending.lock() {
            Ok(p) => p,
            Err(_) => {
                return err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OIDC state store poisoned",
                )
                .into_response()
            }
        };
        match map.remove(state) {
            Some(p) => p,
            None => {
                return err(StatusCode::BAD_REQUEST, "Invalid or expired OIDC state")
                    .into_response()
            }
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(pending.created_unix) > OIDC_PENDING_TTL_SECS {
        return err(StatusCode::BAD_REQUEST, "OIDC state expired").into_response();
    }

    let id_token =
        match exchange_code(cfg, &pending.token_endpoint, code, &pending.code_verifier).await {
            Ok(t) => t,
            Err(e) => {
                log::error!("OIDC token exchange failed: {e:#}");
                return err(StatusCode::BAD_GATEWAY, "OIDC token exchange failed").into_response();
            }
        };

    let claims = match verify_id_token(
        &id_token,
        cfg,
        &pending.issuer,
        &pending.nonce,
        &pending.jwks_uri,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("OIDC id_token verification failed: {e:#}");
            return err(
                StatusCode::UNAUTHORIZED,
                "OIDC id_token verification failed",
            )
            .into_response();
        }
    };

    let display = display_username(&claims);
    let user = match auth.db.upsert_oidc_user(&claims.sub, Role::User) {
        Ok(u) => u,
        Err(e) => {
            log::error!("OIDC JIT user upsert failed: {e:#}");
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to provision user",
            )
            .into_response();
        }
    };
    if !user.enabled {
        return err(StatusCode::FORBIDDEN, "This account has been disabled").into_response();
    }

    let token = match auth
        .jwt
        .generate(&user.id, &display, user.role.clone(), user.token_version)
    {
        Ok(t) => t,
        Err(e) => {
            log::error!("OIDC JWT mint failed: {e:#}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to mint session")
                .into_response();
        }
    };

    // Fragment, not query: it is never sent to a server, logged, or put in a
    // Referer header.
    let redirect = format!("/sign-in#oidc_token={}", urlencoding::encode(&token));
    let mut resp = Redirect::temporary(&redirect).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&oidc_state_cookie("", 0, &cfg.redirect_uri)) {
        resp.headers_mut().insert(axum::http::header::SET_COOKIE, v);
    }
    resp
}

// ── API tokens ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateApiTokenRequest {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiTokenSummary {
    id: String,
    name: String,
    role: String,
    scopes: Vec<String>,
    expires_at: Option<String>,
    created_by: Option<String>,
    created: String,
    last_used: Option<String>,
    revoked: bool,
}

impl From<&ApiTokenRecord> for ApiTokenSummary {
    fn from(t: &ApiTokenRecord) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            role: role_str(&t.role).to_string(),
            scopes: t.scopes.clone(),
            expires_at: t.expires_at.clone(),
            created_by: t.created_by.clone(),
            created: t.created.clone(),
            last_used: t.last_used.clone(),
            revoked: t.revoked,
        }
    }
}

pub async fn list_api_tokens_handler(
    State(auth): State<SharedAuth>,
    _headers: HeaderMap,
) -> impl IntoResponse {
    match auth.db.list_api_tokens() {
        Ok(tokens) => {
            Json(tokens.iter().map(ApiTokenSummary::from).collect::<Vec<_>>()).into_response()
        }
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list tokens").into_response(),
    }
}

pub async fn create_api_token_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    Json(body): Json<CreateApiTokenRequest>,
) -> impl IntoResponse {
    if body.name.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "Token name required").into_response();
    }
    let Some(role) = parse_role(&body.role) else {
        return err(StatusCode::BAD_REQUEST, "Invalid role").into_response();
    };
    let created_by = bearer_token(&headers)
        .and_then(|t| auth.validate_bearer(&t))
        .map(|c| c.username);
    match auth.db.create_api_token(
        body.name.trim(),
        role,
        body.scopes,
        body.expires_at.as_deref(),
        created_by.as_deref(),
    ) {
        Ok((rec, plaintext)) => {
            let mut summary = serde_json::to_value(ApiTokenSummary::from(&rec))
                .unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = summary.as_object_mut() {
                obj.insert("token".into(), serde_json::json!(plaintext));
            }
            (StatusCode::CREATED, Json(summary)).into_response()
        }
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create token").into_response(),
    }
}

pub async fn revoke_api_token_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match auth.db.revoke_api_token(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Token not found").into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to revoke token").into_response(),
    }
}

pub async fn delete_api_token_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match auth.db.delete_api_token(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Token not found").into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete token").into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

/// `POST /v1/auth/password`: change your own password. Bumps `token_version`,
/// which revokes every session including this one; log in again afterwards.
pub async fn change_password_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let Ok(Some(user)) = auth.db.get_by_id(&claims.sub) else {
        return err(
            StatusCode::BAD_REQUEST,
            "This identity has no local password",
        )
        .into_response();
    };
    if !user.verify_password(&body.old_password).unwrap_or(false) {
        return err(StatusCode::UNAUTHORIZED, "Invalid password").into_response();
    }
    if body.new_password.len() < 8 || body.new_password.len() > 72 {
        return err(
            StatusCode::BAD_REQUEST,
            "Password must be 8 to 72 characters",
        )
        .into_response();
    }
    if auth
        .db
        .update_password(&user.id, &body.new_password)
        .is_err()
    {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update password",
        )
        .into_response();
    }
    Json(serde_json::json!({ "success": true, "sessions_revoked": true })).into_response()
}

/// Issue time in seconds, used as the `tv` of stateless PAM sessions.
fn pam_now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

/// `POST /v1/auth/logout`: revoke every session of the caller (token_version
/// bump). PAM sessions are revoked by a per-user not-before time.
pub async fn logout_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let revoked = match claims.sub.strip_prefix("pam:") {
        // Sessions issued up to and including this second are revoked.
        Some(name) => auth
            .db
            .revoke_pam_sessions(name, pam_now().saturating_add(1))
            .is_ok(),
        None => auth.db.bump_token_version(&claims.sub).is_ok(),
    };
    Json(serde_json::json!({ "success": true, "sessions_revoked": revoked })).into_response()
}

#[cfg(test)]
mod totp_pending_tests {
    use super::*;

    #[test]
    fn pending_secret_leaves_current_one_active_until_committed() {
        let db = UserDb::open(":memory:").unwrap();
        let u = db.create_user("carol", "Password-123", Role::User).unwrap();
        db.set_totp(&u.id, "OLDSECRET", true).unwrap();
        db.set_totp_pending(&u.id, Some("NEWSECRET")).unwrap();
        let mid = db.get_by_id(&u.id).unwrap().unwrap();
        assert!(
            mid.totp_enabled,
            "2FA stays on while a replacement is pending"
        );
        assert_eq!(mid.totp_secret.as_deref(), Some("OLDSECRET"));
        let tv = mid.token_version;
        db.commit_totp_pending(&u.id, "NEWSECRET").unwrap();
        let after = db.get_by_id(&u.id).unwrap().unwrap();
        assert!(after.totp_enabled);
        assert_eq!(after.totp_secret.as_deref(), Some("NEWSECRET"));
        assert_eq!(after.token_version, tv + 1, "commit revokes sessions");
        assert_eq!(db.get_totp_pending(&u.id).unwrap(), None);
    }
}

#[cfg(test)]
mod pam_revocation_tests {
    use super::*;

    #[test]
    fn pam_logout_revokes_earlier_sessions_only() {
        let db = UserDb::open(":memory:").unwrap();
        assert_eq!(db.pam_not_before("bob").unwrap(), 0);
        db.revoke_pam_sessions("bob", 1000).unwrap();
        assert_eq!(db.pam_not_before("bob").unwrap(), 1000);
        assert_eq!(db.pam_not_before("alice").unwrap(), 0);
    }
}

#[cfg(test)]
mod oidc_cookie_tests {
    use super::*;

    #[test]
    fn cookie_is_read_back_and_scoped() {
        let c = oidc_state_cookie("abc", 600, "https://z.example/cb");
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Lax") && c.contains("Secure"));
        assert!(!oidc_state_cookie("abc", 600, "http://lab/cb").contains("Secure"));
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::COOKIE,
            "a=1; zorvia_oidc_state=abc; b=2".parse().unwrap(),
        );
        assert_eq!(cookie_value(&h, OIDC_STATE_COOKIE), Some("abc"));
        assert_eq!(cookie_value(&h, "missing"), None);
    }
}

#[derive(Debug, Deserialize)]
pub struct SetNamespacesRequest {
    /// `null` removes the restriction.
    pub namespaces: Option<Vec<String>>,
}

fn valid_ns_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// `PUT /v1/users/{id}/namespaces` (users.admin). Admin accounts are never
/// restricted, so setting a list on one is refused rather than silently ignored.
pub async fn set_namespaces_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
    Json(body): Json<SetNamespacesRequest>,
) -> impl IntoResponse {
    let Ok(Some(user)) = auth.db.get_by_id(&id) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
    if let Some(list) = &body.namespaces {
        if matches!(user.role, Role::Admin) {
            return err(
                StatusCode::BAD_REQUEST,
                "Admin accounts cannot be namespace-restricted",
            )
            .into_response();
        }
        if list.len() > 64 || list.iter().any(|n| !valid_ns_name(n)) {
            return err(StatusCode::BAD_REQUEST, "Invalid namespace list").into_response();
        }
    }
    if auth
        .db
        .set_namespaces(&id, body.namespaces.as_deref())
        .is_err()
    {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to save namespaces",
        )
        .into_response();
    }
    Json(serde_json::json!({ "id": id, "namespaces": body.namespaces })).into_response()
}

pub async fn get_namespaces_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match auth.db.get_namespaces(&id) {
        Ok(n) => Json(serde_json::json!({ "id": id, "namespaces": n })).into_response(),
        Err(_) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to read namespaces",
        )
        .into_response(),
    }
}
