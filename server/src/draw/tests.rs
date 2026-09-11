use super::*;
use crate::*;
use brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions;
use serde_json::{json, Value};

pub(super) fn state_with_pockets(white: usize, black: usize) -> GameState {
    let spec = |count, side| PlayerDeckSpec {
        ruleset: DeckRuleset::Standard,
        name: None,
        starting: vec![StartingPieceSpec {
            piece: DeckPieceRef::BuiltIn {
                piece_type: "king".into(),
            },
            square: Square::new(3, if side == "white" { 0 } else { 7 }),
        }],
        pocket: (0..count)
            .map(|_| DeckPieceRef::BuiltIn {
                piece_type: "knight".into(),
            })
            .collect(),
        extra: vec![DeckPieceRef::BuiltIn {
            piece_type: "bomber".into(),
        }],
    };
    build_game_state_with_variant(
        "draw-test".into(),
        8,
        BoardVariant::Plain,
        &spec(white, "white"),
        &spec(black, "black"),
        vec![],
        false,
        false,
    )
    .unwrap()
}

// Historical v1 fixture: preserve automatic-draw replay coverage.
pub(super) fn initialize_automatic(
    state: &mut GameState,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<Vec<DrawResolution>, String> {
    let mut next = state.clone();
    let mut draws = super::initialize(&mut next, choose)?;
    if next.ruleset == DeckRuleset::Standard {
        next.global_state
            .remove(brainfuck_chess_engine::actions::MANUAL_DRAW);
        next.global_state
            .remove(brainfuck_chess_engine::actions::DRAW_REQUIRED);
        draws.push(resolve(&mut next, "white", DrawTiming::TurnStart, choose)?);
    }
    *state = next;
    Ok(draws)
}

fn value(state: &GameState) -> Value {
    serde_json::to_value(state).unwrap()
}
fn first(_: usize) -> Result<usize, String> {
    Ok(0)
}
fn no_rng(_: usize) -> Result<usize, String> {
    panic!("unexpected RNG consumption")
}

pub(super) fn add_board(
    state: &mut GameState,
    id: &str,
    owner: &str,
    type_id: &str,
    square: Square,
) {
    let mut piece = state.pieces.values().next().unwrap().clone();
    piece.id = id.into();
    piece.owner = owner.into();
    piece.type_id = type_id.into();
    piece.current_square = Some(square);
    piece.in_pocket = false;
    piece.captured = false;
    piece.initialize_from_definition(&state.piece_definitions[type_id]);
    state.board.squares.insert(square.to_id(), Some(id.into()));
    state.pieces.insert(id.into(), piece);
}

fn request(action: TurnAction) -> SubmitActionRequest {
    let action = match action {
        TurnAction::Draw(a) => json!({"type":"draw","turn_number":a.turn_number}),
        TurnAction::Move(a) => {
            json!({"type":"move","piece_id":a.piece_id,"to":a.to,"move_option_id":a.move_option_id,"promotion":a.promotion})
        }
        TurnAction::ExtraSummon(a) => {
            json!({"type":"extra_summon","extra_piece_id":a.extra_piece_id,"sacrifice_piece_ids":a.sacrifice_piece_ids,"target_square":a.target_square})
        }
        TurnAction::Drop(a) => json!({"type":"drop","piece_id":a.piece_id,"to":a.to}),
        TurnAction::Ability(a) => {
            json!({"type":"ability","piece_id":a.piece_id,"ability_id":a.ability_id,"target_piece_id":a.target_piece_id,"target_piece_ids":a.target_piece_ids,"pocket_piece_id":a.pocket_piece_id,"to":a.to,"deployments":a.deployments})
        }
    };
    serde_json::from_value(json!({"action":action})).unwrap()
}

fn store(state: GameState, time: TimeControlId, now: i64) -> AppState {
    let app = AppState::in_memory();
    let mut stored = StoredGame::new(state, time, false, now);
    stored.access = GameAccess::Local {
        client: "controller".into(),
        human: None,
    };
    app.games.insert(stored.id.clone(), stored);
    app
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-game-client-id", "controller".parse().unwrap());
    headers
}

#[test]
fn initial_order_counts_instances_and_short_pockets() {
    for count in [0, 1, 2, 3, 4, 8] {
        let mut state = state_with_pockets(count, count);
        let before = state.clone();
        let mut bounds = Vec::new();
        let draws = initialize_automatic(&mut state, &mut |n| {
            bounds.push(n);
            Ok(n - 1)
        })
        .unwrap();
        assert_eq!(draws.len(), 3);
        assert_eq!(
            draws
                .iter()
                .map(|d| (&*d.player_id, d.timing.clone(), d.piece_ids.len()))
                .collect::<Vec<_>>(),
            vec![
                ("white", DrawTiming::Initial, count.min(3)),
                ("black", DrawTiming::Initial, count.min(3)),
                (
                    "white",
                    DrawTiming::TurnStart,
                    count.saturating_sub(3).min(1)
                )
            ]
        );
        for (side, n) in [("white", count.min(4)), ("black", count.min(3))] {
            let deck = &state.players[side].deck;
            assert_eq!(deck.hand_pieces.len(), n);
            assert_eq!(deck.pocket_pieces.len(), count - n);
            assert_eq!(deck.hand_pieces.iter().collect::<HashSet<_>>().len(), n);
            assert!(deck
                .hand_pieces
                .iter()
                .all(|id| before.players[side].deck.pocket_pieces.contains(id)));
            assert_eq!(
                deck.extra_deck_pieces,
                before.players[side].deck.extra_deck_pieces
            );
            assert_eq!(
                deck.starting_pieces,
                before.players[side].deck.starting_pieces
            );
        }
        assert_eq!(
            serde_json::to_value(&state.board).unwrap(),
            serde_json::to_value(&before.board).unwrap()
        );
        hand::validate_hand_zones(&state).unwrap();
        let mut repeated = before.clone();
        assert_eq!(
            initialize_automatic(&mut repeated, &mut |n| Ok(n - 1)).unwrap(),
            draws
        );
        assert_eq!(value(&repeated), value(&state));
        let mut replayed = before;
        for draw in &draws {
            apply_resolution(&mut replayed, draw).unwrap();
        }
        assert_eq!(value(&replayed), value(&state));
        if count == 8 {
            assert_eq!(bounds, [8, 7, 6, 8, 7, 6, 5]);
        }
    }
}

#[test]
fn rng_and_transition_failures_roll_back_entire_initial_and_multi_draw() {
    for fail_at in [0, 1, 4, 6] {
        let mut state = state_with_pockets(8, 8);
        let before = value(&state);
        let mut call = 0;
        assert!(initialize_automatic(&mut state, &mut |n| {
            call += 1;
            if call - 1 == fail_at {
                Ok(n)
            } else {
                Ok(0)
            }
        })
        .is_err());
        assert_eq!(value(&state), before);
        assert!(initialize_automatic(&mut state, &mut |_| Err("OS unavailable".into())).is_err());
        assert_eq!(value(&state), before);
    }
    let mut state = state_with_pockets(3, 3);
    let bad = state.players["white"].deck.pocket_pieces[1].clone();
    state.pieces.get_mut(&bad).unwrap().captured = true;
    let before = value(&state);
    assert!(initialize_automatic(&mut state, &mut |n| Ok(n - 1)).is_err());
    assert_eq!(value(&state), before);
    let mut state = state_with_pockets(3, 3);
    let id = state.players["white"].deck.pocket_pieces[0].clone();
    state.players.get_mut("white").unwrap().deck.pocket_pieces[1] = id;
    let before = value(&state);
    assert!(initialize_automatic(&mut state, &mut first).is_err());
    assert_eq!(value(&state), before);
}

#[test]
fn actual_turn_draws_and_exact_replay_require_valid_resolution() {
    let mut state = state_with_pockets(8, 8);
    initialize_automatic(&mut state, &mut first).unwrap();
    for turn in 0..5 {
        let before = state.clone();
        let action = TurnAction::Move(generate_legal_move_actions(&state)[0].clone());
        let canonical = submit_engine_action(state, action).unwrap();
        state = canonical.clone();
        let draws = turn_start(&before, &mut state, &mut first).unwrap();
        let side = if turn % 2 == 0 { "black" } else { "white" };
        assert_eq!(state.current_player, side);
        assert_eq!(
            state.players[side].deck.hand_pieces.len(),
            before.players[side].deck.hand_pieces.len() + 1
        );
        let mut replay = canonical.clone();
        replay_turn(&before, &mut replay, &draws).unwrap();
        assert_eq!(value(&state), value(&replay));
        let before_replay = value(&canonical);
        for bad in [
            vec![],
            vec![DrawResolution {
                player_id: before.current_player.clone(),
                ..draws[0].clone()
            }],
            vec![DrawResolution {
                piece_ids: vec!["missing".into()],
                ..draws[0].clone()
            }],
        ] {
            let mut replay = canonical.clone();
            assert!(replay_turn(&before, &mut replay, &bad).is_err());
            assert_eq!(value(&replay), before_replay);
        }
    }
    for count in [0, 1] {
        let before = state_with_pockets(0, count);
        let mut after = before.clone();
        after.current_player = "black".into();
        let draws = turn_start(&before, &mut after, &mut first).unwrap();
        assert_eq!(draws[0].piece_ids.len(), count);
        assert!(after.players["black"].deck.pocket_pieces.is_empty());
    }
}

#[test]
fn legacy_and_hypothetical_search_never_consume_draw_rng() {
    let mut legacy = state_with_pockets(8, 8);
    legacy.ruleset = DeckRuleset::Legacy;
    let before = legacy.clone();
    assert!(initialize_automatic(&mut legacy, &mut no_rng)
        .unwrap()
        .is_empty());
    assert_eq!(value(&legacy), value(&before));
    let drop = generate_legal_drop_actions(&legacy)[0].clone();
    let mut after = submit_engine_action(legacy, TurnAction::Drop(drop)).unwrap();
    assert!(turn_start(&before, &mut after, &mut no_rng)
        .unwrap()
        .is_empty());
    assert!(after
        .players
        .values()
        .all(|p| p.deck.hand_pieces.is_empty()));

    let mut state = state_with_pockets(8, 8);
    initialize_automatic(&mut state, &mut first).unwrap();
    let original = value(&state);
    let result =
        play_bot_turn_detailed(state.clone(), &"white".into(), BotDifficulty::Easy).unwrap();
    assert_eq!(value(&state), original);
    // Bot may use its Hand, but cannot consume either Pocket through turn Draw.
    for side in ["white", "black"] {
        assert_eq!(
            result.state.players[side].deck.pocket_pieces,
            state.players[side].deck.pocket_pieces
        );
    }
    assert_eq!(
        result.state.players["black"].deck.hand_pieces,
        state.players["black"].deck.hand_pieces
    );
}

#[test]
fn frozen_decks_and_playable_initial_state_are_distinct_and_sparse_for_legacy() {
    let state = state_with_pockets(8, 8);
    let mut game = StoredGame::new(state.clone(), TimeControlId::Unlimited, false, now_ms());
    let decks = serde_json::to_value(&game.record.decks).unwrap();
    game.initialize_draws().unwrap();
    assert_eq!(serde_json::to_value(&game.record.decks).unwrap(), decks);
    assert_eq!(
        game.record.decks["white"]
            .pocket
            .iter()
            .map(|p| p.count)
            .sum::<u32>(),
        8
    );
    assert_eq!(game.record.decks["black"].extra[0].piece_type_id, "bomber");
    assert_eq!(game.record.initial_draws.len(), 2);
    assert_eq!(
        game.record.initial_state.players["white"]
            .deck
            .hand_pieces
            .len(),
        3
    );
    assert_eq!(value(&game.record.initial_state), value(&game.state));
    let before = value(&game.state);
    assert!(game.initialize_draws().is_err());
    assert_eq!(value(&game.state), before);
    let mut empty = StoredGame::new(
        state_with_pockets(0, 0),
        TimeControlId::Unlimited,
        false,
        now_ms(),
    );
    empty.initialize_draws().unwrap();
    assert!(empty.initialize_draws().is_err());
    let mut legacy = state;
    legacy.ruleset = DeckRuleset::Legacy;
    for p in legacy.players.values_mut() {
        p.deck.extra_deck_pieces.clear();
    }
    let mut legacy = StoredGame::new(legacy, TimeControlId::Unlimited, false, now_ms());
    legacy.initialize_draws().unwrap();
    let record = serde_json::to_value(&legacy.record).unwrap();
    assert!(record.get("initial_draws").is_none());
    assert!(record["decks"]["white"].get("extra").is_none());
    assert!(validate_analysis_trees(&legacy.record, &[]).is_ok());
    assert!(validate_analysis_trees(&game.record, &[]).is_ok());
}

#[tokio::test]
async fn committed_actions_include_draw_in_record_and_reject_retry_invalid_and_unauthorized() {
    let mut game = StoredGame::new(
        state_with_pockets(8, 8),
        TimeControlId::Unlimited,
        false,
        now_ms(),
    );
    game.record.initial_draws = initialize_automatic(&mut game.state, &mut first).unwrap();
    game.record.initial_state = game.state.clone();
    game.record.ruleset_version = "deck-chess-standard-1".into();
    game.access = GameAccess::Local {
        client: "controller".into(),
        human: None,
    };
    let app = AppState::in_memory();
    app.games.insert(game.id.clone(), game);
    for _ in 0..3 {
        let before = app.games.get("draw-test").unwrap().state.clone();
        let action = TurnAction::Move(generate_legal_move_actions(&before)[0].clone());
        let denied = submit_action(
            State(app.clone()),
            HeaderMap::new(),
            Path("draw-test".into()),
            Json(request(action.clone())),
        )
        .await;
        assert_eq!(denied.unwrap_err().0, StatusCode::FORBIDDEN);
        let invalid = serde_json::from_value(
            json!({"action":{"type":"drop","piece_id":"missing","to":{"file":0,"rank":0}}}),
        )
        .unwrap();
        assert!(submit_action(
            State(app.clone()),
            headers(),
            Path("draw-test".into()),
            Json(invalid)
        )
        .await
        .is_err());
        assert_eq!(
            value(&app.games.get("draw-test").unwrap().state),
            value(&before)
        );
        let _ = submit_action(
            State(app.clone()),
            headers(),
            Path("draw-test".into()),
            Json(request(action.clone())),
        )
        .await
        .unwrap();
        let stored = app.games.get("draw-test").unwrap();
        let after = stored.state.clone();
        let record = stored.record.clone();
        drop(stored);
        assert_eq!(
            after.players[&after.current_player].deck.hand_pieces.len(),
            before.players[&after.current_player].deck.hand_pieces.len() + 1
        );
        assert_eq!(record.actions.last().unwrap().draws.len(), 1);
        let encoded = serde_json::to_vec(&record).unwrap();
        let decoded: game_record::GameRecord = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            analysis::state_hash(&decoded.state_at_ply(record.actions.len() as u32).unwrap())
                .unwrap(),
            analysis::state_hash(&after).unwrap()
        );
        let mut tampered = decoded;
        tampered.actions.last_mut().unwrap().draws[0].piece_ids[0] = "missing".into();
        assert!(tampered.state_at_ply(record.actions.len() as u32).is_err());
        assert!(submit_action(
            State(app.clone()),
            headers(),
            Path("draw-test".into()),
            Json(request(action))
        )
        .await
        .is_err());
        assert_eq!(
            value(&app.games.get("draw-test").unwrap().state),
            value(&after)
        );
    }
}

#[tokio::test]
async fn timeout_and_king_capture_do_not_start_draw() {
    let state = state_with_pockets(8, 8);
    let app = store(state.clone(), TimeControlId::FiveZero, now_ms() - 400_000);
    let action = TurnAction::Move(generate_legal_move_actions(&state)[0].clone());
    assert!(submit_action(
        State(app.clone()),
        headers(),
        Path(state.id.clone()),
        Json(request(action))
    )
    .await
    .is_err());
    let timed_out = app.games.get(&state.id).unwrap();
    assert_eq!(timed_out.phase, GamePhase::Ended);
    assert!(timed_out.record.actions.is_empty());
    for side in ["white", "black"] {
        assert_eq!(
            timed_out.players[side].deck.pocket_pieces,
            state.players[side].deck.pocket_pieces
        );
    }
    drop(timed_out);
    let mut state = state;
    add_board(&mut state, "killer", "white", "rook", Square::new(3, 6));
    let action = generate_piece_legal_move_actions(&state, &"killer".into())
        .into_iter()
        .find(|a| a.to == Square::new(3, 7))
        .unwrap();
    let app = store(state.clone(), TimeControlId::Unlimited, now_ms());
    let _ = submit_action(
        State(app.clone()),
        headers(),
        Path(state.id.clone()),
        Json(request(TurnAction::Move(action))),
    )
    .await
    .unwrap();
    let ended = app.games.get(&state.id).unwrap();
    assert_eq!(ended.phase, GamePhase::Ended);
    assert!(ended.record.actions[0].draws.is_empty());
    assert_eq!(
        ended.players["black"].deck.pocket_pieces,
        state.players["black"].deck.pocket_pieces
    );
    let mut after = ended.state.clone();
    assert!(turn_start(&state, &mut after, &mut no_rng)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn forced_landing_defers_draw_until_actual_player_change() {
    let mut state = state_with_pockets(8, 8);
    initialize_automatic(&mut state, &mut first).unwrap();
    add_board(&mut state, "bomber", "white", "bomber", Square::new(0, 5));
    state.board.squares.insert(Square::new(0, 5).to_id(), None);
    state
        .board
        .air_squares
        .insert(Square::new(0, 5).to_id(), Some("bomber".into()));
    let bomber = state.pieces.get_mut("bomber").unwrap();
    bomber.layer = PieceLayer::Air;
    bomber.remaining_flight_turns = 1;
    bomber
        .state
        .insert("airborne".into(), PieceStateValue::Boolean(true));
    let action = generate_piece_legal_move_actions(&state, &"bomber".into())
        .into_iter()
        .find(|a| a.to == Square::new(0, 2))
        .unwrap();
    let app = store(state.clone(), TimeControlId::Unlimited, now_ms());
    let _ = submit_action(
        State(app.clone()),
        headers(),
        Path(state.id.clone()),
        Json(request(TurnAction::Move(action))),
    )
    .await
    .unwrap();
    let pending = app.games.get(&state.id).unwrap().state.clone();
    assert_eq!(pending.current_player, "white");
    for side in ["white", "black"] {
        assert_eq!(
            pending.players[side].deck.hand_pieces,
            state.players[side].deck.hand_pieces
        );
    }
    let landing =
        generate_piece_legal_ability_actions(&pending, &"bomber".into(), "forced-landing")[0]
            .clone();
    let _ = submit_action(
        State(app.clone()),
        headers(),
        Path(state.id.clone()),
        Json(request(TurnAction::Ability(landing))),
    )
    .await
    .unwrap();
    let after = app.games.get(&state.id).unwrap();
    assert_eq!(after.current_player, "black");
    assert_eq!(after.players["black"].deck.hand_pieces.len(), 4);
    assert_eq!(after.players["white"].deck.hand_pieces.len(), 4);
    assert!(after.record.actions[0].draws.is_empty());
    assert_eq!(after.record.actions[1].draws.len(), 1);
}

#[test]
fn returned_pocket_piece_is_eligible_only_at_normal_draw_time() {
    let mut state = state_with_pockets(0, 0);
    add_board(&mut state, "camp", "white", "green-camp", Square::new(3, 3));
    add_board(&mut state, "returned", "black", "bishop", Square::new(4, 3));
    state
        .players
        .get_mut("black")
        .unwrap()
        .deck
        .starting_pieces
        .push("returned".into());
    let action = generate_piece_legal_ability_actions(&state, &"camp".into(), "recall")
        .into_iter()
        .find(|a| a.target_piece_id.as_ref().map(PieceId::as_str) == Some("returned"))
        .unwrap();
    let mut after = submit_engine_action(state.clone(), TurnAction::Ability(action)).unwrap();
    assert!(after.pieces["returned"].in_pocket);
    assert!(after.players["black"].deck.hand_pieces.is_empty());
    let draws = turn_start(&state, &mut after, &mut first).unwrap();
    assert_eq!(draws[0].piece_ids, vec!["returned".to_string()]);
    assert!(!after.players["black"]
        .deck
        .starting_pieces
        .contains(&"returned".into()));
    hand::validate_hand_zones(&after).unwrap();
}

#[tokio::test]
async fn failed_draw_cannot_commit_action_clock_or_record() {
    let mut state = state_with_pockets(8, 8);
    // Every possible selection fails the zone transition, regardless of OS RNG.
    for id in state.players["black"].deck.pocket_pieces.clone() {
        state.pieces.get_mut(&id).unwrap().captured = true;
    }
    let now = now_ms();
    let app = store(state.clone(), TimeControlId::FiveThree, now);
    let clock_before =
        serde_json::to_value(app.games.get(&state.id).unwrap().clock.snapshot(now, true)).unwrap();
    let action = TurnAction::Move(generate_legal_move_actions(&state)[0].clone());
    let response = submit_action(
        State(app.clone()),
        headers(),
        Path(state.id.clone()),
        Json(request(action)),
    )
    .await;
    assert_eq!(response.unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);
    let stored = app.games.get(&state.id).unwrap();
    assert_eq!(value(&stored.state), value(&state));
    assert!(stored.record.actions.is_empty());
    assert_eq!(
        serde_json::to_value(stored.clock.snapshot(now, true)).unwrap(),
        clock_before
    );
}

#[test]
fn turn_rng_failure_and_empty_turn_preserve_atomicity() {
    let before = state_with_pockets(8, 8);
    let mut after = before.clone();
    assert!(turn_start(&before, &mut after, &mut no_rng)
        .unwrap()
        .is_empty());
    after.current_player = "black".into();
    let canonical = value(&after);
    assert!(turn_start(&before, &mut after, &mut |n| Ok(n)).is_err());
    assert_eq!(value(&after), canonical);
    assert!(turn_start(&before, &mut after, &mut |_| Err("unavailable".into())).is_err());
    assert_eq!(value(&after), canonical);
    let before = state_with_pockets(0, 0);
    let mut after = before.clone();
    after.current_player = "black".into();
    assert_eq!(
        turn_start(&before, &mut after, &mut no_rng).unwrap()[0]
            .piece_ids
            .len(),
        0
    );
}

#[tokio::test]
async fn standard_analysis_endpoints_commit_and_reject_unresolved_pending_actions() {
    let app = AppState::in_memory();
    let mut game = StoredGame::new(
        state_with_pockets(8, 8),
        TimeControlId::Unlimited,
        false,
        now_ms(),
    );
    game.record.initial_draws = initialize_automatic(&mut game.state, &mut first).unwrap();
    game.record.initial_state = game.state.clone();
    game.record.ruleset_version = "deck-chess-standard-1".into();
    game.record.ownership = GameRecordOwnership {
        white_user_id: Some("analysis-owner".into()),
        black_user_id: None,
        persist: true,
    };
    game.record.ended_at_ms = Some(now_ms());
    app.game_records.save(&game.record).await.unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-user-id", "analysis-owner".parse().unwrap());
    let action = TurnAction::Move(generate_legal_move_actions(&game.state)[0].clone());
    let result = create_analysis_tree(
        State(app.clone()),
        Path(game.id.clone()),
        headers.clone(),
        Json(CreateAnalysisRequest {
            base_ply: 0,
            name: None,
            action: action.clone(),
            request_id: "unresolved".into(),
        }),
    )
    .await;
    let tree = result.unwrap().0;
    assert_eq!(tree.nodes[0].draws.len(), 1);
    let result = get_analysis_options(
        State(app.clone()),
        Path(game.id.clone()),
        headers.clone(),
        Json(AnalysisOptionsRequest {
            sacrifice_piece_ids: Vec::new(),
            position: AnalysisPosition {
                base_ply: 0,
                tree_id: None,
                node_id: None,
                pending_actions: vec![action],
            },
            piece_id: game.state.players["white"].deck.hand_pieces[0].to_string(),
            move_option_id: None,
        }),
    )
    .await;
    assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
    let result = list_analysis_trees(State(app.clone()), Path(game.id.clone()), headers).await;
    assert_eq!(result.unwrap().0.len(), 1);
    assert_eq!(
        app.analyses
            .list(&game.id, "analysis-owner")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(serde_json::from_value::<SubmitActionRequest>(
        json!({"action":{"type":"draw","seed":1,"piece_id":"chosen"}})
    )
    .is_err());
}
