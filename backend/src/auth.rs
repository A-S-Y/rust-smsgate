use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::{app_error::{AppError, AppResult}, config::Config};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub fn create_token(config: &Config, username: &str) -> AppResult<String> {
    let claims = Claims {
        sub: username.to_string(),
        exp: (Utc::now() + Duration::hours(12)).timestamp() as usize,
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.app_secret.as_bytes()),
    )?)
}

pub fn verify_login(config: &Config, username: &str, password: &str) -> AppResult<()> {
    if username != config.admin_username {
        return Err(AppError::Unauthorized("Invalid credentials.".into()));
    }

    let ok = bcrypt::verify(password, &config.admin_password_hash)
        .map_err(|_| AppError::Internal("Invalid ADMIN_PASSWORD_HASH.".into()))?;

    if !ok {
        return Err(AppError::Unauthorized("Invalid credentials.".into()));
    }

    Ok(())
}

pub fn require_auth(headers: &HeaderMap, config: &Config) -> AppResult<String> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing bearer token.".into()))?;

    verify_token(config, token)
}

pub fn verify_token(config: &Config, token: &str) -> AppResult<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.app_secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims.sub)
}
