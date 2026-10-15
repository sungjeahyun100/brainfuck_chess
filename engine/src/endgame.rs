use crate::legal_moves::{
    bomber_landing_targets, machine_gun_barrage_targets, mortar_barrage_targets,
    pending_landing_piece_id, repairman_front_squares, sacrificial_piece_ids,
};
use crate::pieces::default_pieces::{
    BOMBER_BOMB_ABILITY_ID, BOMBER_LAND_ABILITY_ID, BOMBER_TAKEOFF_ABILITY_ID, DETONATE_ABILITY_ID,
    INTERCEPT_ABILITY_ID, MACHINE_GUN_BARRAGE_ABILITY_ID, MORTAR_BARRAGE_ABILITY_ID,
    REPAIR_WALLS_ABILITY_ID, SACRIFICE_ABILITY_ID, TANK_FIRE_ABILITY_ID,
};
use crate::rules::get_base_zone_squares_with_ruleset;
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
    // Move the piece
    let (next_state, removal) = move_piece_on_board(game_state, &action);
    game_state = next_state;
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

    apply_removal_result(&mut game_state, &removal, &action.player_id);

    game_state
}

/// Applies one canonical action and advances the turn only when it is consumed.
/// Cooldowns count
/// completed turns: an OwnerTurns cooldown set to N is not decremented on the
/// action that creates it, then decreases after each later action by its owner.
pub fn apply_and_advance_turn(mut game_state: GameState, action: TurnAction) -> GameState {
    if let TurnAction::Draw(draw) = &action {
        game_state
            .global_state
            .insert(crate::actions::DRAW_REQUIRED.into(), 0);
        game_state.history.push(ActionRecord {
            turn_number: game_state.turn_number,
            player_id: draw.player_id.clone(),
            action,
        });
        reconcile_occupation(&mut game_state);
        return game_state;
    }
    let accelerated_move = match &action {
        TurnAction::Move(movement) => {
            game_state
                .pieces
                .get(&movement.piece_id)
                .is_some_and(|piece| {
                    piece.state.get(crate::pieces::default_pieces::EXTRA_MOVE)
                        == Some(&PieceStateValue::Integer(1))
                        && crate::interaction::has_category(
                            piece,
                            crate::interaction::InteractionTag::Wizard,
                        )
                })
        }
        _ => false,
    };
    let keeps_turn = accelerated_move
        || matches!(&action,
        TurnAction::Ability(ability) if ability.ability_id == crate::pieces::default_pieces::ENCOURAGE);
    if accelerated_move {
        if let TurnAction::Move(movement) = &action {
            game_state
                .pieces
                .get_mut(&movement.piece_id)
                .unwrap()
                .state
                .remove(crate::pieces::default_pieces::EXTRA_MOVE);
        }
    }
    let turn_number = game_state.turn_number;
    let player_id = match &action {
        TurnAction::Draw(action) => action.player_id.clone(),
        TurnAction::Move(action) => action.player_id.clone(),
        TurnAction::Drop(action) => action.player_id.clone(),
        TurnAction::Ability(action) => action.player_id.clone(),
        TurnAction::ExtraSummon(action) => action.player_id.clone(),
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
        TurnAction::Draw(_) | TurnAction::Drop(_) | TurnAction::ExtraSummon(_) => {
            Default::default()
        }
    };

    // These versioned lifecycle values belong to the engine, not to a piece's
    // generic Chessembly state effects. Preserve them before advancing the turn.
    let draw_control =
        (game_state.global_state.get(crate::actions::MANUAL_DRAW) == Some(&1)).then(|| {
            game_state
                .global_state
                .get(crate::actions::DRAW_REQUIRED)
                .copied()
                .unwrap_or(0)
        });
    game_state = match action.clone() {
        TurnAction::Draw(_) => unreachable!("draw returns before turn advancement"),
        TurnAction::Move(action) => apply_move_action(game_state, action),
        TurnAction::Drop(action) => apply_drop_action(game_state, action),
        TurnAction::Ability(action) => apply_ability_action(game_state, action),
        TurnAction::ExtraSummon(action) => crate::summon::apply_extra_summon(game_state, action),
    };

    if let Some(required) = draw_control {
        game_state
            .global_state
            .insert(crate::actions::MANUAL_DRAW.into(), 1);
        game_state
            .global_state
            .insert(crate::actions::DRAW_REQUIRED.into(), required);
    }

    game_state.history.push(ActionRecord {
        turn_number,
        player_id: player_id.clone(),
        action,
    });

    reconcile_occupation(&mut game_state);

    if keeps_turn && game_state.phase != GamePhase::Ended && game_state.result.is_none() {
        return game_state;
    }

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
    if !landing_pending || game_state.phase == GamePhase::Ended || game_state.result.is_some() {
        for piece in game_state.pieces.values_mut() {
            if piece
                .state
                .contains_key(crate::pieces::default_pieces::EXTRA_MOVE)
                && crate::interaction::has_category(
                    piece,
                    crate::interaction::InteractionTag::Wizard,
                )
            {
                piece
                    .state
                    .remove(crate::pieces::default_pieces::EXTRA_MOVE);
            }
        }
    }
    if game_state.phase != GamePhase::Ended && game_state.result.is_none() && !landing_pending {
        game_state.current_player = if player_id == "white" {
            "black".into()
        } else {
            "white".into()
        };
        game_state.turn_number += 1;
        // The limit counts completed White/Black turn cycles, not actions.
        if player_id == "black" {
            finish_occupation_limit(&mut game_state);
        }
        if game_state.phase != GamePhase::Ended {
            crate::actions::prepare_draw(&mut game_state);
        }
    }
    game_state
}

fn reconcile_occupation(state: &mut GameState) {
    if state.occupation_zones.is_empty() {
        return;
    }
    // Older saved occupation states have counters and no arrow coordinates.
    if state.occupation_zones.iter().any(|zone| zone.effect_squares.is_empty()) {
        if let Some(zones) = crate::occupation::zones_for_board(state.board.size) {
            state.occupation_zones = zones;
        } else {
            return;
        }
    }
    // Decide every zone from the same post-action board.
    let owners: Vec<Option<PlayerId>> = state.occupation_zones.iter().map(|zone| {
        let (white, black) = state
            .pieces
            .values()
            .filter(|piece| {
                piece.is_on_board()
                    && piece
                        .current_square
                        .is_some_and(|sq| zone.squares.contains(&sq))
            })
            .fold((0, 0), |(white, black), piece| {
                if piece.owner == "white" {
                    (white + 1, black)
                } else if piece.owner == "black" {
                    (white, black + 1)
                } else {
                    (white, black)
                }
            });
        if white > black {
            Some("white".into())
        } else if black > white {
            Some("black".into())
        } else {
            None
        }
    }).collect();
    for (zone, owner) in state.occupation_zones.iter_mut().zip(owners) {
        zone.owner = owner;
    }
}

fn finish_occupation_limit(state: &mut GameState) {
    if state.occupation_zones.is_empty() {
        return;
    }
    // turn_number starts at 1 and advances once per consumed player turn.
    if state.phase != GamePhase::Ended && (state.turn_number - 1) / 2 >= 40 {
        let white: usize = state
            .occupation_zones
            .iter()
            .filter(|zone| zone.owner.as_deref() == Some("white"))
            .map(|zone| zone.squares.len())
            .sum();
        let black: usize = state
            .occupation_zones
            .iter()
            .filter(|zone| zone.owner.as_deref() == Some("black"))
            .map(|zone| zone.squares.len())
            .sum();
        state.phase = GamePhase::Ended;
        state.result = Some(GameResult {
            winner: if white > black {
                Some("white".into())
            } else if black > white {
                Some("black".into())
            } else {
                None
            },
            reason: GameEndReason::OccupationLimit,
        });
    }
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
        crate::pieces::default_pieces::TRANSFER_CIRCLE => {
            let id = action.target_piece_id.as_ref().expect("canonical passenger");
            let passenger = &state.pieces[id];
            let from = passenger.current_square.expect("canonical passenger square");
            let layer = passenger.layer;
            let to = action.to.expect("canonical transfer destination");
            state.board.set_piece_at_layer(from, layer, None);
            state.board.set_piece_at_layer(to, layer, Some(id.clone()));
            let passenger = state.pieces.get_mut(id).unwrap();
            passenger.current_square = Some(to);
            passenger.has_moved = true;
        }

        crate::pieces::default_pieces::ALEKHINES_GUN => {
            let origin = state.pieces[&action.piece_id].current_square.expect("canonical caster");
            let front = state.pieces[action.target_piece_id.as_ref().expect("canonical front rook")]
                .current_square.expect("canonical front square");
            let (dx, dy) = ((front.file - origin.file).signum(), (front.rank - origin.rank).signum());
            let mut square = Square::new(front.file + dx, front.rank + dy);
            let mut targets = Vec::new();
            while state.board.is_in_bounds(&square) {
                for layer in [PieceLayer::Ground, PieceLayer::Air] {
                    if let Some(id) = state.board.get_piece_at_layer(&square, layer) {
                        targets.push(id.clone());
                    }
                }
                square = Square::new(square.file + dx, square.rank + dy);
            }
            let mut removal = RemovalOutcome::default();
            for target in targets {
                removal.merge(remove_captured_piece(&mut state, &target, &action.player_id));
            }
            apply_removal_result(&mut state, &removal, &action.player_id);
        }

        crate::pieces::default_pieces::WIZARD_KNIGHT_CATCH
        | crate::pieces::default_pieces::WIZARD_BISHOP_JUMP_CATCH => {
            let target = action
                .target_piece_id
                .as_ref()
                .expect("canonical wizard catch target");
            let removal = remove_captured_piece(&mut state, target, &action.player_id);
            apply_removal_result(&mut state, &removal, &action.player_id);
        }

        crate::pieces::default_pieces::ENCOURAGE => {
            if let Some(target) = action
                .target_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get_mut(id))
            {
                target.state.insert(
                    crate::pieces::default_pieces::EXTRA_MOVE.into(),
                    PieceStateValue::Integer(1),
                );
            }
        }
        crate::pieces::default_pieces::LINKED_TELEPORT => {
            // Canonical validation has checked both occupants. No Move action,
            // capture, or intermediate movement trigger participates in this swap.
            let target_id = action
                .target_piece_id
                .as_ref()
                .expect("canonical swap target");
            let actor = &state.pieces[&action.piece_id];
            let target = &state.pieces[target_id];
            let from = actor.current_square.expect("canonical actor on board");
            let to = target.current_square.expect("canonical target on board");
            let actor_layer = actor.layer;
            let target_layer = target.layer;
            state.board.set_piece_at_layer(from, actor_layer, None);
            state.board.set_piece_at_layer(to, target_layer, None);
            state
                .board
                .set_piece_at_layer(from, target_layer, Some(target_id.clone()));
            state
                .board
                .set_piece_at_layer(to, actor_layer, Some(action.piece_id.clone()));
            state
                .pieces
                .get_mut(&action.piece_id)
                .unwrap()
                .current_square = Some(to);
            state.pieces.get_mut(target_id).unwrap().current_square = Some(from);
        }

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
        DETONATE_ABILITY_ID => {
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
            let removal = remove_captured_piece(&mut state, &target_id, &action.player_id);
            apply_removal_result(&mut state, &removal, &action.player_id);
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
            let home = get_base_zone_squares_with_ruleset(
                &action.player_id,
                state.board.size,
                state.ruleset,
            );
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
            let mut removal = RemovalOutcome::default();
            for target_id in targets {
                removal.merge(remove_captured_piece(
                    &mut state,
                    &target_id,
                    &action.player_id,
                ));
            }
            apply_removal_result(&mut state, &removal, &action.player_id);
        }
        MACHINE_GUN_BARRAGE_ABILITY_ID => {
            let targets = state
                .pieces
                .get(&action.piece_id)
                .map(|actor| machine_gun_barrage_targets(&state, actor))
                .unwrap_or_default();
            let mut removal = RemovalOutcome::default();
            for target_id in targets {
                removal.merge(remove_captured_piece(
                    &mut state,
                    &target_id,
                    &action.player_id,
                ));
            }
            apply_removal_result(&mut state, &removal, &action.player_id);
        }
        SACRIFICE_ABILITY_ID => {
            let Some(actor) = state.pieces.get(&action.piece_id).cloned() else {
                return state;
            };
            let sacrifices = sacrificial_piece_ids(&state, &actor);
            let targets = if action.target_piece_ids.is_empty() {
                action
                    .target_piece_id
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                action.target_piece_ids.clone()
            };
            let mut removal = RemovalOutcome::default();
            for piece_id in sacrifices.into_iter().chain(targets) {
                removal.merge(remove_captured_piece(
                    &mut state,
                    &piece_id,
                    &action.player_id,
                ));
            }
            apply_removal_result(&mut state, &removal, &action.player_id);
        }
        REPAIR_WALLS_ABILITY_ID => {
            let Some(actor) = state.pieces.get(&action.piece_id).cloned() else {
                return state;
            };
            let Some(definition) = state.piece_definitions.get("wall").cloned() else {
                return state;
            };
            for square in repairman_front_squares(&state, &actor) {
                if !state.board.is_empty(&square) {
                    continue;
                }
                let base = format!(
                    "{}-wall-{}-{}-{}",
                    action.piece_id, state.turn_number, square.file, square.rank
                );
                let mut id: PieceId = base.clone().into();
                let mut suffix = 2;
                while state.pieces.contains_key(&id) {
                    id = format!("{base}-{suffix}").into();
                    suffix += 1;
                }
                let mut wall = Piece {
                    id: id.clone(),
                    owner: action.player_id.clone(),
                    type_id: "wall".into(),
                    current_square: Some(square),
                    in_pocket: false,
                    captured: false,
                    has_moved: false,
                    current_ammo: 0,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                    state: Default::default(),
                    move_option_cooldowns: Default::default(),
                };
                wall.initialize_from_definition(&definition);
                state.board.squares.insert(square.to_id(), Some(id.clone()));
                state.pieces.insert(id, wall);
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
    let removal = remove_ground_blast_pieces(game_state, center, acting_player);
    apply_removal_result(game_state, &removal, acting_player);
}

fn remove_ground_blast_pieces(
    game_state: &mut GameState,
    center: Square,
    acting_player: &PlayerId,
) -> RemovalOutcome {
    let targets = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .filter_map(|(dx, dy)| {
            let square = Square::new(center.file + dx, center.rank + dy);
            game_state.board.get_piece_at(&square).cloned()
        })
        .collect::<Vec<_>>();
    let mut removal = RemovalOutcome::default();
    for target_id in targets {
        removal.merge(remove_captured_piece(game_state, &target_id, acting_player));
    }
    removal
}

#[derive(Default)]
struct RemovalOutcome {
    king_owners: std::collections::HashSet<PlayerId>,
    fanatic_death_squares: std::collections::HashSet<Square>,
}

impl RemovalOutcome {
    fn merge(&mut self, other: Self) {
        self.king_owners.extend(other.king_owners);
        self.fanatic_death_squares
            .extend(other.fanatic_death_squares);
    }
}

fn remove_fanatic_capturer(
    game_state: &mut GameState,
    outcome: &mut RemovalOutcome,
    capturer_id: &PieceId,
    acting_player: &PlayerId,
) {
    let capturer_reached_death_square = game_state
        .pieces
        .get(capturer_id)
        .and_then(|piece| piece.current_square)
        .is_some_and(|square| outcome.fanatic_death_squares.contains(&square));
    if capturer_reached_death_square {
        outcome.merge(remove_captured_piece(
            game_state,
            capturer_id,
            acting_player,
        ));
    }
}

fn apply_removal_result(
    game_state: &mut GameState,
    outcome: &RemovalOutcome,
    acting_player: &PlayerId,
) {
    if outcome.king_owners.is_empty() {
        return;
    }
    let opponent: PlayerId = if acting_player == "white" {
        "black".into()
    } else {
        "white".into()
    };
    game_state.phase = GamePhase::Ended;
    game_state.result = Some(GameResult {
        winner: if outcome.king_owners.contains(&opponent) {
            Some(acting_player.clone())
        } else {
            Some(opponent)
        },
        reason: GameEndReason::KingCapture,
    });
}

/// Normalized piece-removal path shared by capture-like actions. It updates the
/// board, concrete piece state, capture record, and Fanatic death cascade.
fn remove_captured_piece(
    game_state: &mut GameState,
    piece_id: &PieceId,
    record_for_player: &PlayerId,
) -> RemovalOutcome {
    let Some(piece) = game_state.pieces.get(piece_id) else {
        return RemovalOutcome::default();
    };
    if piece.captured || !piece.is_on_board() {
        return RemovalOutcome::default();
    }
    let square = piece.current_square;
    let is_fanatic = piece.type_id == "fanatic";
    let is_king = game_state
        .piece_definitions
        .get(&piece.type_id)
        .is_some_and(is_royal_piece);
    let layer = piece.layer;
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
    let mut outcome = RemovalOutcome::default();
    if is_fanatic {
        if let Some(square) = square {
            outcome.fanatic_death_squares.insert(square);
        }
    }
    if is_king {
        if let Some(owner) = game_state
            .pieces
            .get(piece_id)
            .map(|piece| piece.owner.clone())
        {
            outcome.king_owners.insert(owner);
        }
    }
    outcome
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
                get_base_zone_squares_with_ruleset(
                    &piece.owner,
                    game_state.board.size,
                    game_state.ruleset,
                )
                .contains(&square)
            })
    });
    if should_replenish {
        if let Some(piece) = game_state.pieces.get_mut(piece_id) {
            piece.current_ammo = max_ammo;
        }
    }
}

/// Apply a canonical DropAction from the ruleset-specific runtime reserve.
pub fn apply_drop_action(mut game_state: GameState, action: DropAction) -> GameState {
    crate::summon::remove_sacrifices(&mut game_state, &action.sacrifice_piece_ids);
    let removal = action
        .captured_piece_id
        .as_ref()
        .map(|id| remove_captured_piece(&mut game_state, id, &action.player_id))
        .unwrap_or_default();
    if let Some(player) = game_state.players.get_mut(&action.player_id) {
        let source = match game_state.ruleset {
            DeckRuleset::Legacy => &mut player.deck.pocket_pieces,
            DeckRuleset::Standard => &mut player.deck.hand_pieces,
        };
        source.retain(|id| id != &action.piece_id);
    }

    apply_drop_placement(game_state, action, removal)
}

/// Apply only the destination and existing Drop effects, after the source zone was removed.
fn apply_drop_placement(
    mut game_state: GameState,
    action: DropAction,
    mut removal: RemovalOutcome,
) -> GameState {
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

    remove_fanatic_capturer(
        &mut game_state,
        &mut removal,
        &action.piece_id,
        &action.player_id,
    );

    if game_state
        .pieces
        .get(&action.piece_id)
        .is_some_and(|piece| piece.type_id == "shell")
    {
        removal.merge(remove_ground_blast_pieces(
            &mut game_state,
            action.to,
            &action.player_id,
        ));
    }

    // If the player who could claim en passant used this turn for a drop,
    // the en passant right expires.
    if game_state.en_passant_available_to.as_ref() == Some(&game_state.current_player) {
        game_state.en_passant_target = None;
        game_state.en_passant_available_to = None;
    }

    apply_removal_result(&mut game_state, &removal, &action.player_id);

    game_state
}

pub(crate) fn apply_extra_placement(mut state: GameState, action: DropAction) -> GameState {
    let removal = action
        .captured_piece_id
        .as_ref()
        .map(|id| remove_captured_piece(&mut state, id, &action.player_id))
        .unwrap_or_default();
    apply_drop_placement(state, action, removal)
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn move_piece_on_board(
    mut game_state: GameState,
    action: &MoveAction,
) -> (GameState, RemovalOutcome) {
    let mut removal = RemovalOutcome::default();
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
            .and_then(|(type_id, _)| game_state.piece_definitions.get(type_id))
            .map(is_royal_piece)
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
        removal.merge(remove_captured_piece(
            &mut game_state,
            &captured_id,
            &record_for_player,
        ));
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

    remove_fanatic_capturer(
        &mut game_state,
        &mut removal,
        &action.piece_id,
        &action.player_id,
    );

    (game_state, removal)
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
    let home =
        get_base_zone_squares_with_ruleset(&owner, game_state.board.size, game_state.ruleset);
    if !home.contains(&from) && home.contains(&to) {
        if let Some(piece) = game_state.pieces.get_mut(piece_id) {
            piece.current_ammo = max_ammo;
        }
    }
}

#[cfg(test)]
mod occupation_tests {
    use super::*;
    use crate::occupation::zones_for_board;
    use crate::pieces::default_pieces::all_default_definitions;
    use crate::rules::create_board;
    use std::collections::HashMap;

    fn state(size: i32) -> GameState {
        let definitions: HashMap<_, _> = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect();
        let players = ["white", "black"]
            .into_iter()
            .map(|side| {
                (
                    side.into(),
                    Player {
                        id: side.into(),
                        deck: Deck {
                            player_id: side.into(),
                            starting_pieces: vec![],
                            pocket_pieces: vec![],
                            hand_pieces: vec![],
                            extra_deck_pieces: vec![],
                            score_limit: 39,
                            total_score: 0,
                        },
                        captured_pieces: vec![],
                    },
                )
            })
            .collect();
        GameState {
            ruleset: DeckRuleset::Legacy,
            occupation_zones: zones_for_board(size).unwrap(),
            id: "occupation-test".into(),
            board: create_board(size),
            pieces: HashMap::new(),
            piece_definitions: definitions.clone(),
            custom_piece_manifest: vec![],
            players,
            current_player: "white".into(),
            turn_number: 1,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: vec![],
            result: None,
            chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
        }
    }

    fn place(state: &mut GameState, id: &str, side: &str, square: Square) {
        let id: PieceId = id.into();
        state
            .board
            .set_piece_at_layer(square, PieceLayer::Ground, Some(id.clone()));
        state.pieces.insert(
            id.clone(),
            Piece {
                id,
                owner: side.into(),
                type_id: "rook".into(),
                current_square: Some(square),
                in_pocket: false,
                captured: false,
                has_moved: false,
                current_ammo: 0,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
            },
        );
    }

    fn move_action(side: &str, id: &str, from: Square, to: Square) -> TurnAction {
        TurnAction::Move(MoveAction {
            player_id: side.into(),
            piece_id: id.into(),
            from,
            to,
            captured_piece_id: None,
            promotion: None,
            move_option_id: "normal".into(),
            source_layer_ids: vec![],
            effects: ActionEffects::default(),
        })
    }

    fn sq(label: &str) -> Square {
        Square::new((label.as_bytes()[0] - b'a') as i32, label[1..].parse::<i32>().unwrap() - 1)
    }

    fn labels(squares: &[Square]) -> Vec<String> {
        squares.iter().map(|s| format!("{}{}", (b'a' + s.file as u8) as char, s.rank + 1)).collect()
    }

    #[test]
    fn fixed_judgment_and_effect_squares_match_images() {
        let expected = [
            (8, vec![
                ("b8", "a8 a7 b7 c7 c8"), ("g8", "f8 f7 g7 h7 h8"),
                ("a5 a4", "a6 b6 b5 b4 b3 a3"),
                ("d5 e5 d4 e4", "d6 e6 f6 c6 c5 c4 c3 d3 e3 f3 f4 f5"),
                ("h5 h4", "h6 g6 g5 g4 g3 h3"),
                ("b1", "a1 a2 b2 c2 c1"), ("g1", "f1 f2 g2 h2 h1"),
            ]),
            (12, vec![
                ("f12 g12", "e12 e11 f11 g11 h11 h12"),
                ("c11", "b12 c12 d12 b11 d11 b10 c10 d10"),
                ("j11", "i12 j12 k12 i11 k11 i10 j10 k10"),
                ("b7 c7 b6 c6", "a8 b8 c8 d8 a7 d7 a6 d6 a5 b5 c5 d5"),
                ("f7 g7 f6 g6", "e8 f8 g8 h8 e7 h7 e6 h6 e5 f5 g5 h5"),
                ("j7 k7 j6 k6", "i8 j8 k8 l8 i7 l7 i6 l6 i5 j5 k5 l5"),
                ("c2", "b3 c3 d3 b2 d2 b1 c1 d1"),
                ("j2", "i3 j3 k3 i2 k2 i1 j1 k1"),
                ("f1 g1", "e1 e2 f2 g2 h2 h1"),
            ]),
        ];
        for (size, expected_zones) in expected {
            let zones = zones_for_board(size).unwrap();
            assert_eq!(zones.len(), expected_zones.len());
            for (zone, (circles, arrows)) in zones.iter().zip(expected_zones) {
                assert_eq!(labels(&zone.squares), circles.split_whitespace().collect::<Vec<_>>());
                assert_eq!(labels(&zone.effect_squares), arrows.split_whitespace().collect::<Vec<_>>());
                assert_eq!(zone.owner, None);
            }
        }
        assert!(zones_for_board(10).is_none());
    }

    #[test]
    fn each_zone_uses_only_its_own_current_piece_counts() {
        let mut game = state(8);
        reconcile_occupation(&mut game);
        assert!(game.occupation_zones.iter().all(|zone| zone.owner.is_none()));
        place(&mut game, "w", "white", sq("a5"));
        place(&mut game, "b", "black", sq("a4"));
        place(&mut game, "other", "black", sq("g8"));
        reconcile_occupation(&mut game);
        assert_eq!(game.occupation_zones[2].owner, None);
        assert_eq!(game.occupation_zones[1].owner.as_deref(), Some("black"));
        remove_captured_piece(&mut game, &"b".into(), &"white".into());
        reconcile_occupation(&mut game);
        assert_eq!(game.occupation_zones[2].owner.as_deref(), Some("white"));
        assert_eq!(game.occupation_zones[1].owner.as_deref(), Some("black"));
        remove_captured_piece(&mut game, &"w".into(), &"black".into());
        reconcile_occupation(&mut game);
        assert_eq!(game.occupation_zones[2].owner, None);
    }

    #[test]
    fn new_claim_and_owner_flip_leave_pieces_and_capture_records_unchanged() {
        let mut game = state(8);
        place(&mut game, "holder", "white", sq("b8"));
        place(&mut game, "target", "black", sq("c7"));
        reconcile_occupation(&mut game);
        assert_eq!(game.occupation_zones[0].owner.as_deref(), Some("white"));
        assert!(!game.pieces["target"].captured);
        assert_eq!(game.board.get_piece_at(&sq("c7")), Some(&"target".into()));
        assert!(game.players["white"].captured_pieces.is_empty());
        reconcile_occupation(&mut game);
        assert!(!game.pieces["target"].captured);

        let mut flipped = state(8);
        flipped.occupation_zones[0].owner = Some("white".into());
        place(&mut flipped, "invader", "black", sq("b8"));
        place(&mut flipped, "white-target", "white", sq("a7"));
        reconcile_occupation(&mut flipped);
        assert_eq!(flipped.occupation_zones[0].owner.as_deref(), Some("black"));
        assert!(!flipped.pieces["white-target"].captured);
        assert_eq!(flipped.board.get_piece_at(&sq("a7")), Some(&"white-target".into()));
        assert!(flipped.players["black"].captured_pieces.is_empty());
    }

    #[test]
    fn simultaneous_claims_preserve_pieces_regardless_of_zone_order() {
        let mut game = state(8);
        place(&mut game, "w", "white", sq("b8"));
        place(&mut game, "b", "black", sq("g8"));
        place(&mut game, "enemy-left", "black", sq("c7"));
        place(&mut game, "enemy-right", "white", sq("f7"));
        let mut reversed = game.clone();
        reversed.occupation_zones.reverse();
        reconcile_occupation(&mut game);
        reconcile_occupation(&mut reversed);
        assert_eq!(game.occupation_zones[0].owner.as_deref(), Some("white"));
        assert_eq!(game.occupation_zones[1].owner.as_deref(), Some("black"));
        assert!(!game.pieces["enemy-left"].captured);
        assert!(!game.pieces["enemy-right"].captured);
        assert_eq!(game.pieces["enemy-left"].captured, reversed.pieces["enemy-left"].captured);
        assert_eq!(game.pieces["enemy-right"].captured, reversed.pieces["enemy-right"].captured);
        assert_eq!(game.players["white"].captured_pieces, reversed.players["white"].captured_pieces);
        assert_eq!(game.players["black"].captured_pieces, reversed.players["black"].captured_pieces);
        assert!(game.players.values().all(|player| player.captured_pieces.is_empty()));
    }

    #[test]
    fn normal_move_and_ability_captures_still_work_inside_claimed_arrows() {
        let mut movement = state(8);
        place(&mut movement, "holder", "white", sq("b8"));
        place(&mut movement, "target", "black", sq("c7"));
        place(&mut movement, "rook", "white", sq("c6"));
        reconcile_occupation(&mut movement);
        assert!(!movement.pieces["target"].captured);
        let capture = crate::legal_moves::generate_piece_legal_move_actions_with_options(
            &movement,
            &"rook".into(),
            &crate::legal_moves::MoveGenerationOptions::default(),
        ).into_iter().find(|action| action.to == sq("c7")).unwrap();
        let movement = crate::actions::submit_action(movement, TurnAction::Move(capture)).unwrap();
        assert!(movement.pieces["target"].captured);
        assert!(movement.players["black"].captured_pieces.contains(&"target".into()));

        let mut ability = state(8);
        place(&mut ability, "holder", "white", sq("b8"));
        place(&mut ability, "target", "black", sq("c7"));
        place(&mut ability, "gun", "white", sq("c5"));
        ability.pieces.get_mut("gun").unwrap().type_id = "machine-gunner".into();
        reconcile_occupation(&mut ability);
        assert!(!ability.pieces["target"].captured);
        let barrage = crate::legal_moves::generate_piece_legal_ability_actions(
            &ability,
            &"gun".into(),
            MACHINE_GUN_BARRAGE_ABILITY_ID,
        ).into_iter().next().unwrap();
        let ability = crate::actions::submit_action(ability, TurnAction::Ability(barrage)).unwrap();
        assert!(ability.pieces["target"].captured);
        assert!(ability.players["white"].captured_pieces.contains(&"target".into()));
    }

    #[test]
    fn claimed_arrows_extend_drop_range_and_neutral_retracts_it() {
        let mut game = state(8);
        place(&mut game, "holder", "white", sq("b8"));
        reconcile_occupation(&mut game);
        assert!(crate::placement::get_placement_squares(&game, &"white".into()).contains(&sq("c7")));
        remove_captured_piece(&mut game, &"holder".into(), &"black".into());
        reconcile_occupation(&mut game);
        assert_eq!(game.occupation_zones[0].owner, None);
        assert!(!crate::placement::get_placement_squares(&game, &"white".into()).contains(&sq("c7")));

        place(&mut game, "black-holder", "black", sq("b1"));
        reconcile_occupation(&mut game);
        assert!(crate::placement::get_placement_squares(&game, &"black".into()).contains(&sq("c2")));
        assert!(!crate::placement::get_placement_squares(&game, &"white".into()).contains(&sq("c7")));
    }

    #[test]
    fn turn_preserving_draw_reconciles_without_advancing_the_cycle() {
        let mut game = state(8);
        place(&mut game, "holder", "white", sq("b8"));
        game = apply_and_advance_turn(game, TurnAction::Draw(DrawAction {
            player_id: "white".into(),
            turn_number: 1,
        }));
        assert_eq!(game.occupation_zones[0].owner.as_deref(), Some("white"));
        assert_eq!(game.turn_number, 1);
        assert_eq!(game.current_player, "white");
    }

    #[test]
    fn old_counter_states_load_and_adopt_current_geometry_on_action() {
        let mut game = state(8);
        place(&mut game, "holder", "white", sq("b8"));
        let mut saved = serde_json::to_value(&game).unwrap();
        saved["occupation_zones"][0] = serde_json::json!({
            "squares": [{"file": 3, "rank": 7}, {"file": 4, "rank": 7}],
            "stack": 5,
            "owner": "white"
        });
        let mut loaded: GameState = serde_json::from_value(saved).unwrap();
        assert!(loaded.occupation_zones[0].effect_squares.is_empty());
        reconcile_occupation(&mut loaded);
        assert_eq!(loaded.occupation_zones[0].squares, vec![sq("b8")]);
        assert_eq!(loaded.occupation_zones[0].owner.as_deref(), Some("white"));
        assert!(!loaded.occupation_zones[0].effect_squares.is_empty());
    }

    #[test]
    fn every_action_updates_ownership_but_limit_waits_for_fortieth_black_turn() {
        let mut game = state(8);
        place(&mut game, "holder", "white", sq("b8"));
        place(&mut game, "white-mover", "white", sq("a3"));
        place(&mut game, "black-mover", "black", sq("h3"));
        for round in 1..=40 {
            let (wf, wt, bf, bt) = if round % 2 == 1 {
                ("a3", "a2", "h3", "h2")
            } else {
                ("a2", "a3", "h2", "h3")
            };
            game = apply_and_advance_turn(game, move_action("white", "white-mover", sq(wf), sq(wt)));
            assert_eq!(game.occupation_zones[0].owner.as_deref(), Some("white"));
            assert_eq!(game.phase, GamePhase::Playing);
            game = apply_and_advance_turn(game, move_action("black", "black-mover", sq(bf), sq(bt)));
            assert_eq!(game.turn_number, 2 * round + 1);
            if round < 40 { assert_eq!(game.phase, GamePhase::Playing); }
        }
        assert_eq!(game.phase, GamePhase::Ended);
        assert_eq!(game.result.as_ref().unwrap().winner.as_deref(), Some("white"));
        assert_eq!(game.result.as_ref().unwrap().reason, GameEndReason::OccupationLimit);
    }

    #[test]
    fn equal_claimed_circle_counts_draw_and_king_capture_takes_priority() {
        let mut draw = state(8);
        draw.turn_number = 80;
        draw.current_player = "black".into();
        place(&mut draw, "white-holder", "white", sq("b8"));
        place(&mut draw, "black-holder", "black", sq("g8"));
        place(&mut draw, "mover", "black", sq("h3"));
        draw = apply_and_advance_turn(draw, move_action("black", "mover", sq("h3"), sq("h2")));
        assert_eq!(draw.result.as_ref().unwrap().reason, GameEndReason::OccupationLimit);
        assert_eq!(draw.result.as_ref().unwrap().winner, None);

        let mut capture = state(8);
        capture.turn_number = 80;
        capture.current_player = "black".into();
        place(&mut capture, "mover", "black", sq("a3"));
        place(&mut capture, "king", "white", sq("a4"));
        capture.pieces.get_mut("king").unwrap().type_id = "king".into();
        let mut action = move_action("black", "mover", sq("a3"), sq("a4"));
        if let TurnAction::Move(movement) = &mut action {
            movement.captured_piece_id = Some("king".into());
        }
        capture = apply_and_advance_turn(capture, action);
        assert_eq!(capture.result.as_ref().unwrap().reason, GameEndReason::KingCapture);
        assert_eq!(capture.result.as_ref().unwrap().winner.as_deref(), Some("black"));
    }

    #[test]
    fn ordinary_mode_does_not_get_occupation_limit() {
        let mut game = state(8);
        game.occupation_zones.clear();
        game.turn_number = 80;
        game.current_player = "black".into();
        place(&mut game, "mover", "black", sq("h3"));
        game = apply_and_advance_turn(game, move_action("black", "mover", sq("h3"), sq("h2")));
        assert_eq!(game.phase, GamePhase::Playing);
        assert_eq!(game.turn_number, 81);
    }
}
