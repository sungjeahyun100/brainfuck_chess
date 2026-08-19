use std::collections::HashMap;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::chessembly::run_chessembly_layer_for_piece_checked;
use brainfuck_chess_engine::legal_moves::generate_piece_legal_drop_actions;
use brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions;
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::{
    custom_runtime_type_id, deck_selectable_custom_type_ids, install_runtime_catalog,
    restore_game_snapshot, serialize_game_snapshot, validate_and_build_custom_piece_package, Board,
    ChessemblyProgramCache, CustomPieceError, CustomPiecePackageInput, Deck, GamePhase, GameState,
    Piece, PieceDefinition, PieceId, Player, Square, TurnAction, CUSTOM_PIECE_SCRIPT_FORMAT,
};
use serde_json::json;

fn definition(id: &str, code: &str) -> PieceDefinition {
    let mut definition = all_default_definitions()
        .into_iter()
        .find(|definition| definition.id == "knight")
        .unwrap();
    definition.id = id.into();
    definition.name = id.into();
    definition.is_king = false;
    definition.chessembly_code = code.into();
    definition.move_layers.clear();
    definition.move_options.clear();
    definition
}

fn package(
    package_id: &str,
    exposed: &str,
    definitions: Vec<PieceDefinition>,
) -> brainfuck_chess_engine::CustomPiecePackage {
    let raw_script = serde_json::to_string(&json!({
        "format": CUSTOM_PIECE_SCRIPT_FORMAT,
        "definitions": definitions,
    }))
    .unwrap();
    validate_and_build_custom_piece_package(CustomPiecePackageInput {
        package_id: package_id.into(),
        version: 1,
        expected_content_hash: None,
        raw_script,
        exposed_piece_key: exposed.into(),
        score: 7,
    })
    .unwrap()
}

fn state_with_piece(type_id: &str) -> GameState {
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    let piece_id = PieceId::from("custom-1");
    let mut board = Board {
        size: 8,
        squares: HashMap::new(),
        terrain: HashMap::new(),
    };
    board
        .squares
        .insert(Square::new(3, 3).to_id(), Some(piece_id.clone()));
    let piece = Piece {
        id: piece_id.clone(),
        owner: "white".into(),
        type_id: type_id.into(),
        current_square: Some(Square::new(3, 3)),
        in_pocket: false,
        captured: false,
        has_moved: false,
        state: HashMap::new(),
        move_option_cooldowns: HashMap::new(),
    };
    let deck = Deck {
        player_id: "white".into(),
        starting_pieces: vec![piece_id.clone()],
        pocket_pieces: Vec::new(),
        score_limit: 100,
        total_score: 7,
    };
    let mut players = HashMap::new();
    players.insert(
        "white".into(),
        Player {
            id: "white".into(),
            deck,
            captured_pieces: Vec::new(),
        },
    );
    players.insert(
        "black".into(),
        Player {
            id: "black".into(),
            deck: Deck {
                player_id: "black".into(),
                starting_pieces: Vec::new(),
                pocket_pieces: Vec::new(),
                score_limit: 100,
                total_score: 0,
            },
            captured_pieces: Vec::new(),
        },
    );
    GameState {
        id: "custom-runtime".into(),
        board,
        pieces: HashMap::from([(piece_id, piece)]),
        chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
        piece_definitions: definitions,
        custom_piece_manifest: Vec::new(),
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: HashMap::new(),
        history: Vec::new(),
        result: None,
    }
}

#[test]
fn builds_multi_definition_package_and_preserves_source() {
    let package = package(
        "turner",
        "north",
        vec![
            definition("north", "transition(east) move(0,1);"),
            definition("east", "move(1,0);"),
        ],
    );
    assert!(package.raw_script.contains("transition(east)"));
    assert_eq!(package.definitions.len(), 2);
    assert_eq!(package.internal_type_ids.len(), 1);
    assert_eq!(package.score, 7);
    assert_eq!(
        package.exposed_type_id,
        custom_runtime_type_id("turner", 1, "north")
    );
}

#[test]
fn rejects_missing_exposed_reference_and_invalid_program() {
    let raw_script = serde_json::to_string(&json!({
        "format": CUSTOM_PIECE_SCRIPT_FORMAT,
        "definitions": [definition("only", "unknown-command;")],
    }))
    .unwrap();
    let input = |exposed: &str| CustomPiecePackageInput {
        package_id: "bad".into(),
        version: 1,
        expected_content_hash: None,
        raw_script: raw_script.clone(),
        exposed_piece_key: exposed.into(),
        score: 1,
    };
    assert!(matches!(
        validate_and_build_custom_piece_package(input("missing")),
        Err(CustomPieceError::MissingExposedPiece(_))
    ));
    assert!(matches!(
        validate_and_build_custom_piece_package(input("only")),
        Err(CustomPieceError::ChessemblySyntax {
            line: 1,
            column: 1,
            ..
        })
    ));
}

#[test]
fn preview_is_immutable_commit_transitions_and_snapshot_restores_catalog() {
    let package = package(
        "turner",
        "north",
        vec![
            definition("north", "transition(east) move(0,1);"),
            definition("east", "move(1,0);"),
        ],
    );
    let mut state = state_with_piece(&package.exposed_type_id);
    install_runtime_catalog(&mut state, std::slice::from_ref(&package)).unwrap();
    let before = serde_json::to_value(&state).unwrap();
    let actions = generate_piece_legal_move_actions(&state, &PieceId::from("custom-1"));
    assert_eq!(before, serde_json::to_value(&state).unwrap());
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]
            .effects
            .piece_type_transition
            .as_ref()
            .unwrap()
            .target_type_id,
        custom_runtime_type_id("turner", 1, "east")
    );

    let applied = submit_action(state, TurnAction::Move(actions[0].clone())).unwrap();
    assert_eq!(
        applied.pieces[&PieceId::from("custom-1")].type_id,
        custom_runtime_type_id("turner", 1, "east")
    );
    let snapshot = serialize_game_snapshot(&applied).unwrap();
    let restored = restore_game_snapshot(&snapshot).unwrap();
    assert_eq!(
        deck_selectable_custom_type_ids(&restored),
        vec![package.exposed_type_id.as_str()]
    );
    assert_eq!(
        generate_piece_legal_move_actions(&applied, &PieceId::from("custom-1")),
        generate_piece_legal_move_actions(&restored, &PieceId::from("custom-1"))
    );
}

#[test]
fn package_namespaces_prevent_internal_key_collisions_and_snapshot_fails_closed() {
    let first = package("one", "shared", vec![definition("shared", "move(1,0);")]);
    let second = package("two", "shared", vec![definition("shared", "move(0,1);")]);
    let mut state = state_with_piece(&first.exposed_type_id);
    install_runtime_catalog(&mut state, &[first, second]).unwrap();
    assert!(state
        .piece_definitions
        .contains_key(&custom_runtime_type_id("one", 1, "shared")));
    assert!(state
        .piece_definitions
        .contains_key(&custom_runtime_type_id("two", 1, "shared")));

    state
        .piece_definitions
        .remove(&custom_runtime_type_id("one", 1, "shared"));
    let snapshot = serde_json::to_string(&state).unwrap();
    assert!(matches!(
        restore_game_snapshot(&snapshot),
        Err(CustomPieceError::CorruptSnapshot(_))
    ));
}

#[test]
fn custom_piece_capture_and_pocket_drop_use_the_installed_catalog() {
    let package = package(
        "soldier",
        "soldier",
        vec![definition("soldier", "take-move(1,0);")],
    );
    let mut state = state_with_piece(&package.exposed_type_id);
    install_runtime_catalog(&mut state, std::slice::from_ref(&package)).unwrap();

    let enemy_id = PieceId::from("enemy-1");
    state
        .board
        .squares
        .insert(Square::new(4, 3).to_id(), Some(enemy_id.clone()));
    state.pieces.insert(
        enemy_id.clone(),
        Piece {
            id: enemy_id.clone(),
            owner: "black".into(),
            type_id: "knight".into(),
            current_square: Some(Square::new(4, 3)),
            in_pocket: false,
            captured: false,
            has_moved: false,
            state: HashMap::new(),
            move_option_cooldowns: HashMap::new(),
        },
    );
    let capture = generate_piece_legal_move_actions(&state, &PieceId::from("custom-1"))
        .into_iter()
        .find(|action| action.captured_piece_id.as_ref() == Some(&enemy_id))
        .unwrap();
    let applied = submit_action(state, TurnAction::Move(capture)).unwrap();
    assert!(applied.pieces[&enemy_id].captured);
    assert!(applied
        .piece_definitions
        .contains_key(&package.exposed_type_id));

    let mut pocket_state = state_with_piece(&package.exposed_type_id);
    install_runtime_catalog(&mut pocket_state, &[package]).unwrap();
    let custom_id = PieceId::from("custom-1");
    let custom = pocket_state.pieces.get_mut(&custom_id).unwrap();
    pocket_state
        .board
        .squares
        .insert(custom.current_square.unwrap().to_id(), None);
    custom.current_square = None;
    custom.in_pocket = true;
    pocket_state
        .players
        .get_mut("white")
        .unwrap()
        .deck
        .starting_pieces
        .clear();
    pocket_state
        .players
        .get_mut("white")
        .unwrap()
        .deck
        .pocket_pieces
        .push(custom_id.clone());
    assert!(!generate_piece_legal_drop_actions(&pocket_state, &custom_id).is_empty());
}

#[test]
fn snapshot_rejects_definition_tampering() {
    let package = package("sealed", "piece", vec![definition("piece", "move(1,0);")]);
    let mut state = state_with_piece(&package.exposed_type_id);
    install_runtime_catalog(&mut state, std::slice::from_ref(&package)).unwrap();
    state
        .piece_definitions
        .get_mut(&package.exposed_type_id)
        .unwrap()
        .score += 1;
    let snapshot = serde_json::to_string(&state).unwrap();
    assert!(matches!(
        restore_game_snapshot(&snapshot),
        Err(CustomPieceError::DefinitionVersionMismatch { .. })
    ));
}

#[test]
fn checked_custom_execution_returns_a_structured_limit_error() {
    let package = package("limited", "piece", vec![definition("piece", "move(1,0);")]);
    let mut state = state_with_piece(&package.exposed_type_id);
    install_runtime_catalog(&mut state, std::slice::from_ref(&package)).unwrap();
    let piece = &state.pieces[&PieceId::from("custom-1")];
    let definition = &state.piece_definitions[&package.exposed_type_id];
    let result = run_chessembly_layer_for_piece_checked(
        &state,
        piece,
        definition,
        &definition.move_layers[0],
        "white".into(),
        &state.global_state,
        &HashMap::new(),
        0,
    );
    assert_eq!(
        result.unwrap_err(),
        CustomPieceError::ExecutionLimitExceeded("execution_steps")
    );
}
