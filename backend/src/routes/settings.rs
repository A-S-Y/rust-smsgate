use axum::{extract::State, http::HeaderMap, Json};
use serde_json::json;

use crate::{
    app_error::AppResult,
    auth::require_auth,
    crypto::CryptoBox,
    models::{SettingsRequest, SettingsResponse},
    state::AppState,
};

pub async fn get_settings(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<SettingsResponse>> {
    require_auth(&headers, &state.config)?;
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);

    let response = SettingsResponse {
        server_url: get_setting(&state, &crypto, "server_url").await?,
        username: get_setting(&state, &crypto, "username").await?,
        password: get_setting(&state, &crypto, "password").await?,
        device_id: get_setting(&state, &crypto, "device_id").await?,
        webhook_public_url: get_setting(&state, &crypto, "webhook_public_url").await?,
        webhook_signing_key: get_setting(&state, &crypto, "webhook_signing_key").await?,
        messages_retention_days: get_setting(&state, &crypto, "messages_retention_days")
            .await?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(30),
        has_password: get_setting(&state, &crypto, "password").await?.is_some(),
        has_webhook_signing_key: get_setting(&state, &crypto, "webhook_signing_key").await?.is_some(),
    };

    Ok(Json(response))
}

pub async fn save_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SettingsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let actor = require_auth(&headers, &state.config)?;
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    let password_saved = input.password.as_ref().is_some_and(|value| !value.is_empty());
    let webhook_signing_key_saved = input
        .webhook_signing_key
        .as_ref()
        .is_some_and(|value| !value.is_empty());

    save_plain(&state, "server_url", input.server_url).await?;
    save_plain(&state, "username", input.username).await?;
    save_plain(&state, "device_id", input.device_id).await?;
    save_plain(&state, "webhook_public_url", input.webhook_public_url).await?;
    save_plain(
        &state,
        "messages_retention_days",
        input.messages_retention_days.map(|days| days.clamp(1, 3650).to_string()),
    )
    .await?;
    save_secret(&state, &crypto, "password", input.password).await?;
    save_secret(&state, &crypto, "webhook_signing_key", input.webhook_signing_key).await?;

    sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
        .bind(actor)
        .bind("settings.updated")
        .bind(json!({
            "keys": [
                "server_url",
                "username",
                "password",
                "device_id",
                "webhook_public_url",
                "webhook_signing_key",
                "messages_retention_days"
            ],
            "secrets_saved": {
                "password": password_saved,
                "webhook_signing_key": webhook_signing_key_saved
            }
        }))
        .execute(&state.db)
        .await?;

    Ok(Json(json!({ "message": "Settings saved successfully." })))
}

pub async fn get_setting(state: &AppState, crypto: &CryptoBox, key: &str) -> AppResult<Option<String>> {
    let row: Option<(Option<String>, bool)> =
        sqlx::query_as("SELECT value, encrypted FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(&state.db)
            .await?;

    match row {
        Some((Some(value), true)) => Ok(Some(crypto.decrypt(&value)?)),
        Some((value, false)) => Ok(value),
        _ => Ok(None),
    }
}

async fn save_plain(state: &AppState, key: &str, value: Option<String>) -> AppResult<()> {
    if let Some(value) = value {
        sqlx::query(
            "INSERT INTO settings(key, value, encrypted, updated_at)
             VALUES ($1, $2, false, now())
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, encrypted = false, updated_at = now()",
        )
        .bind(key)
        .bind(value)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

async fn save_secret(state: &AppState, crypto: &CryptoBox, key: &str, value: Option<String>) -> AppResult<()> {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        let encrypted = crypto.encrypt(&value)?;
        sqlx::query(
            "INSERT INTO settings(key, value, encrypted, updated_at)
             VALUES ($1, $2, true, now())
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, encrypted = true, updated_at = now()",
        )
        .bind(key)
        .bind(encrypted)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}
