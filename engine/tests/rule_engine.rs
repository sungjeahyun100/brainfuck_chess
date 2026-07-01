//! Brainfuck Chess rule engine integration tests.

use std::collections::HashMap;
use std::sync::Arc;

use brainfuck_chess_engine::attack_map::generate_attack_map;
use brainfuck_chess_engine::endgame::{apply_move_action, has_living_king};
use brainfuck_chess_engine::legal_moves::{
    generate_drop_candidates_by_type, generate_legal_drop_actions, generate_legal_move_actions,
    generate_piece_legal_drop_actions, generate_piece_legal_move_actions,
};
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
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);

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
        Player {
            id: "white".into(),
            deck: white_deck,
            captured_pieces: Vec::new(),
        },
    );
    players.insert(
        "black".into(),
        Player {
            id: "black".into(),
            deck: black_deck,
            captured_pieces: Vec::new(),
        },
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
        en_passant_target: None,
        en_passant_available_to: None,
        turn_state: TurnState::new(),
        result: None,
        chessembly_program_cache,
    }
}

fn add_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, file: i32, rank: i32) {
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
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .starting_pieces
        .push(id.into());
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
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .pocket_pieces
        .push(id.into());
}

// ─── Board creation ───────────────────────────────────────────────────────────

#[test]
fn test_create_board_8x8() {
    let board = create_board(8);
    assert_eq!(board.size, 8);
    assert_eq!(board.squares.len(), 64);
    for v in board.squares.values() {
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
        promotion: None,
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
        promotion: None,
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
        promotion: None,
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
        promotion: None,
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
        promotion: None,
    };

    let mut new_state = apply_move_action(state, action);
    // Move state board to reflect move
    new_state
        .board
        .squares
        .insert(Square::new(4, 0).to_id(), None);
    new_state
        .board
        .squares
        .insert(Square::new(4, 1).to_id(), Some("k1".into()));

    let final_state = end_turn(new_state);
    // After end_turn, black's pieces get stacks; white's "k1" shouldn't have been reset
    // (it belongs to white but now it's black's turn — white pieces don't get stacks yet)
    // Black's king should have received a stack
    let bk = final_state.pieces.get("k2").unwrap();
    assert_eq!(bk.move_stack, 1, "Black king should have move stack 1");
}

#[test]
fn test_castling_kingside_generated_and_applied() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 7, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let legal = generate_legal_move_actions(&state);
    let castle = legal
        .iter()
        .find(|m| m.piece_id == "wk" && m.to == Square::new(6, 0));
    assert!(
        castle.is_some(),
        "Kingside castling move should be generated"
    );
    let piece_castle = generate_piece_legal_move_actions(&state, &"wk".into())
        .into_iter()
        .find(|m| m.to == Square::new(6, 0));
    assert!(
        piece_castle.is_some(),
        "Kingside castling move should be generated for the selected king"
    );

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "wk".into(),
        from: Square::new(4, 0),
        to: Square::new(6, 0),
        captured_piece_id: None,
        promotion: None,
    };
    let new_state = apply_move_action(state, action);

    let king = new_state.pieces.get("wk").unwrap();
    let rook = new_state.pieces.get("wr").unwrap();
    assert_eq!(king.current_square, Some(Square::new(6, 0)));
    assert_eq!(rook.current_square, Some(Square::new(5, 0)));
}

#[test]
fn test_en_passant_generated_and_applied() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 4);
    add_piece(&mut state, "bp", "black", "pawn-black", 5, 6);

    // Black double-step pawn: (5,6) -> (5,4), enabling white en passant at (5,5)
    state.current_player = "black".into();
    let black_double = MoveAction {
        player_id: "black".into(),
        piece_id: "bp".into(),
        from: Square::new(5, 6),
        to: Square::new(5, 4),
        captured_piece_id: None,
        promotion: None,
    };
    let mut state = apply_move_action(state, black_double);
    state.current_player = "white".into();

    let legal = generate_legal_move_actions(&state);
    let ep = legal
        .iter()
        .find(|m| m.piece_id == "wp" && m.to == Square::new(5, 5));
    assert!(ep.is_some(), "En passant move should be generated");
    let piece_ep = generate_piece_legal_move_actions(&state, &"wp".into())
        .into_iter()
        .find(|m| m.to == Square::new(5, 5));
    assert!(
        piece_ep.is_some(),
        "En passant move should be generated for the selected pawn"
    );

    let white_ep = MoveAction {
        player_id: "white".into(),
        piece_id: "wp".into(),
        from: Square::new(4, 4),
        to: Square::new(5, 5),
        captured_piece_id: Some("bp".into()),
        promotion: None,
    };
    let new_state = apply_move_action(state, white_ep);

    let white_pawn = new_state.pieces.get("wp").unwrap();
    let black_pawn = new_state.pieces.get("bp").unwrap();

    assert_eq!(white_pawn.current_square, Some(Square::new(5, 5)));
    assert!(
        black_pawn.captured,
        "Black pawn should be captured by en passant"
    );
    assert_eq!(new_state.board.get_piece_at(&Square::new(5, 4)), None);
    assert_eq!(new_state.en_passant_target, None);
}

#[test]
fn test_en_passant_survives_remaining_moves_before_opponent_response() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "bp", "black", "pawn-black", 5, 6);
    add_piece(&mut state, "br", "black", "rook", 0, 7);
    state.current_player = "black".into();

    state = apply_move_action(
        state,
        MoveAction {
            player_id: "black".into(),
            piece_id: "bp".into(),
            from: Square::new(5, 6),
            to: Square::new(5, 4),
            captured_piece_id: None,
            promotion: None,
        },
    );
    state = apply_move_action(
        state,
        MoveAction {
            player_id: "black".into(),
            piece_id: "br".into(),
            from: Square::new(0, 7),
            to: Square::new(0, 6),
            captured_piece_id: None,
            promotion: None,
        },
    );

    assert_eq!(state.en_passant_target, Some(Square::new(5, 5)));
    assert_eq!(state.en_passant_available_to.as_deref(), Some("white"));

    state.current_player = "white".into();
    state = apply_move_action(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "wk".into(),
            from: Square::new(4, 0),
            to: Square::new(4, 1),
            captured_piece_id: None,
            promotion: None,
        },
    );
    assert_eq!(state.en_passant_target, None);
    assert_eq!(state.en_passant_available_to, None);
}

// ─── Promotion ───────────────────────────────────────────────────────────────

#[test]
fn test_pawn_reaching_back_rank_generates_promotion_choices() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let promotions: Vec<&MoveAction> = moves
        .iter()
        .filter(|m| m.to == Square::new(4, 7))
        .collect();
    assert_eq!(
        promotions.len(),
        4,
        "Pawn reaching the back rank should offer 4 promotion choices"
    );
    let mut choices: Vec<String> = promotions
        .iter()
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(choices, vec!["bishop", "knight", "queen", "rook"]);
}

#[test]
fn test_pawn_promotion_applies_chosen_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "wp".into(),
        from: Square::new(4, 6),
        to: Square::new(4, 7),
        captured_piece_id: None,
        promotion: Some("queen".into()),
    };
    let new_state = apply_move_action(state, action);
    let promoted = new_state.pieces.get("wp").unwrap();
    assert_eq!(promoted.type_id, "queen");
    assert_eq!(promoted.current_square, Some(Square::new(4, 7)));
}

#[test]
fn test_non_promoting_pawn_move_has_single_action_without_promotion() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 3);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let single_step: Vec<&MoveAction> = moves
        .iter()
        .filter(|m| m.to == Square::new(4, 4))
        .collect();
    assert_eq!(single_step.len(), 1);
    assert_eq!(single_step[0].promotion, None);
}

#[test]
fn test_chessembly_cache_preserves_legal_moves_and_attack_map() {
    let mut cached_state = make_game_state(8);
    add_piece(&mut cached_state, "wk", "white", "king", 4, 0);
    add_piece(&mut cached_state, "wr", "white", "rook", 0, 0);
    add_piece(&mut cached_state, "bk", "black", "king", 4, 7);
    add_piece(&mut cached_state, "bp", "black", "pawn-black", 0, 5);

    let rebuilt_state = cached_state.clone();
    rebuilt_state.rebuild_chessembly_cache();

    let mut cached_moves = generate_legal_move_actions(&cached_state);
    let mut rebuilt_moves = generate_legal_move_actions(&rebuilt_state);
    cached_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    rebuilt_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    assert_eq!(
        cached_moves.len(),
        rebuilt_moves.len(),
        "legal move count should not depend on cache rebuild"
    );
    assert_eq!(
        cached_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>(),
        rebuilt_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>()
    );

    let empty_maps = HashMap::new();
    let cached_attack_map = generate_attack_map(&cached_state, &"white".into(), &empty_maps);
    let rebuilt_attack_map = generate_attack_map(&rebuilt_state, &"white".into(), &empty_maps);
    assert_eq!(
        cached_attack_map.attacked_squares,
        rebuilt_attack_map.attacked_squares
    );
    assert_eq!(cached_attack_map.source_map, rebuilt_attack_map.source_map);
}

#[test]
fn test_chessembly_cache_clone_and_deserialize_rebuild() {
    let state = make_game_state(8);
    assert_eq!(
        state.cached_chessembly_program_count(),
        state.piece_definitions.len()
    );

    let cloned = state.clone();
    let state_rook = state.chessembly_program(&"rook".to_string()).unwrap();
    let cloned_rook = cloned.chessembly_program(&"rook".to_string()).unwrap();
    assert!(Arc::ptr_eq(&state_rook, &cloned_rook));

    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("chessembly_program_cache"));
    assert!(json.contains("chessembly_code"));

    let deserialized: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.cached_chessembly_program_count(), 0);
    deserialized.ensure_chessembly_cache();
    assert_eq!(
        deserialized.cached_chessembly_program_count(),
        deserialized.piece_definitions.len()
    );
}

#[test]
fn test_piece_legal_move_actions_match_filtered_full_generator() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "wp", "white", "pawn-white", 3, 1);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "bp", "black", "pawn-black", 0, 5);

    let mut full_rook_moves = generate_legal_move_actions(&state)
        .into_iter()
        .filter(|m| m.piece_id == "wr")
        .collect::<Vec<_>>();
    let mut piece_rook_moves = generate_piece_legal_move_actions(&state, &"wr".into());

    full_rook_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    piece_rook_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));

    assert_eq!(
        piece_rook_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>(),
        full_rook_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_piece_legal_move_actions_exclude_moved_piece() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let action = generate_piece_legal_move_actions(&state, &"wr".into())
        .into_iter()
        .find(|m| m.to == Square::new(0, 1))
        .unwrap();

    let moved_state = apply_move_action(state, action);
    assert!(
        generate_piece_legal_move_actions(&moved_state, &"wr".into()).is_empty(),
        "a piece that already moved this turn must not have more legal moves"
    );
}

#[test]
fn test_piece_legal_drop_actions_match_filtered_full_drop_generator() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_pocket_piece(&mut state, "wn", "white", "knight");
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    state.turn_state.mode = TurnMode::Drop;

    let direct_moves = generate_legal_move_actions(&state);
    let mut full_piece_drops = generate_legal_drop_actions(&state)
        .into_iter()
        .filter(|d| d.piece_id == "wn")
        .collect::<Vec<_>>();
    let mut piece_drops = generate_piece_legal_drop_actions(&state, &"wn".into());

    full_piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));
    piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));

    assert!(direct_moves.is_empty());
    assert_eq!(piece_drops, full_piece_drops);
}

#[test]
fn test_legal_action_cache_fields_are_not_serialized() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("legal_action_cache"));
    assert!(!json.contains("legal_action_cache_version"));
}

#[test]
fn test_square_id_is_copy_and_preserves_board_json_keys() {
    let id = Square::new(3, 5).to_id();
    let copied = id;
    assert_eq!(id, copied);
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"3_5\"");

    let board = create_board(8);
    let json = serde_json::to_string(&board).unwrap();
    assert!(json.contains("\"3_5\":null"));
    let decoded: Board = serde_json::from_str(&json).unwrap();
    assert!(decoded.squares.contains_key(&id));
}

#[test]
fn test_drop_candidates_are_grouped_by_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_pocket_piece(&mut state, "wn1", "white", "knight");
    add_pocket_piece(&mut state, "wn2", "white", "knight");
    add_pocket_piece(&mut state, "wr1", "white", "rook");
    state.turn_state.mode = TurnMode::Drop;

    let candidates = generate_drop_candidates_by_type(&state, &"white".to_string());
    let knight_candidates = candidates
        .iter()
        .filter(|candidate| candidate.piece_type_id == "knight")
        .collect::<Vec<_>>();
    let rook_candidates = candidates
        .iter()
        .filter(|candidate| candidate.piece_type_id == "rook")
        .collect::<Vec<_>>();

    assert!(!knight_candidates.is_empty());
    assert_eq!(knight_candidates.len(), rook_candidates.len());
    assert!(knight_candidates
        .iter()
        .all(|candidate| candidate.count == 2));
    assert!(rook_candidates.iter().all(|candidate| candidate.count == 1));
}
