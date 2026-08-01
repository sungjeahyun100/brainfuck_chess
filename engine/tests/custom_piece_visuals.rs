use std::collections::HashMap;

use brainfuck_chess_engine::custom_pieces::{
    install_runtime_catalog, serialize_game_snapshot, validate_and_build_custom_piece_package,
    CustomPiecePackageInput, CUSTOM_PIECE_SCRIPT_FORMAT,
};
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;
use serde_json::json;

fn empty_state() -> GameState {
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&definitions);
    GameState {
        id: "custom-visual-test".into(),
        board: create_board(8),
        pieces: HashMap::new(),
        piece_definitions: definitions,
        custom_piece_manifest: Vec::new(),
        players: HashMap::new(),
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

#[test]
fn runtime_install_restores_source_visual_variants_and_keeps_runtime_default_asset() {
    let mut definition = all_default_definitions()
        .into_iter()
        .find(|definition| definition.id == "knight")
        .unwrap();
    definition.id = "hero".into();
    definition.name = "Hero".into();
    definition.is_king = false;
    definition.can_capture_on_drop = false;
    definition.state_schema = vec![PieceStateDefinition {
        key: "mode".into(),
        default_value: PieceStateValue::Text("normal".into()),
    }];
    definition.visual.default_asset_key = "knight".into();
    definition.visual.variants = vec![PieceVisualVariantDefinition {
        id: "rook-mode".into(),
        enabled_when: vec![PieceStatePredicate {
            key: "mode".into(),
            condition: PieceStateCondition::Equals(PieceStateValue::Text("rook".into())),
        }],
        asset_key: "rook".into(),
        priority: 10,
    }];

    let raw_script = serde_json::to_string(&json!({
        "format": CUSTOM_PIECE_SCRIPT_FORMAT,
        "definitions": [definition],
    }))
    .unwrap();
    let mut package = validate_and_build_custom_piece_package(CustomPiecePackageInput {
        package_id: "visual-package".into(),
        version: 2,
        expected_content_hash: None,
        raw_script,
        exposed_piece_key: "hero".into(),
        score: 5,
    })
    .unwrap();
    let exposed_type_id = package.exposed_type_id.clone();

    // The server replaces the base image with the uploaded asset. Older code
    // also cleared variants here; runtime installation must recover them from
    // the integrity-checked raw source without losing this replacement.
    package.definitions[0].visual.default_asset_key =
        "data:image/svg+xml;base64,PHN2Zy8+".into();
    package.definitions[0].visual.variants.clear();

    let mut state = empty_state();
    install_runtime_catalog(&mut state, &[package]).unwrap();

    let installed = &state.piece_definitions[&exposed_type_id];
    assert_eq!(
        installed.visual.default_asset_key,
        "data:image/svg+xml;base64,PHN2Zy8+"
    );
    assert_eq!(installed.visual.variants.len(), 1);
    assert_eq!(installed.visual.variants[0].asset_key, "rook");
    assert!(serialize_game_snapshot(&state).is_ok());
}
