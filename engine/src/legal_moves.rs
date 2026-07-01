use std::collections::HashMap;
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::attack_map::generate_attack_map;
use crate::chessembly::run_effective_chessembly_for_piece;
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

fn has_move_or_drop_action(turn_state: &TurnState) -> bool {
    turn_state
        .actions
        .iter()
        .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
}

fn push_action_if_unique(actions: &mut Vec<MoveAction>, action: MoveAction) {
    let exists = actions.iter().any(|m| {
        m.piece_id == action.piece_id && m.to == action.to && m.promotion == action.promotion
    });
    if !exists {
        actions.push(action);
    }
}

/// Push a move action, expanding it into one action per promotion choice
/// when the moving piece's definition has a matching promotion rule.
fn push_move_or_promotions(
    actions: &mut Vec<MoveAction>,
    definition: &PieceDefinition,
    board_size: i32,
    player_id: &PlayerId,
    piece_id: &PieceId,
    from: Square,
    to: Square,
    captured_piece_id: Option<PieceId>,
) {
    if let Some(promotion_options) = definition.promotion_options_for_rank(to.rank, board_size) {
        for promo in promotion_options {
            push_action_if_unique(
                actions,
                MoveAction {
                    player_id: player_id.clone(),
                    piece_id: piece_id.clone(),
                    from,
                    to,
                    captured_piece_id: captured_piece_id.clone(),
                    promotion: Some(promo.clone()),
                },
            );
        }
    } else {
        push_action_if_unique(
            actions,
            MoveAction {
                player_id: player_id.clone(),
                piece_id: piece_id.clone(),
                from,
                to,
                captured_piece_id,
                promotion: None,
            },
        );
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

/// Generate legal move actions for one piece owned by the current player.
pub fn generate_piece_legal_move_actions(
    game_state: &GameState,
    piece_id: &PieceId,
) -> Vec<MoveAction> {
    game_state.ensure_chessembly_cache();

    let player_id = &game_state.current_player;

    // A turn allows exactly one action: either one move or one pocket drop.
    if game_state.turn_state.mode == TurnMode::Drop
        || has_move_or_drop_action(&game_state.turn_state)
    {
        return Vec::new();
    }

    let mut actions = Vec::new();
    let empty_maps = HashMap::new();
    let empty_global_state = HashMap::new();

    let Some(piece) = game_state.pieces.get(piece_id) else {
        return Vec::new();
    };

    // Must belong to current player and be on board.
    if piece.owner != *player_id || !piece.is_on_board() {
        return Vec::new();
    }

    let Some(definition) = game_state.piece_definitions.get(&piece.type_id) else {
        return Vec::new();
    };

    let result = run_effective_chessembly_for_piece(
        game_state,
        piece,
        definition,
        player_id.clone(),
        &empty_global_state,
        &empty_maps,
    );
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
            if to.file == from.file
                && to.rank - from.rank == 2 * dir
                && (from.rank != start_rank || piece.has_moved)
            {
                continue;
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

        push_move_or_promotions(
            &mut actions,
            definition,
            game_state.board.size,
            player_id,
            piece_id,
            from,
            to,
            captured_piece_id,
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

        push_move_or_promotions(
            &mut actions,
            definition,
            game_state.board.size,
            player_id,
            piece_id,
            from,
            to,
            Some(captured_piece_id),
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
                                        promotion: None,
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
    // Build cheap candidates first; only then compute the enemy attack map.
    if definition.is_king && !piece.has_moved {
        let mut castle_candidates = Vec::new();

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

            if !game_state.board.is_in_bounds(&king_mid) || !game_state.board.is_in_bounds(&king_to)
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

            castle_candidates.push((king_mid, king_to));
        }

        if !castle_candidates.is_empty() {
            let opponent_id = if player_id == "white" {
                "black".to_string()
            } else {
                "white".to_string()
            };
            let enemy_attack_map = generate_attack_map(game_state, &opponent_id, &empty_maps);
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
                    },
                );
            }
        }
    }

    actions
}

/// Generate all legal move actions for the current player in the given state.
pub fn generate_legal_move_actions(game_state: &GameState) -> Vec<MoveAction> {
    #[cfg(feature = "profiling")]
    let started = Instant::now();
    let player_id = &game_state.current_player;

    if game_state.turn_state.mode == TurnMode::Drop
        || has_move_or_drop_action(&game_state.turn_state)
    {
        return Vec::new();
    }

    let mut piece_ids = game_state
        .pieces
        .iter()
        .filter_map(|(piece_id, piece)| {
            if piece.owner == *player_id {
                Some(piece_id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    piece_ids.sort();

    let actions = piece_ids
        .into_iter()
        .flat_map(|piece_id| generate_piece_legal_move_actions(game_state, &piece_id))
        .collect::<Vec<_>>();
    #[cfg(feature = "profiling")]
    crate::profiling::record_legal_moves(started.elapsed(), actions.len());
    actions
}

/// Generate legal drop actions for one pocket piece owned by the current player.
pub fn generate_piece_legal_drop_actions(
    game_state: &GameState,
    piece_id: &PieceId,
) -> Vec<DropAction> {
    let player_id = &game_state.current_player;

    // A turn allows exactly one action: either one move or one pocket drop.
    if game_state.turn_state.mode == TurnMode::Move
        || has_move_or_drop_action(&game_state.turn_state)
    {
        return Vec::new();
    }

    let Some(player) = game_state.players.get(player_id) else {
        return Vec::new();
    };
    if !player.deck.pocket_pieces.contains(piece_id) {
        return Vec::new();
    }

    let Some(piece) = game_state.pieces.get(piece_id) else {
        return Vec::new();
    };
    if piece.owner != *player_id || !piece.in_pocket || piece.captured {
        return Vec::new();
    }

    let Some(def) = game_state.piece_definitions.get(&piece.type_id) else {
        return Vec::new();
    };
    if def.is_king {
        return Vec::new();
    }

    get_placement_squares(game_state, player_id)
        .into_iter()
        .map(|sq| DropAction {
            player_id: player_id.clone(),
            piece_id: piece_id.clone(),
            to: sq,
        })
        .collect()
}

/// Generate all legal drop actions for the current player.
pub fn generate_legal_drop_actions(game_state: &GameState) -> Vec<DropAction> {
    let player_id = &game_state.current_player;

    // A turn allows exactly one action: either one move or one pocket drop.
    if game_state.turn_state.mode == TurnMode::Move
        || has_move_or_drop_action(&game_state.turn_state)
    {
        return Vec::new();
    }

    let player = match game_state.players.get(player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut actions = Vec::new();
    for piece_id in &player.deck.pocket_pieces {
        actions.extend(generate_piece_legal_drop_actions(game_state, piece_id));
    }

    crate::profiling::record_drops(actions.len());
    actions
}

/// Generate search-oriented drop candidates grouped by piece type.
///
/// This intentionally does not select a concrete `piece_id`; that conversion
/// belongs at the boundary where a selected candidate becomes a `DropAction`.
pub fn generate_drop_candidates_by_type(
    game_state: &GameState,
    player_id: &PlayerId,
) -> Vec<DropCandidateByType> {
    if &game_state.current_player != player_id
        || game_state.turn_state.mode == TurnMode::Move
        || has_move_or_drop_action(&game_state.turn_state)
    {
        return Vec::new();
    }

    let Some(player) = game_state.players.get(player_id) else {
        return Vec::new();
    };

    let mut counts: HashMap<PieceTypeId, u16> = HashMap::new();
    for piece_id in &player.deck.pocket_pieces {
        let Some(piece) = game_state.pieces.get(piece_id) else {
            continue;
        };
        if piece.owner != *player_id || !piece.in_pocket || piece.captured {
            continue;
        }
        let Some(definition) = game_state.piece_definitions.get(&piece.type_id) else {
            continue;
        };
        if definition.is_king {
            continue;
        }
        let count = counts.entry(piece.type_id.clone()).or_default();
        *count = count.saturating_add(1);
    }

    let mut type_counts: Vec<_> = counts.into_iter().collect();
    type_counts.sort_by(|left, right| left.0.cmp(&right.0));
    let mut squares = get_placement_squares(game_state, player_id);
    squares.sort_by_key(|square| (square.rank, square.file));

    type_counts
        .into_iter()
        .flat_map(|(piece_type_id, count)| {
            squares.iter().map(move |square| DropCandidateByType {
                player_id: player_id.clone(),
                piece_type_id: piece_type_id.clone(),
                count,
                to: square.to_id(),
            })
        })
        .collect()
}
