use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct Message {
    pub id: Uuid,
    pub direction: String,
    pub status: String,
    pub phone_number: String,
    pub message_content: String,
    pub message_id: Option<String>,
    pub webhook_event_id: Option<String>,
    pub device_id: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub sim_number: Option<i32>,
    pub received_at: Option<DateTime<Utc>>,
    pub raw_payload: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub device_id: Option<String>,
    pub webhook_public_url: Option<String>,
    pub messages_retention_days: i64,
    pub has_password: bool,
    pub has_webhook_signing_key: bool,
}

#[derive(Debug, Deserialize)]
pub struct SettingsRequest {
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub device_id: Option<String>,
    pub webhook_public_url: Option<String>,
    pub webhook_signing_key: Option<String>,
    pub messages_retention_days: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SendMessageRequest {
    pub phone_number: String,
    pub message_content: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportInboxRequest {
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub direction: Option<String>,
    pub status: Option<String>,
    pub device_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmsGateWebhook {
    pub id: Option<String>,
    pub event: String,
    pub device_id: Option<String>,
    pub webhook_id: Option<String>,
    pub payload: Value,
}
