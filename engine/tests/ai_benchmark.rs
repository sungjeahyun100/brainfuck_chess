use std::collections::HashMap;
use std::time::Instant;

use brainfuck_chess_engine::ai::{
    apply_ai_action, choose_bot_action, generate_ai_actions, AiAction, BotDifficulty,
};
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::profiling;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;

struct BenchmarkPosition {
    name: &'static str,
    state: GameState,
}

fn empty_state(name: &str) -> GameState {
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
        id: format!("ai-benchmark-{name}"),
        board: create_board(8),
        pieces: HashMap::new(),
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

fn add_board_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, square: Square) {
    let piece_id: PieceId = id.into();
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

fn add_pocket_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str) {
    let piece_id: PieceId = id.into();
    let definition = &state.piece_definitions[type_id];
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
            state: definition.initial_state(),
            move_option_cooldowns: HashMap::new(),
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

fn benchmark_positions() -> Vec<BenchmarkPosition> {
    let mut middlegame = empty_state("middlegame");
    for (id, owner, kind, file, rank) in [
        ("wk", "white", "king", 6, 0),
        ("wq", "white", "queen", 3, 2),
        ("wr", "white", "rook", 0, 0),
        ("wb", "white", "bishop", 2, 3),
        ("wn", "white", "knight", 5, 2),
        ("wp1", "white", "pawn-white", 3, 3),
        ("wp2", "white", "pawn-white", 6, 1),
        ("bk", "black", "king", 7, 6),
        ("bq", "black", "queen", 3, 5),
        ("br", "black", "rook", 7, 7),
        ("bb", "black", "bishop", 5, 4),
        ("bn", "black", "knight", 2, 5),
        ("bp1", "black", "pawn-black", 4, 4),
        ("bp2", "black", "pawn-black", 6, 6),
    ] {
        add_board_piece(&mut middlegame, id, owner, kind, Square::new(file, rank));
    }

    let mut tactical = empty_state("tactical-captures");
    for (id, owner, kind, file, rank) in [
        ("wk", "white", "king", 0, 0),
        ("wq", "white", "queen", 3, 3),
        ("wr", "white", "rook", 4, 2),
        ("bk", "black", "king", 7, 6),
        ("br1", "black", "rook", 3, 6),
        ("br2", "black", "rook", 6, 3),
        ("bb1", "black", "bishop", 1, 3),
        ("bn1", "black", "knight", 4, 5),
        ("bp1", "black", "pawn-black", 3, 4),
    ] {
        add_board_piece(&mut tactical, id, owner, kind, Square::new(file, rank));
    }

    let mut drops = empty_state("drop-branching");
    add_board_piece(&mut drops, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut drops, "bk", "black", "king", Square::new(4, 7));
    add_board_piece(&mut drops, "bp", "black", "pawn-black", Square::new(3, 5));
    for (id, kind) in [
        ("wq", "queen"),
        ("wr", "rook"),
        ("wb", "bishop"),
        ("wn", "knight"),
    ] {
        add_pocket_piece(&mut drops, id, "white", kind);
    }

    let mut ability = empty_state("standalone-ability");
    add_board_piece(&mut ability, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut ability, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut ability,
        "camp",
        "white",
        "green-camp",
        Square::new(3, 3),
    );
    add_board_piece(&mut ability, "enemy", "black", "rook", Square::new(4, 3));

    let mut stateful = empty_state("piece-state-cooldown");
    add_board_piece(&mut stateful, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut stateful, "bk", "black", "king", Square::new(7, 6));
    add_board_piece(
        &mut stateful,
        "windmill",
        "white",
        "windmill",
        Square::new(3, 3),
    );
    add_board_piece(
        &mut stateful,
        "cannon",
        "white",
        "cannon-rook",
        Square::new(5, 2),
    );
    stateful
        .pieces
        .get_mut("cannon")
        .unwrap()
        .move_option_cooldowns
        .insert("cannon_move".into(), CooldownState { remaining: 2 });

    let mut king_capture = empty_state("immediate-king-capture");
    add_board_piece(&mut king_capture, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut king_capture, "wr", "white", "rook", Square::new(4, 0));
    add_board_piece(&mut king_capture, "bk", "black", "king", Square::new(4, 7));

    let mut drop_capture = empty_state("drop-capture");
    add_board_piece(&mut drop_capture, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut drop_capture, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut drop_capture,
        "enemy",
        "black",
        "knight",
        Square::new(3, 0),
    );
    add_pocket_piece(&mut drop_capture, "para", "white", "paratrooper");

    let mut airborne = empty_state("airborne-deployment");
    add_board_piece(&mut airborne, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut airborne, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut airborne,
        "airborne",
        "white",
        "airborne",
        Square::new(3, 3),
    );
    for (id, kind) in [
        ("air-bishop", "bishop"),
        ("air-knight", "knight"),
        ("air-pawn", "pawn-white"),
    ] {
        add_pocket_piece(&mut airborne, id, "white", kind);
    }

    let mut pocket_swap = empty_state("alternating-soldier-pocket-swap");
    add_board_piece(&mut pocket_swap, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut pocket_swap, "bk", "black", "king", Square::new(7, 6));
    add_board_piece(
        &mut pocket_swap,
        "soldier",
        "white",
        "alternating-soldier",
        Square::new(3, 3),
    );
    add_board_piece(
        &mut pocket_swap,
        "swap-target",
        "white",
        "bishop",
        Square::new(4, 4),
    );
    add_board_piece(
        &mut pocket_swap,
        "enemy",
        "black",
        "bishop",
        Square::new(2, 2),
    );
    add_pocket_piece(&mut pocket_swap, "swap-reserve", "white", "knight");

    vec![
        BenchmarkPosition {
            name: "middlegame",
            state: middlegame,
        },
        BenchmarkPosition {
            name: "tactical-captures",
            state: tactical,
        },
        BenchmarkPosition {
            name: "drop-branching",
            state: drops,
        },
        BenchmarkPosition {
            name: "standalone-ability",
            state: ability,
        },
        BenchmarkPosition {
            name: "piece-state-cooldown",
            state: stateful,
        },
        BenchmarkPosition {
            name: "immediate-king-capture",
            state: king_capture,
        },
        BenchmarkPosition {
            name: "drop-capture",
            state: drop_capture,
        },
        BenchmarkPosition {
            name: "airborne-deployment",
            state: airborne,
        },
        BenchmarkPosition {
            name: "alternating-soldier-pocket-swap",
            state: pocket_swap,
        },
    ]
}

fn action_is_legal(state: &GameState, selected: &AiAction) -> bool {
    generate_ai_actions(state)
        .iter()
        .any(|candidate| candidate == selected)
        && apply_ai_action(state.clone(), selected).is_ok()
}

#[test]
fn benchmark_positions_produce_legal_ai_decisions() {
    let positions = benchmark_positions();
    assert_eq!(positions.len(), 9);
    for position in &positions {
        let decision = choose_bot_action(&position.state, &"white".into(), BotDifficulty::Easy)
            .unwrap_or_else(|| panic!("{} produced no AI decision", position.name));
        assert!(
            action_is_legal(&position.state, &decision.action),
            "{} selected an illegal action",
            position.name
        );
    }
    let actions = |name: &str| {
        generate_ai_actions(
            &positions
                .iter()
                .find(|position| position.name == name)
                .unwrap()
                .state,
        )
    };
    assert!(actions("middlegame").iter().all(|action| !matches!(
        action,
        AiAction::Move(action) if action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("bk")
    )));
    assert!(actions("tactical-captures")
        .iter()
        .filter(|action| matches!(action, AiAction::Move(action) if action.captured_piece_id.is_some()))
        .count()
        >= 4);
    assert!(
        actions("drop-branching")
            .iter()
            .filter(|action| matches!(action, AiAction::Drop(_)))
            .count()
            >= 16
    );
    assert!(actions("standalone-ability")
        .iter()
        .any(|action| matches!(action, AiAction::Ability(_))));
    let stateful = &positions
        .iter()
        .find(|position| position.name == "piece-state-cooldown")
        .unwrap()
        .state;
    assert!(!stateful.pieces["windmill"].state.is_empty());
    assert_eq!(
        stateful.pieces["cannon"].move_option_cooldowns["cannon_move"].remaining,
        2
    );
    assert!(actions("immediate-king-capture")
        .iter()
        .any(|action| matches!(
            action,
            AiAction::Move(action) if action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("bk")
        )));
    assert!(actions("drop-capture").iter().any(|action| matches!(
        action,
        AiAction::Drop(action)
            if action.piece_id.as_str() == "para"
                && action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("enemy")
    )));
    assert!(actions("airborne-deployment").iter().any(|action| matches!(
        action,
        AiAction::Ability(action)
            if action.piece_id.as_str() == "airborne"
                && action.ability_id == "airdrop"
                && action.deployments.len() >= 2
    )));
    assert!(actions("alternating-soldier-pocket-swap")
        .iter()
        .any(|action| matches!(
            action,
            AiAction::Ability(action)
                if action.piece_id.as_str() == "soldier"
                    && action.ability_id == "relieve"
                    && action.target_piece_id.as_ref().map(PieceId::as_str) == Some("swap-target")
                    && action.pocket_piece_id.as_ref().map(PieceId::as_str) == Some("swap-reserve")
        )));
}

#[test]
#[ignore = "repeatable AI baseline; run explicitly with --features profiling --ignored --nocapture"]
fn ai_search_baseline() {
    for position in benchmark_positions() {
        let before = profiling::snapshot();
        let started = Instant::now();
        let decision = choose_bot_action(&position.state, &"white".into(), BotDifficulty::Normal)
            .unwrap_or_else(|| panic!("{} produced no AI decision", position.name));
        let elapsed = started.elapsed();
        let counters = profiling::snapshot().since(before);
        assert!(action_is_legal(&position.state, &decision.action));
        println!(
            "position={} action={} score={} nodes={} reached_depth={} completed_depth={} beta_cutoffs={} elapsed_ms={:.3} legal_gen={} drop_gen={} attack_map_gen={} chessembly_runs={} evaluations={} action_applications={}",
            position.name,
            serde_json::to_string(&decision.action).unwrap(),
            decision.score,
            decision.searched_nodes,
            decision.depth_reached,
            decision.completed_depth,
            decision.stats.beta_cutoffs,
            elapsed.as_secs_f64() * 1_000.0,
            counters.legal_move_generation_calls,
            counters.drop_generation_calls,
            counters.attack_map_generation_calls,
            counters.chessembly_run_calls,
            counters.evaluation_calls,
            counters.action_application_calls,
        );
    }
}
