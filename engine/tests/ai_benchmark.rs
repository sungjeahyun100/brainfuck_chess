use std::collections::HashMap;
use std::time::Instant;

use brainfuck_chess_engine::ai::{
    apply_ai_action, choose_bot_action, choose_bot_action_with_limits_and_options,
    choose_bot_action_with_options, generate_ai_actions, AiAction, BotDifficulty, SearchLimits,
    SearchOptions,
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

fn qsearch_horizon_positions() -> Vec<BenchmarkPosition> {
    let mut recapture = empty_state("qsearch-recapture");
    add_board_piece(&mut recapture, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut recapture, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut recapture,
        "white-queen",
        "white",
        "queen",
        Square::new(3, 3),
    );
    add_board_piece(
        &mut recapture,
        "black-rook",
        "black",
        "rook",
        Square::new(3, 7),
    );
    recapture.current_player = "black".into();

    let mut capture_drop = empty_state("qsearch-capture-drop");
    add_board_piece(&mut capture_drop, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut capture_drop, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut capture_drop,
        "drop-victim",
        "white",
        "queen",
        Square::new(3, 7),
    );
    add_pocket_piece(&mut capture_drop, "black-para", "black", "paratrooper");
    capture_drop.current_player = "black".into();

    let mut recall = empty_state("qsearch-enemy-recall");
    add_board_piece(&mut recall, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut recall, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(
        &mut recall,
        "black-camp",
        "black",
        "green-camp",
        Square::new(3, 3),
    );
    add_board_piece(
        &mut recall,
        "recall-victim",
        "white",
        "queen",
        Square::new(4, 3),
    );
    recall
        .pieces
        .get_mut("black-camp")
        .unwrap()
        .move_option_cooldowns
        .insert("normal".into(), CooldownState { remaining: 1 });
    recall.current_player = "black".into();

    vec![
        BenchmarkPosition {
            name: "qsearch-recapture",
            state: recapture,
        },
        BenchmarkPosition {
            name: "qsearch-capture-drop",
            state: capture_drop,
        },
        BenchmarkPosition {
            name: "qsearch-enemy-recall",
            state: recall,
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
fn benchmark_positions_match_with_transposition_table_enabled_and_disabled() {
    let limits = SearchLimits {
        max_depth_actions: 2,
        max_nodes: 100_000,
        soft_time_ms: 10_000,
        hard_time_ms: 20_000,
    };
    for position in benchmark_positions() {
        let configurations = [
            SearchOptions {
                use_transposition_table: true,
                use_aspiration_window: false,
            },
            SearchOptions {
                use_transposition_table: true,
                use_aspiration_window: true,
            },
            SearchOptions {
                use_transposition_table: false,
                use_aspiration_window: false,
            },
            SearchOptions {
                use_transposition_table: false,
                use_aspiration_window: true,
            },
        ];
        let decisions = configurations.map(|options| {
            choose_bot_action_with_limits_and_options(
                &position.state,
                &"white".into(),
                BotDifficulty::Normal,
                limits,
                options,
            )
            .unwrap_or_else(|| panic!("{} produced no AI decision", position.name))
        });
        let expected = &decisions[0];
        for (options, decision) in configurations.into_iter().zip(&decisions) {
            assert_eq!(
                decision.completed_depth, expected.completed_depth,
                "{} depth",
                position.name
            );
            assert_eq!(decision.action, expected.action, "{} action", position.name);
            assert_eq!(decision.score, expected.score, "{} score", position.name);
            if !options.use_transposition_table {
                assert_eq!(
                    decision.stats.tt_probes, 0,
                    "{} TT-off probes",
                    position.name
                );
                assert_eq!(decision.stats.tt_hits, 0, "{} TT-off hits", position.name);
                assert_eq!(
                    decision.stats.tt_cutoffs, 0,
                    "{} TT-off cutoffs",
                    position.name
                );
                assert_eq!(
                    decision.stats.tt_stores, 0,
                    "{} TT-off stores",
                    position.name
                );
            }
            if !options.use_aspiration_window {
                assert_eq!(decision.stats.aspiration_searches, 0);
                assert_eq!(decision.stats.aspiration_researches, 0);
                assert_eq!(decision.stats.aspiration_fail_lows, 0);
                assert_eq!(decision.stats.aspiration_fail_highs, 0);
            }
        }
    }
}

#[cfg(feature = "profiling")]
#[test]
fn position_keys_are_generated_only_when_transposition_table_is_enabled() {
    let position = benchmark_positions().into_iter().next().unwrap();
    let limits = SearchLimits {
        max_depth_actions: 2,
        max_nodes: 100_000,
        soft_time_ms: 10_000,
        hard_time_ms: 20_000,
    };
    let run = |use_transposition_table| {
        let before = profiling::snapshot();
        let decision = choose_bot_action_with_limits_and_options(
            &position.state,
            &"white".into(),
            BotDifficulty::Normal,
            limits,
            SearchOptions {
                use_transposition_table,
                ..SearchOptions::default()
            },
        )
        .unwrap();
        (decision, profiling::snapshot().since(before))
    };

    let (without, without_counters) = run(false);
    assert_eq!(without_counters.position_key_generation_calls, 0);
    assert_eq!(without.stats.tt_probes, 0);
    assert_eq!(without.stats.tt_hits, 0);
    assert_eq!(without.stats.tt_cutoffs, 0);
    assert_eq!(without.stats.tt_stores, 0);

    let (with, with_counters) = run(true);
    assert!(with_counters.position_key_generation_calls > 0);
    assert!(with.stats.tt_probes > 0);
}

#[test]
#[ignore = "repeatable AI baseline; run explicitly with --features profiling --ignored --nocapture"]
fn ai_search_baseline() {
    run_search_baseline(SearchOptions::default());
}

#[test]
#[ignore = "TT-off comparison baseline; run explicitly with --features profiling --ignored --nocapture"]
fn ai_search_tt_off_comparison() {
    run_search_baseline(SearchOptions {
        use_transposition_table: false,
        ..SearchOptions::default()
    });
}

#[test]
#[ignore = "aspiration-off full-window ID comparison; run explicitly with --features profiling --ignored --nocapture"]
fn ai_search_aspiration_off_comparison() {
    run_search_baseline(SearchOptions {
        use_aspiration_window: false,
        ..SearchOptions::default()
    });
}

#[test]
#[ignore = "actual Easy/Normal/Hard budgets; run explicitly with --features profiling --ignored --nocapture"]
fn ai_search_difficulty_budgets() {
    for position in benchmark_positions() {
        for difficulty in [
            BotDifficulty::Easy,
            BotDifficulty::Normal,
            BotDifficulty::Hard,
        ] {
            let started = Instant::now();
            let decision = choose_bot_action(&position.state, &"white".into(), difficulty)
                .unwrap_or_else(|| panic!("{} produced no AI decision", position.name));
            assert!(action_is_legal(&position.state, &decision.action));
            println!(
                "position={} difficulty={:?} max_depth={} nodes={} reached_depth={} completed_depth={} iterations_started={} iterations_completed={} aspiration_searches={} aspiration_researches={} aspiration_fail_lows={} aspiration_fail_highs={} elapsed_ms={:.3}",
                position.name,
                difficulty,
                difficulty.limits().max_depth_actions,
                decision.searched_nodes,
                decision.depth_reached,
                decision.completed_depth,
                decision.stats.iterations_started,
                decision.stats.iterations_completed,
                decision.stats.aspiration_searches,
                decision.stats.aspiration_researches,
                decision.stats.aspiration_fail_lows,
                decision.stats.aspiration_fail_highs,
                started.elapsed().as_secs_f64() * 1_000.0,
            );
        }
    }
}

#[test]
#[ignore = "QSearch horizon profiling; run explicitly with --features profiling --ignored --nocapture"]
fn qsearch_horizon_benchmark() {
    let limits = SearchLimits {
        max_depth_actions: 1,
        max_nodes: 100_000,
        soft_time_ms: 10_000,
        hard_time_ms: 20_000,
    };
    for position in qsearch_horizon_positions() {
        let before = profiling::snapshot();
        let started = Instant::now();
        let decision = choose_bot_action_with_limits_and_options(
            &position.state,
            &"black".into(),
            BotDifficulty::Normal,
            limits,
            SearchOptions::default(),
        )
        .unwrap();
        let counters = profiling::snapshot().since(before);
        assert!(action_is_legal(&position.state, &decision.action));
        assert!(decision.stats.qnodes <= decision.searched_nodes);
        println!(
            "position={} action={} score={} nodes={} qnodes={} completed_depth={} elapsed_ms={:.3} legal_gen={} drop_gen={} evaluations={} action_applications={}",
            position.name,
            serde_json::to_string(&decision.action).unwrap(),
            decision.score,
            decision.searched_nodes,
            decision.stats.qnodes,
            decision.completed_depth,
            started.elapsed().as_secs_f64() * 1_000.0,
            counters.legal_move_generation_calls,
            counters.drop_generation_calls,
            counters.evaluation_calls,
            counters.action_application_calls,
        );
    }
}

fn run_search_baseline(options: SearchOptions) {
    for position in benchmark_positions() {
        let before = profiling::snapshot();
        let started = Instant::now();
        let decision = choose_bot_action_with_options(
            &position.state,
            &"white".into(),
            BotDifficulty::Normal,
            options,
        )
        .unwrap_or_else(|| panic!("{} produced no AI decision", position.name));
        let elapsed = started.elapsed();
        let counters = profiling::snapshot().since(before);
        assert!(action_is_legal(&position.state, &decision.action));
        println!(
            "position={} tt_enabled={} aspiration_enabled={} action={} score={} nodes={} qnodes={} reached_depth={} completed_depth={} iterations_started={} iterations_completed={} aspiration_searches={} aspiration_researches={} aspiration_fail_lows={} aspiration_fail_highs={} beta_cutoffs={} position_key_generations={} tt_probes={} tt_hits={} tt_cutoffs={} tt_stores={} elapsed_ms={:.3} legal_gen={} drop_gen={} attack_map_gen={} chessembly_runs={} evaluations={} action_applications={}",
            position.name,
            options.use_transposition_table,
            options.use_aspiration_window,
            serde_json::to_string(&decision.action).unwrap(),
            decision.score,
            decision.searched_nodes,
            decision.stats.qnodes,
            decision.depth_reached,
            decision.completed_depth,
            decision.stats.iterations_started,
            decision.stats.iterations_completed,
            decision.stats.aspiration_searches,
            decision.stats.aspiration_researches,
            decision.stats.aspiration_fail_lows,
            decision.stats.aspiration_fail_highs,
            decision.stats.beta_cutoffs,
            counters.position_key_generation_calls,
            decision.stats.tt_probes,
            decision.stats.tt_hits,
            decision.stats.tt_cutoffs,
            decision.stats.tt_stores,
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
