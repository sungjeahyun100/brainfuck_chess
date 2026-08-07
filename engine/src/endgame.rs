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
    if !is_pawn_type(&piece.type_id) {
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
    let target_piece_id = game_state.board.get_piece_at(&action.to).cloned();
    let target_is_king = target_piece_id.as_ref().and_then(|id| {
        game_state
            .pieces
            .get(id)
            .and_then(|p| game_state.piece_definitions.get(&p.type_id))
            .map(is_royal_piece)
    });
    // Move the piece
    game_state = move_piece_on_board(game_state, &action);

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
    };
    let newly_set_cooldowns: std::collections::HashSet<(PieceId, String)> = match &action {
        TurnAction::Move(action) => action
            .effects
            .cooldown_updates
            .iter()
            .map(|update| (update.piece_id.clone(), update.move_option_id.clone()))
            .collect(),
        _ => Default::default(),
    };

    game_state = match action.clone() {
        TurnAction::Move(action) => apply_move_action(game_state, action),
        TurnAction::Drop(action) => apply_drop_action(game_state, action),
    };

    game_state.history.push(ActionRecord {
        turn_number,
        player_id: player_id.clone(),
        action,
    });

    tick_move_option_cooldowns(&mut game_state, &player_id, &newly_set_cooldowns);

    if game_state.phase != GamePhase::Ended && game_state.result.is_none() {
        game_state.current_player = if player_id == "white" {
            "black".into()
        } else {
            "white".into()
        };
        game_state.turn_number += 1;
    }
    game_state
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

/// Apply a DropAction: move a pocket piece onto the board.
pub fn apply_drop_action(mut game_state: GameState, action: DropAction) -> GameState {
    let captured_is_king = action.captured_piece_id.as_ref().is_some_and(|id| {
        game_state
            .pieces
            .get(id)
            .and_then(|piece| game_state.piece_definitions.get(&piece.type_id))
            .is_some_and(is_royal_piece)
    });
    if let Some(captured_id) = &action.captured_piece_id {
        if let Some(captured) = game_state.pieces.get_mut(captured_id) {
            captured.captured = true;
            captured.current_square = None;
        }
        if let Some(player) = game_state.players.get_mut(&action.player_id) {
            player.captured_pieces.push(captured_id.clone());
        }
    }
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
    let moved_piece_type = game_state
        .pieces
        .get(&action.piece_id)
        .map(|p| p.type_id.clone());
    let is_castling = moved_piece_type
        .as_deref()
        .map(is_king_type)
        .unwrap_or(false)
        && (action.to.file - action.from.file).abs() == 2
        && action.to.rank == action.from.rank;

    // Remove piece from source square
    game_state.board.squares.insert(action.from.to_id(), None);

    // Capture target piece if present (including en passant capture square).
    let mut capture_square = action.to;
    if is_en_passant_capture(&game_state, action) {
        capture_square = Square::new(action.to.file, action.from.rank);
    }

    if let Some(captured_id) = game_state.board.get_piece_at(&capture_square).cloned() {
        game_state
            .board
            .squares
            .insert(capture_square.to_id(), None);
        if let Some(captured) = game_state.pieces.get_mut(&captured_id) {
            captured.captured = true;
            captured.current_square = None;
        }
        if let Some(opponent) = game_state
            .players
            .values_mut()
            .find(|p| p.id != action.player_id)
        {
            opponent.captured_pieces.push(captured_id);
        }
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
        .squares
        .insert(action.to.to_id(), Some(action.piece_id.clone()));

    // Update piece position
    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.current_square = Some(action.to);
    }

    game_state
}
