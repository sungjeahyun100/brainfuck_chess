use std::collections::HashMap;

use brainfuck_chess_engine::ai::{
    apply_ai_action, choose_bot_action, generate_ai_actions, play_bot_turn, AiAction, BotDifficulty,
};
use brainfuck_chess_engine::endgame::apply_move_action;
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::{create_board, grant_move_stacks};
use brainfuck_chess_engine::types::*;

fn make_state() -> GameState {
    let definitions: HashMap<_, _> = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect();
    let players = ["white", "black"]
        .into_iter()
        .map(|id| {
            (
                id.to_string(),
                Player {
                    id: id.to_string(),
                    deck: Deck {
                        player_id: id.to_string(),
                        starting_pieces: Vec::new(),
                        pocket_pieces: Vec::new(),
                        score_limit: 39,
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            )
        })
        .collect();

    GameState {
        id: "ai-test".into(),
        board: create_board(8),
        pieces: HashMap::new(),
        chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
        piece_definitions: definitions,
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        turn_state: TurnState::new(),
        result: None,
    }
}

fn add_board_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, square: Square) {
    let piece_id: PieceId = id.into();
    state
        .board
        .squares
        .insert(square.to_id(), Some(piece_id.clone()));
    state.pieces.insert(
        piece_id.clone(),
        Piece {
            id: piece_id.clone(),
            owner: owner.into(),
            type_id: type_id.into(),
            current_square: Some(square),
            in_pocket: false,
            captured: false,
            move_stack: 1,
            has_moved: false,
            active_ability: None,
        },
    );
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .starting_pieces
        .push(piece_id);
}

fn add_pocket_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str) {
    let piece_id: PieceId = id.into();
    state.pieces.insert(
        piece_id.clone(),
        Piece {
            id: piece_id.clone(),
            owner: owner.into(),
            type_id: type_id.into(),
            current_square: None,
            in_pocket: true,
            captured: false,
            move_stack: 0,
            has_moved: false,
            active_ability: None,
        },
    );
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .pocket_pieces
        .push(piece_id);
}

#[test]
fn empty_candidates_do_not_panic() {
    let state = make_state();
    assert!(generate_ai_actions(&state).is_empty());
    let decision = choose_bot_action(&state, &"white".into(), BotDifficulty::Easy);
    assert_eq!(decision.action, AiAction::EndTurn);
}

#[test]
fn generated_actions_are_accepted_by_the_ai_apply_boundary() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(0, 0));
    add_pocket_piece(&mut state, "wn", "white", "knight");

    for action in generate_ai_actions(&state) {
        assert!(
            apply_ai_action(state.clone(), &action).is_ok(),
            "{action:?}"
        );
    }
}

#[test]
fn bot_always_selects_an_immediate_king_capture() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));

    for difficulty in [
        BotDifficulty::Easy,
        BotDifficulty::Normal,
        BotDifficulty::Hard,
    ] {
        let decision = choose_bot_action(&state, &"white".into(), difficulty);
        assert!(matches!(
            decision.action,
            AiAction::Move(MoveAction { captured_piece_id: Some(ref id), .. }) if id == "bk"
        ));
    }
}

#[test]
fn drop_and_move_modes_cannot_be_mixed() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    add_pocket_piece(&mut state, "wn", "white", "knight");

    let drop = generate_ai_actions(&state)
        .into_iter()
        .find(|action| matches!(action, AiAction::Drop(_)))
        .unwrap();
    let dropped = apply_ai_action(state.clone(), &drop).unwrap();
    assert_eq!(generate_ai_actions(&dropped), vec![AiAction::EndTurn]);

    let movement = generate_ai_actions(&state)
        .into_iter()
        .find(|action| matches!(action, AiAction::Move(_)))
        .unwrap();
    let moved = apply_ai_action(state, &movement).unwrap();
    assert!(generate_ai_actions(&moved)
        .iter()
        .all(|action| !matches!(action, AiAction::Drop(_))));
}

#[test]
fn bot_turn_finishes_or_ends_the_game_within_the_difficulty_limit() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    add_board_piece(&mut state, "wn", "white", "knight", Square::new(1, 0));
    grant_move_stacks(&mut state);

    let next = play_bot_turn(state, &"white".into(), BotDifficulty::Easy).unwrap();
    assert!(next.phase == GamePhase::Ended || next.current_player == "black");
}

#[test]
fn special_moves_are_exposed_through_ai_actions() {
    let mut castle_state = make_state();
    add_board_piece(&mut castle_state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut castle_state, "wr", "white", "rook", Square::new(7, 0));
    add_board_piece(&mut castle_state, "bk", "black", "king", Square::new(4, 7));
    assert!(generate_ai_actions(&castle_state).iter().any(|action| {
        matches!(action, AiAction::Move(movement) if movement.piece_id == "wk" && movement.to == Square::new(6, 0))
    }));

    let mut en_passant_state = make_state();
    add_board_piece(
        &mut en_passant_state,
        "wk",
        "white",
        "king",
        Square::new(4, 0),
    );
    add_board_piece(
        &mut en_passant_state,
        "bk",
        "black",
        "king",
        Square::new(4, 7),
    );
    add_board_piece(
        &mut en_passant_state,
        "wp",
        "white",
        "pawn-white",
        Square::new(4, 4),
    );
    add_board_piece(
        &mut en_passant_state,
        "bp",
        "black",
        "pawn-black",
        Square::new(5, 6),
    );
    en_passant_state.current_player = "black".into();
    en_passant_state = apply_move_action(
        en_passant_state,
        MoveAction {
            player_id: "black".into(),
            piece_id: "bp".into(),
            from: Square::new(5, 6),
            to: Square::new(5, 4),
            captured_piece_id: None,
            promotion: None,
        },
    );
    en_passant_state.current_player = "white".into();
    en_passant_state.turn_state = TurnState::new();
    assert!(generate_ai_actions(&en_passant_state).iter().any(|action| {
        matches!(action, AiAction::Move(movement) if movement.piece_id == "wp" && movement.to == Square::new(5, 5))
    }));
}

#[test]
fn ended_games_have_no_actions_and_are_not_mutated() {
    let mut state = make_state();
    state.phase = GamePhase::Ended;
    state.result = Some(GameResult {
        winner: Some("black".into()),
        reason: GameEndReason::KingCapture,
    });
    assert!(generate_ai_actions(&state).is_empty());
    assert!(play_bot_turn(state.clone(), &"white".into(), BotDifficulty::Normal).is_err());
    assert_eq!(state.phase, GamePhase::Ended);
    assert_eq!(state.result.unwrap().winner.as_deref(), Some("black"));
}

#[test]
fn difficulty_limits_are_bounded_as_configured() {
    let easy = BotDifficulty::Easy.limits();
    let normal = BotDifficulty::Normal.limits();
    let hard = BotDifficulty::Hard.limits();
    assert_eq!((easy.max_depth_actions, easy.max_nodes), (1, 500));
    assert_eq!((normal.max_depth_actions, normal.max_nodes), (2, 3_000));
    assert_eq!((hard.max_depth_actions, hard.max_nodes), (3, 10_000));
    assert!(easy.hard_time_ms < normal.hard_time_ms);
    assert!(normal.hard_time_ms < hard.hard_time_ms);
}
