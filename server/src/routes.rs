use axum::routing::{get, post};
use axum::Router;

use crate::app_state::AppState;
use crate::*;

/// HTTP-only routing table. Request parsing remains in handlers while game
/// rules are delegated to engine services/boundaries.
pub(crate) fn api(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/actions", post(submit_action))
        .route("/games/:id/bot-turn", post(run_bot_turn))
        .route("/games/:id/end-turn", post(end_game_turn))
        .route("/games/:id/resign", post(resign_game))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/piece-attacks/:piece_id", get(get_piece_attacks))
        .route(
            "/games/:id/pieces/:piece_id/options",
            get(get_piece_options),
        )
        .route("/lab/piece-options", post(get_lab_piece_options))
        .route("/games/:id/legal-drops", get(get_legal_drops))
        .route("/rooms", post(create_room))
        .route("/rooms/:id", get(get_room))
        .route("/rooms/:id/join", post(join_room))
        .route("/rooms/:id/select-deck", post(select_room_deck))
        .route("/rooms/:id/ready", post(ready_room))
        .route("/rooms/:id/unready", post(unready_room))
        .route("/rooms/:id/resign", post(resign_room))
        .with_state(state)
}
