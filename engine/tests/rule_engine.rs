//! Brainfuck Chess rule engine integration tests.

use std::collections::HashMap;

use brainfuck_chess_engine::endgame::{apply_drop_action, apply_move_action, has_living_king};
use brainfuck_chess_engine::pieces::default_pieces::*;
use brainfuck_chess_engine::rules::*;
use brainfuck_chess_engine::types::*;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_game_state(board_size: i32) -> GameState {
    let board = create_board(board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    let white_deck = Deck {
        player_id: "white".into(),
        starting_pieces: Vec::new(),
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };
    let black_deck = Deck {
        player_id: "black".into(),
        starting_pieces: Vec::new(),
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };

    let mut players = HashMap::new();
    players.insert(
        "white".into(),
        Player { id: "white".into(), deck: white_deck, captured_pieces: Vec::new() },
    );
    players.insert(
        "black".into(),
        Player { id: "black".into(), deck: black_deck, captured_pieces: Vec::new() },
    );

    GameState {
        id: "test".into(),
        board,
        pieces: HashMap::new(),
        piece_definitions: defs,
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        turn_state: TurnState::new(),
        result: None,
    }
}

fn add_piece(
    state: &mut GameState,
    id: &str,
    owner: &str,
    type_id: &str,
    file: i32,
    rank: i32,
) {
    let sq = Square::new(file, rank);
    let piece = Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: Some(sq),
        in_pocket: false,
        captured: false,
        move_stack: 1,
        has_moved: false,
    };
    state.board.squares.insert(sq.to_id(), Some(id.into()));
    state.pieces.insert(id.into(), piece.clone());
    state.players.get_mut(owner).unwrap().deck.starting_pieces.push(id.into());
}

fn add_pocket_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str) {
    let piece = Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: None,
        in_pocket: true,
        captured: false,
        move_stack: 0,
        has_moved: false,
    };
    state.pieces.insert(id.into(), piece);
    state.players.get_mut(owner).unwrap().deck.pocket_pieces.push(id.into());
}

// ─── Board creation ───────────────────────────────────────────────────────────

#[test]
fn test_create_board_8x8() {
    let board = create_board(8);
    assert_eq!(board.size, 8);
    assert_eq!(board.squares.len(), 64);
    for (_, v) in &board.squares {
        assert!(v.is_none());
    }
}

#[test]
fn test_create_board_10x10() {
    let board = create_board(10);
    assert_eq!(board.size, 10);
    assert_eq!(board.squares.len(), 100);
}

// ─── Score limit ─────────────────────────────────────────────────────────────

#[test]
fn test_score_limit_8x8() {
    assert_eq!(calculate_score_limit(8), 39);
}

#[test]
fn test_score_limit_9x9() {
    assert_eq!(calculate_score_limit(9), 56);
}

#[test]
fn test_score_limit_10x10() {
    assert_eq!(calculate_score_limit(10), 75);
}

// ─── Base zone ───────────────────────────────────────────────────────────────

#[test]
fn test_white_base_zone_8x8() {
    let zones = get_base_zone_squares(&"white".to_string(), 8);
    assert_eq!(zones.len(), 16); // 2 ranks × 8 files
    assert!(zones.contains(&Square::new(0, 0)));
    assert!(zones.contains(&Square::new(7, 1)));
    assert!(!zones.contains(&Square::new(0, 2)));
}

#[test]
fn test_black_base_zone_8x8() {
    let zones = get_base_zone_squares(&"black".to_string(), 8);
    assert_eq!(zones.len(), 16);
    assert!(zones.contains(&Square::new(0, 6)));
    assert!(zones.contains(&Square::new(7, 7)));
    assert!(!zones.contains(&Square::new(0, 5)));
}

// ─── Deck validation ─────────────────────────────────────────────────────────

#[test]
fn test_deck_validation_no_king() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "p1", "white", "pawn-white", 0, 0);
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("King")));
}

#[test]
fn test_deck_validation_king_in_starting() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(result.valid, "errors: {:?}", result.errors);
}

#[test]
fn test_deck_validation_king_in_pocket_forbidden() {
    let mut state = make_game_state(8);
    // Add king to starting pieces first (so the "no king" check passes)
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    // Also add king to pocket — should fail
    add_pocket_piece(&mut state, "k2", "white", "king");
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("포켓")));
}

#[test]
fn test_deck_score_over_limit() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    // 5 queens = 45 points > 39 limit
    for i in 0..5 {
        add_piece(&mut state, &format!("q{}", i), "white", "queen", i, 0);
    }
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("점수")));
}

// ─── Turn management ─────────────────────────────────────────────────────────

#[test]
fn test_cannot_end_turn_without_action() {
    let state = make_game_state(8);
    assert!(!can_end_turn(&state));
}

#[test]
fn test_end_turn_switches_player() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "k2", "black", "king", 4, 7);

    // Manually push an action so we can end the turn
    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: None,
    };
    state.turn_state.actions.push(TurnAction::Move(action));
    assert!(can_end_turn(&state));

    let new_state = end_turn(state);
    assert_eq!(new_state.current_player, "black");
    assert_eq!(new_state.turn_number, 2);
    assert!(new_state.turn_state.actions.is_empty());
}

// ─── King capture / game end ─────────────────────────────────────────────────

#[test]
fn test_king_capture_ends_game() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "k2", "black", "king", 4, 1);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: Some("k2".into()),
    };

    let result_state = apply_move_action(state, action);
    assert_eq!(result_state.phase, GamePhase::Ended);
    assert_eq!(
        result_state.result.as_ref().unwrap().winner,
        Some("white".to_string())
    );
    assert_eq!(
        result_state.result.as_ref().unwrap().reason,
        GameEndReason::KingCapture
    );
}

#[test]
fn test_normal_capture_does_not_end_game() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "p1", "black", "pawn-black", 4, 1);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: Some("p1".into()),
    };

    let result_state = apply_move_action(state, action);
    assert_eq!(result_state.phase, GamePhase::Playing);
    assert!(result_state.result.is_none());
}

#[test]
fn test_has_living_king() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    assert!(has_living_king(&state, &"white".to_string()));
    assert!(!has_living_king(&state, &"black".to_string()));
}

// ─── Move stack ───────────────────────────────────────────────────────────────

#[test]
fn test_move_stack_consumed() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: None,
    };

    let new_state = apply_move_action(state, action);
    let piece = new_state.pieces.get("k1").unwrap();
    assert_eq!(piece.move_stack, 0, "Move stack should be consumed");
}

#[test]
fn test_move_stack_reset_on_end_turn() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "k2", "black", "king", 4, 7);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: None,
    };

    let mut new_state = apply_move_action(state, action);
    // Move state board to reflect move
    new_state.board.squares.insert(Square::new(4, 0).to_id(), None);
    new_state.board.squares.insert(Square::new(4, 1).to_id(), Some("k1".into()));

    let final_state = end_turn(new_state);
    // After end_turn, black's pieces get stacks; white's "k1" shouldn't have been reset
    // (it belongs to white but now it's black's turn — white pieces don't get stacks yet)
    // Black's king should have received a stack
    let bk = final_state.pieces.get("k2").unwrap();
    assert_eq!(bk.move_stack, 1, "Black king should have move stack 1");
}
