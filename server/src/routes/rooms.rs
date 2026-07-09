use axum::routing::{get, post};
use axum::Router;

use crate::app_state::AppState;
use crate::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/rooms", post(handlers::rooms::create_room))
        .route("/api/rooms/:id", get(handlers::rooms::get_room))
        .route("/api/rooms/:id/join", post(handlers::rooms::join_room))
        .route(
            "/api/rooms/:id/select-deck",
            post(handlers::rooms::select_room_deck),
        )
        .route("/api/rooms/:id/ready", post(handlers::rooms::ready_room))
        .route(
            "/api/rooms/:id/unready",
            post(handlers::rooms::unready_room),
        )
        .route("/api/rooms/:id/resign", post(handlers::rooms::resign_room))
}
