use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Upstream(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),
    #[error("{0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::Sqlx(_) | AppError::Reqwest(_) | AppError::JsonWebToken(_) | AppError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let body = Json(ErrorBody {
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
