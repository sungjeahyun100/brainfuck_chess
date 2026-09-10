use super::tests::{add_board, state_with_pockets};
use super::*;
use crate::analysis::InMemoryAnalysisRepository;
use crate::*;
use brainfuck_chess_engine::summon::generate_extra_summon_actions;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn game() -> StoredGame {
    let mut state = state_with_pockets(8, 8);
    for side in ["white", "black"] {
        for i in 0..3 {
            add_board(
                &mut state,
                &format!("{side}-s{i}"),
                side,
                "queen",
                Square::new(i, if side == "white" { 2 } else { 5 }),
            );
        }
    }
    let mut game = StoredGame::new(state, TimeControlId::Unlimited, false, now_ms());
    game.record.initial_draws = initialize(&mut game.state, &mut |_| Ok(0)).unwrap();
    game.record.initial_state = game.state.clone();
    game.access = GameAccess::Multiplayer {
        clients: [
            ("host".into(), "white".into()),
            ("guest".into(), "black".into()),
        ]
        .into_iter()
        .collect(),
    };
    game
}
fn headers(client: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-game-client-id", client.parse().unwrap());
    h.insert("x-user-id", "owner".parse().unwrap());
    h
}
fn canonical(state: &GameState) -> TurnAction {
    let side = &state.current_player;
    let ids = (0..3)
        .map(|i| format!("{side}-s{i}").into())
        .collect::<Vec<_>>();
    TurnAction::ExtraSummon(
        generate_extra_summon_actions(state, &state.players[side].deck.extra_deck_pieces[0], &ids)
            .unwrap()
            .remove(0),
    )
}
fn request(action: &TurnAction) -> SubmitActionRequest {
    let mut value = serde_json::to_value(action).unwrap();
    value.as_object_mut().unwrap().remove("player_id");
    serde_json::from_value(json!({"action":value})).unwrap()
}
#[tokio::test]
async fn multiplayer_atomic_submit_public_extra_and_exact_record() {
    let app = AppState::in_memory();
    let game = game();
    let id = game.id.clone();
    app.games.insert(id.clone(), game);
    for (side, client) in [("white", "host"), ("black", "guest")] {
        let before = app.games.get(&id).unwrap().state.clone();
        let action = canonical(&before);
        let clock =
            serde_json::to_value(app.games.get(&id).unwrap().clock.snapshot(0, true)).unwrap();
        let mut bad = action.clone();
        if let TurnAction::ExtraSummon(a) = &mut bad {
            a.sacrifice_piece_ids.push("black-s0".into());
        }
        assert!(submit_action(
            State(app.clone()),
            headers(client),
            Path(id.clone()),
            Json(request(&bad))
        )
        .await
        .is_err());
        assert_eq!(
            analysis::state_hash(&before),
            analysis::state_hash(&app.games.get(&id).unwrap().state)
        );
        assert_eq!(
            clock,
            serde_json::to_value(app.games.get(&id).unwrap().clock.snapshot(0, true)).unwrap()
        );
        assert!(submit_action(
            State(app.clone()),
            headers(if client == "host" { "guest" } else { "host" }),
            Path(id.clone()),
            Json(request(&action))
        )
        .await
        .is_err());
        let view = submit_action(
            State(app.clone()),
            headers(client),
            Path(id.clone()),
            Json(request(&action)),
        )
        .await
        .unwrap()
        .0;
        let wire = serde_json::to_value(view).unwrap();
        let opponent = if side == "white" { "black" } else { "white" };
        assert!(wire["players"][side]["deck"]
            .get("extra_deck_pieces")
            .is_none());
        for hidden in before.players[opponent]
            .deck
            .hand_pieces
            .iter()
            .chain(&before.players[opponent].deck.pocket_pieces)
        {
            assert!(wire["pieces"].get(hidden.as_str()).is_none());
        }
        let stored = app.games.get(&id).unwrap();
        let entry = stored.record.actions.last().unwrap();
        assert_eq!(
            serde_json::to_value(&entry.action).unwrap(),
            serde_json::to_value(&action).unwrap()
        );
        assert_eq!(entry.draws.len(), 1);
        assert_eq!(entry.draws[0].player_id, opponent);
        assert_eq!(
            stored.players[opponent].deck.hand_pieces.len(),
            before.players[opponent].deck.hand_pieces.len() + 1
        );
        assert_eq!(
            analysis::state_hash(
                &stored
                    .record
                    .state_at_ply(stored.record.actions.len().try_into().unwrap())
                    .unwrap()
            ),
            analysis::state_hash(&stored.state)
        );
        let mut bad = stored.record.clone();
        if let TurnAction::ExtraSummon(a) = &mut bad.actions.last_mut().unwrap().action {
            a.sacrifice_piece_ids[0] = "missing".into();
        }
        assert!(bad
            .state_at_ply(bad.actions.len().try_into().unwrap())
            .is_err());
    }
}

#[tokio::test]
async fn analysis_preview_no_rng_commit_once_exact_reload_and_tamper_rejection() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut app = AppState::in_memory();
    app.analyses = Arc::new(InMemoryAnalysisRepository::with_draw_source(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }));
    let mut record = game().record;
    record.ended_at_ms = Some(now_ms());
    record.ownership = GameRecordOwnership {
        white_user_id: Some("owner".into()),
        black_user_id: None,
        persist: true,
    };
    app.game_records.save(&record).await.unwrap();
    let parent = record.initial_state.clone();
    let action = canonical(&parent);
    let TurnAction::ExtraSummon(a) = &action else {
        unreachable!()
    };
    for _ in 0..3 {
        let response = get_analysis_options(
            State(app.clone()),
            Path(record.game_id.clone()),
            headers("host"),
            Json(AnalysisOptionsRequest {
                position: AnalysisPosition {
                    base_ply: 0,
                    tree_id: None,
                    node_id: None,
                    pending_actions: vec![],
                },
                piece_id: a.extra_piece_id.to_string(),
                move_option_id: None,
                sacrifice_piece_ids: a.sacrifice_piece_ids.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(!response.previews.is_empty());
        assert!(response
            .previews
            .iter()
            .all(|p| p.draw_pending && p.state_hash.is_none()));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let create = || {
        create_analysis_tree(
            State(app.clone()),
            Path(record.game_id.clone()),
            headers("host"),
            Json(CreateAnalysisRequest {
                base_ply: 0,
                name: None,
                action: action.clone(),
                request_id: "g5-idempotent".into(),
            }),
        )
    };
    let (a, b) = tokio::join!(create(), create());
    let tree = a.unwrap().0;
    assert_eq!(tree.id, b.unwrap().0.id);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let position = AnalysisPosition {
        base_ply: 0,
        tree_id: Some(tree.id.clone()),
        node_id: Some(tree.nodes[0].id.clone()),
        pending_actions: vec![],
    };
    let reloaded = list_analysis_trees(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers("host"),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(
        analysis::state_hash(&analysis_state(&record, &reloaded, &position).unwrap()),
        Ok(tree.nodes[0].state_hash.clone())
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    for mutation in 0..3 {
        let mut bad = tree.clone();
        if let TurnAction::ExtraSummon(a) = &mut bad.nodes[0].action {
            match mutation {
                0 => a.extra_piece_id = "missing".into(),
                1 => a.sacrifice_piece_ids.push(a.sacrifice_piece_ids[0].clone()),
                _ => a.target_square = Square::new(-1, 0),
            }
        };
        assert!(analysis_state(&record, &[bad], &position).is_err());
    }
    let child_action = canonical(&tree.nodes[0].state_after);
    let append = || {
        append_analysis_node(
            State(app.clone()),
            Path((record.game_id.clone(), tree.id.clone())),
            headers("host"),
            Json(AppendAnalysisRequest {
                parent_node_id: tree.nodes[0].id.clone(),
                action: child_action.clone(),
                expected_version: tree.version,
                request_id: "g5-child".into(),
            }),
        )
    };
    let (left, right) = tokio::join!(append(), append());
    let child = left.unwrap().0.node;
    assert_eq!(child.id, right.unwrap().0.node.id);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let trees = list_analysis_trees(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers("host"),
    )
    .await
    .unwrap()
    .0;
    let position = AnalysisPosition {
        base_ply: 0,
        tree_id: Some(tree.id.clone()),
        node_id: Some(child.id.clone()),
        pending_actions: vec![],
    };
    assert_eq!(
        analysis::state_hash(&analysis_state(&record, &trees, &position).unwrap()),
        Ok(child.state_hash)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn private_extra_projection_and_hand_sacrifice_reveal_only_used_pieces() {
    let mut game = game();
    let state = &mut game.state;
    let extra = state.players["white"].deck.extra_deck_pieces[0].clone();
    state.pieces.get_mut(&extra).unwrap().type_id = "guhang".into();
    let hands = state.players["white"].deck.hand_pieces.clone();
    for id in &hands {
        state.pieces.get_mut(id).unwrap().type_id = "queen".into();
    }
    let before = game_view::project_state(state, game_view::Audience::Player("black"));
    for side in ["white", "black"] {
        for id in &state.players[side].deck.extra_deck_pieces {
            assert_eq!(before.pieces.contains_key(id), side == "black");
        }
    }
    assert!(!before.pieces.contains_key(&hands[0]));
    let action = generate_extra_summon_actions(state, &extra, &hands[..3])
        .unwrap()
        .remove(0);
    let after = submit_engine_action(state.clone(), TurnAction::ExtraSummon(action)).unwrap();
    let view = game_view::project_state(&after, game_view::Audience::Player("black"));
    for id in &hands[..3] {
        assert!(view.pieces[id].captured);
    }
    assert!(!view.pieces.contains_key(&hands[3]));
    assert!(view.pieces[&extra].current_square.is_some());
    assert!(view.players["white"].deck.extra_deck_pieces.is_empty());
}

#[test]
fn summon_forced_landing_game_end_and_later_pocket_draw_contracts() {
    let mut parent = game().state;
    add_board(&mut parent, "air", "white", "bomber", Square::new(0, 4));
    parent
        .board
        .set_piece_at_layer(Square::new(0, 4), PieceLayer::Ground, None);
    parent
        .board
        .set_piece_at_layer(Square::new(0, 4), PieceLayer::Air, Some("air".into()));
    let p = parent.pieces.get_mut("air").unwrap();
    p.layer = PieceLayer::Air;
    p.remaining_flight_turns = 1;
    p.state
        .insert("airborne".into(), PieceStateValue::Boolean(true));
    let mut after = submit_engine_action(parent.clone(), canonical(&parent)).unwrap();
    assert_eq!(after.current_player, "white");
    assert!(
        turn_start(&parent, &mut after, &mut |_| panic!("mid-turn RNG"))
            .unwrap()
            .is_empty()
    );
    let landing =
        generate_piece_legal_ability_actions(&after, &"air".into(), "forced-landing").remove(0);
    let mut landed = submit_engine_action(after.clone(), TurnAction::Ability(landing)).unwrap();
    assert_eq!(
        turn_start(&after, &mut landed, &mut |_| Ok(0))
            .unwrap()
            .len(),
        1
    );

    let mut parent = game().state;
    let extra = parent.players["white"].deck.extra_deck_pieces[0].clone();
    parent
        .piece_definitions
        .get_mut("bomber")
        .unwrap()
        .can_capture_on_drop = true;
    add_board(
        &mut parent,
        "victim-king",
        "black",
        "king",
        Square::new(4, 0),
    );
    let mut action = canonical(&parent);
    if let TurnAction::ExtraSummon(a) = &mut action {
        a.target_square = Square::new(4, 0);
    }
    let mut ended = submit_engine_action(parent.clone(), action).unwrap();
    assert_eq!(ended.phase, GamePhase::Ended);
    assert!(
        turn_start(&parent, &mut ended, &mut |_| panic!("game-ending RNG"))
            .unwrap()
            .is_empty()
    );

    let mut parent = game().state;
    // The summoned instance can be recalled by the opponent and drawn normally.
    let mut action = canonical(&parent);
    if let TurnAction::ExtraSummon(a) = &mut action {
        a.target_square = Square::new(4, 0);
    }
    let mut summoned = submit_engine_action(parent.clone(), action).unwrap();
    add_board(
        &mut summoned,
        "camp",
        "black",
        "green-camp",
        Square::new(4, 1),
    );
    let recall = generate_piece_legal_ability_actions(&summoned, &"camp".into(), "recall")
        .into_iter()
        .find(|a| a.target_piece_id.as_ref() == Some(&extra))
        .unwrap();
    let mut returned = submit_engine_action(summoned.clone(), TurnAction::Ability(recall)).unwrap();
    assert!(returned.pieces[&extra].in_pocket);
    assert!(!returned.players["white"].deck.hand_pieces.contains(&extra));
    assert!(returned.players["white"].deck.extra_deck_pieces.is_empty());
    let draws = turn_start(&summoned, &mut returned, &mut |n| Ok(n - 1)).unwrap();
    assert_eq!(draws[0].piece_ids, vec![extra.clone()]);
    assert!(returned.players["white"].deck.hand_pieces.contains(&extra));
    assert!(returned.players["white"].deck.extra_deck_pieces.is_empty());
    assert!(
        brainfuck_chess_engine::legal_moves::generate_piece_legal_drop_actions(&returned, &extra)
            .len()
            > 0
    );
    // A normal enemy capture also never replenishes Extra membership.
    parent.current_player = "white".into();
    let mut action = canonical(&parent);
    if let TurnAction::ExtraSummon(a) = &mut action {
        a.target_square = Square::new(4, 0);
    }
    let mut summoned = submit_engine_action(parent, action).unwrap();
    add_board(
        &mut summoned,
        "rook-capturer",
        "black",
        "rook",
        Square::new(4, 3),
    );
    let capture = brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions(
        &summoned,
        &"rook-capturer".into(),
    )
    .into_iter()
    .find(|a| a.to == Square::new(4, 0))
    .unwrap();
    let captured = submit_engine_action(summoned, TurnAction::Move(capture)).unwrap();
    assert!(captured.pieces[&extra].captured);
    assert!(captured.players["white"].deck.extra_deck_pieces.is_empty());
}
