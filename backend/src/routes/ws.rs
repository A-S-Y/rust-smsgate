use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query, State},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::{auth::verify_token, realtime::RealtimeEvent, state::AppState};

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

pub async fn ws_handler(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if verify_token(&state.config, &query.token).is_err() {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let mut rx = state.realtime.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if send_event(&mut sender, &event).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        if matches!(message, Message::Close(_)) {
            break;
        }
    }

    send_task.abort();
}

async fn send_event(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    event: &RealtimeEvent,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(event).unwrap_or_else(|_| "{}".into());
    sender.send(Message::Text(text.into())).await
}

use axum::response::IntoResponse;
