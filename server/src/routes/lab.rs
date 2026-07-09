use axum::routing::post;
use axum::Router;

use crate::app_state::AppState;
use crate::handlers;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/api/lab/piece-options",
        post(handlers::lab::piece_lab_options),
    )
}
