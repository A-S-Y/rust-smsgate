use std::env;

#[derive(Clone)]
pub struct Config {
    pub server_addr: String,
    pub database_url: String,
    pub public_base_url: String,
    pub app_secret: String,
    pub settings_encryption_key: String,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub frontend_origin: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            server_addr: env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            database_url: env::var("DATABASE_URL")?,
            public_base_url: env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://alsiyniisms.ddns.net".into()),
            app_secret: env::var("APP_SECRET")?,
            settings_encryption_key: env::var("SETTINGS_ENCRYPTION_KEY")?,
            admin_username: env::var("ADMIN_USERNAME")?,
            admin_password_hash: env::var("ADMIN_PASSWORD_HASH")?,
            frontend_origin: env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:5173".into()),
        })
    }
}
