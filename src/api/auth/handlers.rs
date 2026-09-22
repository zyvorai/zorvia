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
        if claims.sub.starts_with("pam:") {
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
        Some(AuthIdentity::from_jwt(
            claims.sub,
            claims.username,
            claims.role,
        ))
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

pub async fn login_handler(
    State(auth): State<SharedAuth>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
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
        return match auth.jwt.generate(&uid, &req.username, Role::User, 0) {
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

pub async fn totp_setup_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
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
    let _ = auth.db.set_totp(&claims.sub, &encoded, false);
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
    let Ok(Some(user)) = auth.db.get_by_username(&claims.username) else {
        return err(StatusCode::NOT_FOUND, "User not found").into_response();
    };
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
    Json(serde_json::json!({ "url": url })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn oidc_callback_handler(
    State(auth): State<SharedAuth>,
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

    let redirect = format!("/sign-in?oidc_token={}", urlencoding::encode(&token));
    Redirect::temporary(&redirect).into_response()
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
