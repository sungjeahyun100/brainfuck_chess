use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

#[allow(dead_code)]
pub fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
        .into_response()
}

#[allow(dead_code)]
pub fn not_found(message: impl Into<String>) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
        .into_response()
}

#[allow(dead_code)]
pub fn conflict(message: impl Into<String>) -> Response {
    (
        StatusCode::CONFLICT,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
        .into_response()
}
