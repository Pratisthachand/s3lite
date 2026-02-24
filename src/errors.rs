use thiserror::Error;
use axum::{http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Internal Server Error")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, axum::Json(ErrorBody { error: msg })).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;