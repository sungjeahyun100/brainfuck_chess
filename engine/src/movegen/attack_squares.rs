use std::collections::HashMap;

use crate::chessembly::run_effective_chessembly_for_piece;
use crate::types::*;

pub fn generate_piece_attack_squares(game_state: &GameState, piece_id: &PieceId) -> Vec<Square> {
    game_state.ensure_chessembly_cache();

    let Some(piece) = game_state.pieces.get(piece_id) else {
        return Vec::new();
    };
    if piece.owner != game_state.current_player || !piece.is_on_board() {
        return Vec::new();
    }

    let Some(definition) = game_state.piece_definitions.get(&piece.type_id) else {
        return Vec::new();
    };

    let empty_global_state = HashMap::new();
    let empty_maps = HashMap::new();
    let result = run_effective_chessembly_for_piece(
        game_state,
        piece,
        definition,
        game_state.current_player.clone(),
        &empty_global_state,
        &empty_maps,
    );
    result
        .attack_squares
        .into_iter()
        .filter(|sq| game_state.board.is_in_bounds(sq))
        .collect()
}
