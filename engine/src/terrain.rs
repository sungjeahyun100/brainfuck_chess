use crate::rules::HIGH_GROUND_TERRAIN_ID;
use crate::types::{Board, GameState, Piece, Square};

/// Terrain height is intentionally resolved independently from piece rules.
/// Unknown terrain remains neutral until a policy for it is implemented.
pub fn elevation_at(board: &Board, square: Square) -> i16 {
    match board
        .terrain
        .get(&square.to_id())
        .map(|cell| cell.type_id.as_str())
    {
        Some(HIGH_GROUND_TERRAIN_ID) => 1,
        _ => 0,
    }
}

/// A piece may threaten or capture a square at its own elevation or lower.
/// Pocket pieces have no board square and therefore attack from ground level.
pub fn can_affect_square(state: &GameState, actor: &Piece, target: Square) -> bool {
    let actor_elevation = actor
        .current_square
        .map(|square| elevation_at(&state.board, square))
        .unwrap_or(0);
    actor_elevation >= elevation_at(&state.board, target)
}

pub fn can_capture_piece(state: &GameState, actor: &Piece, victim: &Piece) -> bool {
    victim
        .current_square
        .is_none_or(|square| can_affect_square(state, actor, square))
}
