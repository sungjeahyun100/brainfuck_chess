use std::collections::HashMap;
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::attack_map::generate_attack_map;
use crate::chessembly::run_chessembly_layer_for_piece;
use crate::interaction::{
    destination_is_blocked_by_interaction, neighboring_pieces, resolve_piece_interactions,
};
use crate::pieces::default_pieces::{MACHINE_GUN_BARRAGE_ABILITY_ID, MORTAR_BARRAGE_ABILITY_ID};
use crate::rules::{get_base_zone_squares, player_forward_direction};
use crate::terrain::{can_affect_square, can_capture_piece};
use crate::types::*;

fn is_pawn_type(type_id: &str) -> bool {
    matches!(
        type_id,
        "pawn-white"
            | "pawn-black"
            | "tempest-pawn-white"
            | "tempest-pawn-black"
            | "bouncing-pawn-white"
            | "bouncing-pawn-black"
    )
}

fn pawn_forward_dir(type_id: &str) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" | "bouncing-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" | "bouncing-pawn-black" => Some(-1),
        _ => None,
    }
}

fn pawn_start_rank(type_id: &str, board_size: i32) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" | "bouncing-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" | "bouncing-pawn-black" => Some(board_size - 2),
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
    if destination_is_blocked_by_interaction(
        context.game_state,
        context.piece,
        to,
        &context.option.id,
    ) {
        return;
    }

    if captured_piece_id
        .as_ref()
        .and_then(|piece_id| context.game_state.pieces.get(piece_id))
        .is_some_and(|victim| !can_capture_piece(context.game_state, context.piece, victim))
    {
        return;
    }

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
    let piece_type_transition = result
        .effects
        .get(&to.to_id())
        .and_then(|effect| effect.transition_to.clone())
        .map(|target_type_id| PieceTypeTransition {
            piece_id: piece_id.clone(),
            target_type_id,
        });
    ActionEffects {
        global_state_updates,
        piece_state_updates,
        cooldown_updates,
        piece_type_transition,
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
    for option in definition
        .move_options
        .iter()
        .filter(|option| option.contributes_to_attack_map && can_use_move_option(piece, option))
    {
        let mut option_attacked = Vec::new();
        for layer_id in &option.layer_ids {
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
            option_attacked.extend(
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
        option_attacked.retain(|to| {
            !destination_is_blocked_by_interaction(game_state, piece, *to, &option.id)
                && can_affect_square(game_state, piece, *to)
        });
        attacked.extend(option_attacked);
        attacked.extend(
            resolve_piece_interactions(game_state, piece, &option.id)
                .attack_squares
                .into_iter()
                .filter(|square| can_affect_square(game_state, piece, *square)),
        );
    }
    attacked.retain(|sq| game_state.board.is_in_bounds(sq));
    attacked.sort_by_key(|sq| (sq.rank, sq.file));
    attacked.dedup();
    attacked
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
    let interaction_context = MoveBuildContext {
        game_state,
        piece_id,
        piece,
        definition,
        player_id,
        option: selected_option,
        layer: special_layer,
    };
    for candidate in resolve_piece_interactions(game_state, piece, &selected_option.id).moves {
        push_move_or_promotions(
            &mut actions,
            &interaction_context,
            candidate.to,
            candidate.captured_piece_id,
            &special_effects,
        );
    }

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
                                && can_capture_piece(game_state, piece, captured_piece)
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
        .filter(|sq| {
            game_state
                .board
                .get_piece_at(sq)
                .and_then(|target_id| game_state.pieces.get(target_id))
                .is_none_or(|victim| can_capture_piece(game_state, piece, victim))
        })
        .map(|sq| DropAction {
            player_id: player_id.clone(),
            piece_id: piece_id.clone(),
            to: sq,
            captured_piece_id: game_state.board.get_piece_at(&sq).cloned(),
        })
        .collect()
}

/// Generate canonical targets for the three built-in standalone abilities.
pub fn generate_piece_legal_ability_actions(
    game_state: &GameState,
    piece_id: &PieceId,
    ability_id: &str,
) -> Vec<AbilityAction> {
    let Some(actor) = game_state.pieces.get(piece_id) else {
        return Vec::new();
    };
    if actor.owner != game_state.current_player || !actor.is_on_board() {
        return Vec::new();
    }
    let Some(origin) = actor.current_square else {
        return Vec::new();
    };
    let ability_is_available = game_state
        .piece_definitions
        .get(&actor.type_id)
        .and_then(|definition| {
            definition
                .move_options
                .iter()
                .find(|option| option.id == ability_id)
        })
        .is_some_and(|option| {
            option.execution_mode == MoveOptionExecutionMode::StandaloneAction
                && actor
                    .move_option_cooldowns
                    .get(ability_id)
                    .is_none_or(|cooldown| cooldown.remaining == 0)
        });
    if !ability_is_available {
        return Vec::new();
    }
    let mut actions = Vec::new();
    let adjacent = neighboring_pieces(game_state, actor);
    match (actor.type_id.as_str(), ability_id) {
        ("mortar", MORTAR_BARRAGE_ABILITY_ID) => {
            let opponent_id = if actor.owner == "white" {
                "black".into()
            } else {
                "white".into()
            };
            let opponent_base_zone = get_base_zone_squares(&opponent_id, game_state.board.size);
            for file in 0..game_state.board.size {
                let has_friendly_piece = game_state.pieces.values().any(|piece| {
                    piece.owner == actor.owner
                        && piece
                            .current_square
                            .is_some_and(|square| square.file == file)
                });
                if !has_friendly_piece {
                    continue;
                }
                for rank in 0..game_state.board.size {
                    let target = Square::new(file, rank);
                    if !opponent_base_zone.contains(&target) {
                        actions.push(AbilityAction {
                            player_id: actor.owner.clone(),
                            piece_id: piece_id.clone(),
                            ability_id: ability_id.into(),
                            target_piece_id: None,
                            pocket_piece_id: None,
                            to: Some(target),
                            deployments: Vec::new(),
                        });
                    }
                }
            }
        }
        ("machine-gunner", MACHINE_GUN_BARRAGE_ABILITY_ID) => {
            actions.push(AbilityAction {
                player_id: actor.owner.clone(),
                piece_id: piece_id.clone(),
                ability_id: ability_id.into(),
                target_piece_id: None,
                pocket_piece_id: None,
                to: Some(origin),
                deployments: Vec::new(),
            });
        }
        ("alternating-soldier", "relieve") => {
            let Some(player) = game_state.players.get(&actor.owner) else {
                return Vec::new();
            };
            for target in adjacent.iter().filter(|target| target.owner == actor.owner) {
                for pocket_id in &player.deck.pocket_pieces {
                    let Some(pocket) = game_state.pieces.get(pocket_id) else {
                        continue;
                    };
                    if pocket.in_pocket && !pocket.captured {
                        actions.push(AbilityAction {
                            player_id: actor.owner.clone(),
                            piece_id: piece_id.clone(),
                            ability_id: ability_id.into(),
                            target_piece_id: Some(target.id.clone()),
                            pocket_piece_id: Some(pocket_id.clone()),
                            to: target.current_square,
                            deployments: Vec::new(),
                        });
                    }
                }
            }
        }
        ("airborne", "airdrop") => {
            let Some(player) = game_state.players.get(&actor.owner) else {
                return Vec::new();
            };
            let forward = if actor.owner == "white" { 1 } else { -1 };
            for pocket_id in &player.deck.pocket_pieces {
                let Some(pocket) = game_state.pieces.get(pocket_id) else {
                    continue;
                };
                let eligible = pocket.in_pocket
                    && !pocket.captured
                    && game_state
                        .piece_definitions
                        .get(&pocket.type_id)
                        .is_some_and(|def| !def.is_king && def.score <= 4);
                if !eligible {
                    continue;
                }
                for depth in 1..=2 {
                    for width in -1..=1 {
                        let to = Square::new(origin.file + width, origin.rank + forward * depth);
                        if game_state.board.is_in_bounds(&to) && game_state.board.is_empty(&to) {
                            actions.push(AbilityAction {
                                player_id: actor.owner.clone(),
                                piece_id: piece_id.clone(),
                                ability_id: ability_id.into(),
                                target_piece_id: None,
                                pocket_piece_id: Some(pocket_id.clone()),
                                to: Some(to),
                                deployments: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
        ("green-camp", "recall") => {
            for target in adjacent {
                if game_state
                    .piece_definitions
                    .get(&target.type_id)
                    .is_some_and(|def| !def.is_king)
                {
                    actions.push(AbilityAction {
                        player_id: actor.owner.clone(),
                        piece_id: piece_id.clone(),
                        ability_id: ability_id.into(),
                        target_piece_id: Some(target.id.clone()),
                        pocket_piece_id: None,
                        to: target.current_square,
                        deployments: Vec::new(),
                    });
                }
            }
        }
        _ => {}
    }
    actions
}

/// Mortar removes pieces on the selected point and its four orthogonally
/// adjacent squares. Target-square range validation is handled by canonical
/// ability generation before an action can be submitted.
pub(crate) fn mortar_barrage_targets(
    game_state: &GameState,
    actor: &Piece,
    target: Square,
) -> Vec<PieceId> {
    [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .filter_map(|(file_offset, rank_offset)| {
            let square = Square::new(target.file + file_offset, target.rank + rank_offset);
            game_state
                .board
                .get_piece_at(&square)
                .and_then(|piece_id| {
                    game_state
                        .pieces
                        .get(piece_id)
                        .map(|piece| (piece_id, piece))
                })
                .filter(|(_, victim)| can_capture_piece(game_state, actor, victim))
                .map(|(piece_id, _)| piece_id.clone())
        })
        .collect()
}

/// Machine Gunner removes pieces in the 2x3 rectangle immediately ahead of it.
pub(crate) fn machine_gun_barrage_targets(game_state: &GameState, actor: &Piece) -> Vec<PieceId> {
    let Some(origin) = actor.current_square else {
        return Vec::new();
    };
    let forward = player_forward_direction(&actor.owner);
    let mut targets = Vec::new();
    for rank_offset in 1..=2 {
        for file_offset in -1..=1 {
            let square = Square::new(
                origin.file + file_offset,
                origin.rank + forward * rank_offset,
            );
            if let Some(piece_id) = game_state.board.get_piece_at(&square) {
                if game_state
                    .pieces
                    .get(piece_id)
                    .is_some_and(|victim| can_capture_piece(game_state, actor, victim))
                {
                    targets.push(piece_id.clone());
                }
            }
        }
    }
    targets
}

/// Validate a submitted ability against canonical candidates. Airborne is a
/// bounded multi-deployment action, so each unique deployment must be one of
/// the canonical single-piece candidates generated from the same state.
pub fn is_legal_ability_action(game_state: &GameState, action: &AbilityAction) -> bool {
    if action.ability_id != "airdrop" {
        return action.deployments.is_empty()
            && generate_piece_legal_ability_actions(
                game_state,
                &action.piece_id,
                &action.ability_id,
            )
            .into_iter()
            .any(|candidate| candidate == *action);
    }
    if action.player_id != game_state.current_player
        || action.deployments.is_empty()
        || action.target_piece_id.is_some()
        || action.pocket_piece_id.is_some()
        || action.to.is_some()
    {
        return false;
    }
    let singles = generate_piece_legal_ability_actions(game_state, &action.piece_id, "airdrop");
    let mut piece_ids = std::collections::HashSet::new();
    let mut squares = std::collections::HashSet::new();
    action.deployments.iter().all(|deployment| {
        piece_ids.insert(deployment.pocket_piece_id.clone())
            && squares.insert(deployment.to.to_id())
            && singles.iter().any(|candidate| {
                candidate.pocket_piece_id.as_ref() == Some(&deployment.pocket_piece_id)
                    && candidate.to == Some(deployment.to)
            })
    })
}

/// Generate every standalone ability action available to the current player.
pub fn generate_legal_ability_actions(game_state: &GameState) -> Vec<AbilityAction> {
    fn canonical_airdrop_actions(
        player_id: &PlayerId,
        actor_id: &PieceId,
        singles: Vec<AbilityAction>,
    ) -> Vec<AbilityAction> {
        let mut by_piece = std::collections::BTreeMap::<PieceId, Vec<Square>>::new();
        for single in singles {
            if let (Some(piece_id), Some(to)) = (single.pocket_piece_id, single.to) {
                by_piece.entry(piece_id).or_default().push(to);
            }
        }
        let choices = by_piece.into_iter().collect::<Vec<_>>();
        let mut actions = Vec::new();
        let mut deployments = Vec::new();
        let mut occupied = std::collections::HashSet::new();

        fn extend(
            index: usize,
            choices: &[(PieceId, Vec<Square>)],
            player_id: &PlayerId,
            actor_id: &PieceId,
            deployments: &mut Vec<AbilityDeployment>,
            occupied: &mut std::collections::HashSet<SquareId>,
            actions: &mut Vec<AbilityAction>,
        ) {
            if index == choices.len() {
                return;
            }
            extend(
                index + 1,
                choices,
                player_id,
                actor_id,
                deployments,
                occupied,
                actions,
            );
            let (pocket_piece_id, squares) = &choices[index];
            for to in squares {
                if !occupied.insert(to.to_id()) {
                    continue;
                }
                deployments.push(AbilityDeployment {
                    pocket_piece_id: pocket_piece_id.clone(),
                    to: *to,
                });
                actions.push(AbilityAction {
                    player_id: player_id.clone(),
                    piece_id: actor_id.clone(),
                    ability_id: "airdrop".into(),
                    target_piece_id: None,
                    pocket_piece_id: None,
                    to: None,
                    deployments: deployments.clone(),
                });
                extend(
                    index + 1,
                    choices,
                    player_id,
                    actor_id,
                    deployments,
                    occupied,
                    actions,
                );
                deployments.pop();
                occupied.remove(&to.to_id());
            }
        }

        extend(
            0,
            &choices,
            player_id,
            actor_id,
            &mut deployments,
            &mut occupied,
            &mut actions,
        );
        actions
    }

    let mut piece_ids = game_state
        .pieces
        .iter()
        .filter_map(|(id, piece)| (piece.owner == game_state.current_player).then_some(id.clone()))
        .collect::<Vec<_>>();
    piece_ids.sort();
    piece_ids
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
                            option.execution_mode == MoveOptionExecutionMode::StandaloneAction
                        })
                        .map(|option| option.id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            option_ids.into_iter().flat_map(move |option_id| {
                let actions =
                    generate_piece_legal_ability_actions(game_state, &piece_id, &option_id);
                if option_id == "airdrop" {
                    canonical_airdrop_actions(&game_state.current_player, &piece_id, actions)
                } else {
                    actions
                }
            })
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
