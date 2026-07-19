//! Brainfuck Chess rule engine integration tests.

use std::collections::HashMap;
use std::sync::Arc;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::attack_map::generate_attack_map;
use brainfuck_chess_engine::context::PieceCatalog;
use brainfuck_chess_engine::endgame::{apply_and_advance_turn, apply_move_action, has_living_king};
use brainfuck_chess_engine::legal_moves::{
    generate_drop_candidates_by_type, generate_legal_drop_actions, generate_legal_move_actions,
    generate_piece_legal_drop_actions, generate_piece_legal_move_actions,
    generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
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
        global_state: HashMap::new(),
        history: Vec::new(),
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
        has_moved: false,
        state: state
            .piece_definitions
            .get(type_id)
            .map(PieceDefinition::initial_state)
            .unwrap_or_default(),
        move_option_cooldowns: HashMap::new(),
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
        has_moved: false,
        state: state
            .piece_definitions
            .get(type_id)
            .map(PieceDefinition::initial_state)
            .unwrap_or_default(),
        move_option_cooldowns: HashMap::new(),
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

#[test]
fn paratrooper_drops_on_empty_square_and_cannot_move_afterward() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    let action = generate_piece_legal_drop_actions(&state, &"para".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 0))
        .unwrap();
    assert_eq!(action.captured_piece_id, None);

    let state = submit_action(state, TurnAction::Drop(action)).unwrap();
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(3, 0))
            .map(PieceId::as_str),
        Some("para")
    );
    assert!(state.players["white"].deck.pocket_pieces.is_empty());
    assert_eq!(state.current_player, "black");
    assert_eq!(state.turn_number, 2);
    assert_eq!(state.history.len(), 1);
    assert!(generate_piece_legal_move_actions(&state, &"para".into()).is_empty());
}

#[test]
fn paratrooper_captures_enemy_on_drop_and_records_capture() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    add_piece(&mut state, "enemy", "black", "knight", 3, 0);
    let action = generate_piece_legal_drop_actions(&state, &"para".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 0))
        .unwrap();
    assert_eq!(
        action.captured_piece_id.as_ref().map(PieceId::as_str),
        Some("enemy")
    );

    let state = submit_action(state, TurnAction::Drop(action)).unwrap();
    assert!(state.pieces["enemy"].captured);
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(3, 0))
            .map(PieceId::as_str),
        Some("para")
    );
    assert_eq!(state.players["white"].captured_pieces, vec!["enemy"]);
    let TurnAction::Drop(recorded) = &state.history[0].action else {
        panic!()
    };
    assert_eq!(
        recorded.captured_piece_id.as_ref().map(PieceId::as_str),
        Some("enemy")
    );
}

#[test]
fn illegal_paratrooper_drop_is_atomic_and_regular_piece_cannot_capture_on_drop() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    add_pocket_piece(&mut state, "knight", "white", "knight");
    add_piece(&mut state, "friend", "white", "bishop", 2, 0);
    add_piece(&mut state, "enemy", "black", "bishop", 3, 0);
    let before = serde_json::to_value(&state).unwrap();
    let illegal = DropAction {
        player_id: "white".into(),
        piece_id: "para".into(),
        to: Square::new(2, 0),
        captured_piece_id: Some("friend".into()),
    };
    assert!(submit_action(state.clone(), TurnAction::Drop(illegal)).is_err());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert!(!generate_piece_legal_drop_actions(&state, &"knight".into())
        .iter()
        .any(|action| action.to == Square::new(3, 0)));
    assert!(generate_piece_legal_drop_actions(&state, &"para".into())
        .iter()
        .any(|action| action.to == Square::new(3, 0)));
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

#[test]
fn test_windmill_piece_state_toggle_move_modes() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wm", "white", "windmill", 3, 3);

    let bishop_mode_moves = generate_piece_legal_move_actions(&state, &"wm".into());
    assert!(bishop_mode_moves
        .iter()
        .any(|action| action.to == Square::new(4, 4)));
    assert!(bishop_mode_moves
        .iter()
        .all(|action| (action.to.file - 3).abs() == (action.to.rank - 3).abs()));
    let bishop_move = bishop_mode_moves
        .into_iter()
        .find(|action| action.to == Square::new(4, 4))
        .unwrap();
    assert_eq!(bishop_move.source_layer_ids, vec!["bishop_mode"]);
    assert_eq!(
        bishop_move.effects.piece_state_updates[0].value,
        PieceStateValue::Text("rook".into())
    );

    state = apply_move_action(state, bishop_move);
    assert_eq!(
        state.pieces["wm"].state.get("mode"),
        Some(&PieceStateValue::Text("rook".into()))
    );
    assert!(!state.global_state.contains_key("mode"));

    let rook_mode_moves = generate_piece_legal_move_actions(&state, &"wm".into());
    assert!(rook_mode_moves
        .iter()
        .any(|action| action.to == Square::new(4, 5)));
    assert!(rook_mode_moves
        .iter()
        .all(|action| action.to.file == 4 || action.to.rank == 4));
    let rook_move = rook_mode_moves
        .into_iter()
        .find(|action| action.to == Square::new(4, 5))
        .unwrap();
    assert_eq!(rook_move.source_layer_ids, vec!["rook_mode"]);

    state = apply_move_action(state, rook_move);
    assert_eq!(
        state.pieces["wm"].state.get("mode"),
        Some(&PieceStateValue::Text("bishop".into()))
    );
}

#[test]
fn windmills_own_independent_state_and_legal_queries_are_pure() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "first", "white", "windmill", 2, 2);
    add_piece(&mut state, "second", "white", "windmill", 6, 2);
    let before = state.pieces["first"].state.clone();

    let action = generate_piece_legal_move_actions(&state, &"first".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 3))
        .unwrap();
    assert_eq!(state.pieces["first"].state, before);
    assert_eq!(
        state.pieces["second"].state["mode"],
        PieceStateValue::Text("bishop".into())
    );

    state = apply_move_action(state, action);
    assert_eq!(
        state.pieces["first"].state["mode"],
        PieceStateValue::Text("rook".into())
    );
    assert_eq!(
        state.pieces["second"].state["mode"],
        PieceStateValue::Text("bishop".into())
    );
}

#[test]
fn layers_with_same_destination_and_different_effects_stay_distinct() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "layered".into(),
        name: "Layered".into(),
        score: 1,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: vec![
            PieceStateDefinition {
                key: "a".into(),
                default_value: PieceStateValue::Integer(0),
            },
            PieceStateDefinition {
                key: "b".into(),
                default_value: PieceStateValue::Integer(0),
            },
        ],
        move_layers: vec![
            MoveLayerDefinition {
                id: "a".into(),
                chessembly_code: "move(1, 0);".into(),
                enabled_when: Vec::new(),
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "a".into(),
                    value: PieceStateValue::Integer(1),
                }],
            },
            MoveLayerDefinition {
                id: "b".into(),
                chessembly_code: "move(1, 0);".into(),
                enabled_when: Vec::new(),
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "b".into(),
                    value: PieceStateValue::Integer(1),
                }],
            },
        ],
        move_options: vec![MoveOptionDefinition {
            id: "normal".into(),
            name: "Normal".into(),
            description: String::new(),
            kind: MoveOptionKind::Normal,
            layer_ids: vec!["a".into(), "b".into()],
            execution_mode: MoveOptionExecutionMode::MoveModifier,
            contributes_to_attack_map: true,
            cooldown: None,
        }],
        visual: PieceVisualDefinition {
            default_asset_key: "layered".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .unwrap();
    state.piece_definitions.insert("layered".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "layered", "white", "layered", 3, 3);

    let actions: Vec<_> = generate_piece_legal_move_actions(&state, &"layered".into())
        .into_iter()
        .filter(|action| action.to == Square::new(4, 3))
        .collect();
    assert_eq!(actions.len(), 2);
    assert_ne!(actions[0].effects, actions[1].effects);
}

#[test]
fn chessembly_set_state_remains_global_and_separate_from_piece_state() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "global-writer".into(),
        name: "Global writer".into(),
        score: 1,
        chessembly_code: "set-state(flag, 7) move(1, 0);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition::default(),
    }
    .normalize_and_validate()
    .unwrap();
    state
        .piece_definitions
        .insert("global-writer".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "writer", "white", "global-writer", 3, 3);
    let action = generate_piece_legal_move_actions(&state, &"writer".into())
        .into_iter()
        .find(|action| action.to == Square::new(4, 3))
        .unwrap();
    assert_eq!(action.effects.global_state_updates[0].key, "flag");
    assert!(action.effects.piece_state_updates.is_empty());
    let state = apply_move_action(state, action);
    assert_eq!(state.global_state.get("flag"), Some(&7));
    assert!(state.pieces["writer"].state.is_empty());
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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
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

// ─── Single-action turns ─────────────────────────────────────────────────────

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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let state = apply_and_advance_turn(state, TurnAction::Move(black_double));

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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
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
fn test_en_passant_expires_when_opponent_chooses_another_action() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "bp", "black", "pawn-black", 5, 6);
    state.current_player = "black".into();

    state = apply_and_advance_turn(
        state,
        TurnAction::Move(MoveAction {
            player_id: "black".into(),
            piece_id: "bp".into(),
            from: Square::new(5, 6),
            to: Square::new(5, 4),
            captured_piece_id: None,
            promotion: None,
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        }),
    );

    assert_eq!(state.en_passant_target, Some(Square::new(5, 5)));
    assert_eq!(state.en_passant_available_to.as_deref(), Some("white"));

    state = apply_move_action(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "wk".into(),
            from: Square::new(4, 0),
            to: Square::new(4, 1),
            captured_piece_id: None,
            promotion: None,
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        },
    );
    assert_eq!(state.en_passant_target, None);
    assert_eq!(state.en_passant_available_to, None);
}

// ─── Promotion ───────────────────────────────────────────────────────────────

#[test]
fn test_tempest_pawn_moves_like_pawn() {
    let mut pawn_state = make_game_state(8);
    add_piece(&mut pawn_state, "wk", "white", "king", 0, 0);
    add_piece(&mut pawn_state, "bk", "black", "king", 7, 7);
    add_piece(&mut pawn_state, "wp", "white", "pawn-white", 3, 1);

    let mut tempest_state = make_game_state(8);
    add_piece(&mut tempest_state, "wk", "white", "king", 0, 0);
    add_piece(&mut tempest_state, "bk", "black", "king", 7, 7);
    add_piece(
        &mut tempest_state,
        "tp",
        "white",
        "tempest-pawn-white",
        3,
        1,
    );

    let mut pawn_moves: Vec<(i32, i32, Option<String>)> =
        generate_piece_legal_move_actions(&pawn_state, &"wp".into())
            .into_iter()
            .map(|action| (action.to.file, action.to.rank, action.promotion))
            .collect();
    let mut tempest_moves: Vec<(i32, i32, Option<String>)> =
        generate_piece_legal_move_actions(&tempest_state, &"tp".into())
            .into_iter()
            .map(|action| (action.to.file, action.to.rank, action.promotion))
            .collect();

    pawn_moves.sort();
    tempest_moves.sort();
    assert_eq!(tempest_moves, pawn_moves);
}

#[test]
fn test_tempest_pawn_attacks_like_pawn() {
    let mut pawn_state = make_game_state(8);
    add_piece(&mut pawn_state, "wk", "white", "king", 0, 0);
    add_piece(&mut pawn_state, "bk", "black", "king", 7, 7);
    add_piece(&mut pawn_state, "wp", "white", "pawn-white", 3, 3);
    add_piece(&mut pawn_state, "be1", "black", "knight", 2, 4);
    add_piece(&mut pawn_state, "be2", "black", "bishop", 4, 4);

    let mut tempest_state = make_game_state(8);
    add_piece(&mut tempest_state, "wk", "white", "king", 0, 0);
    add_piece(&mut tempest_state, "bk", "black", "king", 7, 7);
    add_piece(
        &mut tempest_state,
        "tp",
        "white",
        "tempest-pawn-white",
        3,
        3,
    );
    add_piece(&mut tempest_state, "be1", "black", "knight", 2, 4);
    add_piece(&mut tempest_state, "be2", "black", "bishop", 4, 4);

    let mut pawn_captures: Vec<(i32, i32)> =
        generate_piece_legal_move_actions(&pawn_state, &"wp".into())
            .into_iter()
            .filter(|action| action.captured_piece_id.is_some())
            .map(|action| (action.to.file, action.to.rank))
            .collect();
    let mut tempest_captures: Vec<(i32, i32)> =
        generate_piece_legal_move_actions(&tempest_state, &"tp".into())
            .into_iter()
            .filter(|action| action.captured_piece_id.is_some())
            .map(|action| (action.to.file, action.to.rank))
            .collect();

    pawn_captures.sort();
    tempest_captures.sort();
    assert_eq!(tempest_captures, pawn_captures);
}

#[test]
fn test_pawn_reaching_back_rank_generates_promotion_choices() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let promotions: Vec<&MoveAction> = moves.iter().filter(|m| m.to == Square::new(4, 7)).collect();
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
fn test_tempest_pawn_reaching_back_rank_generates_tempest_promotion_choices() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"tp".into());
    let promotions: Vec<&MoveAction> = moves.iter().filter(|m| m.to == Square::new(4, 7)).collect();
    assert_eq!(
        promotions.len(),
        4,
        "Tempest Pawn reaching the back rank should offer 4 promotion choices"
    );
    let mut choices: Vec<String> = promotions
        .iter()
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(
        choices,
        vec![
            "tempest-bishop",
            "tempest-knight",
            "tempest-queen",
            "tempest-rook"
        ]
    );
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
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, action);
    let promoted = new_state.pieces.get("wp").unwrap();
    assert_eq!(promoted.type_id, "queen");
    assert_eq!(promoted.current_square, Some(Square::new(4, 7)));
}

#[test]
fn test_tempest_pawn_promotion_applies_chosen_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 6);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "tp".into(),
        from: Square::new(4, 6),
        to: Square::new(4, 7),
        captured_piece_id: None,
        promotion: Some("tempest-queen".into()),
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, action);
    let promoted = new_state.pieces.get("tp").unwrap();
    assert_eq!(promoted.type_id, "tempest-queen");
    assert_eq!(promoted.current_square, Some(Square::new(4, 7)));
}

#[test]
fn test_tempest_pawn_type_survives_save_load() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 1);

    let json = serde_json::to_string(&state).unwrap();
    let restored: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.pieces.get("tp").unwrap().type_id,
        "tempest-pawn-white"
    );
}

#[test]
fn test_non_promoting_pawn_move_has_single_action_without_promotion() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 3);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let single_step: Vec<&MoveAction> =
        moves.iter().filter(|m| m.to == Square::new(4, 4)).collect();
    assert_eq!(single_step.len(), 1);
    assert_eq!(single_step[0].promotion, None);
}

#[test]
fn test_game_catalog_overrides_a_builtin_definition() {
    let mut state = make_game_state(8);
    let mut bishop = bishop_definition();
    bishop.name = "Game Bishop".into();
    state.piece_definitions.insert("bishop".into(), bishop);

    let catalog = PieceCatalog::for_state(&state);
    assert_eq!(catalog.get("bishop").unwrap().name, "Game Bishop");
}

#[test]
fn test_custom_piece_definition_can_generate_promotion_choices() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "promoter".into(),
        name: "Promoter".into(),
        score: 2,
        chessembly_code: "move(0, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec!["queen".into(), "knight".into()],
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition::default(),
    }
    .normalize_and_validate()
    .unwrap();
    state
        .piece_definitions
        .insert("promoter".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "pr", "white", "promoter", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"pr".into());
    let mut choices: Vec<String> = moves
        .iter()
        .filter(|m| m.to == Square::new(4, 7))
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(choices, vec!["knight", "queen"]);

    let promoted_state = apply_move_action(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "pr".into(),
            from: Square::new(4, 6),
            to: Square::new(4, 7),
            captured_piece_id: None,
            promotion: Some("knight".into()),
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        },
    );
    assert_eq!(promoted_state.pieces.get("pr").unwrap().type_id, "knight");
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
    let expected_program_count: usize = state
        .piece_definitions
        .values()
        .map(|definition| definition.move_layers.len())
        .sum();
    assert_eq!(
        state.cached_chessembly_program_count(),
        expected_program_count
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
        expected_program_count
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

    let moved_state = submit_action(state, TurnAction::Move(action)).unwrap();
    assert_eq!(moved_state.current_player, "black");
    assert_eq!(moved_state.history.len(), 1);
}

#[test]
fn test_piece_legal_drop_actions_match_filtered_full_drop_generator() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_pocket_piece(&mut state, "wn", "white", "knight");
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let mut full_piece_drops = generate_legal_drop_actions(&state)
        .into_iter()
        .filter(|d| d.piece_id == "wn")
        .collect::<Vec<_>>();
    let mut piece_drops = generate_piece_legal_drop_actions(&state, &"wn".into());

    full_piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));
    piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));

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

#[test]
fn test_cannon_rook_ability_uses_cannon_move_for_selected_move_only() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "cr", "white", "cannon-rook", 3, 3);
    add_piece(&mut state, "screen", "white", "pawn-white", 3, 4);
    add_piece(&mut state, "enemy", "black", "pawn-black", 3, 6);
    add_piece(&mut state, "screen2", "black", "pawn-black", 5, 3);
    add_piece(&mut state, "blocked", "white", "pawn-white", 6, 3);

    let base_moves = generate_piece_legal_move_actions(&state, &"cr".into());
    assert!(base_moves
        .iter()
        .any(|action| action.to == Square::new(4, 3)));
    assert!(!base_moves
        .iter()
        .any(|action| action.to == Square::new(3, 5)));

    let cannon_moves = generate_piece_legal_move_actions_with_options(
        &state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    );

    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(3, 5) && action.move_option_id == "cannon_move"
    }));
    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(3, 6)
            && action.captured_piece_id.as_ref().map(|id| id.as_str()) == Some("enemy")
            && action.move_option_id == "cannon_move"
    }));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(4, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(5, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(6, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(7, 3)));

    let action = cannon_moves
        .iter()
        .find(|action| action.to == Square::new(3, 5))
        .cloned()
        .unwrap();
    let mut used_state = apply_move_action(state, action);
    let cooldown = used_state
        .pieces
        .get("cr")
        .unwrap()
        .move_option_cooldowns
        .get("cannon_move")
        .map(|cooldown| cooldown.remaining);
    assert_eq!(cooldown, Some(3));

    used_state.current_player = "white".into();
    assert!(generate_piece_legal_move_actions_with_options(
        &used_state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .is_empty());

    used_state
        .pieces
        .get_mut("cr")
        .unwrap()
        .move_option_cooldowns
        .remove("cannon_move");
    assert!(!generate_piece_legal_move_actions_with_options(
        &used_state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .is_empty());
}

#[test]
fn owner_turn_cooldown_is_set_on_commit_and_ticks_only_after_later_owner_actions() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "cr", "white", "cannon-rook", 3, 3);
    add_piece(&mut state, "screen", "white", "pawn-white", 3, 4);

    let cannon_action = generate_piece_legal_move_actions_with_options(
        &state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .into_iter()
    .find(|action| action.to == Square::new(3, 5))
    .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(cannon_action));
    assert_eq!(state.current_player, "black");
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        3
    );

    let black_move = generate_piece_legal_move_actions(&state, &"bk".into())
        .into_iter()
        .next()
        .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(black_move));
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        3
    );

    let white_move = generate_piece_legal_move_actions(&state, &"wk".into())
        .into_iter()
        .next()
        .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(white_move));
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        2
    );
    assert_eq!(state.history.len(), 3);
}

#[test]
fn promotion_resets_piece_state_cooldowns_and_visual_definition() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);
    {
        let pawn = state.pieces.get_mut("wp").unwrap();
        pawn.state
            .insert("legacy".into(), PieceStateValue::Boolean(true));
        pawn.move_option_cooldowns
            .insert("legacy".into(), CooldownState { remaining: 9 });
    }
    let action = generate_piece_legal_move_actions(&state, &"wp".into())
        .into_iter()
        .find(|action| {
            action.to == Square::new(4, 7) && action.promotion.as_deref() == Some("knight")
        })
        .unwrap();
    let state = apply_move_action(state, action);
    let promoted = &state.pieces["wp"];
    assert_eq!(promoted.type_id, "knight");
    assert!(promoted.state.is_empty());
    assert!(promoted.move_option_cooldowns.is_empty());
    assert_eq!(
        state.piece_definitions["knight"].resolve_asset_key(promoted),
        "knight"
    );
}

#[test]
fn piece_definition_validation_rejects_unknown_references() {
    let mut definition = rook_definition();
    definition.move_options[0].layer_ids = vec!["missing".into()];
    assert!(definition.validate().unwrap_err().contains("unknown layer"));

    let mut definition = windmill_definition();
    definition.visual.variants[0].enabled_when[0].key = "typo".into();
    assert!(definition
        .validate()
        .unwrap_err()
        .contains("unknown state key"));
}
