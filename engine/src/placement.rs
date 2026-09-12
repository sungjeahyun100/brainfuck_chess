use std::collections::{HashMap, HashSet};

use crate::attack_map::generate_attack_map;
use crate::rules::get_base_zone_squares_with_ruleset;
use crate::types::*;
#[cfg(feature = "profiling")]
use std::time::Instant;

/// Compute the set of squares where a player can drop a pocket piece.
///
/// placementSquares = baseZoneSquares ∪ playerAttackMap.
/// Occupancy is handled by the selected pocket piece's drop capability.
fn get_placement_candidates(game_state: &GameState, player_id: &PlayerId) -> Vec<Square> {
    #[cfg(feature = "profiling")]
    let started = Instant::now();
    let attack_map = generate_attack_map(game_state, player_id, &HashMap::new());

    let base_zone =
        get_base_zone_squares_with_ruleset(player_id, game_state.board.size, game_state.ruleset);

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

    // Filter only by bounds. Drop-specific occupancy rules are applied below.
    let squares = candidates
        .into_iter()
        .filter_map(|sq_id| {
            let sq = sq_id.to_square();
            if game_state.board.is_in_bounds(&sq) {
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

pub fn get_placement_squares(game_state: &GameState, player_id: &PlayerId) -> Vec<Square> {
    get_placement_candidates(game_state, player_id)
        .into_iter()
        .filter(|square| game_state.board.is_empty(square))
        .collect()
}

pub fn get_piece_placement_squares(
    game_state: &GameState,
    player_id: &PlayerId,
    piece: &Piece,
) -> Vec<Square> {
    let captures_on_drop = game_state
        .piece_definitions
        .get(&piece.type_id)
        .is_some_and(|definition| definition.can_capture_on_drop);
    get_placement_candidates(game_state, player_id)
        .into_iter()
        .filter(|square| match game_state.board.get_piece_at(square) {
            None => true,
            Some(target_id) if captures_on_drop => game_state
                .pieces
                .get(target_id)
                .is_some_and(|target| target.owner != *player_id),
            Some(_) => false,
        })
        .collect()
}

/// Validate a drop action.
pub fn validate_drop_action(game_state: &GameState, action: &DropAction) -> Result<(), String> {
    if !crate::hand::is_ordinary_drop_source(game_state, &action.player_id, &action.piece_id) {
        return Err("일반 착수 출처가 유효하지 않습니다.".into());
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

    if !crate::legal_moves::generate_selected_drop_actions(
        game_state,
        &action.piece_id,
        &action.sacrifice_piece_ids,
    )
    .iter()
    .any(|candidate| candidate == action)
    {
        return Err("착수 가능한 제물 또는 칸이 아닙니다.".into());
    }
    Ok(())
}

/// Ordinary Standard hand drops pay by body count; Legacy and Extra keep their rules.
pub fn drop_sacrifice_count(state: &GameState, piece_id: &PieceId) -> usize {
    if state.ruleset != DeckRuleset::Standard {
        return 0;
    }
    match state
        .pieces
        .get(piece_id)
        .and_then(|p| state.piece_definitions.get(&p.type_id))
        .map(|d| d.score)
    {
        Some(10..) => 2,
        Some(5..=9) => 1,
        _ => 0,
    }
}

pub fn drop_sacrifice_candidates(state: &GameState) -> Vec<PieceId> {
    let mut ids: Vec<_> = state
        .pieces
        .values()
        .filter(|p| {
            p.owner == state.current_player
                && p.is_on_board()
                && p.current_square
                    .is_some_and(|sq| state.board.get_piece_at_layer(&sq, p.layer) == Some(&p.id))
                && state
                    .piece_definitions
                    .get(&p.type_id)
                    .is_some_and(|d| !d.is_king)
        })
        .map(|p| p.id.clone())
        .collect();
    ids.sort();
    ids
}
