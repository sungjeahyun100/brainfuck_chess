use std::collections::HashMap;

use brainfuck_chess_engine::actions::{apply_turn_action_with_effects, ActionEffect};
use brainfuck_chess_engine::endgame::{
    apply_drop_action_with_effects, apply_move_action_with_effects,
};
use brainfuck_chess_engine::legal_moves::generate_legal_move_actions;
use brainfuck_chess_engine::rules::{calculate_score_limit, create_board};
use brainfuck_chess_engine::types::*;

fn make_game_state() -> GameState {
    let board_size = 8;
    let mut players = HashMap::new();
    for player_id in ["white", "black"] {
        players.insert(
            player_id.to_string(),
            Player {
                id: player_id.to_string(),
                deck: Deck {
                    player_id: player_id.to_string(),
                    starting_pieces: Vec::new(),
                    pocket_pieces: Vec::new(),
                    score_limit: calculate_score_limit(board_size),
                    total_score: 0,
                },
                captured_pieces: Vec::new(),
            },
        );
    }

    GameState {
        id: "action-effects".into(),
        board: create_board(board_size),
        pieces: HashMap::new(),
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
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
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
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
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
fn submitted_move_includes_board_and_turn_effects() {
    let mut state = make_game_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(0, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));

    let action = generate_legal_move_actions(&state)
        .into_iter()
        .find(|action| action.piece_id == "wr" && action.to == Square::new(0, 1))
        .unwrap();
    let applied = apply_turn_action_with_effects(state, TurnAction::Move(action)).unwrap();

    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::MovePiece { piece_id, from, to }
            if piece_id == "wr"
                && *from == Square::new(0, 0)
                && *to == Square::new(0, 1)
    )));
    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::AdvanceTurn { from_player, to_player, turn_number }
            if from_player == "white" && to_player == "black" && *turn_number == 2
    )));
    assert_eq!(applied.state.current_player, "black");
    assert!(applied.state.turn_state.actions.is_empty());
}

#[test]
fn ability_effect_does_not_advance_the_turn() {
    let mut state = make_game_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "wb", "white", "bishop", Square::new(2, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));

    let applied = apply_turn_action_with_effects(
        state,
        TurnAction::ActivateAbility(ActivateAbilityAction {
            player_id: "white".into(),
            piece_id: "wb".into(),
            ability_id: "bounce_mode".into(),
        }),
    )
    .unwrap();

    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::SetPieceAbility { piece_id, ability_id }
            if piece_id == "wb" && ability_id == "bounce_mode"
    )));
    assert!(!applied
        .effects
        .iter()
        .any(|effect| matches!(effect, ActionEffect::AdvanceTurn { .. })));
    assert_eq!(applied.state.current_player, "white");
    assert_eq!(applied.state.turn_number, 1);
}

#[test]
fn castling_emits_a_move_effect_for_the_king_and_rook() {
    let mut state = make_game_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(7, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));

    let applied = apply_move_action_with_effects(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "wk".into(),
            from: Square::new(4, 0),
            to: Square::new(6, 0),
            captured_piece_id: None,
            promotion: None,
            ability_id: None,
        },
    );

    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::MovePiece { piece_id, from, to }
            if piece_id == "wk"
                && *from == Square::new(4, 0)
                && *to == Square::new(6, 0)
    )));
    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::MovePiece { piece_id, from, to }
            if piece_id == "wr"
                && *from == Square::new(7, 0)
                && *to == Square::new(5, 0)
    )));
}

#[test]
fn drop_effect_identifies_the_concrete_pocket_piece() {
    let mut state = make_game_state();
    add_pocket_piece(&mut state, "wn", "white", "knight");

    let applied = apply_drop_action_with_effects(
        state,
        DropAction {
            player_id: "white".into(),
            piece_id: "wn".into(),
            to: Square::new(2, 2),
        },
    );

    assert!(matches!(
        applied.effects.as_slice(),
        [ActionEffect::DropPiece { piece_id, to }]
            if piece_id == "wn" && *to == Square::new(2, 2)
    ));
}

#[test]
fn king_capture_emits_capture_and_end_game_without_advancing_turn() {
    let mut state = make_game_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(4, 6));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));

    let action = generate_legal_move_actions(&state)
        .into_iter()
        .find(|action| action.piece_id == "wr" && action.to == Square::new(4, 7))
        .unwrap();
    let applied = apply_turn_action_with_effects(state, TurnAction::Move(action)).unwrap();

    assert!(applied.effects.iter().any(|effect| matches!(
        effect,
        ActionEffect::CapturePiece { piece_id, at }
            if piece_id == "bk" && *at == Square::new(4, 7)
    )));
    assert!(applied
        .effects
        .iter()
        .any(|effect| matches!(effect, ActionEffect::EndGame { .. })));
    assert!(!applied
        .effects
        .iter()
        .any(|effect| matches!(effect, ActionEffect::AdvanceTurn { .. })));
    assert_eq!(applied.state.phase, GamePhase::Ended);
}
