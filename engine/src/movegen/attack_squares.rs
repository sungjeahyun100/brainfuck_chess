use std::collections::HashMap;

use crate::chessembly::run_effective_chessembly_for_context;
use crate::context::GameContext;
use crate::types::*;

pub fn generate_piece_attack_squares(context: &GameContext<'_>, piece_id: &PieceId) -> Vec<Square> {
    context.ensure_chessembly_cache();
    let state = context.state;

    let Some(piece) = state.pieces.get(piece_id) else {
        return Vec::new();
    };
    if piece.owner != state.current_player || !piece.is_on_board() {
        return Vec::new();
    }

    let Some(definition) = context.catalog.get(&piece.type_id) else {
        return Vec::new();
    };

    let empty_global_state = HashMap::new();
    let empty_maps = HashMap::new();
    let result = run_effective_chessembly_for_context(
        context,
        piece,
        definition,
        state.current_player.clone(),
        &empty_global_state,
        &empty_maps,
    );
    result
        .attack_squares
        .into_iter()
        .filter(|sq| state.board.is_in_bounds(sq))
        .collect()
}
