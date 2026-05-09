use serde::Serialize;

use crate::models::Message;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum RealtimeEvent {
    #[serde(rename = "message.created")]
    MessageCreated(Message),
    #[serde(rename = "message.updated")]
    MessageUpdated(Message),
    #[serde(rename = "system.status")]
    SystemStatus { status: String },
}
