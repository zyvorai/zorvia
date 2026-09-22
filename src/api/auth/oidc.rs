//! Secure OIDC (Authorization Code + PKCE + token exchange + JWKS).
//!
//! Opt-in via `ZORVIA_OIDC_ENABLED=1` plus issuer/client/secret/redirect.
//! Never mints a local JWT without a verified ID token from the IdP.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// OIDC provider config loaded from env when explicitly enabled.
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

#[derive(Debug, Clone)]
pub struct OidcEndpoints {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub issuer: String,
}

#[derive(Debug, Clone)]
pub struct PendingOidc {
    pub code_verifier: String,
    pub nonce: String,
    pub created_unix: u64,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub issuer: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct DiscoveryDocument {
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    issuer: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
    #[allow(dead_code)]
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    #[serde(rename = "use")]
    #[allow(dead_code)]
    use_: Option<String>,
    n: Option<String>,
    e: Option<String>,
    alg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    #[allow(dead_code)]
    pub iss: String,
    pub aud: Aud,
    pub exp: i64,
    #[allow(dead_code)]
    pub iat: Option<i64>,
    pub nonce: Option<String>,
    pub email: Option<String>,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Aud {
    One(String),
    Many(Vec<String>),
}

impl Aud {
    fn contains(&self, client_id: &str) -> bool {
        match self {
            Aud::One(a) => a == client_id,
            Aud::Many(v) => v.iter().any(|a| a == client_id),
        }
    }
}

impl OidcConfig {
    /// Load OIDC when `ZORVIA_OIDC_ENABLED=1` and required vars are present.
    pub fn from_env() -> Option<Self> {
        let enabled = match std::env::var("ZORVIA_OIDC_ENABLED") {
            Ok(v) => {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        };

        let issuer = std::env::var("ZORVIA_OIDC_ISSUER").ok();
        let client_id = std::env::var("ZORVIA_OIDC_CLIENT_ID").ok();

        if !enabled {
            if issuer.is_some() || client_id.is_some() {
                log::warn!(
                    "ZORVIA_OIDC_* is set but OIDC is off; set ZORVIA_OIDC_ENABLED=1 to activate \
                     (requires PKCE + token exchange + JWKS verification)"
                );
            }
            return None;
        }

        let issuer = issuer.filter(|s| !s.trim().is_empty())?;
        let client_id = client_id.filter(|s| !s.trim().is_empty())?;
        let client_secret = std::env::var("ZORVIA_OIDC_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let redirect_uri = std::env::var("ZORVIA_OIDC_REDIRECT_URI")
            .unwrap_or_else(|_| "https://127.0.0.1:30152/api/v1/auth/oidc/callback".to_string());
        let name = std::env::var("ZORVIA_OIDC_NAME").unwrap_or_else(|_| "OIDC".to_string());
        let scopes = std::env::var("ZORVIA_OIDC_SCOPES")
            .unwrap_or_else(|_| "openid profile email".to_string());

        log::info!(
            "OIDC enabled for issuer={} client_id={} redirect={}",
            issuer,
            client_id,
            redirect_uri
        );

        Some(Self {
            id: "default".into(),
            name,
            issuer: issuer.trim_end_matches('/').to_string(),
            client_id,
            client_secret,
            redirect_uri,
            scopes,
        })
    }

    pub fn discovery_url(&self) -> String {
        format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        )
    }
}

pub fn random_urlsafe(nbytes: usize) -> String {
    let mut buf = vec![0u8; nbytes];
    getrandom_fill(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn getrandom_fill(buf: &mut [u8]) {
    use rand::Rng;
    rand::rng().fill_bytes(buf);
}

pub fn pkce_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub fn build_authorize_url(
    authorization_endpoint: &str,
    cfg: &OidcConfig,
    state: &str,
    nonce: &str,
    code_challenge: &str,
) -> String {
    let mut url = authorization_endpoint.to_string();
    if !url.contains('?') {
        url.push('?');
    } else if !url.ends_with('&') && !url.ends_with('?') {
        url.push('&');
    }
    url.push_str(&format!(
        "response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(&cfg.redirect_uri),
        urlencoding::encode(&cfg.scopes),
        urlencoding::encode(state),
        urlencoding::encode(nonce),
        urlencoding::encode(code_challenge),
    ));
    url
}

pub async fn discover(cfg: &OidcConfig) -> anyhow::Result<OidcEndpoints> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let doc: DiscoveryDocument = client
        .get(cfg.discovery_url())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if doc.issuer.trim_end_matches('/') != cfg.issuer.trim_end_matches('/') {
        anyhow::bail!(
            "OIDC discovery issuer mismatch: got {} expected {}",
            doc.issuer,
            cfg.issuer
        );
    }
    Ok(OidcEndpoints {
        authorization_endpoint: doc.authorization_endpoint,
        token_endpoint: doc.token_endpoint,
        jwks_uri: doc.jwks_uri,
        issuer: doc.issuer,
    })
}

pub async fn exchange_code(
    cfg: &OidcConfig,
    token_endpoint: &str,
    code: &str,
    code_verifier: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let basic = STANDARD.encode(format!("{}:{}", cfg.client_id, cfg.client_secret));
    let resp = client
        .post(token_endpoint)
        .header("Authorization", format!("Basic {basic}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?
        .error_for_status()?;
    let body: TokenResponse = resp.json().await?;
    if let Some(err) = body.error {
        anyhow::bail!(
            "token endpoint error: {} ({})",
            err,
            body.error_description.unwrap_or_default()
        );
    }
    body.id_token
        .ok_or_else(|| anyhow::anyhow!("token response missing id_token"))
}

pub async fn verify_id_token(
    id_token: &str,
    cfg: &OidcConfig,
    expected_issuer: &str,
    expected_nonce: &str,
    jwks_uri: &str,
) -> anyhow::Result<IdTokenClaims> {
    let header = decode_header(id_token)?;
    let kid = header.kid;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let jwks: Jwks = client
        .get(jwks_uri)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| {
            k.kty == "RSA"
                && k.n.is_some()
                && k.e.is_some()
                && (kid.as_ref().is_none() || k.kid.as_ref() == kid.as_ref())
        })
        .ok_or_else(|| anyhow::anyhow!("no matching RSA JWK for id_token"))?;

    let n = jwk.n.as_deref().unwrap();
    let e = jwk.e.as_deref().unwrap();
    let key = DecodingKey::from_rsa_components(n, e)?;

    let mut validation = Validation::new(Algorithm::RS256);
    if let Some(alg) = jwk.alg.as_deref() {
        if alg == "RS384" {
            validation = Validation::new(Algorithm::RS384);
        } else if alg == "RS512" {
            validation = Validation::new(Algorithm::RS512);
        }
    }
    validation.set_issuer(&[expected_issuer.trim_end_matches('/')]);
    validation.validate_aud = false; // checked manually (string or array)
    validation.set_required_spec_claims(&["sub", "iss", "exp", "aud"]);

    let data = decode::<IdTokenClaims>(id_token, &key, &validation)?;
    let claims = data.claims;

    if !claims.aud.contains(&cfg.client_id) {
        anyhow::bail!("id_token aud does not include client_id");
    }
    let nonce = claims
        .nonce
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("id_token missing nonce"))?;
    if nonce != expected_nonce {
        anyhow::bail!("id_token nonce mismatch");
    }
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        anyhow::bail!("id_token expired");
    }
    Ok(claims)
}

pub fn display_username(claims: &IdTokenClaims) -> String {
    if let Some(u) = claims
        .preferred_username
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        return u.to_string();
    }
    if let Some(e) = claims.email.as_deref().filter(|s| !s.is_empty()) {
        return e.to_string();
    }
    if let Some(n) = claims.name.as_deref().filter(|s| !s.is_empty()) {
        return n.to_string();
    }
    format!("oidc-{}", claims.sub.chars().take(12).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_stable() {
        let v = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let c = pkce_challenge_s256(v);
        assert_eq!(c, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn from_env_disabled_without_flag() {
        // Assumes ZORVIA_OIDC_ENABLED is unset in the unit-test process.
        if std::env::var("ZORVIA_OIDC_ENABLED").is_ok() {
            return;
        }
        assert!(OidcConfig::from_env().is_none());
    }

    #[test]
    fn aud_contains() {
        assert!(Aud::One("a".into()).contains("a"));
        assert!(Aud::Many(vec!["x".into(), "a".into()]).contains("a"));
        assert!(!Aud::One("b".into()).contains("a"));
    }
}
