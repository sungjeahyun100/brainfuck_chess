use std::collections::HashMap;

use crate::attack_map::generate_attack_map;
use crate::chessembly::interpreter::{run, ExecutionContext};
use crate::placement::get_placement_squares;
use crate::types::*;

fn is_pawn_type(type_id: &str) -> bool {
    type_id == "pawn-white" || type_id == "pawn-black"
}

fn pawn_forward_dir(type_id: &str) -> Option<i32> {
    match type_id {
        "pawn-white" => Some(1),
        "pawn-black" => Some(-1),
        _ => None,
    }
}

fn pawn_start_rank(type_id: &str, board_size: i32) -> Option<i32> {
    match type_id {
        "pawn-white" => Some(1),
        "pawn-black" => Some(board_size - 2),
        _ => None,
    }
}

fn is_rook_piece(piece: &Piece) -> bool {
    piece.type_id == "rook"
}

fn push_action_if_unique(actions: &mut Vec<MoveAction>, action: MoveAction) {
    let exists = actions
        .iter()
        .any(|m| m.piece_id == action.piece_id && m.to == action.to);
    if !exists {
        actions.push(action);
    }
}

/// Generate attack/threat squares for a specific piece.
///
/// This is intentionally separate from legal moves so UI can visualize
/// attacked squares (including empty threatened squares) without making
/// those squares executable move targets.
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

    let Some(program) = game_state.chessembly_program(&piece.type_id) else {
        return Vec::new();
    };
    let empty_maps = HashMap::new();
    let empty_global_state = HashMap::new();
    let ctx = ExecutionContext {
        board: &game_state.board,
        piece,
        piece_definition: definition,
        all_definitions: &game_state.piece_definitions,
        all_pieces: &game_state.pieces,
        player: game_state.current_player.clone(),
        global_state: &empty_global_state,
        attack_maps: &empty_maps,
    };

    let result = run(program.as_ref(), &ctx);
    result
        .attack_squares
        .into_iter()
        .filter(|sq| game_state.board.is_in_bounds(sq))
        .collect()
}

/// Generate all legal move actions for the current player in the given state.
pub fn generate_legal_move_actions(game_state: &GameState) -> Vec<MoveAction> {
    game_state.ensure_chessembly_cache();

    let player_id = &game_state.current_player;
    let opponent_id = if player_id == "white" {
        "black".to_string()
    } else {
        "white".to_string()
    };

    // Cannot generate move actions during a drop turn
    if game_state.turn_state.mode == TurnMode::Drop {
        return Vec::new();
    }

    let mut actions = Vec::new();
    let empty_maps = HashMap::new();
    let empty_global_state = HashMap::new();
    let enemy_attack_map = generate_attack_map(game_state, &opponent_id, &empty_maps);

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

        let Some(program) = game_state.chessembly_program(&piece.type_id) else {
            continue;
        };
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

        let result = run(program.as_ref(), &ctx);
        let from = piece.current_square.unwrap();
        let pawn_dir = pawn_forward_dir(&piece.type_id);
        let pawn_start = pawn_start_rank(&piece.type_id, game_state.board.size);

        // Executable movement/capture squares from movement paths.
        for to in result.movement_squares.iter().copied() {
            if !game_state.board.is_in_bounds(&to) {
                continue;
            }

            if let (Some(dir), Some(start_rank)) = (pawn_dir, pawn_start) {
                // Restrict pawn 2-step to starting rank and only before first move.
                if to.file == from.file && to.rank - from.rank == 2 * dir {
                    if from.rank != start_rank || piece.has_moved {
                        continue;
                    }
                }
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

            push_action_if_unique(
                &mut actions,
                MoveAction {
                    player_id: player_id.clone(),
                    piece_id: piece_id.clone(),
                    from,
                    to,
                    captured_piece_id,
                },
            );
        }

        // Attack-only squares are legal only when an enemy occupies the square.
        // This keeps legal moves executable while still allowing attack map UI
        // to show empty threatened squares through generate_piece_attack_squares.
        for to in result.attack_squares.iter().copied() {
            if !game_state.board.is_in_bounds(&to) {
                continue;
            }

            let Some(captured_piece_id) = game_state.board.get_piece_at(&to).cloned() else {
                continue;
            };
            let Some(captured_piece) = game_state.pieces.get(&captured_piece_id) else {
                continue;
            };
            if captured_piece.owner == *player_id {
                continue;
            }

            push_action_if_unique(
                &mut actions,
                MoveAction {
                    player_id: player_id.clone(),
                    piece_id: piece_id.clone(),
                    from,
                    to,
                    captured_piece_id: Some(captured_piece_id),
                },
            );
        }

        // En passant: pawn can capture onto target square even when destination is empty.
        if let Some(dir) = pawn_dir {
            if game_state.en_passant_available_to.as_ref() == Some(player_id) {
                if let Some(target) = game_state.en_passant_target {
                    if target.rank == from.rank + dir
                        && (target.file - from.file).abs() == 1
                        && game_state.board.is_empty(&target)
                    {
                        let adjacent = Square::new(target.file, from.rank);
                        if let Some(captured_id) = game_state.board.get_piece_at(&adjacent) {
                            if let Some(captured_piece) = game_state.pieces.get(captured_id) {
                                if captured_piece.owner != *player_id
                                    && is_pawn_type(&captured_piece.type_id)
                                {
                                    push_action_if_unique(
                                        &mut actions,
                                        MoveAction {
                                            player_id: player_id.clone(),
                                            piece_id: piece_id.clone(),
                                            from,
                                            to: target,
                                            captured_piece_id: Some(captured_id.clone()),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Castling: handled as special king move (2 squares toward an unmoved rook).
        if definition.is_king && !piece.has_moved {
            if enemy_attack_map.attacked_squares.contains(&from.to_id()) {
                continue;
            }

            for rook in game_state.pieces.values() {
                if rook.owner != *player_id
                    || rook.has_moved
                    || !rook.is_on_board()
                    || !is_rook_piece(rook)
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

                if !game_state.board.is_in_bounds(&king_mid)
                    || !game_state.board.is_in_bounds(&king_to)
                {
                    continue;
                }
                if !game_state.board.is_empty(&king_mid) || !game_state.board.is_empty(&king_to) {
                    continue;
                }

                // Every square between king and rook must be empty.
                let mut blocked = false;
                let mut file = from.file + dir;
                while file != rook_sq.file {
                    if !game_state.board.is_empty(&Square::new(file, from.rank)) {
                        blocked = true;
                        break;
                    }
                    file += dir;
                }
                if blocked {
                    continue;
                }

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
                    },
                );
            }
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
