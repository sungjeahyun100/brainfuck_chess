use crate::types::*;

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

    // Consume move stack
    if let Some(piece) = game_state.pieces.get_mut(&action.piece_id) {
        piece.move_stack = piece.move_stack.saturating_sub(1);
        piece.has_moved = true;
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

    game_state
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn move_piece_on_board(mut game_state: GameState, action: &MoveAction) -> GameState {
    // Remove piece from source square
    game_state
        .board
        .squares
        .insert(action.from.to_id(), None);

    // Capture target piece if present
    if let Some(captured_id) = game_state.board.get_piece_at(&action.to).cloned() {
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
