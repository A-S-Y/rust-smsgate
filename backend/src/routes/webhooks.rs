use axum::{body::Bytes, extract::State, http::HeaderMap, Json};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_error::{AppError, AppResult},
    auth::require_auth,
    crypto::CryptoBox,
    models::{Message, SmsGateWebhook},
    realtime::RealtimeEvent,
    routes::{messages::sms_settings, settings::get_setting},
    state::AppState,
};

type HmacSha256 = Hmac<Sha256>;

pub async fn smsgate_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<serde_json::Value>> {
    validate_signature(&state, &headers, &body).await?;

    let webhook: SmsGateWebhook = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("Invalid SMSGate webhook JSON.".into()))?;
    let raw: Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("Invalid SMSGate webhook JSON.".into()))?;
    let event_id = webhook.id.clone().unwrap_or_else(|| fallback_event_id(&webhook));

    let inserted_event = sqlx::query(
        "INSERT INTO webhook_events(event_id, event, device_id, webhook_id, payload)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(&event_id)
    .bind(&webhook.event)
    .bind(&webhook.device_id)
    .bind(&webhook.webhook_id)
    .bind(&raw)
    .execute(&state.db)
    .await?
    .rows_affected();

    if inserted_event == 0 {
        return Ok(Json(json!({ "status": "duplicate", "event_id": event_id })));
    }

    match webhook.event.as_str() {
        "sms:received" => {
            let message = store_received_message(&state, &webhook, &event_id, raw).await?;
            let _ = state.realtime.send(RealtimeEvent::MessageCreated(message.clone()));
            Ok(Json(json!({ "status": "success", "data": message })))
        }
        "sms:sent" | "sms:delivered" | "sms:failed" => {
            if let Some(message) = update_outgoing_message(&state, &webhook, raw).await? {
                let _ = state.realtime.send(RealtimeEvent::MessageUpdated(message.clone()));
                Ok(Json(json!({ "status": "success", "data": message })))
            } else {
                Ok(Json(json!({ "status": "ignored" })))
            }
        }
        _ => Ok(Json(json!({ "status": "ignored" }))),
    }
}

pub async fn register_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let actor = require_auth(&headers, &state.config)?;
    let sms = sms_settings(&state).await?;
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    let public_url = get_setting(&state, &crypto, "webhook_public_url")
        .await?
        .unwrap_or_else(|| state.config.public_base_url.clone());
    let webhook_url = normalize_webhook_url(&public_url);

    if !webhook_url.starts_with("https://") {
        return Err(AppError::BadRequest(
            "Cloud webhooks require a public HTTPS URL.".into(),
        ));
    }

    let mut payload = json!({
        "id": "smsgate-received",
        "event": "sms:received",
        "url": webhook_url,
    });

    if let Some(device_id) = sms.device_id {
        payload["deviceId"] = json!(device_id);
    }

    let res = state
        .http
        .post(format!("{}/3rdparty/v1/webhooks", sms.server_url.trim_end_matches('/')))
        .basic_auth(sms.username, Some(sms.password))
        .json(&payload)
        .send()
        .await?;

    let status = res.status();
    let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(AppError::Upstream(format!("SMSGate webhook registration failed with status {status}")));
    }

    sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
        .bind(actor)
        .bind("webhook.registered")
        .bind(json!({ "url": webhook_url }))
        .execute(&state.db)
        .await?;

    Ok(Json(json!({
        "message": "Webhook registered successfully.",
        "webhook_url": payload["url"],
        "data": body,
    })))
}

async fn validate_signature(state: &AppState, headers: &HeaderMap, body: &[u8]) -> AppResult<()> {
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    let Some(signing_key) = get_setting(state, &crypto, "webhook_signing_key").await? else {
        return Ok(());
    };

    let timestamp = headers
        .get("X-Timestamp")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Timestamp header.".into()))?;
    let signature = headers
        .get("X-Signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Signature header.".into()))?;

    let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes())
        .map_err(|_| AppError::Internal("Invalid webhook signing key.".into()))?;
    mac.update(body);
    mac.update(timestamp.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    let provided = signature.strip_prefix("sha256=").unwrap_or(signature);

    if expected != provided {
        return Err(AppError::Unauthorized("Invalid SMSGate webhook signature.".into()));
    }

    Ok(())
}

async fn store_received_message(
    state: &AppState,
    webhook: &SmsGateWebhook,
    event_id: &str,
    raw: Value,
) -> AppResult<Message> {
    let payload = &webhook.payload;
    let sender = str_value(payload, "sender").or_else(|| str_value(payload, "phoneNumber"));
    let message_id = str_value(payload, "messageId");
    let text = str_value(payload, "message").unwrap_or_default();
    let received_at = str_value(payload, "receivedAt")
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc));

    let message = sqlx::query_as::<_, Message>(
        "INSERT INTO messages(
            direction, status, phone_number, message_content, message_id, webhook_event_id,
            device_id, sender, recipient, sim_number, received_at, raw_payload
         )
         VALUES ('received', 'Received', $1, $2, $3, $4, $5, $1, $6, $7, $8, $9)
         ON CONFLICT (webhook_event_id) DO UPDATE SET
            raw_payload = EXCLUDED.raw_payload,
            updated_at = now()
         RETURNING *",
    )
    .bind(sender.clone().unwrap_or_else(|| "Unknown".into()))
    .bind(text)
    .bind(message_id)
    .bind(event_id)
    .bind(&webhook.device_id)
    .bind(str_value(payload, "recipient"))
    .bind(payload.get("simNumber").and_then(Value::as_i64).map(|value| value as i32))
    .bind(received_at)
    .bind(raw)
    .fetch_one(&state.db)
    .await?;

    Ok(message)
}

async fn update_outgoing_message(
    state: &AppState,
    webhook: &SmsGateWebhook,
    raw: Value,
) -> AppResult<Option<Message>> {
    let Some(message_id) = str_value(&webhook.payload, "messageId") else {
        return Ok(None);
    };

    let status = match webhook.event.as_str() {
        "sms:sent" => "Sent",
        "sms:delivered" => "Delivered",
        "sms:failed" => "Failed",
        _ => "Updated",
    };

    let message = sqlx::query_as::<_, Message>(
        "UPDATE messages
         SET status = $2, raw_payload = $3, updated_at = now()
         WHERE message_id = $1
         RETURNING *",
    )
    .bind(message_id)
    .bind(status)
    .bind(raw)
    .fetch_optional(&state.db)
    .await?;

    Ok(message)
}

fn str_value(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn fallback_event_id(webhook: &SmsGateWebhook) -> String {
    let mut hasher = Sha256::new();
    hasher.update(webhook.event.as_bytes());
    if let Some(device_id) = &webhook.device_id {
        hasher.update(device_id.as_bytes());
    }
    hasher.update(webhook.payload.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_webhook_url(public_url: &str) -> String {
    let url = public_url.trim_end_matches('/');
    if url.ends_with("/api/webhooks/smsgate") {
        url.to_string()
    } else {
        format!("{url}/api/webhooks/smsgate")
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_webhook_url;

    #[test]
    fn normalizes_webhook_url() {
        assert_eq!(
            normalize_webhook_url("https://example.com"),
            "https://example.com/api/webhooks/smsgate"
        );
        assert_eq!(
            normalize_webhook_url("https://example.com/api/webhooks/smsgate"),
            "https://example.com/api/webhooks/smsgate"
        );
    }
}
