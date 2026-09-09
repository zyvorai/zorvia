//! Authentication: JWT login, bootstrap admin DB, TOTP 2FA, OIDC, PAM fallback.

pub mod handlers;
mod jwt;
mod oidc;
mod user_db;

pub use handlers::{
    login_handler, me_handler, oidc_callback_handler, oidc_login_handler, providers_handler,
    totp_disable_handler, totp_setup_handler, totp_verify_handler, AuthState, LoginRequest,
    OidcCallbackQuery, SharedAuth, TotpVerifyRequest,
};
pub use jwt::{Claims, JwtConfig, Role};
pub use oidc::OidcConfig;
pub use user_db::UserDb;
