use std::collections::{HashMap, HashSet};

use crate::attack_map::generate_attack_map;
use crate::movegen::action_builder::push_action_if_unique;
use crate::types::*;

fn is_rook_piece(piece: &Piece) -> bool {
    piece.type_id == "rook"
}

pub fn generate_castling_actions(
    state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    piece_id: &PieceId,
    player_id: &PlayerId,
    from: Square,
    ability_id: Option<&str>,
) -> Vec<MoveAction> {
    let mut actions = Vec::new();

    if !definition.is_king || piece.has_moved {
        return actions;
    }

    let mut castle_candidates = Vec::new();

    for rook in state.pieces.values() {
        if rook.owner != *player_id || rook.has_moved || !rook.is_on_board() || !is_rook_piece(rook)
        {
            continue;
        }

        let rook_sq = rook.current_square.unwrap();
        if rook_sq.rank != from.rank {
            continue;
        }

        let diff = rook_sq.file - from.file;
        if diff.abs() < 3 {
            continue;
        }

        let dir = diff.signum();
        let king_mid = Square::new(from.file + dir, from.rank);
        let king_to = Square::new(from.file + 2 * dir, from.rank);

        if !state.board.is_in_bounds(&king_mid) || !state.board.is_in_bounds(&king_to) {
            continue;
        }
        if !state.board.is_empty(&king_mid) || !state.board.is_empty(&king_to) {
            continue;
        }

        let mut blocked = false;
        let mut file = from.file + dir;
        while file != rook_sq.file {
            if !state.board.is_empty(&Square::new(file, from.rank)) {
                blocked = true;
                break;
            }
            file += dir;
        }
        if blocked {
            continue;
        }

        castle_candidates.push((king_mid, king_to));
    }

    if castle_candidates.is_empty() {
        return actions;
    }

    let opponent_id = if player_id == "white" {
        "black".to_string()
    } else {
        "white".to_string()
    };
    let empty_maps = HashMap::<PlayerId, HashSet<SquareId>>::new();
    let enemy_attack_map = generate_attack_map(state, &opponent_id, &empty_maps);
    if enemy_attack_map.attacked_squares.contains(&from.to_id()) {
        return actions;
    }

    for (king_mid, king_to) in castle_candidates {
        if enemy_attack_map
            .attacked_squares
            .contains(&king_mid.to_id())
            || enemy_attack_map.attacked_squares.contains(&king_to.to_id())
        {
            continue;
        }

        push_action_if_unique(
            &mut actions,
            MoveAction {
                player_id: player_id.clone(),
                piece_id: piece_id.clone(),
                from,
                to: king_to,
                captured_piece_id: None,
                promotion: None,
                ability_id: ability_id.map(str::to_string),
            },
        );
    }

    actions
}
