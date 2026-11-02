use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

use crate::{config::Config, error::AppError};

pub const ISSUER: &str = "open_kanban";
pub const TOKEN_TYPE: &str = "Bearer";
const ACCESS_ALGORITHM: Algorithm = Algorithm::HS256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// User id.
    pub sub: String,
    /// Session id the token was issued for.
    pub sid: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue_access_token(
    config: &Config,
    user_id: &str,
    session_id: &str,
) -> Result<(String, u64), AppError> {
    let issued_at = OffsetDateTime::now_utc();
    let expires_at = issued_at + Duration::seconds(config.access_token_ttl_seconds as i64);

    let claims = AccessClaims {
        sub: user_id.to_owned(),
        sid: session_id.to_owned(),
        iss: ISSUER.to_owned(),
        iat: issued_at.unix_timestamp(),
        exp: expires_at.unix_timestamp(),
    };

    let token = encode(
        &Header::new(ACCESS_ALGORITHM),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|error| AppError::Internal(anyhow::anyhow!("cannot sign access token: {error}")))?;

    Ok((token, config.access_token_ttl_seconds))
}

pub fn verify_access_token(config: &Config, token: &str) -> Result<AccessClaims, AppError> {
    let mut validation = Validation::new(ACCESS_ALGORITHM);
    validation.set_issuer(&[ISSUER]);
    validation.set_required_spec_claims(&["exp", "iss", "sub"]);

    decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::unauthorized("INVALID_TOKEN", "access token is invalid or expired"))
}

#[derive(Debug, Clone)]
pub struct RefreshToken {
    pub raw: String,
    pub hash: String,
}

/// 256 bits of randomness; only the hash is stored in the database.
pub fn generate_refresh_token() -> RefreshToken {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let raw = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_refresh_token(&raw);

    RefreshToken { raw, hash }
}

pub fn hash_refresh_token(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::from_lookup(|key| match key {
            "OK_JWT_SECRET" => Some("unit-test-secret-with-at-least-32-chars".to_owned()),
            _ => None,
        })
        .expect("valid test config")
    }

    #[test]
    fn access_tokens_round_trip() {
        let config = config();
        let (token, ttl) = issue_access_token(&config, "user-1", "session-1").expect("token");

        assert_eq!(ttl, config.access_token_ttl_seconds);

        let claims = verify_access_token(&config, &token).expect("claims");
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.sid, "session-1");
        assert_eq!(claims.iss, ISSUER);
    }

    #[test]
    fn tampered_tokens_are_rejected() {
        let config = config();
        let (token, _) = issue_access_token(&config, "user-1", "session-1").expect("token");
        let tampered = format!("{token}x");

        let error = verify_access_token(&config, &tampered).expect_err("must fail");
        assert!(error.to_string().contains("invalid"));
    }

    #[test]
    fn refresh_tokens_are_random_and_hashed() {
        let first = generate_refresh_token();
        let second = generate_refresh_token();

        assert_ne!(first.raw, second.raw);
        assert_ne!(first.raw, first.hash);
        assert_eq!(first.hash, hash_refresh_token(&first.raw));
    }
}
