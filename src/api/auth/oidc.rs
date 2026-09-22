use serde::{Deserialize, Serialize};

/// OIDC provider config. Disabled in 0.3.3 — unsafe mint-without-exchange
/// implementation removed. Re-enable only with discovery, PKCE, token
/// exchange, issuer/audience/nonce verification, and JWKS rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    pub id: String,
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: String,
}

impl OidcConfig {
    /// Always returns `None`. `ZORVIA_OIDC_*` env vars are ignored until a
    /// secure OIDC implementation ships.
    pub fn from_env() -> Option<Self> {
        if std::env::var("ZORVIA_OIDC_ISSUER").is_ok()
            || std::env::var("ZORVIA_OIDC_CLIENT_ID").is_ok()
        {
            log::warn!(
                "ZORVIA_OIDC_* is set but OIDC is disabled in this release (unsafe callback removed). \
                 Enterprise OIDC will return in a later version."
            );
        }
        None
    }
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
}
