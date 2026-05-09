use axum::{extract::{Query, State}, http::HeaderMap, Json};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    auth::require_auth,
    crypto::CryptoBox,
    models::{ImportInboxRequest, Message, MessageQuery, SendMessageRequest},
    realtime::RealtimeEvent,
    routes::settings::get_setting,
    state::AppState,
};

pub async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> AppResult<Json<Vec<Message>>> {
    require_auth(&headers, &state.config)?;

    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    let retention_days = get_setting(&state, &crypto, "messages_retention_days")
        .await?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 3650);
    let from = Utc::now() - Duration::days(retention_days);

    let messages = sqlx::query_as::<_, Message>(
        "SELECT * FROM messages
         WHERE ($1::text IS NULL OR direction = $1)
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR device_id = $3)
           AND COALESCE(received_at, created_at) >= $4
         ORDER BY COALESCE(received_at, created_at) DESC
         LIMIT $5 OFFSET $6",
    )
    .bind(query.direction)
    .bind(query.status)
    .bind(query.device_id)
    .bind(from)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(messages))
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SendMessageRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let actor = require_auth(&headers, &state.config)?;
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    let device_id = get_setting(&state, &crypto, "device_id").await?;
    let normalized_phone = normalize_phone_number(&input.phone_number);
    let normalized_input = SendMessageRequest {
        phone_number: normalized_phone.clone(),
        message_content: input.message_content.clone(),
    };

    let message = sqlx::query_as::<_, Message>(
        "INSERT INTO messages(direction, status, phone_number, message_content, recipient, device_id)
         VALUES ('sent', 'Queued', $1, $2, $1, $3)
         RETURNING *",
    )
    .bind(&normalized_phone)
    .bind(&input.message_content)
    .bind(&device_id)
    .fetch_one(&state.db)
    .await?;

    let _ = state.realtime.send(RealtimeEvent::MessageCreated(message.clone()));
    sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
        .bind(actor)
        .bind("messages.send.queued")
        .bind(json!({
            "message_id": message.id,
            "phone_number": message.phone_number.clone(),
            "status": message.status.clone()
        }))
        .execute(&state.db)
        .await?;

    let queued = message.clone();
    let state_for_send = state.clone();
    tokio::spawn(async move {
        if let Err(error) = deliver_outgoing_message(state_for_send, queued.id, normalized_input).await {
            eprintln!("failed to deliver outgoing SMS: {error}");
        }
    });

    Ok(Json(json!({ "message": "Message queued locally.", "data": message })))
}

async fn deliver_outgoing_message(
    state: AppState,
    message_id: Uuid,
    input: SendMessageRequest,
) -> AppResult<()> {
    let sms = match sms_settings(&state).await {
        Ok(sms) => sms,
        Err(error) => {
            let failed = mark_message_failed(
                &state,
                message_id,
                json!({ "error": error.to_string(), "stage": "settings" }),
            )
            .await?;
            sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
                .bind("system")
                .bind("messages.send.failed")
                .bind(json!({
                    "message_id": failed.id,
                    "phone_number": failed.phone_number.clone(),
                    "error": error.to_string(),
                    "stage": "settings"
                }))
                .execute(&state.db)
                .await?;
            let _ = state.realtime.send(RealtimeEvent::MessageUpdated(failed));
            return Err(error);
        }
    };
    let mut body = json!({
        "textMessage": { "text": input.message_content },
        "phoneNumbers": [input.phone_number],
    });
    if let Some(device_id) = &sms.device_id {
        body["deviceId"] = json!(device_id);
    }

    let res = match state
        .http
        .post(format!("{}/3rdparty/v1/messages", sms.server_url.trim_end_matches('/')))
        .basic_auth(sms.username, Some(sms.password))
        .json(&body)
        .send()
        .await
    {
        Ok(res) => res,
        Err(error) => {
            let failed = mark_message_failed(
                &state,
                message_id,
                json!({ "error": error.to_string(), "stage": "request" }),
            )
            .await?;
            sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
                .bind("system")
                .bind("messages.send.failed")
                .bind(json!({
                    "message_id": failed.id,
                    "phone_number": failed.phone_number.clone(),
                    "error": error.to_string(),
                    "stage": "request"
                }))
                .execute(&state.db)
                .await?;
            let _ = state.realtime.send(RealtimeEvent::MessageUpdated(failed));
            return Err(AppError::Upstream(format!("SMSGate send request failed: {error}")));
        }
    };

    let status = res.status();
    let upstream: Value = res.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        let failed = mark_message_failed(&state, message_id, upstream.clone()).await?;
        sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
            .bind("system")
            .bind("messages.send.failed")
            .bind(json!({
                "message_id": failed.id,
                "phone_number": failed.phone_number.clone(),
                "upstream_status": status.as_u16(),
                "upstream": upstream
            }))
            .execute(&state.db)
            .await?;
        let _ = state.realtime.send(RealtimeEvent::MessageUpdated(failed.clone()));
        return Err(AppError::Upstream(format!("SMSGate send failed with status {status}")));
    }

    let external_id = upstream
        .get("id")
        .or_else(|| upstream.get("messageId"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let updated = sqlx::query_as::<_, Message>(
        "UPDATE messages
         SET status = 'Sent to Server', message_id = $2, raw_payload = $3, updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(message_id)
    .bind(external_id)
    .bind(upstream)
    .fetch_one(&state.db)
    .await?;

    let _ = state.realtime.send(RealtimeEvent::MessageUpdated(updated.clone()));
    sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
        .bind("system")
        .bind("messages.send.accepted")
        .bind(json!({
            "message_id": updated.id,
            "phone_number": updated.phone_number.clone(),
            "external_message_id": updated.message_id.clone(),
            "upstream": updated.raw_payload.clone()
        }))
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn import_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImportInboxRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let actor = require_auth(&headers, &state.config)?;
    if input.until < input.since {
        return Err(AppError::BadRequest("until must be after since.".into()));
    }

    let sms = sms_settings(&state).await?;
    let device_id = input.device_id.or(sms.device_id).ok_or_else(|| {
        AppError::BadRequest("Device ID is required for inbox export.".into())
    })?;

    let res = state
        .http
        .post(format!(
            "{}/3rdparty/v1/messages/inbox/export",
            sms.server_url.trim_end_matches('/')
        ))
        .basic_auth(sms.username, Some(sms.password))
        .json(&json!({
            "deviceId": device_id,
            "since": input.since.to_rfc3339(),
            "until": input.until.to_rfc3339(),
        }))
        .send()
        .await?;

    let status = res.status();
    let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
            .bind("system")
            .bind("messages.import.failed")
            .bind(json!({
                "upstream_status": status.as_u16(),
                "upstream": body
            }))
            .execute(&state.db)
            .await?;
        return Err(AppError::Upstream(format!("SMSGate inbox export failed with status {status}")));
    }

    sqlx::query("INSERT INTO audit_logs(actor, action, metadata) VALUES ($1, $2, $3)")
        .bind(actor)
        .bind("messages.import.requested")
        .bind(json!({
            "device_id": device_id,
            "since": input.since,
            "until": input.until,
            "upstream": body
        }))
        .execute(&state.db)
        .await?;

    Ok(Json(json!({
        "message": "Inbox export request accepted. Messages will arrive through WebSocket after webhooks are delivered.",
        "requested_at": Utc::now(),
    })))
}

pub struct SmsSettings {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub device_id: Option<String>,
}

pub async fn sms_settings(state: &AppState) -> AppResult<SmsSettings> {
    let crypto = CryptoBox::new(&state.config.settings_encryption_key);
    Ok(SmsSettings {
        server_url: get_setting(state, &crypto, "server_url")
            .await?
            .unwrap_or_else(|| "https://api.sms-gate.app".into()),
        username: get_setting(state, &crypto, "username")
            .await?
            .ok_or_else(|| AppError::BadRequest("SMSGate username is not configured.".into()))?,
        password: get_setting(state, &crypto, "password")
            .await?
            .ok_or_else(|| AppError::BadRequest("SMSGate password is not configured.".into()))?,
        device_id: get_setting(state, &crypto, "device_id").await?,
    })
}

async fn mark_message_failed(state: &AppState, id: uuid::Uuid, raw_payload: Value) -> AppResult<Message> {
    let message = sqlx::query_as::<_, Message>(
        "UPDATE messages
         SET status = 'Failed', raw_payload = $2, updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
        .bind(id)
        .bind(raw_payload)
        .fetch_one(&state.db)
        .await?;
    Ok(message)
}

fn normalize_phone_number(value: &str) -> String {
    let compact: String = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '+')
        .collect();

    if compact.starts_with('+') {
        return compact;
    }

    if let Some(rest) = compact.strip_prefix("00") {
        return format!("+{rest}");
    }

    if compact.starts_with("967") {
        return format!("+{compact}");
    }

    if compact.starts_with("07") && compact.len() == 10 {
        return format!("+967{}", compact.trim_start_matches('0'));
    }

    if compact.starts_with('7') && compact.len() == 9 {
        return format!("+967{compact}");
    }

    compact
}

#[cfg(test)]
mod tests {
    use super::normalize_phone_number;

    #[test]
    fn normalizes_yemeni_mobile_numbers() {
        assert_eq!(normalize_phone_number("783285859"), "+967783285859");
        assert_eq!(normalize_phone_number("0783285859"), "+967783285859");
        assert_eq!(normalize_phone_number("967783285859"), "+967783285859");
        assert_eq!(normalize_phone_number("00967783285859"), "+967783285859");
        assert_eq!(normalize_phone_number("+967783285859"), "+967783285859");
    }
}
