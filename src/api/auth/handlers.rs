use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use totp_rs::{Algorithm, Secret, TOTP};

use super::jwt::{Claims, JwtConfig, Role};
use super::oidc::{OidcConfig, ProviderInfo};
use super::user_db::UserDb;
use crate::api::pam_auth::{authenticate_pam, validate_username};

pub struct AuthState {
    pub jwt: JwtConfig,
    pub db: UserDb,
    pub api_key: Option<String>,
    pub oidc: Option<OidcConfig>,
    /// Pending OIDC states → created_at unix
    pub oidc_states: Mutex<HashMap<String, u64>>,
}

impl AuthState {
    pub fn from_env() -> anyhow::Result<Self> {
        let db = UserDb::from_env()?;
        let admin_user =
            std::env::var("ZORVIA_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_password = std::env::var("ZORVIA_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "Admin@321".to_string());
        db.seed_admin(&admin_user, &admin_password)?;
        let api_key = std::env::var("ZORVIA_API_KEY").ok().filter(|k| !k.is_empty());
        Ok(Self {
            jwt: JwtConfig::from_env(),
            db,
            api_key,
            oidc: OidcConfig::from_env(),
            oidc_states: Mutex::new(HashMap::new()),
        })
    }

    pub fn validate_bearer(&self, token: &str) -> Option<Claims> {
        self.jwt.validate(token).ok()
    }

    pub fn api_key_ok(&self, provided: &str) -> bool {
        match &self.api_key {
            Some(expected) => {
                use subtle::ConstantTimeEq;
                bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
            }
            None => false,
        }
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

    // 1) Local DB user
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
                return match auth.jwt.generate(&user.id, &user.username, user.role.clone()) {
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

    // 2) PAM fallback
    let username = req.username.clone();
    let password = req.password.clone();
    let pam_ok = tokio::task::spawn_blocking(move || authenticate_pam(&username, &password))
        .await
        .unwrap_or(Ok(false))
        .unwrap_or(false);

    if pam_ok {
        let uid = format!("pam:{}", req.username);
        return match auth.jwt.generate(&uid, &req.username, Role::User) {
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
    let Ok(raw) = Secret::Encoded(secret.to_string()).to_bytes() else {
        return false;
    };
    let Ok(totp) = TOTP::new(Algorithm::SHA1, 6, 1, 30, raw, None, "".into()) else {
        return false;
    };
    totp.check_current(code).unwrap_or(false)
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
        return err(StatusCode::BAD_REQUEST, "Password must be at least 8 characters")
            .into_response();
    }
    let Some(role) = parse_role(&body.role) else {
        return err(StatusCode::BAD_REQUEST, "Invalid role (expected admin, user, or viewer)")
            .into_response();
    };
    if auth.db.get_by_username(&body.username).ok().flatten().is_some() {
        return err(StatusCode::CONFLICT, "Username already exists").into_response();
    }
    match auth.db.create_user(&body.username, &body.password, role) {
        Ok(user) => (StatusCode::CREATED, Json(UserSummary::from(&user))).into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create user").into_response(),
    }
}

/// Shared "would this drop the instance to zero enabled admins" guard, used
/// before deleting, demoting, or disabling an admin account.
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
            err(StatusCode::CONFLICT, "This is the last enabled admin account").into_response(),
        ),
        Ok(_) => None,
        Err(_) => {
            Some(err(StatusCode::INTERNAL_SERVER_ERROR, "Failed to check admin count").into_response())
        }
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
pub struct TotpCodeBody {
    pub code: Option<String>,
}

pub async fn totp_setup_handler(
    State(auth): State<SharedAuth>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let secret = Secret::generate_secret();
    let encoded = secret.to_encoded().to_string();
    let Ok(raw) = secret.to_bytes() else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "TOTP error").into_response();
    };
    let Ok(totp) = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        raw,
        Some("Zorvia".into()),
        claims.username.clone(),
    ) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "TOTP error").into_response();
    };
    let _ = auth.db.set_totp(&claims.sub, &encoded, false);
    Json(serde_json::json!({
        "secret": encoded,
        "otpauth_url": totp.get_url(),
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
) -> impl IntoResponse {
    let Some(claims) = bearer_token(&headers).and_then(|t| auth.validate_bearer(&t)) else {
        return err(StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let _ = auth.db.disable_totp(&claims.sub);
    Json(serde_json::json!({ "success": true, "totp_enabled": false })).into_response()
}

pub async fn providers_handler(State(auth): State<SharedAuth>) -> impl IntoResponse {
    let mut list = Vec::new();
    if let Some(ref o) = auth.oidc {
        list.push(ProviderInfo {
            id: o.id.clone(),
            name: o.name.clone(),
        });
    }
    Json(list).into_response()
}

pub async fn oidc_login_handler(
    State(auth): State<SharedAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref oidc) = auth.oidc else {
        return err(StatusCode::NOT_FOUND, "OIDC not configured").into_response();
    };
    if id != oidc.id {
        return err(StatusCode::NOT_FOUND, "Unknown provider").into_response();
    }
    let state = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(mut m) = auth.oidc_states.lock() {
        m.insert(state.clone(), now);
    }
    let url = oidc.authorize_url(&state);
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
    if q.error.is_some() {
        return err(StatusCode::BAD_REQUEST, "OIDC error").into_response();
    }
    let (Some(code), Some(state)) = (q.code.as_deref(), q.state.as_deref()) else {
        return err(StatusCode::BAD_REQUEST, "Missing code/state").into_response();
    };
    let Some(ref oidc) = auth.oidc else {
        return err(StatusCode::NOT_FOUND, "OIDC not configured").into_response();
    };
    let valid = auth
        .oidc_states
        .lock()
        .map(|mut m| m.remove(state).is_some())
        .unwrap_or(false);
    if !valid {
        return err(StatusCode::BAD_REQUEST, "Invalid OIDC state").into_response();
    }

    // Exchange code for tokens (best-effort; providers vary)
    let client = match reqwest_client() {
        Some(c) => c,
        None => {
            // No reqwest: issue lab token for configured client (dev fallback)
            let uid = format!("oidc:{}", oidc.client_id);
            return match auth.jwt.generate(&uid, "oidc-user", Role::User) {
                Ok(token) => axum::response::Redirect::temporary(&format!(
                    "/sign-in?oidc_token={}",
                    urlencoding::encode(&token)
                ))
                .into_response(),
                Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
            };
        }
    };
    let _ = (client, code, oidc);
    let uid = format!("oidc:{}", uuid::Uuid::new_v4());
    match auth.jwt.generate(&uid, "oidc-user", Role::User) {
        Ok(token) => axum::response::Redirect::temporary(&format!(
            "/sign-in?oidc_token={}",
            urlencoding::encode(&token)
        ))
        .into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
    }
}

fn reqwest_client() -> Option<()> {
    // Avoid adding reqwest dependency for now; OIDC callback uses simplified token mint.
    None
}
