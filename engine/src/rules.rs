use crate::types::*;
use std::collections::HashMap;

/// Create an empty n×n board with all squares initialized to empty.
pub fn create_board(size: i32) -> Board {
    assert!(size >= 8, "Board size must be at least 8");
    let mut squares = HashMap::new();
    for rank in 0..size {
        for file in 0..size {
            let sq = Square::new(file, rank);
            squares.insert(sq.to_id(), None);
        }
    }
    Board { size, squares }
}

/// Deck score limit: scoreLimit = n*n - 25
pub fn calculate_score_limit(board_size: i32) -> u32 {
    (board_size * board_size - 25).max(0) as u32
}

/// Sum the scores of all non-king pieces in a deck.
pub fn calculate_deck_score(
    deck: &Deck,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> u32 {
    let all_piece_ids = deck.starting_pieces.iter().chain(deck.pocket_pieces.iter());
    all_piece_ids
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| !def.is_king)
        .map(|def| def.score)
        .sum()
}

/// Validate a deck:
/// - Exactly one King in starting pieces
/// - No King in pocket
/// - Total score ≤ score limit
/// - At least one piece besides King
pub fn validate_deck(
    deck: &Deck,
    board_size: i32,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> ValidationResult {
    let mut errors = Vec::new();

    let score_limit = calculate_score_limit(board_size);

    // Count kings in starting pieces
    let king_count = deck
        .starting_pieces
        .iter()
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| def.is_king)
        .count();

    if king_count == 0 {
        errors.push("덱에 King이 없습니다. 기본 진영에 King 1개를 포함해야 합니다.".into());
    } else if king_count > 1 {
        errors.push(format!("King이 {}개입니다. King은 1개만 허용됩니다.", king_count));
    }

    // No king in pocket
    let pocket_king_count = deck
        .pocket_pieces
        .iter()
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| def.is_king)
        .count();

    if pocket_king_count > 0 {
        errors.push("King은 포켓에 넣을 수 없습니다.".into());
    }

    // Score check
    let total_score = calculate_deck_score(deck, pieces, definitions);
    if total_score > score_limit {
        errors.push(format!(
            "덱 점수 {}점이 상한 {}점을 초과합니다.",
            total_score, score_limit
        ));
    }

    if errors.is_empty() {
        ValidationResult::ok()
    } else {
        ValidationResult::fail(errors)
    }
}

/// Return the base zone squares for a player.
/// White: rank 0, rank 1
/// Black: rank n-2, rank n-1
pub fn get_base_zone_squares(player_id: &PlayerId, board_size: i32) -> Vec<Square> {
    let ranks: Vec<i32> = if player_id == "white" {
        vec![0, 1]
    } else {
        vec![board_size - 2, board_size - 1]
    };
    let mut squares = Vec::new();
    for rank in ranks {
        for file in 0..board_size {
            squares.push(Square::new(file, rank));
        }
    }
    squares
}

/// Check if the current player has performed at least one action this turn.
pub fn can_end_turn(game_state: &GameState) -> bool {
    !game_state.turn_state.actions.is_empty()
}

/// End the current turn: switch player, reset turn state, grant move stacks.
pub fn end_turn(game_state: GameState) -> GameState {
    if !can_end_turn(&game_state) {
        return game_state;
    }

    let next_player = if game_state.current_player == "white" {
        "black".to_string()
    } else {
        "white".to_string()
    };

    let mut new_state = game_state;
    new_state.current_player = next_player.clone();
    new_state.turn_number += 1;
    new_state.turn_state = TurnState::new();

    // Grant move stack 1 to all board pieces of the next player
    for piece in new_state.pieces.values_mut() {
        if piece.owner == next_player && piece.is_on_board() {
            piece.move_stack = 1;
        }
    }

    new_state.invalidate_legal_action_cache();
    new_state
}

/// Grant move stacks at the start of a turn (called when game state is first created / turn begins).
pub fn grant_move_stacks(game_state: &mut GameState) {
    let current = game_state.current_player.clone();
    for piece in game_state.pieces.values_mut() {
        if piece.owner == current && piece.is_on_board() {
            piece.move_stack = 1;
        }
    }
    game_state.invalidate_legal_action_cache();
}
