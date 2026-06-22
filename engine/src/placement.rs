use std::collections::{HashMap, HashSet};

use crate::attack_map::generate_attack_map;
use crate::rules::get_base_zone_squares;
use crate::types::*;
#[cfg(feature = "profiling")]
use std::time::Instant;

/// Compute the set of squares where a player can drop a pocket piece.
///
/// placementSquares = baseZoneSquares ∪ playerAttackMap   (minus occupied squares)
pub fn get_placement_squares(game_state: &GameState, player_id: &PlayerId) -> Vec<Square> {
    #[cfg(feature = "profiling")]
    let started = Instant::now();
    let attack_map = generate_attack_map(game_state, player_id, &HashMap::new());

    let base_zone = get_base_zone_squares(player_id, game_state.board.size);

    let mut candidates: HashSet<SquareId> = HashSet::new();

    // Add base zone squares
    for sq in &base_zone {
        if game_state.board.is_in_bounds(sq) {
            candidates.insert(sq.to_id());
        }
    }

    // Add attack map squares
    for sq_id in &attack_map.attacked_squares {
        candidates.insert(*sq_id);
    }

    // Filter: must be empty and in bounds
    let squares = candidates
        .into_iter()
        .filter_map(|sq_id| {
            let sq = sq_id.to_square();
            if game_state.board.is_in_bounds(&sq) && game_state.board.is_empty(&sq) {
                Some(sq)
            } else {
                None
            }
        })
        .collect();
    #[cfg(feature = "profiling")]
    crate::profiling::record_placement(started.elapsed());
    squares
}

/// Validate a drop action.
pub fn validate_drop_action(game_state: &GameState, action: &DropAction) -> Result<(), String> {
    // Must be drop turn mode
    if game_state.turn_state.mode == crate::types::TurnMode::Move {
        return Err("이동 턴에는 착수할 수 없습니다.".into());
    }

    // Only one drop per turn
    let drop_count = game_state
        .turn_state
        .actions
        .iter()
        .filter(|a| matches!(a, TurnAction::Drop(_)))
        .count();
    if drop_count > 0 {
        return Err("착수 턴에는 포켓 기물 1개만 착수할 수 있습니다.".into());
    }

    // Piece must exist in the player's pocket
    let player = game_state
        .players
        .get(&action.player_id)
        .ok_or("플레이어를 찾을 수 없습니다.")?;
    if !player.deck.pocket_pieces.contains(&action.piece_id) {
        return Err("해당 기물이 포켓에 없습니다.".into());
    }

    // Piece must not be a King
    let piece = game_state
        .pieces
        .get(&action.piece_id)
        .ok_or("기물을 찾을 수 없습니다.")?;
    if let Some(def) = game_state.piece_definitions.get(&piece.type_id) {
        if def.is_king {
            return Err("King은 착수할 수 없습니다.".into());
        }
    }

    // Target square must be in bounds and empty
    if !game_state.board.is_in_bounds(&action.to) {
        return Err("보드 밖에는 착수할 수 없습니다.".into());
    }
    if !game_state.board.is_empty(&action.to) {
        return Err("이미 기물이 있는 칸에는 착수할 수 없습니다.".into());
    }

    // Target must be in placement squares
    let placement_squares = get_placement_squares(game_state, &action.player_id);
    if !placement_squares.contains(&action.to) {
        return Err("착수 가능한 칸이 아닙니다.".into());
    }

    Ok(())
}
