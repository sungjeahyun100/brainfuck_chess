use axum::routing::{get, post};
use axum::Router;

use crate::app_state::AppState;
use crate::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/games", post(handlers::games::create_game))
        .route("/api/games/:id", get(handlers::games::get_game))
        .route(
            "/api/games/:id/actions",
            post(handlers::games::submit_action),
        )
        .route("/api/games/:id/bot-turn", post(handlers::games::bot_turn))
        .route(
            "/api/games/:id/end-turn",
            post(handlers::games::end_game_turn),
        )
        .route("/api/games/:id/resign", post(handlers::games::resign_game))
        .route(
            "/api/games/:id/legal-moves",
            get(handlers::games::get_legal_moves),
        )
        .route(
            "/api/games/:id/legal-drops",
            get(handlers::games::get_legal_drops),
        )
        .route(
            "/api/games/:id/piece-attacks/:piece_id",
            get(handlers::games::get_piece_attacks),
        )
        .route(
            "/api/games/:id/pieces/:piece_id/options",
            get(handlers::games::get_piece_options),
        )
}
