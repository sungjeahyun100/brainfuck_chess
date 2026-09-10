use std::collections::HashMap;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::ai::{
    apply_ai_action, choose_bot_action, generate_ai_actions, play_bot_turn, play_bot_turn_detailed,
    AiAction, BotDifficulty,
};
use brainfuck_chess_engine::endgame::apply_move_action;
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::create_board;
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
                        hand_pieces: Vec::new(),
                        extra_deck_pieces: Vec::new(),
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
        ruleset: Default::default(),
        id: "ai-test".into(),
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
            current_ammo: 0,
            layer: brainfuck_chess_engine::types::PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: HashMap::new(),
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
            current_ammo: 0,
            layer: brainfuck_chess_engine::types::PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: HashMap::new(),
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

#[test]
fn empty_candidates_do_not_panic() {
    let state = make_state();
    assert!(generate_ai_actions(&state).is_empty());
    let decision = choose_bot_action(&state, &"white".into(), BotDifficulty::Easy);
    assert!(decision.is_none());
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
fn standalone_piece_abilities_are_exposed_to_ai_and_apply_canonically() {
    let mut state = make_state();
    add_board_piece(&mut state, "camp", "white", "green-camp", Square::new(3, 3));
    add_board_piece(&mut state, "enemy", "black", "rook", Square::new(4, 3));

    let ability = generate_ai_actions(&state)
        .into_iter()
        .find(|action| matches!(action, AiAction::Ability(_)))
        .expect("AI should receive the Green Camp recall action");
    let applied = apply_ai_action(state, &ability).unwrap();
    assert!(applied.pieces["enemy"].in_pocket);
    assert_eq!(applied.current_player, "black");
}

#[test]
fn bot_timeline_last_snapshot_matches_final_state() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "wr", "white", "rook", Square::new(0, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    let result = play_bot_turn_detailed(state, &"white".to_string(), BotDifficulty::Easy).unwrap();

    assert_eq!(result.timeline.len(), result.actions.len());
    let timeline_state = serde_json::to_value(&result.timeline.last().unwrap().state).unwrap();
    let final_state = serde_json::to_value(&result.state).unwrap();
    assert_eq!(timeline_state, final_state);
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
        let decision = choose_bot_action(&state, &"white".into(), difficulty).unwrap();
        assert!(matches!(
            decision.action,
            AiAction::Move(MoveAction { captured_piece_id: Some(ref id), .. }) if id == "bk"
        ));
    }
}

#[test]
fn drop_and_move_actions_end_the_turn() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    add_pocket_piece(&mut state, "wn", "white", "knight");

    let drop = generate_ai_actions(&state)
        .into_iter()
        .find(|action| matches!(action, AiAction::Drop(_)))
        .unwrap();
    let dropped = apply_ai_action(state.clone(), &drop).unwrap();
    assert_eq!(dropped.current_player, "black");

    let movement = generate_ai_actions(&state)
        .into_iter()
        .find(|action| matches!(action, AiAction::Move(_)))
        .unwrap();
    let moved = apply_ai_action(state, &movement).unwrap();
    assert_eq!(moved.current_player, "black");
}

#[test]
fn bot_turn_finishes_or_ends_the_game_within_the_difficulty_limit() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(4, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(4, 7));
    add_board_piece(&mut state, "wn", "white", "knight", Square::new(1, 0));

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
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        },
    );
    en_passant_state.current_player = "white".into();
    assert!(generate_ai_actions(&en_passant_state).iter().any(|action| {
        matches!(action, AiAction::Move(movement) if movement.piece_id == "wp" && movement.to == Square::new(5, 5))
    }));
}

#[test]
fn ai_includes_available_move_options_and_excludes_cooldowns() {
    let mut state = make_state();
    add_board_piece(&mut state, "wk", "white", "king", Square::new(0, 0));
    add_board_piece(&mut state, "bk", "black", "king", Square::new(7, 7));
    add_board_piece(&mut state, "cr", "white", "cannon-rook", Square::new(3, 3));
    add_board_piece(
        &mut state,
        "screen",
        "white",
        "pawn-white",
        Square::new(3, 4),
    );

    assert!(generate_ai_actions(&state).iter().any(|action| {
        matches!(action, AiAction::Move(movement) if movement.piece_id == "cr" && movement.move_option_id == "cannon_move")
    }));

    state
        .pieces
        .get_mut("cr")
        .unwrap()
        .move_option_cooldowns
        .insert("cannon_move".into(), CooldownState { remaining: 1 });
    assert!(!generate_ai_actions(&state).iter().any(|action| {
        matches!(action, AiAction::Move(movement) if movement.piece_id == "cr" && movement.move_option_id == "cannon_move")
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
    assert_eq!((easy.max_depth_actions, easy.max_nodes), (3, 500));
    assert_eq!((normal.max_depth_actions, normal.max_nodes), (4, 3_000));
    assert_eq!((hard.max_depth_actions, hard.max_nodes), (5, 10_000));
    assert!(easy.hard_time_ms < normal.hard_time_ms);
    assert!(normal.hard_time_ms < hard.hard_time_ms);
}

#[test]
fn airborne_actions_include_unique_canonical_multi_deployments() {
    let mut state = make_state();
    add_board_piece(
        &mut state,
        "airborne",
        "white",
        "airborne",
        Square::new(3, 3),
    );
    add_pocket_piece(&mut state, "bishop", "white", "bishop");
    add_pocket_piece(&mut state, "knight", "white", "knight");

    let actions = generate_ai_actions(&state)
        .into_iter()
        .filter_map(|action| match action {
            AiAction::Ability(action) if action.ability_id == "airdrop" => Some(action),
            _ => None,
        })
        .collect::<Vec<_>>();
    let multi = actions
        .iter()
        .find(|action| action.deployments.len() >= 2)
        .expect("AI should generate a multi-deployment airdrop");
    assert!(submit_action(state.clone(), TurnAction::Ability(multi.clone())).is_ok());

    let mut canonical_sets = std::collections::HashSet::new();
    for action in actions {
        let mut pieces = std::collections::HashSet::new();
        let mut squares = std::collections::HashSet::new();
        assert!(action.deployments.iter().all(|deployment| {
            pieces.insert(deployment.pocket_piece_id.clone())
                && squares.insert(deployment.to.to_id())
        }));
        let mut signature = action
            .deployments
            .iter()
            .map(|deployment| format!("{}:{}", deployment.pocket_piece_id, deployment.to.to_id()))
            .collect::<Vec<_>>();
        signature.sort();
        assert!(
            canonical_sets.insert(signature),
            "duplicate deployment permutation"
        );
    }
}

mod g8a {
    use super::*;
    use brainfuck_chess_engine::ai::{
        choose_bot_action_with_limits_and_options, evaluate, order_ai_actions, sacrifice_utility,
        select_sacrifice_subsets, SearchLimits, SearchOptions, EXTRA_SUBSET_LIMIT,
    };

    fn reserve(state: &mut GameState, id: &str, owner: &str, kind: &str, extra: bool) {
        add_pocket_piece(state, id, owner, kind);
        state
            .players
            .get_mut(owner)
            .unwrap()
            .deck
            .pocket_pieces
            .retain(|p| p != id);
        state.pieces.get_mut(&PieceId::from(id)).unwrap().in_pocket = false;
        let deck = &mut state.players.get_mut(owner).unwrap().deck;
        if extra {
            deck.extra_deck_pieces.push(id.into());
        } else {
            deck.hand_pieces.push(id.into());
        }
    }

    fn fixture() -> GameState {
        let mut s = make_state();
        s.ruleset = DeckRuleset::Standard;
        add_board_piece(&mut s, "wk", "white", "king", Square::new(3, 0));
        add_board_piece(&mut s, "bk", "black", "king", Square::new(7, 7));
        add_board_piece(&mut s, "q1", "white", "queen", Square::new(2, 2));
        add_board_piece(&mut s, "q2", "white", "queen", Square::new(3, 2));
        add_board_piece(
            &mut s,
            "ability",
            "white",
            "alternating-soldier",
            Square::new(4, 0),
        );
        reserve(&mut s, "h1", "white", "queen", false);
        reserve(&mut s, "h2", "white", "queen", false);
        reserve(&mut s, "h3", "white", "knight", false);
        add_pocket_piece(&mut s, "p1", "white", "knight");
        reserve(&mut s, "x1", "white", "guhang", true);
        reserve(&mut s, "x2", "white", "bomber", true);
        reserve(&mut s, "x3", "white", "guhang", true);
        reserve(&mut s, "secret-hand", "black", "queen", false);
        add_pocket_piece(&mut s, "secret-pocket", "black", "queen");
        s
    }

    #[test]
    fn legal_actions_include_hand_abilities_and_bounded_exact_summons() {
        let s = fixture();
        let actions = generate_ai_actions(&s);
        assert!(actions.iter().any(|a| matches!(a, AiAction::Move(_))));
        assert!(actions
            .iter()
            .any(|a| matches!(a, AiAction::Drop(d) if d.piece_id == "h3")));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, AiAction::Drop(d) if d.piece_id == "p1")));
        assert!(actions.iter().any(|a| matches!(a, AiAction::Ability(_))));
        for extra in ["x1", "x2", "x3"] {
            assert!(actions
                .iter()
                .any(|a| matches!(a, AiAction::ExtraSummon(x) if x.extra_piece_id == extra)));
            let subsets = select_sacrifice_subsets(&s, &extra.into());
            assert!(!subsets.is_empty());
            assert!(subsets.len() <= EXTRA_SUBSET_LIMIT);
            for subset in subsets {
                assert!(
                    subset.score >= u64::from(s.piece_definitions[&s.pieces[extra].type_id].score)
                );
                assert_eq!(
                    subset
                        .piece_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    subset.piece_ids.len()
                );
                assert!(!subset.piece_ids.contains(&"wk".into()));
                assert!(!subset.piece_ids.contains(&"p1".into()));
                if extra == "x2" {
                    assert!(subset.piece_ids.iter().all(|id| s.pieces[id].is_on_board()));
                }
            }
        }
        assert!(actions.iter().any(|a| matches!(a, AiAction::ExtraSummon(x) if x.extra_piece_id == "x1" && x.sacrifice_piece_ids.iter().any(|id| s.players["white"].deck.hand_pieces.contains(id)))));
        for mut action in actions {
            if let AiAction::ExtraSummon(x) = &mut action {
                x.sacrifice_piece_ids.reverse();
            }
            let canonical: TurnAction = action.clone().into();
            assert_eq!(AiAction::from(canonical), action);
            let next = apply_ai_action(s.clone(), &action).unwrap();
            if let AiAction::ExtraSummon(x) = action {
                assert!(!next.players["white"]
                    .deck
                    .extra_deck_pieces
                    .contains(&x.extra_piece_id));
                assert_eq!(
                    next.pieces[&x.extra_piece_id].current_square,
                    Some(x.target_square)
                );
                assert!(x
                    .sacrifice_piece_ids
                    .iter()
                    .all(|id| next.pieces[id].captured));
                assert_eq!(
                    next.players["black"].deck.hand_pieces,
                    s.players["black"].deck.hand_pieces
                );
                assert_eq!(
                    next.players["black"].deck.pocket_pieces,
                    s.players["black"].deck.pocket_pieces
                );
            }
        }
    }

    #[test]
    fn hidden_reserve_types_counts_and_order_cannot_change_root_input_or_score() {
        let s = fixture();
        let mut other = s.clone();
        other.pieces.get_mut("secret-hand").unwrap().type_id = "shell".into();
        other.pieces.get_mut("secret-pocket").unwrap().type_id = "wall".into();
        for n in 0..7 {
            add_pocket_piece(&mut other, &format!("hidden-{n}"), "black", "bomber");
        }
        let bot = "white".into();
        assert_eq!(evaluate(&s, &bot), evaluate(&other, &bot));
        let mut a = generate_ai_actions(&s);
        let mut b = generate_ai_actions(&other);
        order_ai_actions(&s, &mut a, &bot);
        order_ai_actions(&other, &mut b, &bot);
        assert_eq!(a, b);
        let limits = SearchLimits {
            max_depth_actions: 1,
            max_nodes: 0,
            soft_time_ms: 60_000,
            hard_time_ms: 60_000,
        };
        // No completed depth: all difficulties use the identical ordered fallback,
        // independent of Easy's deliberate final random choice.
        for difficulty in [
            BotDifficulty::Easy,
            BotDifficulty::Normal,
            BotDifficulty::Hard,
        ] {
            let x = choose_bot_action_with_limits_and_options(
                &s,
                &bot,
                difficulty,
                limits,
                SearchOptions::default(),
            )
            .unwrap();
            let y = choose_bot_action_with_limits_and_options(
                &other,
                &bot,
                difficulty,
                limits,
                SearchOptions::default(),
            )
            .unwrap();
            assert_eq!(x.action, y.action);
            assert_eq!(x.score, y.score);
        }
        let before = serde_json::to_value(&s).unwrap();
        let limits = SearchLimits {
            max_depth_actions: 1,
            max_nodes: 100_000,
            soft_time_ms: 60_000,
            hard_time_ms: 60_000,
        };
        for tt in [false, true] {
            let options = SearchOptions {
                use_transposition_table: tt,
                ..SearchOptions::default()
            };
            let x = choose_bot_action_with_limits_and_options(
                &s,
                &bot,
                BotDifficulty::Hard,
                limits,
                options,
            )
            .unwrap();
            let y = choose_bot_action_with_limits_and_options(
                &other,
                &bot,
                BotDifficulty::Hard,
                limits,
                options,
            )
            .unwrap();
            assert_eq!(x.completed_depth, 1);
            assert_eq!(x.action, y.action);
            assert_eq!(x.score, y.score);
            assert_eq!(x.searched_nodes, y.searched_nodes);
        }
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }

    #[test]
    fn own_zone_values_and_opportunity_cost_are_distinct() {
        let mut s = fixture();
        let bot = "white".into();
        let before = evaluate(&s, &bot);
        s.pieces.get_mut("h3").unwrap().type_id = "queen".into();
        assert!(evaluate(&s, &bot) > before);
        let before = evaluate(&s, &bot);
        s.pieces.get_mut("p1").unwrap().type_id = "queen".into();
        assert!(evaluate(&s, &bot) > before);
        let hand_value = evaluate(&s, &bot);
        s.players
            .get_mut("white")
            .unwrap()
            .deck
            .hand_pieces
            .retain(|id| id != "h3");
        s.players
            .get_mut("white")
            .unwrap()
            .deck
            .pocket_pieces
            .push("h3".into());
        s.pieces.get_mut("h3").unwrap().in_pocket = true;
        assert!(evaluate(&s, &bot) < hand_value);
        assert!(sacrifice_utility(&s, &"q1".into()) > sacrifice_utility(&s, &"h1".into()));
        let subsets = select_sacrifice_subsets(&s, &"x1".into());
        assert!(subsets
            .windows(2)
            .all(|w| w[0].utility_loss <= w[1].utility_loss));
        let before = evaluate(&s, &bot);
        s.players
            .get_mut("white")
            .unwrap()
            .deck
            .extra_deck_pieces
            .clear();
        assert!(evaluate(&s, &bot) < before);
    }

    #[test]
    fn selector_bounds_30_and_2003_candidates_without_power_set() {
        for count in [30, 2003] {
            let mut s = make_state();
            s.ruleset = DeckRuleset::Standard;
            reserve(&mut s, "extra", "white", "guhang", true);
            for n in 0..count {
                reserve(
                    &mut s,
                    &format!("candidate-{n:04}"),
                    "white",
                    "queen",
                    false,
                );
            }
            let subsets = select_sacrifice_subsets(&s, &"extra".into());
            assert_eq!(subsets.len(), EXTRA_SUBSET_LIMIT);
            assert!(subsets
                .iter()
                .all(|s| s.score == 27 && s.piece_ids.len() == 3));
        }
    }

    #[test]
    #[ignore = "G8-A representative Standard generation and depth benchmark"]
    fn standard_search_benchmark() {
        let mut s = fixture();
        for n in 0..24 {
            reserve(
                &mut s,
                &format!("hand-{n}"),
                "white",
                if n % 2 == 0 { "pawn" } else { "knight" },
                false,
            );
        }
        for n in 0..6 {
            add_board_piece(
                &mut s,
                &format!("board-{n}"),
                "white",
                "pawn",
                Square::new(n, 3),
            );
        }
        let start = std::time::Instant::now();
        let actions = generate_ai_actions(&s);
        let subsets: usize = s.players["white"]
            .deck
            .extra_deck_pieces
            .iter()
            .map(|id| select_sacrifice_subsets(&s, id).len())
            .sum();
        println!(
            "G8-A generated={} subsets={} generation_ms={}",
            actions.len(),
            subsets,
            start.elapsed().as_millis()
        );
        for depth in 1..=3 {
            let start = std::time::Instant::now();
            let d = choose_bot_action_with_limits_and_options(
                &s,
                &"white".into(),
                BotDifficulty::Hard,
                SearchLimits {
                    max_depth_actions: depth,
                    max_nodes: 500,
                    soft_time_ms: 2000,
                    hard_time_ms: 4000,
                },
                SearchOptions::default(),
            )
            .unwrap();
            println!(
                "G8-A depth={depth} completed={} nodes={} generated={} elapsed_ms={}",
                d.completed_depth,
                d.searched_nodes,
                d.stats.generated_legal_actions,
                start.elapsed().as_millis()
            );
            apply_ai_action(s.clone(), &d.action).unwrap();
        }
    }
}
