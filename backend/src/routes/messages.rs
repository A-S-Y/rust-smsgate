use axum::{extract::{Query, State}, http::HeaderMap, Json};
use chrono::Utc;
use serde_json::{json, Value};

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

    let messages = sqlx::query_as::<_, Message>(
        "SELECT * FROM messages
         WHERE ($1::text IS NULL OR direction = $1)
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR device_id = $3)
         ORDER BY COALESCE(received_at, created_at) DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(query.direction)
    .bind(query.status)
    .bind(query.device_id)
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
    require_auth(&headers, &state.config)?;
    let sms = sms_settings(&state).await?;

    let message = sqlx::query_as::<_, Message>(
        "INSERT INTO messages(direction, status, phone_number, message_content, recipient, device_id)
         VALUES ('sent', 'Pending', $1, $2, $1, $3)
         RETURNING *",
    )
    .bind(&input.phone_number)
    .bind(&input.message_content)
    .bind(&sms.device_id)
    .fetch_one(&state.db)
    .await?;

    let mut body = json!({
        "textMessage": { "text": input.message_content },
        "phoneNumbers": [input.phone_number],
    });
    if let Some(device_id) = &sms.device_id {
        body["deviceId"] = json!(device_id);
    }

    let res = state
        .http
        .post(format!("{}/3rdparty/v1/messages", sms.server_url.trim_end_matches('/')))
        .basic_auth(sms.username, Some(sms.password))
        .json(&body)
        .send()
        .await?;

    let status = res.status();
    let upstream: Value = res.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        mark_message_failed(&state, message.id, upstream.clone()).await?;
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
    .bind(message.id)
    .bind(external_id)
    .bind(upstream)
    .fetch_one(&state.db)
    .await?;

    let _ = state.realtime.send(RealtimeEvent::MessageCreated(updated.clone()));
    Ok(Json(json!({ "message": "Message queued successfully.", "data": updated })))
}

pub async fn import_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImportInboxRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_auth(&headers, &state.config)?;
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
        return Err(AppError::Upstream(format!("SMSGate inbox export failed with status {status}")));
    }

    Ok(Json(json!({
        "message": "Inbox export request accepted. Messages will arrive through WebSocket after webhooks are delivered.",
        "data": body,
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

async fn mark_message_failed(state: &AppState, id: uuid::Uuid, raw_payload: Value) -> AppResult<()> {
    sqlx::query("UPDATE messages SET status = 'Failed', raw_payload = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(raw_payload)
        .execute(&state.db)
        .await?;
    Ok(())
}
