use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    User,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: Role,
    pub exp: usize,
    pub jti: String,
}

pub struct JwtConfig {
    secret: String,
    expiration_hours: i64,
}

impl JwtConfig {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            expiration_hours: 24,
        }
    }

    pub fn from_env() -> Self {
        let secret = std::env::var("ZORVIA_JWT_SECRET").unwrap_or_else(|_| {
            let fallback = uuid::Uuid::new_v4().to_string();
            log::warn!("ZORVIA_JWT_SECRET unset; using ephemeral secret (tokens reset on restart)");
            fallback
        });
        Self::new(secret)
    }

    pub fn generate(&self, user_id: &str, username: &str, role: Role) -> Result<String> {
        let exp = (Utc::now() + Duration::hours(self.expiration_hours.max(1))).timestamp() as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            role,
            exp,
            jti: uuid::Uuid::new_v4().to_string(),
        };
        Ok(encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?)
    }

    pub fn validate(&self, token: &str) -> Result<Claims> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(data.claims)
    }
}
