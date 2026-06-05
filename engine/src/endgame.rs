use crate::types::*;

fn is_pawn_type(type_id: &str) -> bool {
    type_id == "pawn-white" || type_id == "pawn-black"
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
    Some(Square::new(action.from.file, (action.from.rank + action.to.rank) / 2))
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
    let moved_piece_type = game_state
        .pieces
        .get(&action.piece_id)
        .map(|p| p.type_id.clone());

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

    // Consume move stack
    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.move_stack = piece.move_stack.saturating_sub(1);
        piece.has_moved = true;
    }

    // En passant availability lasts only until the eligible player's next action.
    game_state.en_passant_target = en_passant_target_for_action(&game_state, &action);
    game_state.en_passant_available_to = game_state.en_passant_target.map(|_| {
        if action.player_id == "white" {
            "black".to_string()
        } else {
            "white".to_string()
        }
    });

    // Defensive clear if the moved piece wasn't actually a pawn 2-step.
    if moved_piece_type.as_deref().map(is_pawn_type) != Some(true)
        || action.from.file != action.to.file
        || (action.to.rank - action.from.rank).abs() != 2
    {
        game_state.en_passant_target = None;
        game_state.en_passant_available_to = None;
    }

    // Record action
    game_state
        .turn_state
        .moved_piece_ids
        .insert(action.piece_id.clone());
    game_state
        .turn_state
        .actions
        .push(TurnAction::Move(action.clone()));

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

/// Apply a DropAction: move a pocket piece onto the board.
pub fn apply_drop_action(mut game_state: GameState, action: DropAction) -> GameState {
    // Remove from pocket list
    if let Some(player) = game_state.players.get_mut(&action.player_id) {
        player.deck.pocket_pieces.retain(|id| id != &action.piece_id);
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

    // Record action
    game_state
        .turn_state
        .actions
        .push(TurnAction::Drop(action));

    // If the player who could claim en passant used this turn for a drop,
    // the en passant right expires.
    if game_state.en_passant_available_to.as_ref() == Some(&game_state.current_player) {
        game_state.en_passant_target = None;
        game_state.en_passant_available_to = None;
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
    game_state
        .board
        .squares
        .insert(action.from.to_id(), None);

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
        let mut rook_piece_id: Option<String> = None;
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
