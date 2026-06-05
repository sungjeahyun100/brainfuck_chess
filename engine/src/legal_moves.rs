use std::collections::HashMap;

use crate::chessembly::interpreter::{run, ExecutionContext};
use crate::chessembly::parser::parse;
use crate::placement::get_placement_squares;
use crate::types::*;

/// Generate all legal move actions for the current player in the given state.
pub fn generate_legal_move_actions(game_state: &GameState) -> Vec<MoveAction> {
    let player_id = &game_state.current_player;

    // Cannot generate move actions during a drop turn
    if game_state.turn_state.mode == TurnMode::Drop {
        return Vec::new();
    }

    let mut actions = Vec::new();
    let empty_maps = HashMap::new();
    let empty_global_state = HashMap::new();

    for (piece_id, piece) in &game_state.pieces {
        // Must belong to current player, be on board, and have move stack
        if piece.owner != *player_id
            || !piece.is_on_board()
            || piece.move_stack == 0
            || game_state.turn_state.moved_piece_ids.contains(piece_id)
        {
            continue;
        }

        let definition = match game_state.piece_definitions.get(&piece.type_id) {
            Some(d) => d,
            None => continue,
        };

        let program = parse(&definition.chessembly_code);
        let ctx = ExecutionContext {
            board: &game_state.board,
            piece,
            piece_definition: definition,
            all_definitions: &game_state.piece_definitions,
            all_pieces: &game_state.pieces,
            player: player_id.clone(),
            global_state: &empty_global_state,
            attack_maps: &empty_maps,
        };

        let result = run(&program, &ctx);
        let from = piece.current_square.unwrap();

        // Combine movement and attack squares
        let mut candidate_squares: Vec<Square> = result.movement_squares.clone();
        for sq in &result.attack_squares {
            if !candidate_squares.contains(sq) {
                candidate_squares.push(*sq);
            }
        }

        for to in candidate_squares {
            if !game_state.board.is_in_bounds(&to) {
                continue;
            }
            let captured_piece_id = game_state.board.get_piece_at(&to).cloned();

            // Cannot capture own piece
            if let Some(ref cap_id) = captured_piece_id {
                if let Some(cap_piece) = game_state.pieces.get(cap_id) {
                    if cap_piece.owner == *player_id {
                        continue;
                    }
                }
            }

            actions.push(MoveAction {
                player_id: player_id.clone(),
                piece_id: piece_id.clone(),
                from,
                to,
                captured_piece_id,
            });
        }
    }

    actions
}

/// Generate all legal drop actions for the current player.
pub fn generate_legal_drop_actions(game_state: &GameState) -> Vec<DropAction> {
    let player_id = &game_state.current_player;

    // Cannot drop during a move turn
    if game_state.turn_state.mode == TurnMode::Move {
        return Vec::new();
    }

    // Only one drop allowed per turn
    let already_dropped = game_state
        .turn_state
        .actions
        .iter()
        .any(|a| matches!(a, TurnAction::Drop(_)));
    if already_dropped {
        return Vec::new();
    }

    let player = match game_state.players.get(player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let placement_squares = get_placement_squares(game_state, player_id);

    let mut actions = Vec::new();

    for piece_id in &player.deck.pocket_pieces {
        let piece = match game_state.pieces.get(piece_id) {
            Some(p) => p,
            None => continue,
        };
        let def = match game_state.piece_definitions.get(&piece.type_id) {
            Some(d) => d,
            None => continue,
        };
        if def.is_king {
            continue; // King cannot be dropped
        }

        for &sq in &placement_squares {
            actions.push(DropAction {
                player_id: player_id.clone(),
                piece_id: piece_id.clone(),
                to: sq,
            });
        }
    }

    actions
}
