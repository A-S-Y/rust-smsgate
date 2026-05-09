use axum::{extract::State, Json};

use crate::{
    app_error::AppResult,
    auth::{create_token, verify_login},
    models::{LoginRequest, LoginResponse},
    state::AppState,
};

pub async fn login(State(state): State<AppState>, Json(input): Json<LoginRequest>) -> AppResult<Json<LoginResponse>> {
    verify_login(&state.config, &input.username, &input.password)?;
    let token = create_token(&state.config, &input.username)?;
    Ok(Json(LoginResponse { token }))
}
