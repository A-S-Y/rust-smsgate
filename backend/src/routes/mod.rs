pub mod auth;
pub mod health;
pub mod messages;
pub mod settings;
pub mod webhooks;
pub mod ws;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/settings", get(settings::get_settings).post(settings::save_settings))
        .route("/api/messages", get(messages::list_messages))
        .route("/api/messages/send", post(messages::send_message))
        .route("/api/messages/import-inbox", post(messages::import_inbox))
        .route("/api/webhooks/smsgate", post(webhooks::smsgate_webhook))
        .route("/api/webhooks/smsgate/register", post(webhooks::register_webhook))
        .route("/api/ws", get(ws::ws_handler))
}
