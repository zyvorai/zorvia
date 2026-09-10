use serde::{Deserialize, Serialize};

/// OIDC provider config from environment (single provider for lab).
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
    pub fn from_env() -> Option<Self> {
        let issuer = std::env::var("ZORVIA_OIDC_ISSUER").ok()?.trim().to_string();
        let client_id = std::env::var("ZORVIA_OIDC_CLIENT_ID")
            .ok()?
            .trim()
            .to_string();
        let client_secret = std::env::var("ZORVIA_OIDC_CLIENT_SECRET")
            .ok()?
            .trim()
            .to_string();
        if issuer.is_empty() || client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        let redirect_uri = std::env::var("ZORVIA_OIDC_REDIRECT_URI")
            .unwrap_or_else(|_| "https://127.0.0.1:30152/api/v1/auth/oidc/callback".to_string());
        let name = std::env::var("ZORVIA_OIDC_NAME").unwrap_or_else(|_| "OIDC".to_string());
        Some(Self {
            id: "default".into(),
            name,
            issuer,
            client_id,
            client_secret,
            redirect_uri,
            scopes: "openid profile email".into(),
        })
    }

    pub fn authorize_url(&self, state: &str) -> String {
        let base = self.issuer.trim_end_matches('/');
        format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            base,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&self.scopes),
            urlencoding::encode(state),
        )
    }

    pub fn token_url(&self) -> String {
        format!("{}/token", self.issuer.trim_end_matches('/'))
    }
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
}
