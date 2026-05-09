use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::broadcast;

use crate::{config::Config, realtime::RealtimeEvent};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PgPool,
    pub http: Client,
    pub realtime: broadcast::Sender<RealtimeEvent>,
}

impl AppState {
    pub fn new(config: Config, db: PgPool) -> Self {
        let (realtime, _) = broadcast::channel(512);
        Self {
            config,
            db,
            http: Client::new(),
            realtime,
        }
    }
}
