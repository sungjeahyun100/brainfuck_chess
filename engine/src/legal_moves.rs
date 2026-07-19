use std::collections::HashMap;
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::attack_map::generate_attack_map;
use crate::chessembly::run_chessembly_layer_for_piece;
use crate::types::*;

fn is_pawn_type(type_id: &str) -> bool {
    matches!(
        type_id,
        "pawn-white" | "pawn-black" | "tempest-pawn-white" | "tempest-pawn-black"
    )
}

fn pawn_forward_dir(type_id: &str) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" => Some(-1),
        _ => None,
    }
}

fn pawn_start_rank(type_id: &str, board_size: i32) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" => Some(board_size - 2),
        _ => None,
    }
}

fn is_rook_piece(piece: &Piece) -> bool {
    piece.type_id == "rook"
}

fn push_action_if_unique(actions: &mut Vec<MoveAction>, action: MoveAction) {
    let exists = actions.iter().any(|m| {
        m.piece_id == action.piece_id
            && m.to == action.to
            && m.promotion == action.promotion
            && m.move_option_id == action.move_option_id
            && m.source_layer_ids == action.source_layer_ids
            && m.effects == action.effects
    });
    if !exists {
        actions.push(action);
    }
}

#[derive(Debug, Clone, Default)]
pub struct MoveGenerationOptions {
    pub move_option_id: Option<String>,
}

struct MoveBuildContext<'a> {
    game_state: &'a GameState,
    piece_id: &'a PieceId,
    piece: &'a Piece,
    definition: &'a PieceDefinition,
    player_id: &'a PlayerId,
    option: &'a MoveOptionDefinition,
    layer: &'a MoveLayerDefinition,
}

/// Push a move action, expanding it into one action per promotion choice
/// when the moving piece's definition has a matching promotion rule.
fn push_move_or_promotions(
    actions: &mut Vec<MoveAction>,
    context: &MoveBuildContext<'_>,
    to: Square,
    captured_piece_id: Option<PieceId>,
    effects: &ActionEffects,
) {
    let from = context.piece.current_square.unwrap();
    if let Some(promotion_options) = context
        .definition
        .promotion_options_for_rank(to.rank, context.game_state.board.size)
    {
        for promo in promotion_options {
            push_action_if_unique(
                actions,
                MoveAction {
                    player_id: context.player_id.clone(),
                    piece_id: context.piece_id.clone(),
                    from,
                    to,
                    captured_piece_id: captured_piece_id.clone(),
                    promotion: Some(promo.clone()),
                    move_option_id: context.option.id.clone(),
                    source_layer_ids: vec![context.layer.id.clone()],
                    effects: effects.clone(),
                },
            );
        }
    } else {
        push_action_if_unique(
            actions,
            MoveAction {
                player_id: context.player_id.clone(),
                piece_id: context.piece_id.clone(),
                from,
                to,
                captured_piece_id,
                promotion: None,
                move_option_id: context.option.id.clone(),
                source_layer_ids: vec![context.layer.id.clone()],
                effects: effects.clone(),
            },
        );
    }
}

fn append_actions_from_result(
    actions: &mut Vec<MoveAction>,
    result: &ChessemblyResult,
    context: &MoveBuildContext<'_>,
) {
    let from = context.piece.current_square.unwrap();
    let pawn_dir = pawn_forward_dir(&context.piece.type_id);
    let pawn_start = pawn_start_rank(&context.piece.type_id, context.game_state.board.size);

    for to in result.movement_squares.iter().copied() {
        if !context.game_state.board.is_in_bounds(&to) {
            continue;
        }

        if let (Some(dir), Some(start_rank)) = (pawn_dir, pawn_start) {
            if to.file == from.file
                && to.rank - from.rank == 2 * dir
                && (from.rank != start_rank || context.piece.has_moved)
            {
                continue;
            }
        }

        let captured_piece_id = context.game_state.board.get_piece_at(&to).cloned();
        let effects =
            effects_for_candidate(result, to, context.piece_id, context.option, context.layer);
        if let Some(ref cap_id) = captured_piece_id {
            if let Some(cap_piece) = context.game_state.pieces.get(cap_id) {
                if cap_piece.owner == *context.player_id {
                    continue;
                }
            }
        }

        push_move_or_promotions(actions, context, to, captured_piece_id, &effects);
    }

    for to in result.attack_squares.iter().copied() {
        if !context.game_state.board.is_in_bounds(&to) {
            continue;
        }

        let Some(captured_piece_id) = context.game_state.board.get_piece_at(&to).cloned() else {
            continue;
        };
        let Some(captured_piece) = context.game_state.pieces.get(&captured_piece_id) else {
            continue;
        };
        if captured_piece.owner == *context.player_id {
            continue;
        }
        let effects =
            effects_for_candidate(result, to, context.piece_id, context.option, context.layer);

        push_move_or_promotions(actions, context, to, Some(captured_piece_id), &effects);
    }
}

fn effects_for_candidate(
    result: &ChessemblyResult,
    to: Square,
    piece_id: &PieceId,
    option: &MoveOptionDefinition,
    layer: &MoveLayerDefinition,
) -> ActionEffects {
    let global_state_updates = result
        .effects
        .get(&to.to_id())
        .and_then(|effect| effect.set_state.clone())
        .into_iter()
        .collect();
    let piece_state_updates = layer
        .on_commit
        .iter()
        .map(|update| PieceStateUpdate {
            piece_id: piece_id.clone(),
            key: update.key.clone(),
            value: update.value.clone(),
        })
        .collect();
    let cooldown_updates = option
        .cooldown
        .as_ref()
        .filter(|cooldown| cooldown.turns > 0)
        .map(|cooldown| CooldownUpdate {
            piece_id: piece_id.clone(),
            move_option_id: option.id.clone(),
            remaining: cooldown.turns,
        })
        .into_iter()
        .collect();
    ActionEffects {
        global_state_updates,
        piece_state_updates,
        cooldown_updates,
    }
}

fn can_use_move_option(piece: &Piece, option: &MoveOptionDefinition) -> bool {
    option.execution_mode == MoveOptionExecutionMode::MoveModifier
        && piece
            .move_option_cooldowns
            .get(&option.id)
            .is_none_or(|cooldown| cooldown.remaining == 0)
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

    let empty_maps = HashMap::new();
    let mut attacked = Vec::new();
    for layer_id in definition
        .move_options
        .iter()
        .filter(|option| option.contributes_to_attack_map && can_use_move_option(piece, option))
        .flat_map(|option| &option.layer_ids)
    {
        let Some(layer) = definition
            .move_layers
            .iter()
            .find(|layer| &layer.id == layer_id)
        else {
            continue;
        };
        if !layer.is_enabled_for(piece) {
            continue;
        }
        attacked.extend(
            run_chessembly_layer_for_piece(
                game_state,
                piece,
                definition,
                layer,
                game_state.current_player.clone(),
                &game_state.global_state,
                &empty_maps,
            )
            .attack_squares,
        );
    }
    attacked
        .into_iter()
        .filter(|sq| game_state.board.is_in_bounds(sq))
        .collect()
}

/// Generate legal move actions for one piece owned by the current player.
pub fn generate_piece_legal_move_actions(
    game_state: &GameState,
    piece_id: &PieceId,
) -> Vec<MoveAction> {
    generate_piece_legal_move_actions_with_options(
        game_state,
        piece_id,
        &MoveGenerationOptions::default(),
    )
}

/// Generate legal move actions for a selected move option. Selecting the option
/// is UI state only and never mutates the game or consumes a turn.
pub fn generate_piece_legal_move_actions_with_options(
    game_state: &GameState,
    piece_id: &PieceId,
    options: &MoveGenerationOptions,
) -> Vec<MoveAction> {
    game_state.ensure_chessembly_cache();

    let player_id = &game_state.current_player;

    // A turn allows exactly one action: either one move or one pocket drop.
    let mut actions = Vec::new();
    let empty_maps = HashMap::new();

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
    let selected_option = if let Some(option_id) = options.move_option_id.as_deref() {
        definition
            .move_options
            .iter()
            .find(|option| option.id == option_id)
    } else {
        definition.normal_move_option()
    };
    let Some(selected_option) = selected_option.filter(|option| can_use_move_option(piece, option))
    else {
        return Vec::new();
    };
    let from = piece.current_square.unwrap();
    let mut enabled_layers = Vec::new();
    for layer_id in &selected_option.layer_ids {
        let Some(layer) = definition
            .move_layers
            .iter()
            .find(|layer| &layer.id == layer_id)
        else {
            continue;
        };
        if !layer.is_enabled_for(piece) {
            continue;
        }
        enabled_layers.push(layer);
        let result = run_chessembly_layer_for_piece(
            game_state,
            piece,
            definition,
            layer,
            player_id.clone(),
            &game_state.global_state,
            &empty_maps,
        );
        let context = MoveBuildContext {
            game_state,
            piece_id,
            piece,
            definition,
            player_id,
            option: selected_option,
            layer,
        };
        append_actions_from_result(&mut actions, &result, &context);
    }

    if enabled_layers.is_empty() {
        return Vec::new();
    }
    let special_layer = enabled_layers[0];
    let special_effects = effects_for_candidate(
        &ChessemblyResult::default(),
        from,
        piece_id,
        selected_option,
        special_layer,
    );
    let special_source_layers = vec![special_layer.id.clone()];

    // En passant: pawn can capture onto target square even when destination is empty.
    if let Some(dir) = pawn_forward_dir(&piece.type_id) {
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
                                        move_option_id: selected_option.id.clone(),
                                        source_layer_ids: special_source_layers.clone(),
                                        effects: special_effects.clone(),
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
                        move_option_id: selected_option.id.clone(),
                        source_layer_ids: special_source_layers.clone(),
                        effects: special_effects.clone(),
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
        .flat_map(|piece_id| {
            let option_ids = game_state
                .pieces
                .get(&piece_id)
                .and_then(|piece| game_state.piece_definitions.get(&piece.type_id))
                .map(|definition| {
                    definition
                        .move_options
                        .iter()
                        .filter(|option| {
                            option.execution_mode == MoveOptionExecutionMode::MoveModifier
                        })
                        .map(|option| option.id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            option_ids.into_iter().flat_map(move |move_option_id| {
                generate_piece_legal_move_actions_with_options(
                    game_state,
                    &piece_id,
                    &MoveGenerationOptions {
                        move_option_id: Some(move_option_id),
                    },
                )
            })
        })
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

    crate::placement::get_piece_placement_squares(game_state, player_id, piece)
        .into_iter()
        .map(|sq| DropAction {
            player_id: player_id.clone(),
            piece_id: piece_id.clone(),
            to: sq,
            captured_piece_id: game_state.board.get_piece_at(&sq).cloned(),
        })
        .collect()
}

/// Generate all legal drop actions for the current player.
pub fn generate_legal_drop_actions(game_state: &GameState) -> Vec<DropAction> {
    let player_id = &game_state.current_player;

    // A turn allows exactly one action: either one move or one pocket drop.
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
    if &game_state.current_player != player_id {
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
    type_counts
        .into_iter()
        .flat_map(|(piece_type_id, count)| {
            let squares = player
                .deck
                .pocket_pieces
                .iter()
                .filter_map(|id| game_state.pieces.get(id))
                .find(|piece| piece.type_id == piece_type_id)
                .map(|piece| {
                    crate::placement::get_piece_placement_squares(game_state, player_id, piece)
                })
                .unwrap_or_default();
            squares.into_iter().map(move |square| DropCandidateByType {
                player_id: player_id.clone(),
                piece_type_id: piece_type_id.clone(),
                count,
                to: square.to_id(),
            })
        })
        .collect()
}
