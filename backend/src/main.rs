mod app_error;
mod auth;
mod config;
mod crypto;
mod models;
mod realtime;
mod routes;
mod state;

use std::{net::SocketAddr, time::Duration};

use axum::{routing::get, Router};
use config::Config;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow_free::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let state = AppState::new(config.clone(), db);
    let cors = CorsLayer::new()
        .allow_origin(config.frontend_origin.parse::<axum::http::HeaderValue>()?)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(routes::health::health))
        .merge(routes::api_router())
        .layer((
            TraceLayer::new_for_http(),
            TimeoutLayer::new(Duration::from_secs(30)),
            CompressionLayer::new(),
            cors,
        ))
        .with_state(state);

    let addr: SocketAddr = config.server_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "smsgate backend listening");
    axum::serve(listener, app).await?;

    Ok(())
}

mod anyhow_free {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
