use crate::legal_moves::{
    bomber_landing_targets, machine_gun_barrage_targets, mortar_barrage_targets,
    pending_landing_piece_id,
};
use crate::pieces::default_pieces::{
    BOMBER_BOMB_ABILITY_ID, BOMBER_LAND_ABILITY_ID, BOMBER_TAKEOFF_ABILITY_ID,
    INTERCEPT_ABILITY_ID, MACHINE_GUN_BARRAGE_ABILITY_ID, MORTAR_BARRAGE_ABILITY_ID,
    TANK_FIRE_ABILITY_ID,
};
use crate::rules::get_base_zone_squares;
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

fn is_king_type(type_id: &str) -> bool {
    type_id == "king"
}

fn is_rook_type(type_id: &str) -> bool {
    type_id == "rook"
}

fn is_en_passant_capture(game_state: &GameState, action: &MoveAction) -> bool {
    let Some(piece) = game_state.pieces.get(&action.piece_id) else {
        return false;
    };
    if piece.layer != PieceLayer::Ground || !is_pawn_type(&piece.type_id) {
        return false;
    }
    if action.from.file == action.to.file || !game_state.board.is_empty(&action.to) {
        return false;
    }
    game_state.en_passant_target == Some(action.to)
}

fn en_passant_target_for_action(game_state: &GameState, action: &MoveAction) -> Option<Square> {
    let piece = game_state.pieces.get(&action.piece_id)?;
    if !is_pawn_type(&piece.type_id) {
        return None;
    }
    if action.from.file != action.to.file || (action.to.rank - action.from.rank).abs() != 2 {
        return None;
    }
    Some(Square::new(
        action.from.file,
        (action.from.rank + action.to.rank) / 2,
    ))
}

/// Returns true if the piece definition marks this piece as royal (King).
pub fn is_royal_piece(definition: &PieceDefinition) -> bool {
    definition.is_king
}

/// Returns true if the given player still has a living King on the board.
pub fn has_living_king(game_state: &GameState, player_id: &PlayerId) -> bool {
    game_state.pieces.values().any(|p| {
        p.owner == *player_id
            && p.is_on_board()
            && game_state
                .piece_definitions
                .get(&p.type_id)
                .map(is_royal_piece)
                .unwrap_or(false)
    })
}

/// Apply a MoveAction to the game state.
/// If the captured piece is a King, the game ends immediately.
pub fn apply_move_action(mut game_state: GameState, action: MoveAction) -> GameState {
    // Detect what is on the destination square before moving
    let target_piece_id = action.captured_piece_id.clone();
    let target_is_king = target_piece_id.as_ref().and_then(|id| {
        game_state
            .pieces
            .get(id)
            .and_then(|p| game_state.piece_definitions.get(&p.type_id))
            .map(is_royal_piece)
    });
    // Move the piece
    game_state = move_piece_on_board(game_state, &action);
    consume_option_ammo(&mut game_state, &action.piece_id, &action.move_option_id);

    // Promote when the moving piece's definition allows this target type.
    if let Some(promotion_type) = action.promotion.as_ref() {
        let can_promote = game_state
            .pieces
            .get(&action.piece_id)
            .and_then(|piece| game_state.piece_definitions.get(&piece.type_id))
            .and_then(|definition| {
                definition.promotion_options_for_rank(action.to.rank, game_state.board.size)
            })
            .map(|options| options.iter().any(|option| option == promotion_type))
            .unwrap_or(false);

        if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
            if can_promote {
                piece.type_id = promotion_type.clone();
                if let Some(promoted_definition) = game_state.piece_definitions.get(promotion_type)
                {
                    piece.initialize_from_definition(promoted_definition);
                } else {
                    piece.state.clear();
                    piece.move_option_cooldowns.clear();
                }
            }
        }
    }

    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.has_moved = true;
    }

    replenish_ammo_on_home_entry(&mut game_state, &action.piece_id, action.from, action.to);

    for update in &action.effects.piece_state_updates {
        let Some(piece) = game_state.pieces.get(&update.piece_id) else {
            continue;
        };
        let value_is_valid = game_state
            .piece_definitions
            .get(&piece.type_id)
            .is_some_and(|definition| {
                definition.state_schema.iter().any(|state| {
                    state.key == update.key
                        && state.default_value.value_type() == update.value.value_type()
                })
            });
        if value_is_valid {
            if let Some(piece) = game_state.pieces.get_mut(&update.piece_id) {
                piece.state.insert(update.key.clone(), update.value.clone());
            }
        }
    }

    for update in &action.effects.global_state_updates {
        game_state
            .global_state
            .insert(update.key.clone(), update.value);
    }

    for update in &action.effects.cooldown_updates {
        let option_is_valid = game_state
            .pieces
            .get(&update.piece_id)
            .and_then(|piece| game_state.piece_definitions.get(&piece.type_id))
            .is_some_and(|definition| {
                definition
                    .move_options
                    .iter()
                    .any(|option| option.id == update.move_option_id && option.cooldown.is_some())
            });
        if option_is_valid {
            let Some(piece) = game_state.pieces.get_mut(&update.piece_id) else {
                continue;
            };
            piece.move_option_cooldowns.insert(
                update.move_option_id.clone(),
                CooldownState {
                    remaining: update.remaining,
                },
            );
        }
    }

    if let Some(transition) = &action.effects.piece_type_transition {
        let target_definition = game_state
            .piece_definitions
            .get(&transition.target_type_id)
            .cloned();
        if transition.piece_id == action.piece_id {
            if let (Some(piece), Some(definition)) = (
                game_state.pieces.get_mut(&transition.piece_id),
                target_definition.as_ref(),
            ) {
                piece.type_id = transition.target_type_id.clone();
                piece.initialize_from_definition(definition);
            }
        }
    }

    // A new pawn double-step replaces the previous right. Otherwise, only the
    // player who can claim en passant consumes that right by taking another action.
    if let Some(target) = en_passant_target_for_action(&game_state, &action) {
        game_state.en_passant_target = Some(target);
        game_state.en_passant_available_to = Some(if action.player_id == "white" {
            "black".to_string()
        } else {
            "white".to_string()
        });
    } else if game_state.en_passant_available_to.as_ref() == Some(&action.player_id) {
        game_state.en_passant_target = None;
        game_state.en_passant_available_to = None;
    }

    // Check if a King was captured → end the game immediately
    if target_is_king == Some(true) {
        game_state.phase = GamePhase::Ended;
        game_state.result = Some(GameResult {
            winner: Some(action.player_id.clone()),
            reason: GameEndReason::KingCapture,
        });
    }

    game_state
}

/// Applies one canonical action and advances exactly one turn. Cooldowns count
/// completed turns: an OwnerTurns cooldown set to N is not decremented on the
/// action that creates it, then decreases after each later action by its owner.
pub fn apply_and_advance_turn(mut game_state: GameState, action: TurnAction) -> GameState {
    let turn_number = game_state.turn_number;
    let player_id = match &action {
        TurnAction::Move(action) => action.player_id.clone(),
        TurnAction::Drop(action) => action.player_id.clone(),
        TurnAction::Ability(action) => action.player_id.clone(),
    };
    let is_forced_landing = matches!(
        &action,
        TurnAction::Ability(ability) if ability.ability_id == BOMBER_LAND_ABILITY_ID
    );
    let airborne_before = if is_forced_landing {
        Vec::new()
    } else {
        game_state
            .pieces
            .values()
            .filter(|piece| {
                piece.owner == player_id
                    && piece.is_on_board()
                    && piece.layer == PieceLayer::Air
                    && piece.remaining_flight_turns > 0
            })
            .map(|piece| piece.id.clone())
            .collect::<Vec<_>>()
    };
    let newly_set_cooldowns: std::collections::HashSet<(PieceId, String)> = match &action {
        TurnAction::Move(action) => action
            .effects
            .cooldown_updates
            .iter()
            .map(|update| (update.piece_id.clone(), update.move_option_id.clone()))
            .collect(),
        TurnAction::Ability(action) => {
            std::iter::once((action.piece_id.clone(), action.ability_id.clone())).collect()
        }
        TurnAction::Drop(_) => Default::default(),
    };

    game_state = match action.clone() {
        TurnAction::Move(action) => apply_move_action(game_state, action),
        TurnAction::Drop(action) => apply_drop_action(game_state, action),
        TurnAction::Ability(action) => apply_ability_action(game_state, action),
    };

    game_state.history.push(ActionRecord {
        turn_number,
        player_id: player_id.clone(),
        action,
    });

    if !is_forced_landing {
        tick_move_option_cooldowns(&mut game_state, &player_id, &newly_set_cooldowns);
        for piece_id in airborne_before {
            if let Some(piece) = game_state.pieces.get_mut(&piece_id) {
                piece.remaining_flight_turns = piece.remaining_flight_turns.saturating_sub(1);
            }
        }
        resolve_unlandable_bombers(&mut game_state, &player_id);
    }

    let landing_pending = pending_landing_piece_id(&game_state).is_some();
    if game_state.phase != GamePhase::Ended && game_state.result.is_none() && !landing_pending {
        game_state.current_player = if player_id == "white" {
            "black".into()
        } else {
            "white".into()
        };
        game_state.turn_number += 1;
    }
    game_state
}

pub fn apply_ability_action(mut state: GameState, action: AbilityAction) -> GameState {
    let cooldown_turns = state
        .pieces
        .get(&action.piece_id)
        .and_then(|piece| state.piece_definitions.get(&piece.type_id))
        .and_then(|definition| {
            definition
                .move_options
                .iter()
                .find(|option| option.id == action.ability_id)
        })
        .and_then(|option| option.cooldown.as_ref())
        .map(|cooldown| cooldown.turns)
        .filter(|turns| *turns > 0);
    if let (Some(piece), Some(remaining)) = (state.pieces.get_mut(&action.piece_id), cooldown_turns)
    {
        piece
            .move_option_cooldowns
            .insert(action.ability_id.clone(), CooldownState { remaining });
    }
    consume_option_ammo(&mut state, &action.piece_id, &action.ability_id);

    match action.ability_id.as_str() {
        TANK_FIRE_ABILITY_ID => {
            if let Some(impact) = action.to {
                apply_ground_blast(&mut state, impact, &action.player_id);
            }
        }
        BOMBER_TAKEOFF_ABILITY_ID => {
            let Some(to) = action.to else {
                return state;
            };
            let Some(from) = state
                .pieces
                .get(&action.piece_id)
                .and_then(|piece| piece.current_square)
            else {
                return state;
            };
            state
                .board
                .set_piece_at_layer(from, PieceLayer::Ground, None);
            state
                .board
                .set_piece_at_layer(to, PieceLayer::Air, Some(action.piece_id.clone()));
            if let Some(piece) = state.pieces.get_mut(&action.piece_id) {
                piece.current_square = Some(to);
                piece.layer = PieceLayer::Air;
                piece.remaining_flight_turns = 5;
                piece
                    .state
                    .insert("airborne".into(), PieceStateValue::Boolean(true));
            }
        }
        BOMBER_BOMB_ABILITY_ID => {
            if let Some(origin) = state
                .pieces
                .get(&action.piece_id)
                .and_then(|piece| piece.current_square)
            {
                apply_ground_blast(&mut state, origin, &action.player_id);
            }
        }
        INTERCEPT_ABILITY_ID => {
            let Some(target_id) = action.target_piece_id else {
                return state;
            };
            if remove_captured_piece(&mut state, &target_id, &action.player_id) {
                state.phase = GamePhase::Ended;
                state.result = Some(GameResult {
                    winner: Some(action.player_id.clone()),
                    reason: GameEndReason::KingCapture,
                });
            }
        }
        BOMBER_LAND_ABILITY_ID => {
            let Some(to) = action.to else {
                return state;
            };
            let Some(from) = state
                .pieces
                .get(&action.piece_id)
                .and_then(|piece| piece.current_square)
            else {
                return state;
            };
            state.board.set_piece_at_layer(from, PieceLayer::Air, None);
            state
                .board
                .set_piece_at_layer(to, PieceLayer::Ground, Some(action.piece_id.clone()));
            let home = get_base_zone_squares(&action.player_id, state.board.size);
            let max_ammo = state
                .pieces
                .get(&action.piece_id)
                .and_then(|piece| state.piece_definitions.get(&piece.type_id))
                .map_or(0, |definition| definition.max_ammo);
            if let Some(piece) = state.pieces.get_mut(&action.piece_id) {
                piece.current_square = Some(to);
                piece.layer = PieceLayer::Ground;
                piece.remaining_flight_turns = 0;
                piece
                    .state
                    .insert("airborne".into(), PieceStateValue::Boolean(false));
                if home.contains(&to) {
                    piece.current_ammo = max_ammo;
                }
            }
        }
        MORTAR_BARRAGE_ABILITY_ID => {
            let targets = action
                .to
                .and_then(|target| {
                    state
                        .pieces
                        .get(&action.piece_id)
                        .map(|actor| mortar_barrage_targets(&state, actor, target))
                })
                .unwrap_or_default();
            let mut removed_enemy_king = false;
            let mut removed_friendly_king = false;
            for target_id in targets {
                let target_owner = state
                    .pieces
                    .get(&target_id)
                    .map(|piece| piece.owner.clone());
                let was_king = remove_captured_piece(&mut state, &target_id, &action.player_id);
                if was_king {
                    if target_owner.as_ref() == Some(&action.player_id) {
                        removed_friendly_king = true;
                    } else {
                        removed_enemy_king = true;
                    }
                }
            }
            if removed_enemy_king || removed_friendly_king {
                state.phase = GamePhase::Ended;
                state.result = Some(GameResult {
                    winner: if removed_enemy_king {
                        Some(action.player_id.clone())
                    } else if action.player_id == "white" {
                        Some("black".into())
                    } else {
                        Some("white".into())
                    },
                    reason: GameEndReason::KingCapture,
                });
            }
        }
        MACHINE_GUN_BARRAGE_ABILITY_ID => {
            let targets = state
                .pieces
                .get(&action.piece_id)
                .map(|actor| machine_gun_barrage_targets(&state, actor))
                .unwrap_or_default();
            let mut removed_enemy_king = false;
            let mut removed_friendly_king = false;
            for target_id in targets {
                let target_owner = state
                    .pieces
                    .get(&target_id)
                    .map(|piece| piece.owner.clone());
                let was_king = remove_captured_piece(&mut state, &target_id, &action.player_id);
                if was_king {
                    if target_owner.as_ref() == Some(&action.player_id) {
                        removed_friendly_king = true;
                    } else {
                        removed_enemy_king = true;
                    }
                }
            }
            if removed_enemy_king || removed_friendly_king {
                state.phase = GamePhase::Ended;
                state.result = Some(GameResult {
                    winner: if removed_enemy_king {
                        Some(action.player_id.clone())
                    } else if action.player_id == "white" {
                        Some("black".into())
                    } else {
                        Some("white".into())
                    },
                    reason: GameEndReason::KingCapture,
                });
            }
        }
        "relieve" => {
            let Some(target_id) = action.target_piece_id else {
                return state;
            };
            let Some(pocket_id) = action.pocket_piece_id else {
                return state;
            };
            let Some(square) = action.to else {
                return state;
            };
            if let Some(target) = state.pieces.get_mut(&target_id) {
                target.current_square = None;
                target.in_pocket = true;
            }
            if let Some(pocket) = state.pieces.get_mut(&pocket_id) {
                pocket.current_square = Some(square);
                pocket.in_pocket = false;
            }
            if let Some(player) = state.players.get_mut(&action.player_id) {
                player.deck.pocket_pieces.retain(|id| id != &pocket_id);
                player.deck.pocket_pieces.push(target_id);
            }
            state.board.squares.insert(square.to_id(), Some(pocket_id));
        }
        "airdrop" => {
            for deployment in action.deployments {
                if let Some(pocket) = state.pieces.get_mut(&deployment.pocket_piece_id) {
                    pocket.current_square = Some(deployment.to);
                    pocket.in_pocket = false;
                }
                if let Some(player) = state.players.get_mut(&action.player_id) {
                    player
                        .deck
                        .pocket_pieces
                        .retain(|id| id != &deployment.pocket_piece_id);
                }
                state
                    .board
                    .squares
                    .insert(deployment.to.to_id(), Some(deployment.pocket_piece_id));
            }
        }
        "recall" => {
            let Some(target_id) = action.target_piece_id else {
                return state;
            };
            let Some(square) = action.to else {
                return state;
            };
            let Some(owner) = state
                .pieces
                .get(&target_id)
                .map(|piece| piece.owner.clone())
            else {
                return state;
            };
            if let Some(target) = state.pieces.get_mut(&target_id) {
                target.current_square = None;
                target.in_pocket = true;
            }
            state.board.squares.insert(square.to_id(), None);
            if let Some(player) = state.players.get_mut(&owner) {
                player.deck.pocket_pieces.push(target_id);
            }
        }
        _ => {}
    }
    if state.en_passant_available_to.as_ref() == Some(&action.player_id) {
        state.en_passant_target = None;
        state.en_passant_available_to = None;
    }
    state
}

fn apply_ground_blast(game_state: &mut GameState, center: Square, acting_player: &PlayerId) {
    let targets = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .filter_map(|(dx, dy)| {
            let square = Square::new(center.file + dx, center.rank + dy);
            game_state.board.get_piece_at(&square).cloned()
        })
        .collect::<Vec<_>>();
    let mut removed_enemy_king = false;
    let mut removed_friendly_king = false;
    for target_id in targets {
        let target_owner = game_state
            .pieces
            .get(&target_id)
            .map(|piece| piece.owner.clone());
        if remove_captured_piece(game_state, &target_id, acting_player) {
            if target_owner.as_ref() == Some(acting_player) {
                removed_friendly_king = true;
            } else {
                removed_enemy_king = true;
            }
        }
    }
    if removed_enemy_king || removed_friendly_king {
        game_state.phase = GamePhase::Ended;
        game_state.result = Some(GameResult {
            winner: if removed_enemy_king {
                Some(acting_player.clone())
            } else if acting_player == "white" {
                Some("black".into())
            } else {
                Some("white".into())
            },
            reason: GameEndReason::KingCapture,
        });
    }
}

/// Normalized piece-removal path shared by capture-like actions. It updates the
/// board, concrete piece state, and capture record, and reports royal removal.
fn remove_captured_piece(
    game_state: &mut GameState,
    piece_id: &PieceId,
    record_for_player: &PlayerId,
) -> bool {
    let Some(piece) = game_state.pieces.get(piece_id) else {
        return false;
    };
    let square = piece.current_square;
    let layer = piece.layer;
    let is_king = game_state
        .piece_definitions
        .get(&piece.type_id)
        .is_some_and(is_royal_piece);
    if let Some(square) = square {
        game_state.board.set_piece_at_layer(square, layer, None);
    }
    if let Some(piece) = game_state.pieces.get_mut(piece_id) {
        piece.captured = true;
        piece.current_square = None;
        piece.in_pocket = false;
    }
    if let Some(player) = game_state.players.get_mut(record_for_player) {
        if !player.captured_pieces.contains(piece_id) {
            player.captured_pieces.push(piece_id.clone());
        }
    }
    is_king
}

fn resolve_unlandable_bombers(game_state: &mut GameState, owner: &PlayerId) {
    let crashed = game_state
        .pieces
        .values()
        .filter(|piece| {
            piece.owner == *owner
                && piece.is_on_board()
                && piece.layer == PieceLayer::Air
                && piece.remaining_flight_turns == 0
                && piece.state.get("airborne") == Some(&PieceStateValue::Boolean(true))
                && bomber_landing_targets(game_state, piece).is_empty()
        })
        .map(|piece| piece.id.clone())
        .collect::<Vec<_>>();
    for piece_id in crashed {
        let Some((square, layer)) = game_state
            .pieces
            .get(&piece_id)
            .and_then(|piece| piece.current_square.map(|square| (square, piece.layer)))
        else {
            continue;
        };
        game_state.board.set_piece_at_layer(square, layer, None);
        if let Some(piece) = game_state.pieces.get_mut(&piece_id) {
            piece.captured = true;
            piece.current_square = None;
            piece.in_pocket = false;
        }
    }
}

fn tick_move_option_cooldowns(
    game_state: &mut GameState,
    acting_player: &PlayerId,
    newly_set: &std::collections::HashSet<(PieceId, String)>,
) {
    for (piece_id, piece) in &mut game_state.pieces {
        let option_clocks: std::collections::HashMap<String, CooldownClock> = game_state
            .piece_definitions
            .get(&piece.type_id)
            .map(|definition| {
                definition
                    .move_options
                    .iter()
                    .filter_map(|option| {
                        option
                            .cooldown
                            .as_ref()
                            .map(|cooldown| (option.id.clone(), cooldown.clock))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (option_id, cooldown) in &mut piece.move_option_cooldowns {
            if newly_set.contains(&(piece_id.clone(), option_id.clone())) {
                continue;
            }
            let should_tick = match option_clocks.get(option_id).copied() {
                Some(CooldownClock::GlobalTurns) => true,
                Some(CooldownClock::OwnerTurns) => &piece.owner == acting_player,
                None => false,
            };
            if should_tick {
                cooldown.remaining = cooldown.remaining.saturating_sub(1);
            }
        }
        piece
            .move_option_cooldowns
            .retain(|_, cooldown| cooldown.remaining > 0);
    }
}

fn consume_option_ammo(game_state: &mut GameState, piece_id: &PieceId, option_id: &str) {
    let (ammo_cost, max_ammo) = game_state
        .pieces
        .get(piece_id)
        .and_then(|piece| game_state.piece_definitions.get(&piece.type_id))
        .map(|definition| {
            let ammo_cost = definition
                .move_options
                .iter()
                .find(|option| option.id == option_id)
                .map_or(0, |option| option.ammo_cost);
            (ammo_cost, definition.max_ammo)
        })
        .unwrap_or((0, 0));
    if let Some(piece) = game_state.pieces.get_mut(piece_id) {
        piece.current_ammo = piece.current_ammo.saturating_sub(ammo_cost);
    }
    if ammo_cost > 0 && max_ammo > 0 {
        replenish_depleted_ammo_at_home(game_state, piece_id, max_ammo);
    }
}

fn replenish_depleted_ammo_at_home(game_state: &mut GameState, piece_id: &PieceId, max_ammo: u32) {
    let should_replenish = game_state.pieces.get(piece_id).is_some_and(|piece| {
        piece.current_ammo == 0
            && piece.layer == PieceLayer::Ground
            && piece.current_square.is_some_and(|square| {
                get_base_zone_squares(&piece.owner, game_state.board.size).contains(&square)
            })
    });
    if should_replenish {
        if let Some(piece) = game_state.pieces.get_mut(piece_id) {
            piece.current_ammo = max_ammo;
        }
    }
}

/// Apply a DropAction: move a pocket piece onto the board.
pub fn apply_drop_action(mut game_state: GameState, action: DropAction) -> GameState {
    let captured_is_king = action
        .captured_piece_id
        .as_ref()
        .is_some_and(|id| remove_captured_piece(&mut game_state, id, &action.player_id));
    // Remove from pocket list
    if let Some(player) = game_state.players.get_mut(&action.player_id) {
        player
            .deck
            .pocket_pieces
            .retain(|id| id != &action.piece_id);
    }

    // Update piece state
    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.in_pocket = false;
        piece.current_square = Some(action.to);
    }

    // Place on board
    game_state
        .board
        .squares
        .insert(action.to.to_id(), Some(action.piece_id.clone()));

    // If the player who could claim en passant used this turn for a drop,
    // the en passant right expires.
    if game_state.en_passant_available_to.as_ref() == Some(&game_state.current_player) {
        game_state.en_passant_target = None;
        game_state.en_passant_available_to = None;
    }

    if captured_is_king {
        game_state.phase = GamePhase::Ended;
        game_state.result = Some(GameResult {
            winner: Some(action.player_id),
            reason: GameEndReason::KingCapture,
        });
    }

    game_state
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn move_piece_on_board(mut game_state: GameState, action: &MoveAction) -> GameState {
    let moved_piece = game_state
        .pieces
        .get(&action.piece_id)
        .map(|piece| (piece.type_id.clone(), piece.layer));
    let moved_layer = moved_piece
        .as_ref()
        .map(|(_, layer)| *layer)
        .unwrap_or_default();
    let is_castling = moved_layer == PieceLayer::Ground
        && moved_piece
            .as_ref()
            .map(|(type_id, _)| type_id.as_str())
            .map(is_king_type)
            .unwrap_or(false)
        && (action.to.file - action.from.file).abs() == 2
        && action.to.rank == action.from.rank;

    // Remove piece from source square
    game_state
        .board
        .set_piece_at_layer(action.from, moved_layer, None);

    // Capture target piece if present (including en passant capture square).
    let mut capture_square = action.to;
    if is_en_passant_capture(&game_state, action) {
        capture_square = Square::new(action.to.file, action.from.rank);
    }

    if let Some(captured_id) = game_state
        .board
        .get_piece_at_layer(&capture_square, moved_layer)
        .cloned()
    {
        let record_for_player = game_state
            .players
            .values()
            .find(|p| p.id != action.player_id)
            .map(|player| player.id.clone())
            .unwrap_or_else(|| action.player_id.clone());
        remove_captured_piece(&mut game_state, &captured_id, &record_for_player);
    }

    if is_castling {
        let dir = (action.to.file - action.from.file).signum();
        let mut rook_file = action.from.file + dir;
        let rank = action.from.rank;
        let mut rook_piece_id: Option<PieceId> = None;
        let mut rook_from: Option<Square> = None;

        while rook_file >= 0 && rook_file < game_state.board.size {
            let sq = Square::new(rook_file, rank);
            if let Some(pid) = game_state.board.get_piece_at(&sq).cloned() {
                if let Some(rook) = game_state.pieces.get(&pid) {
                    if rook.owner == action.player_id
                        && is_rook_type(&rook.type_id)
                        && rook.current_square.is_some()
                    {
                        rook_piece_id = Some(pid);
                        rook_from = Some(sq);
                    }
                }
                break;
            }
            rook_file += dir;
        }

        if let (Some(rook_id), Some(from_sq)) = (rook_piece_id, rook_from) {
            let rook_to = Square::new(action.from.file + dir, rank);
            game_state.board.squares.insert(from_sq.to_id(), None);
            game_state
                .board
                .squares
                .insert(rook_to.to_id(), Some(rook_id.clone()));
            if let Some(rook_piece) = game_state.pieces.get_mut(&rook_id) {
                rook_piece.current_square = Some(rook_to);
                rook_piece.has_moved = true;
            }
        }
    }

    // Place piece on destination
    game_state
        .board
        .set_piece_at_layer(action.to, moved_layer, Some(action.piece_id.clone()));

    // Update piece position
    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.current_square = Some(action.to);
    }

    game_state
}

fn replenish_ammo_on_home_entry(
    game_state: &mut GameState,
    piece_id: &PieceId,
    from: Square,
    to: Square,
) {
    let Some(piece) = game_state.pieces.get(piece_id) else {
        return;
    };
    if piece.layer != PieceLayer::Ground {
        return;
    }
    let owner = piece.owner.clone();
    let max_ammo = game_state
        .piece_definitions
        .get(&piece.type_id)
        .map_or(0, |definition| definition.max_ammo);
    if max_ammo == 0 {
        return;
    }
    let home = get_base_zone_squares(&owner, game_state.board.size);
    if !home.contains(&from) && home.contains(&to) {
        if let Some(piece) = game_state.pieces.get_mut(piece_id) {
            piece.current_ammo = max_ammo;
        }
    }
}
