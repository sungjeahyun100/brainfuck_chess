use axum::Router;

use crate::app_state::AppState;

pub mod games;
pub mod lab;
pub mod rooms;
pub mod static_files;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(games::routes())
        .merge(rooms::routes())
        .merge(lab::routes())
        .merge(static_files::routes())
        .with_state(state)
}
