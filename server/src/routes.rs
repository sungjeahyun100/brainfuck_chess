use axum::routing::{get, post};
use axum::Router;

use crate::app_state::AppState;
use crate::*;

mod custom_piece_image;

/// HTTP-only routing table. Request parsing remains in handlers while game
/// rules are delegated to engine services/boundaries.
pub(crate) fn api(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/piece-scores", get(get_piece_scores))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/actions", post(submit_action))
        .route("/games/:id/bot-turn", post(run_bot_turn))
        .route("/games/:id/resign", post(resign_game))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/piece-attacks/:piece_id", get(get_piece_attacks))
        .route(
            "/games/:id/players/:player_id/attacks",
            get(get_player_attacks),
        )
        .route(
            "/games/:id/pieces/:piece_id/options",
            get(get_piece_options),
        )
        .route("/lab/piece-options", post(get_lab_piece_options))
        .route(
            "/custom-pieces",
            get(custom_piece::list).post(custom_piece::create),
        )
        .route("/custom-pieces/validate", post(custom_piece::validate))
        .route(
            "/custom-pieces/:id",
            get(custom_piece::get)
                .put(custom_piece::update)
                .delete(custom_piece::deactivate),
        )
        .route(
            "/custom-pieces/:id/versions/:version/image-asset",
            get(custom_piece_image::get),
        )
        .route(
            "/custom-pieces/:id/versions/:version",
            get(custom_piece::get_version),
        )
        .route("/custom-piece-images", post(custom_piece::upload_image))
        .route(
            "/custom-pieces/test/options",
            post(custom_piece::test_options),
        )
        .route(
            "/custom-pieces/test/actions",
            post(custom_piece::test_action),
        )
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
