use std::collections::HashMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use brainfuck_chess_engine::legal_moves::{
    generate_piece_attack_squares, generate_piece_legal_move_actions,
};
use brainfuck_chess_engine::pieces::default_pieces::{
    all_default_definitions, queen_definition, rook_definition,
};
use brainfuck_chess_engine::profiling::{self, ProfilingSnapshot};
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;

const ITERATIONS: u32 = 500;

fn empty_state(name: &str, board_size: i32) -> GameState {
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
                        score_limit: 100,
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            )
        })
        .collect();
    GameState {
        id: format!("piece-options-benchmark-{name}"),
        board: create_board(board_size),
        pieces: HashMap::new(),
        piece_definitions: definitions.clone(),
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
        chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
    }
}

fn add_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, square: Square) {
    let piece_id = PieceId::from(id);
    let definition = &state.piece_definitions[type_id];
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
            has_moved: true,
            current_ammo: definition.max_ammo,
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: definition.initial_state(),
            move_option_cooldowns: HashMap::new(),
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

fn positions() -> Vec<(&'static str, GameState, PieceId)> {
    let mut standard = empty_state("standard-8x8", 8);
    add_piece(
        &mut standard,
        "selected",
        "white",
        "queen",
        Square::new(3, 3),
    );
    add_piece(&mut standard, "enemy", "black", "rook", Square::new(3, 6));

    let mut crowded = empty_state("crowded-12x12", 12);
    add_piece(
        &mut crowded,
        "selected",
        "white",
        "queen",
        Square::new(5, 5),
    );
    for file in 0..12 {
        add_piece(
            &mut crowded,
            &format!("white-{file}"),
            "white",
            "pawn-white",
            Square::new(file, 2),
        );
        add_piece(
            &mut crowded,
            &format!("black-{file}"),
            "black",
            "pawn-black",
            Square::new(file, 9),
        );
    }

    let mut layered = empty_state("many-layers", 12);
    let mut layered_definition = rook_definition();
    layered_definition.id = "layered-benchmark".into();
    layered_definition.name = "Layered Benchmark".into();
    let base = layered_definition.move_layers[0].clone();
    layered_definition.move_layers = (0..12)
        .map(|index| MoveLayerDefinition {
            id: format!("layer-{index}"),
            ..base.clone()
        })
        .collect();
    layered_definition.move_options[0].layer_ids = layered_definition
        .move_layers
        .iter()
        .map(|layer| layer.id.clone())
        .collect();
    layered
        .piece_definitions
        .insert(layered_definition.id.clone(), layered_definition);
    layered.rebuild_chessembly_cache();
    add_piece(
        &mut layered,
        "selected",
        "white",
        "layered-benchmark",
        Square::new(5, 5),
    );

    let mut custom = empty_state("custom-piece", 10);
    let mut custom_definition = queen_definition();
    custom_definition.id = "custom-benchmark".into();
    custom_definition.name = "Custom Benchmark".into();
    custom
        .piece_definitions
        .insert(custom_definition.id.clone(), custom_definition);
    custom.rebuild_chessembly_cache();
    add_piece(
        &mut custom,
        "selected",
        "white",
        "custom-benchmark",
        Square::new(4, 4),
    );

    vec![
        ("8x8 standard", standard, PieceId::from("selected")),
        ("12x12 crowded", crowded, PieceId::from("selected")),
        ("12x12 many layers", layered, PieceId::from("selected")),
        ("10x10 custom", custom, PieceId::from("selected")),
    ]
}

fn measure(
    state: &GameState,
    piece_id: &PieceId,
    include_attacks: bool,
) -> (Duration, ProfilingSnapshot, usize) {
    for _ in 0..20 {
        black_box(generate_piece_legal_move_actions(state, piece_id));
        if include_attacks {
            black_box(generate_piece_attack_squares(state, piece_id));
        }
    }
    let before = profiling::snapshot();
    let started = Instant::now();
    let mut response_size = 0;
    for _ in 0..ITERATIONS {
        let moves = black_box(generate_piece_legal_move_actions(state, piece_id));
        let attacks = if include_attacks {
            generate_piece_attack_squares(state, piece_id)
        } else {
            Vec::new()
        };
        response_size = if include_attacks {
            serde_json::to_vec(&serde_json::json!({
                "moves": moves,
                "attacks": attacks,
                "ability_actions": [],
            }))
        } else {
            serde_json::to_vec(&serde_json::json!({
                "moves": moves,
                "ability_actions": [],
            }))
        }
        .unwrap()
        .len();
    }
    (
        started.elapsed() / ITERATIONS,
        profiling::snapshot().since(before),
        response_size,
    )
}

#[test]
#[ignore = "repeatable piece-options baseline; run with --release --features profiling --ignored --nocapture"]
fn piece_options_hot_path_profile() {
    println!("scenario,variant,mean_us,chessembly_runs,cache_checks,cache_rebuilds,candidates,dedup_us,json_bytes");
    for (name, state, piece_id) in positions() {
        for (variant, include_attacks) in [("before", true), ("after", false)] {
            let (elapsed, counters, response_size) = measure(&state, &piece_id, include_attacks);
            println!(
                "{name},{variant},{},{},{},{},{},{},{}",
                elapsed.as_micros(),
                counters.chessembly_run_calls / u64::from(ITERATIONS),
                counters.chessembly_cache_checks / u64::from(ITERATIONS),
                counters.chessembly_cache_rebuilds,
                counters.generated_move_candidates / u64::from(ITERATIONS),
                counters.deduplication_nanos / u64::from(ITERATIONS) / 1_000,
                response_size,
            );
        }
    }
}
