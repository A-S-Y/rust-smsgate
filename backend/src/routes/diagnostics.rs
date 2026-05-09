use axum::{extract::State, http::HeaderMap, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_error::AppResult,
    auth::require_auth,
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WebhookLogRow {
    event_id: String,
    event: String,
    device_id: Option<String>,
    webhook_id: Option<String>,
    received_at: DateTime<Utc>,
    payload: Value,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AuditLogRow {
    actor: Option<String>,
    action: String,
    metadata: Option<Value>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct OutgoingLogRow {
    id: Uuid,
    status: String,
    phone_number: String,
    message_id: Option<String>,
    webhook_event_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    raw_payload: Option<Value>,
}

pub async fn sync_log(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    require_auth(&headers, &state.config)?;

    let recent_webhooks = sqlx::query_as::<_, WebhookLogRow>(
        "SELECT event_id, event, device_id, webhook_id, received_at, payload
         FROM webhook_events
         ORDER BY received_at DESC
         LIMIT 30",
    )
    .fetch_all(&state.db)
    .await?;

    let recent_audit_logs = sqlx::query_as::<_, AuditLogRow>(
        "SELECT actor, action, metadata, created_at
         FROM audit_logs
         WHERE action LIKE 'messages.%'
            OR action LIKE 'webhook.%'
            OR action LIKE 'settings.%'
         ORDER BY created_at DESC
         LIMIT 40",
    )
    .fetch_all(&state.db)
    .await?;

    let recent_outgoing_messages = sqlx::query_as::<_, OutgoingLogRow>(
        "SELECT id, status, phone_number, message_id, webhook_event_id, created_at, updated_at, raw_payload
         FROM messages
         WHERE direction = 'sent'
         ORDER BY updated_at DESC
         LIMIT 30",
    )
    .fetch_all(&state.db)
    .await?;

    let outgoing_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE direction = 'sent'")
        .fetch_one(&state.db)
        .await?;
    let received_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE direction = 'received'")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(json!({
        "summary": {
            "sent_messages": outgoing_count.0,
            "received_messages": received_count.0,
            "recent_webhooks": recent_webhooks.len(),
            "recent_audit_logs": recent_audit_logs.len()
        },
        "recent_webhooks": recent_webhooks,
        "recent_audit_logs": recent_audit_logs,
        "recent_outgoing_messages": recent_outgoing_messages
    })))
}
