use std::collections::{HashMap, HashSet};

use crate::attack_map::generate_attack_map;
use crate::rules::get_base_zone_squares;
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

    // Target square must be in bounds and legal for this piece's drop capability.
    if !game_state.board.is_in_bounds(&action.to) {
        return Err("보드 밖에는 착수할 수 없습니다.".into());
    }
    let placement_squares = get_piece_placement_squares(game_state, &action.player_id, piece);
    if !placement_squares.contains(&action.to) {
        return Err("착수 가능한 칸이 아닙니다.".into());
    }

    Ok(())
}
