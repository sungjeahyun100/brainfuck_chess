use super::tests::{add_board, state_with_pockets};
use super::*;
use crate::analysis::{AnalysisNode, AnalysisRepository, InMemoryAnalysisRepository};
use crate::*;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn completed(white: usize, black: usize) -> game_record::GameRecord {
    let mut game = StoredGame::new(
        state_with_pockets(white, black),
        TimeControlId::Unlimited,
        false,
        now_ms(),
    );
    game.record.initial_draws =
        super::tests::initialize_automatic(&mut game.state, &mut |_| Ok(0)).unwrap();
    game.record.initial_state = game.state.clone();
    game.record.ended_at_ms = Some(now_ms());
    game.record.ownership = GameRecordOwnership {
        white_user_id: Some("owner".into()),
        black_user_id: None,
        persist: true,
    };
    game.record
}

#[tokio::test]
#[ignore = "requires TEST_ANALYSIS_DATABASE_URL for disposable G9 PostgreSQL"]
async fn postgres_g9_formats_and_challenge_clear_survive_reconnection() {
    use crate::challenge::{ChallengeProgressRepository, PostgresChallengeProgressRepository};
    use crate::deck::{DeckInput, DeckRepository, PostgresDeckRepository};
    use crate::game_record::GameRecordRepository;
    let url = std::env::var("TEST_ANALYSIS_DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let owner = format!("g9-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO shared.users (id) VALUES ($1)")
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    for schema in [database::DataSchema::Test, database::DataSchema::Prod] {
        let decks = PostgresDeckRepository::new(Some(pool.clone()), schema);
        let records = game_record::PostgresGameRecordRepository::new(pool.clone(), schema);
        let clears = PostgresChallengeProgressRepository::new(pool.clone(), schema);
        clears
            .record_clear(&owner, "raining_men", 123)
            .await
            .unwrap();
        for standard in [false, true] {
            let mut data = json!({"name":"G9 fixture","deckData":{
                "mapId":"standard-8x8","boardSize":8,"starting":[],
                "pocket":{"knight":8},"customPieces":[]}});
            if standard {
                data["deckData"]["ruleset"] = json!("standard");
                data["deckData"]["extra"] = json!(["bomber", "bomber"]);
            }
            let input: DeckInput = serde_json::from_value(data).unwrap();
            let saved = decks
                .create(&owner, &format!("g9-{standard}"), input)
                .await
                .unwrap();
            let mut record = completed(8, 8);
            record.game_id = Uuid::new_v4().to_string();
            record.initial_state.id = record.game_id.clone();
            record.ownership.white_user_id = Some(owner.clone());
            if !standard {
                record.initial_state.ruleset = brainfuck_chess_engine::types::DeckRuleset::Legacy;
                record.ruleset_version = game_record::LEGACY_RULES_VERSION.into();
                record.initial_draws.clear();
            }
            records.save(&record).await.unwrap();
            let fresh = sqlx::PgPool::connect(&url).await.unwrap();
            let loaded_deck = PostgresDeckRepository::new(Some(fresh.clone()), schema)
                .get(
                    &owner,
                    serde_json::to_value(&saved).unwrap()["id"]
                        .as_str()
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(&loaded_deck).unwrap(),
                serde_json::to_value(&saved).unwrap()
            );
            let loaded = game_record::PostgresGameRecordRepository::new(fresh.clone(), schema)
                .get(&record.game_id)
                .await
                .unwrap()
                .unwrap();
            loaded.validate_rules_version().unwrap();
            assert_eq!(
                json_state(&loaded.state_at_ply(0).unwrap()),
                json_state(&record.state_at_ply(0).unwrap())
            );
            if standard {
                assert_eq!(loaded.initial_draws, record.initial_draws);
                let mut development = loaded;
                development.ruleset_version = game_record::LEGACY_RULES_VERSION.into();
                records.save(&development).await.unwrap();
                let unsupported = records.get(&record.game_id).await.unwrap().unwrap();
                assert_eq!(
                    unsupported.validate_rules_version().unwrap_err(),
                    "unsupported_development_standard_record"
                );
            }
            let reconnected = PostgresChallengeProgressRepository::new(fresh, schema);
            reconnected
                .record_clear(&owner, "raining_men", 999)
                .await
                .unwrap();
            let loaded_clears = reconnected.list_clears(&owner).await.unwrap();
            assert_eq!(loaded_clears.len(), 1);
            assert_eq!(loaded_clears[0].challenge_id, "raining_men");
            assert_eq!(loaded_clears[0].first_cleared_at_ms, 123);
        }
    }
    // Keep this uniquely named fixture in the disposable DB for migration reapply checks.
}
fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-user-id", "owner".parse().unwrap());
    headers
}
fn action(state: &GameState) -> TurnAction {
    TurnAction::Move(generate_legal_move_actions(state)[0].clone())
}
fn prepared(parent: &GameState, action: TurnAction) -> AnalysisNode {
    let next = submit_engine_action(parent.clone(), action.clone()).unwrap();
    AnalysisNode {
        id: Uuid::new_v4().to_string(),
        parent_node_id: None,
        action,
        draws: vec![],
        state_hash: analysis::state_hash(&next).unwrap(),
        state_after: analysis::normalized_state(next),
        created_at_ms: now_ms(),
        request_id: Uuid::new_v4().to_string(),
    }
}
fn json_state(state: &GameState) -> Value {
    serde_json::to_value(state).unwrap()
}

#[test]
fn completed_initial_and_ordered_turns_restore_exact_hand_without_rng() {
    let mut record = completed(8, 8);
    let initial = record.state_at_ply(0).unwrap();
    assert_eq!(initial.players["white"].deck.hand_pieces.len(), 4);
    assert_eq!(initial.players["black"].deck.hand_pieces.len(), 3);
    let mut before = initial;
    for ply in 1..=5 {
        let action = action(&before);
        let mut after = submit_engine_action(before.clone(), action.clone()).unwrap();
        let draws = turn_start(&before, &mut after, &mut |upper| Ok(upper - 1)).unwrap();
        record
            .push_action(
                before.current_player.clone(),
                action,
                0,
                None,
                None,
                record.initial_clock.clone(),
                &before,
                after.clone(),
            )
            .draws = draws.clone();
        assert_eq!(
            analysis::state_hash(&record.state_at_ply(ply).unwrap()),
            analysis::state_hash(&after)
        );
        for id in &draws[0].piece_ids {
            assert_eq!(after.pieces[id].type_id, "knight");
            assert!(after.players[&draws[0].player_id]
                .deck
                .hand_pieces
                .contains(id));
        }
        if ply == 1 {
            assert_eq!(after.players["black"].deck.hand_pieces.len(), 4);
        }
        before = after;
    }
    let restored: game_record::GameRecord =
        serde_json::from_value(serde_json::to_value(&record).unwrap()).unwrap();
    assert_eq!(
        analysis::state_hash(&restored.state_at_ply(5).unwrap()),
        analysis::state_hash(&before)
    );
    let mut bad = record.clone();
    bad.initial_draws.clear();
    assert!(bad.state_at_ply(0).is_err());
    for index in 0..3 {
        let mut bad = record.clone();
        bad.initial_draws[index].player_id = "attacker".into();
        assert!(bad.state_at_ply(0).is_err());
        let mut bad = record.clone();
        bad.initial_draws[index].piece_ids[0] = "missing".into();
        assert!(bad.state_at_ply(0).is_err());
    }
    let mut bad = record.clone();
    bad.initial_draws.swap(0, 1);
    assert!(bad.state_at_ply(0).is_err());
    let mut bad = record.clone();
    bad.initial_state
        .players
        .get_mut("white")
        .unwrap()
        .deck
        .hand_pieces
        .reverse();
    assert!(bad.state_at_ply(0).is_err());
    for mutation in 0..6 {
        let mut bad = record.clone();
        match mutation {
            0 => bad.actions[0].draws.clear(),
            1 => bad.actions[0].draws[0].piece_ids[0] = "missing".into(),
            2 => bad.actions[0].draws[0].player_id = "white".into(),
            3 => bad.actions[0].draws[0].timing = DrawTiming::Initial,
            4 => bad.actions[0].draws[0].piece_ids.clear(),
            _ => bad.actions[0].state_delta.clear(),
        }
        assert!(bad.state_at_ply(1).is_err(), "mutation {mutation}");
    }
}

#[tokio::test]
async fn preview_commit_idempotency_children_and_reload_use_one_resolution() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut app = AppState::in_memory();
    app.analyses = Arc::new(InMemoryAnalysisRepository::with_draw_source(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }));
    let record = completed(8, 8);
    app.game_records.save(&record).await.unwrap();
    let parent = record.state_at_ply(0).unwrap();
    let canonical = action(&parent);
    let piece_id = match &canonical {
        TurnAction::Move(a) => a.piece_id.to_string(),
        _ => unreachable!(),
    };
    let mut previous = None;
    for _ in 0..3 {
        let response = get_analysis_options(
            State(app.clone()),
            Path(record.game_id.clone()),
            headers(),
            Json(AnalysisOptionsRequest {
                sacrifice_piece_ids: Vec::new(),
                position: AnalysisPosition {
                    base_ply: 0,
                    tree_id: None,
                    node_id: None,
                    pending_actions: vec![],
                },
                piece_id: piece_id.clone(),
                move_option_id: None,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(!response.previews.is_empty());
        for preview in &response.previews {
            assert!(preview.draw_pending);
            assert!(preview.state_hash.is_none());
            let mut state = serde_json::to_value(&parent).unwrap();
            // The preview still has Black's initial 3; no chosen PieceId appears as a Draw.
            for op in &preview.state_delta {
                if let game_record::StateDeltaOperation::Set { path, value } = op {
                    if path
                        == &vec![
                            "players".to_owned(),
                            "black".to_owned(),
                            "deck".to_owned(),
                            "hand_pieces".to_owned(),
                        ]
                    {
                        state["players"]["black"]["deck"]["hand_pieces"] = value.clone();
                    }
                }
            }
            assert_eq!(
                state["players"]["black"]["deck"]["hand_pieces"]
                    .as_array()
                    .unwrap()
                    .len(),
                3
            );
        }
        let value = serde_json::to_value(response).unwrap();
        if let Some(previous) = &previous {
            assert_eq!(&value, previous);
        }
        previous = Some(value);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let create = || {
        create_analysis_tree(
            State(app.clone()),
            Path(record.game_id.clone()),
            headers(),
            Json(CreateAnalysisRequest {
                base_ply: 0,
                name: None,
                action: canonical.clone(),
                request_id: "same-create".into(),
            }),
        )
    };
    let (left, right) = tokio::join!(create(), create());
    let tree = left.unwrap().0;
    assert_eq!(tree.id, right.unwrap().0.id);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        tree.nodes[0].draws[0].piece_ids,
        vec![parent.players["black"].deck.pocket_pieces[0].clone()]
    );
    assert_eq!(
        tree.nodes[0].state_after.players["black"]
            .deck
            .hand_pieces
            .len(),
        4
    );
    assert_eq!(
        tree.nodes[0].state_hash,
        analysis::state_hash(&tree.nodes[0].state_after).unwrap()
    );
    let child_action = action(&tree.nodes[0].state_after);
    let append = || {
        append_analysis_node(
            State(app.clone()),
            Path((record.game_id.clone(), tree.id.clone())),
            headers(),
            Json(AppendAnalysisRequest {
                parent_node_id: tree.nodes[0].id.clone(),
                action: child_action.clone(),
                expected_version: tree.version,
                request_id: "same-append".into(),
            }),
        )
    };
    let (left, right) = tokio::join!(append(), append());
    let child = left.unwrap().0;
    assert_eq!(child.node.id, right.unwrap().0.node.id);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let trees = list_analysis_trees(State(app.clone()), Path(record.game_id.clone()), headers())
        .await
        .unwrap()
        .0;
    let mut serialized = serde_json::to_value(&trees[0]).unwrap();
    // Internal request/owner fields are not public wire fields; SQL retains them separately.
    serialized["owner_user_id"] = json!("owner");
    serialized["request_id"] = json!("same-create");
    for node in serialized["nodes"].as_array_mut().unwrap() {
        node["request_id"] = json!("restored");
    }
    let restored: analysis::AnalysisTree = serde_json::from_value(serialized).unwrap();
    let restarted = InMemoryAnalysisRepository::with_draw_source(|_| panic!("reload consumed RNG"));
    restarted
        .create(restored, "same-create", None)
        .await
        .unwrap();
    let trees = restarted.list(&record.game_id, "owner").await.unwrap();
    validate_analysis_trees(&record, &trees).unwrap();
    let position = AnalysisPosition {
        base_ply: 0,
        tree_id: Some(tree.id),
        node_id: Some(child.node.id),
        pending_actions: vec![],
    };
    let state = analysis_state(&record, &trees, &position).unwrap();
    assert_eq!(analysis::state_hash(&state).unwrap(), child.node.state_hash);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn malformed_analysis_resolutions_and_hashes_are_rejected() {
    let record = completed(8, 8);
    let parent = record.state_at_ply(0).unwrap();
    let mut node = prepared(&parent, action(&parent));
    analysis::resolve_node(&mut node, Some(&parent), &mut |_| Ok(0)).unwrap();
    let mut tree = analysis::new_tree(
        record.game_id.clone(),
        "owner".into(),
        "Variation".into(),
        0,
        node.action.clone(),
        node.state_after.clone(),
        now_ms(),
        "request".into(),
    )
    .unwrap();
    tree.nodes = vec![node];
    validate_analysis_trees(&record, &[tree.clone()]).unwrap();
    for mutation in 0..9 {
        let mut bad = tree.clone();
        let node = &mut bad.nodes[0];
        match mutation {
            0 => node.draws.clear(),
            1 => node.draws[0].piece_ids[0] = "missing".into(),
            2 => node.draws[0].piece_ids[0] = parent.players["black"].deck.hand_pieces[0].clone(),
            3 => node.draws[0].player_id = "white".into(),
            4 => node.draws[0].timing = DrawTiming::Initial,
            5 => node.draws[0].piece_ids.clear(),
            6 => node.state_after.turn_number += 1,
            7 => node.state_hash = "sha256:old-bypass".into(),
            _ => node.parent_node_id = Some("missing".into()),
        }
        assert!(
            validate_analysis_trees(&record, &[bad]).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn analysis_forced_landing_endgame_empty_and_legacy_keep_transition_contracts() {
    let record = completed(8, 8);
    let mut state = record.initial_state;
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
    let flying = brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions(
        &state,
        &"bomber".into(),
    )
    .into_iter()
    .find(|a| a.to == Square::new(0, 2))
    .unwrap();
    let mut node = prepared(&state, TurnAction::Move(flying));
    assert!(!starts_turn(&state, &node.state_after));
    analysis::resolve_node(&mut node, Some(&state), &mut |_| {
        panic!("forced landing midpoint RNG")
    })
    .unwrap();
    assert!(node.draws.is_empty());
    let landing =
        generate_piece_legal_ability_actions(&node.state_after, &"bomber".into(), "forced-landing")
            [0]
        .clone();
    let mut landed = prepared(&node.state_after, TurnAction::Ability(landing));
    assert!(starts_turn(&node.state_after, &landed.state_after));
    analysis::resolve_node(&mut landed, Some(&node.state_after), &mut |_| Ok(0)).unwrap();
    assert_eq!(landed.draws.len(), 1);
    let mut invalid = node.clone();
    invalid.draws = landed.draws.clone();
    assert!(replay_analysis_node(state.clone(), &invalid).is_err());
    let empty = completed(0, 0).initial_state;
    let mut node = prepared(&empty, action(&empty));
    analysis::resolve_node(&mut node, Some(&empty), &mut |_| panic!("empty RNG")).unwrap();
    assert_eq!(node.draws.len(), 1);
    assert!(node.draws[0].piece_ids.is_empty());
    assert!(replay_analysis_node(empty.clone(), &node).is_ok());
    let mut legacy = state_with_pockets(1, 1);
    legacy.ruleset = DeckRuleset::Legacy;
    for p in legacy.players.values_mut() {
        p.deck.extra_deck_pieces.clear();
    }
    let mut node = prepared(&legacy, action(&legacy));
    analysis::resolve_node(&mut node, Some(&legacy), &mut |_| panic!("legacy RNG")).unwrap();
    let mut wire = serde_json::to_value(&node).unwrap();
    assert!(wire.get("draws").is_none());
    wire["request_id"] = json!("old");
    let old: AnalysisNode = serde_json::from_value(wire).unwrap();
    assert!(old.draws.is_empty());
    assert!(replay_analysis_node(legacy.clone(), &old).is_ok());
    // A rook capture of Black's king must never resolve a next-turn Draw.
    let mut end = completed(8, 8).initial_state;
    add_board(&mut end, "attacker", "white", "rook", Square::new(3, 6));
    let capture = brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions(
        &end,
        &"attacker".into(),
    )
    .into_iter()
    .find(|a| a.to == Square::new(3, 7))
    .unwrap();
    let mut node = prepared(&end, TurnAction::Move(capture));
    assert_eq!(node.state_after.phase, GamePhase::Ended);
    assert!(!starts_turn(&end, &node.state_after));
    analysis::resolve_node(&mut node, Some(&end), &mut |_| panic!("ended RNG")).unwrap();
    assert!(node.draws.is_empty());
    node.draws = landed.draws;
    assert!(replay_analysis_node(end.clone(), &node).is_err());
}

#[tokio::test]
async fn completed_record_publication_is_separate_from_live_privacy() {
    let app = AppState::in_memory();
    let record = completed(8, 8);
    let live = record.initial_state.clone();
    let projected = game_view::project_state(&live, game_view::Audience::Player("white"));
    let wire = serde_json::to_string(&projected).unwrap();
    for id in live.players["black"]
        .deck
        .hand_pieces
        .iter()
        .chain(&live.players["black"].deck.pocket_pieces)
    {
        assert!(!wire.contains(id.as_str()));
    }
    let published = completed_record_view(record.clone()).unwrap().0;
    assert_eq!(
        json_state(&published.state_at_ply(0).unwrap()),
        json_state(&live)
    );
    let mut unfinished = record.clone();
    unfinished.ended_at_ms = None;
    app.game_records.save(&unfinished).await.unwrap();
    assert!(
        get_game_record(State(app.clone()), Path(record.game_id.clone()), headers())
            .await
            .is_err()
    );
    assert!(get_analysis_options(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers(),
        Json(AnalysisOptionsRequest {
            sacrifice_piece_ids: Vec::new(),
            position: AnalysisPosition {
                base_ply: 0,
                tree_id: None,
                node_id: None,
                pending_actions: vec![]
            },
            piece_id: live.players["white"].deck.hand_pieces[0].to_string(),
            move_option_id: None
        })
    )
    .await
    .is_err());
    app.game_records.save(&record).await.unwrap();
    let published = get_game_record(State(app), Path(record.game_id.clone()), headers())
        .await
        .unwrap()
        .0;
    assert_eq!(published.initial_draws, record.initial_draws);
}

#[tokio::test]
async fn repository_draw_failure_does_not_create_or_advance_tree() {
    let record = completed(8, 8);
    let parent = record.initial_state;
    let node = prepared(&parent, action(&parent));
    let tree = analysis::new_tree(
        record.game_id,
        "owner".into(),
        "Variation".into(),
        0,
        node.action,
        node.state_after,
        now_ms(),
        "create".into(),
    )
    .unwrap();
    let repo = InMemoryAnalysisRepository::with_draw_source(|_| Err("entropy unavailable".into()));
    assert_eq!(
        repo.create(tree.clone(), "create", Some(&parent))
            .await
            .unwrap_err(),
        "draw_failed"
    );
    assert!(repo.list(&tree.game_id, "owner").await.unwrap().is_empty());
    // Store an exact root, then fail a child under the same repository lock.
    let mut stored = tree.clone();
    analysis::resolve_node(&mut stored.nodes[0], Some(&parent), &mut |_| Ok(0)).unwrap();
    let stored = repo.create(stored, "create", None).await.unwrap();
    let child_parent = &stored.nodes[0].state_after;
    let mut child = prepared(child_parent, action(child_parent));
    child.parent_node_id = Some(stored.nodes[0].id.clone());
    assert_eq!(
        repo.append(
            &stored.id,
            "owner",
            child,
            stored.version,
            "child",
            Some(child_parent)
        )
        .await
        .unwrap_err(),
        "draw_failed"
    );
    let reloaded = repo.list(&stored.game_id, "owner").await.unwrap();
    assert_eq!(reloaded[0].version, stored.version);
    assert_eq!(reloaded[0].nodes.len(), 1);
}

#[tokio::test]
#[ignore = "requires TEST_ANALYSIS_DATABASE_URL for a disposable database with current test releases and fixture-user"]
async fn postgres_analysis_draws_roundtrip_and_concurrent_idempotency() {
    use crate::game_record::GameRecordRepository;
    let url =
        std::env::var("TEST_ANALYSIS_DATABASE_URL").expect("disposable database URL required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    let records =
        game_record::PostgresGameRecordRepository::new(pool.clone(), database::DataSchema::Test);
    let repository =
        analysis::PostgresAnalysisRepository::new(pool.clone(), database::DataSchema::Test);
    let mut record = completed(8, 8);
    record.game_id = Uuid::new_v4().to_string();
    record.initial_state.id = record.game_id.clone();
    record.ownership.white_user_id = Some("fixture-user".into());
    records.save(&record).await.unwrap();
    let parent = &record.initial_state;
    let node = prepared(parent, action(parent));
    let tree = analysis::new_tree(
        record.game_id.clone(),
        "fixture-user".into(),
        "SQL Draw".into(),
        0,
        node.action,
        node.state_after,
        now_ms(),
        Uuid::new_v4().to_string(),
    )
    .unwrap();
    let (a, b) = tokio::join!(
        repository.create(tree.clone(), &tree.request_id, Some(parent)),
        repository.create(tree.clone(), &tree.request_id, Some(parent))
    );
    let stored = a.unwrap();
    assert_eq!(stored.id, b.unwrap().id);
    assert_eq!(stored.nodes[0].draws.len(), 1);
    let parent = &stored.nodes[0].state_after;
    let mut node = prepared(parent, action(parent));
    node.parent_node_id = Some(stored.nodes[0].id.clone());
    let request_id = node.request_id.clone();
    let (a, b) = tokio::join!(
        repository.append(
            &stored.id,
            "fixture-user",
            node.clone(),
            stored.version,
            &request_id,
            Some(parent)
        ),
        repository.append(
            &stored.id,
            "fixture-user",
            node,
            stored.version,
            &request_id,
            Some(parent)
        )
    );
    let appended = a.unwrap().unwrap();
    assert_eq!(appended.node.id, b.unwrap().unwrap().node.id);
    let reloaded =
        analysis::PostgresAnalysisRepository::new(pool.clone(), database::DataSchema::Test)
            .list(&record.game_id, "fixture-user")
            .await
            .unwrap();
    validate_analysis_trees(&record, &reloaded).unwrap();
    assert_eq!(reloaded[0].nodes.len(), 2);
    assert_eq!(
        reloaded[0]
            .nodes
            .iter()
            .find(|n| n.id == appended.node.id)
            .unwrap()
            .draws,
        appended.node.draws
    );
    sqlx::query("DELETE FROM test.game_records WHERE id=$1")
        .bind(&record.game_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires TEST_ANALYSIS_DATABASE_URL for a disposable database with current test releases and fixture-user"]
async fn postgres_g9_draws_roundtrip_serial_create_and_concurrent_append() {
    use crate::game_record::GameRecordRepository;
    let url =
        std::env::var("TEST_ANALYSIS_DATABASE_URL").expect("disposable database URL required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    let records =
        game_record::PostgresGameRecordRepository::new(pool.clone(), database::DataSchema::Test);
    let repository =
        analysis::PostgresAnalysisRepository::new(pool.clone(), database::DataSchema::Test);
    let mut record = completed(8, 8);
    record.game_id = Uuid::new_v4().to_string();
    record.initial_state.id = record.game_id.clone();
    record.ownership.white_user_id = Some("fixture-user".into());
    records.save(&record).await.unwrap();
    let parent = &record.initial_state;
    let node = prepared(parent, action(parent));
    let tree = analysis::new_tree(
        record.game_id.clone(),
        "fixture-user".into(),
        "SQL Draw".into(),
        0,
        node.action,
        node.state_after,
        now_ms(),
        Uuid::new_v4().to_string(),
    )
    .unwrap();
    let stored = repository
        .create(tree.clone(), &tree.request_id, Some(parent))
        .await
        .unwrap();
    let retry = repository
        .create(tree.clone(), &tree.request_id, Some(parent))
        .await
        .unwrap();
    assert_eq!(stored.id, retry.id);
    assert_eq!(stored.nodes[0].draws.len(), 1);
    let parent = &stored.nodes[0].state_after;
    let mut node = prepared(parent, action(parent));
    node.parent_node_id = Some(stored.nodes[0].id.clone());
    let request_id = node.request_id.clone();
    let (a, b) = tokio::join!(
        repository.append(
            &stored.id,
            "fixture-user",
            node.clone(),
            stored.version,
            &request_id,
            Some(parent)
        ),
        repository.append(
            &stored.id,
            "fixture-user",
            node,
            stored.version,
            &request_id,
            Some(parent)
        )
    );
    let appended = a.unwrap().unwrap();
    assert_eq!(appended.node.id, b.unwrap().unwrap().node.id);
    let reloaded =
        analysis::PostgresAnalysisRepository::new(pool.clone(), database::DataSchema::Test)
            .list(&record.game_id, "fixture-user")
            .await
            .unwrap();
    validate_analysis_trees(&record, &reloaded).unwrap();
    assert_eq!(reloaded[0].nodes.len(), 2);
    assert_eq!(
        reloaded[0]
            .nodes
            .iter()
            .find(|n| n.id == appended.node.id)
            .unwrap()
            .draws,
        appended.node.draws
    );
    sqlx::query("DELETE FROM test.game_records WHERE id=$1")
        .bind(&record.game_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn forced_landing_preview_and_commit_distinguish_exact_and_unresolved_states() {
    let mut record = completed(8, 8);
    let state = &mut record.initial_state;
    add_board(state, "bomber", "white", "bomber", Square::new(0, 5));
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
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let mut app = AppState::in_memory();
    app.analyses = Arc::new(InMemoryAnalysisRepository::with_draw_source(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }));
    app.game_records.save(&record).await.unwrap();
    let options = get_analysis_options(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers(),
        Json(AnalysisOptionsRequest {
            sacrifice_piece_ids: Vec::new(),
            position: AnalysisPosition {
                base_ply: 0,
                tree_id: None,
                node_id: None,
                pending_actions: vec![],
            },
            piece_id: "bomber".into(),
            move_option_id: None,
        }),
    )
    .await
    .unwrap()
    .0;
    let preview = options
        .previews
        .iter()
        .find(|p| matches!(&p.action, TurnAction::Move(a) if a.to == Square::new(0,2)))
        .unwrap();
    assert!(!preview.draw_pending);
    assert!(preview.state_hash.is_some());
    let expected_hash = preview.state_hash.clone().unwrap();
    let tree = create_analysis_tree(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers(),
        Json(CreateAnalysisRequest {
            base_ply: 0,
            name: None,
            action: preview.action.clone(),
            request_id: "flying".into(),
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(tree.nodes[0].draws.is_empty());
    assert_eq!(tree.nodes[0].state_hash, expected_hash);
    let options = get_analysis_options(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers(),
        Json(AnalysisOptionsRequest {
            sacrifice_piece_ids: Vec::new(),
            position: AnalysisPosition {
                base_ply: 0,
                tree_id: Some(tree.id.clone()),
                node_id: Some(tree.nodes[0].id.clone()),
                pending_actions: vec![],
            },
            piece_id: "bomber".into(),
            move_option_id: None,
        }),
    )
    .await
    .unwrap()
    .0;
    let landing = options
        .previews
        .iter()
        .find(|p| matches!(&p.action, TurnAction::Ability(a) if a.ability_id == "forced-landing"))
        .unwrap();
    assert!(landing.draw_pending);
    assert!(landing.state_hash.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let child = append_analysis_node(
        State(app.clone()),
        Path((record.game_id.clone(), tree.id)),
        headers(),
        Json(AppendAnalysisRequest {
            parent_node_id: tree.nodes[0].id.clone(),
            action: landing.action.clone(),
            expected_version: tree.version,
            request_id: "landed".into(),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(child.node.draws[0].player_id, "black");
    assert!(
        list_analysis_trees(State(app), Path(record.game_id), headers())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn empty_pocket_analysis_commit_persists_empty_resolution_without_rng() {
    let record = completed(0, 0);
    let mut app = AppState::in_memory();
    app.analyses = Arc::new(InMemoryAnalysisRepository::with_draw_source(|_| {
        panic!("empty Draw sampled RNG")
    }));
    app.game_records.save(&record).await.unwrap();
    let tree = create_analysis_tree(
        State(app.clone()),
        Path(record.game_id.clone()),
        headers(),
        Json(CreateAnalysisRequest {
            base_ply: 0,
            name: None,
            action: action(&record.initial_state),
            request_id: "empty".into(),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(tree.nodes[0].draws.len(), 1);
    assert!(tree.nodes[0].draws[0].piece_ids.is_empty());
    let trees = list_analysis_trees(State(app), Path(record.game_id), headers())
        .await
        .unwrap()
        .0;
    assert_eq!(trees[0].nodes[0].draws, tree.nodes[0].draws);
}

#[tokio::test]
async fn g7_semantic_versions_gate_replay_and_all_analysis_before_execution() {
    use crate::game_record::{
        current_rules_version_for, LEGACY_RULES_VERSION, STANDARD_RULES_VERSION,
    };
    let mut app = AppState::in_memory();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    app.analyses = Arc::new(InMemoryAnalysisRepository::with_draw_source(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        panic!("unsupported version must never consume RNG")
    }));
    let stable = completed(8, 8);
    assert_eq!(stable.ruleset_version, STANDARD_RULES_VERSION);
    assert_eq!(stable.format_version, 2);
    assert_eq!(
        current_rules_version_for(DeckRuleset::Legacy),
        LEGACY_RULES_VERSION
    );
    let restored: game_record::GameRecord =
        serde_json::from_value(serde_json::to_value(&stable).unwrap()).unwrap();
    assert_eq!(restored.ruleset_version, STANDARD_RULES_VERSION);
    assert!(completed_record_view(restored).is_ok());
    let canonical = action(&stable.initial_state);
    for (ruleset, version, expected) in [
        (
            DeckRuleset::Standard,
            LEGACY_RULES_VERSION,
            "unsupported_development_standard_record",
        ),
        (
            DeckRuleset::Standard,
            "deck-chess-standard-3",
            "unsupported_rules_version",
        ),
        (
            DeckRuleset::Legacy,
            STANDARD_RULES_VERSION,
            "unsupported_rules_version",
        ),
        (DeckRuleset::Legacy, "future", "unsupported_rules_version"),
    ] {
        let mut bad = stable.clone();
        bad.initial_state.ruleset = ruleset;
        bad.ruleset_version = version.into();
        // Invalid initial Draw and ply would fail differently if version validation were late.
        bad.initial_draws.clear();
        assert_eq!(bad.state_at_ply(u32::MAX).unwrap_err(), expected);
        assert_eq!(
            completed_record_view(bad.clone()).unwrap_err().1.error,
            expected
        );
        assert_eq!(
            validate_analysis_trees(&bad, &[]).unwrap_err().1.error,
            expected
        );
        let position = || AnalysisPosition {
            base_ply: 0,
            tree_id: None,
            node_id: None,
            pending_actions: vec![],
        };
        assert_eq!(
            analysis_state(&bad, &[], &position()).unwrap_err().1.error,
            expected
        );
        app.game_records.save(&bad).await.unwrap();
        let options = get_analysis_options(
            State(app.clone()),
            Path(bad.game_id.clone()),
            headers(),
            Json(AnalysisOptionsRequest {
                position: position(),
                piece_id: "missing".into(),
                move_option_id: None,
                sacrifice_piece_ids: vec![],
            }),
        )
        .await
        .err()
        .expect("unsupported record must be rejected");
        assert_eq!(options.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(options.1.error, expected);
        let create = create_analysis_tree(
            State(app.clone()),
            Path(bad.game_id.clone()),
            headers(),
            Json(CreateAnalysisRequest {
                base_ply: 0,
                name: None,
                action: canonical.clone(),
                request_id: "g7-create".into(),
            }),
        )
        .await
        .err()
        .expect("unsupported record must be rejected");
        assert_eq!(create.1.error, expected);
        let append = append_analysis_node(
            State(app.clone()),
            Path((bad.game_id.clone(), "missing-tree".into())),
            headers(),
            Json(AppendAnalysisRequest {
                parent_node_id: "missing-node".into(),
                action: canonical.clone(),
                expected_version: 1,
                request_id: "g7-append".into(),
            }),
        )
        .await
        .err()
        .expect("unsupported record must be rejected");
        assert_eq!(append.1.error, expected);
        assert!(app
            .analyses
            .list(&bad.game_id, "owner")
            .await
            .unwrap()
            .is_empty());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
#[ignore = "requires TEST_ANALYSIS_DATABASE_URL for disposable G9-DB-01 PostgreSQL"]
async fn postgres_g9_db01_create_identity_rng_atomicity_and_lock_scope() {
    use crate::analysis::{AnalysisTree, PostgresAnalysisRepository};
    use crate::game_record::GameRecordRepository;
    use tokio::sync::Barrier;
    use tokio::time::{timeout, Duration};

    async fn concurrent(
        repository: Arc<PostgresAnalysisRepository>,
        trees: Vec<AnalysisTree>,
        parent: &GameState,
    ) -> Vec<AnalysisTree> {
        let barrier = Arc::new(Barrier::new(trees.len()));
        let mut tasks = tokio::task::JoinSet::new();
        for tree in trees {
            let repository = repository.clone();
            let barrier = barrier.clone();
            let parent = parent.clone();
            tasks.spawn(async move {
                barrier.wait().await;
                repository
                    .create(tree.clone(), &tree.request_id, Some(&parent))
                    .await
                    .unwrap()
            });
        }
        let mut results = Vec::new();
        while let Some(result) = timeout(Duration::from_secs(10), tasks.join_next())
            .await
            .unwrap()
        {
            results.push(result.unwrap());
        }
        results
    }

    fn same_result(left: &AnalysisTree, right: &AnalysisTree) {
        assert_eq!(
            serde_json::to_value(left).unwrap(),
            serde_json::to_value(right).unwrap()
        );
        assert_eq!(left.owner_user_id, right.owner_user_id);
        assert_eq!(left.request_id, right.request_id);
        assert_eq!(left.nodes[0].request_id, right.nodes[0].request_id);
        assert_eq!(left.nodes[0].draws, right.nodes[0].draws);
    }

    let url = std::env::var("TEST_ANALYSIS_DATABASE_URL").unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect(&url)
        .await
        .unwrap();
    for schema in [database::DataSchema::Test, database::DataSchema::Prod] {
        let owners = [
            format!("db01-a-{}", Uuid::new_v4()),
            format!("db01-b-{}", Uuid::new_v4()),
        ];
        for owner in &owners {
            sqlx::query("INSERT INTO shared.users (id) VALUES ($1)")
                .bind(owner)
                .execute(&pool)
                .await
                .unwrap();
        }
        let mut record = completed(8, 8);
        record.game_id = Uuid::new_v4().to_string();
        record.initial_state.id = record.game_id.clone();
        record.ownership.white_user_id = Some(owners[0].clone());
        game_record::PostgresGameRecordRepository::new(pool.clone(), schema)
            .save(&record)
            .await
            .unwrap();
        let parent = &record.initial_state;
        let make = |owner: &str, request: &str| {
            let node = prepared(parent, action(parent));
            analysis::new_tree(
                record.game_id.clone(),
                owner.into(),
                "DB-01".into(),
                0,
                node.action,
                node.state_after,
                now_ms(),
                request.into(),
            )
            .unwrap()
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let repository = Arc::new(
            PostgresAnalysisRepository::new(pool.clone(), schema).with_draw_source(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(0)
            }),
        );
        let table = schema.table("game_analysis_trees");
        let nodes = schema.table("game_analysis_nodes");

        // Same ID, actual parallel tasks, one committed Draw across all calls.
        let candidate = make(&owners[0], "same-id");
        let results = concurrent(repository.clone(), vec![candidate.clone(); 8], parent).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        for result in &results {
            same_result(&results[0], result);
        }
        assert_eq!(results[0].nodes[0].draws.len(), 1);
        assert_eq!(results[0].nodes[0].draws[0].piece_ids.len(), 1);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE owner_user_id=$1 AND request_id=$2"
        ))
        .bind(&owners[0])
        .bind("same-id")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {nodes} WHERE analysis_tree_id=$1"
        ))
        .bind(&candidate.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        // HTTP retries construct different tree/node IDs for the same key.
        let alternatives: Vec<_> = (0..8).map(|_| make(&owners[0], "different-ids")).collect();
        let ids: Vec<_> = alternatives.iter().map(|t| t.id.clone()).collect();
        let results = concurrent(repository.clone(), alternatives, parent).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(ids.contains(&results[0].id));
        for result in &results {
            same_result(&results[0], result);
        }
        let authoritative = results[0].clone();
        let retry = make(&owners[0], "different-ids");
        let sequential = repository
            .create(retry, "different-ids", Some(parent))
            .await
            .unwrap();
        same_result(&authoritative, &sequential);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE owner_user_id=$1 AND request_id=$2"
        ))
        .bind(&owners[0])
        .bind("different-ids")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        let distinct = concurrent(
            repository.clone(),
            vec![
                make(&owners[0], "independent-1"),
                make(&owners[0], "independent-2"),
            ],
            parent,
        )
        .await;
        assert_ne!(distinct[0].id, distinct[1].id);
        let distinct = concurrent(
            repository.clone(),
            vec![
                make(&owners[0], "shared-text"),
                make(&owners[1], "shared-text"),
            ],
            parent,
        )
        .await;
        assert_ne!(distinct[0].id, distinct[1].id);
        assert_ne!(distinct[0].owner_user_id, distinct[1].owner_user_id);
        assert_eq!(calls.load(Ordering::SeqCst), 6);

        // PK alone never authorizes an idempotent return, even for the same owner.
        for owner in &owners {
            let mut collision = make(owner, "pk-collision");
            collision.id = authoritative.id.clone();
            assert_eq!(
                repository
                    .create(collision, "pk-collision", Some(parent))
                    .await
                    .unwrap_err(),
                "conflict"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 6);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE game_id=$1 AND request_id='pk-collision'"
        ))
        .bind(&record.game_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);

        // SQL failure after Draw rolls back the reserved tree and node metadata.
        let mut invalid = make(&owners[0], "failed-node");
        invalid.nodes[0].parent_node_id = Some("missing-parent".into());
        assert!(repository
            .create(invalid.clone(), "failed-node", Some(parent))
            .await
            .is_err());
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE id=$1"))
            .bind(&invalid.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {nodes} WHERE analysis_tree_id=$1"
        ))
        .bind(&invalid.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        invalid.nodes[0].parent_node_id = None;
        assert_eq!(
            repository
                .create(invalid, "failed-node", Some(parent))
                .await
                .unwrap()
                .version,
            1
        );
        assert_eq!(calls.load(Ordering::SeqCst), 8); // failed entropy is not rewindable

        // A later node error also rolls back earlier inserts in the transaction.
        let mut partial = make(&owners[0], "partial");
        partial.nodes.push(partial.nodes[0].clone());
        assert!(repository
            .create(partial.clone(), "partial", None)
            .await
            .is_err());
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE id=$1"))
            .bind(&partial.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {nodes} WHERE analysis_tree_id=$1"
        ))
        .bind(&partial.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);

        let failures = Arc::new(AtomicUsize::new(0));
        let counted = failures.clone();
        let failing =
            PostgresAnalysisRepository::new(pool.clone(), schema).with_draw_source(move |_| {
                counted.fetch_add(1, Ordering::SeqCst);
                Err("injected RNG failure".into())
            });
        let rng_failed = make(&owners[0], "rng-failed");
        assert_eq!(
            failing
                .create(rng_failed.clone(), "rng-failed", Some(parent))
                .await
                .unwrap_err(),
            "draw_failed"
        );
        assert_eq!(failures.load(Ordering::SeqCst), 1);
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE id=$1"))
            .bind(&rng_failed.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Hold one exact key and observe the waiting backend, not a timing sleep.
        let key =
            serde_json::to_string(&["analysis-create-v1", &table, &owners[0], "held"]).unwrap();
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(&key)
            .execute(&mut *held)
            .await
            .unwrap();
        let blocked_tree = make(&owners[0], "held");
        let blocked_repo = repository.clone();
        let blocked_parent = parent.clone();
        let blocked = tokio::spawn(async move {
            blocked_repo
                .create(blocked_tree, "held", Some(&blocked_parent))
                .await
        });
        timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND classid=((hashtextextended($1,0) >> 32) & 4294967295)::oid AND objid=(hashtextextended($1,0) & 4294967295)::oid AND objsubid=1)")
                    .bind(&key).fetch_one(&pool).await.unwrap();
                if waiting { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("same request must wait for its transaction lock");
        assert!(!blocked.is_finished());
        assert_eq!(calls.load(Ordering::SeqCst), 8); // RNG must not run before lock
                                                     // Different request AND different owner with identical text must finish
                                                     // while the first key is still locked, so no global/per-owner mutex fits.
        timeout(
            Duration::from_secs(5),
            concurrent(
                repository.clone(),
                vec![make(&owners[0], "unblocked"), make(&owners[1], "held")],
                parent,
            ),
        )
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 10);
        assert!(!blocked.is_finished());
        held.commit().await.unwrap();
        timeout(Duration::from_secs(5), blocked)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 11);

        // Reload/retry with a new single-connection repository must not acquire
        // a second pool connection or consume RNG when an exact key exists.
        let fresh_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let fresh = PostgresAnalysisRepository::new(fresh_pool, schema)
            .with_draw_source(|_| panic!("retry must reuse stored Draw"));
        let retry = make(&owners[0], "different-ids");
        let reloaded = timeout(
            Duration::from_secs(5),
            fresh.create(retry, "different-ids", Some(parent)),
        )
        .await
        .unwrap()
        .unwrap();
        same_result(&authoritative, &reloaded);
        for owner in &owners {
            let trees = fresh.list(&record.game_id, owner).await.unwrap();
            validate_analysis_trees(&record, &trees).unwrap();
            assert!(trees
                .iter()
                .all(|tree| tree.version == 1 && tree.nodes.len() == 1));
        }
        // Remove only this test's unique fixture in the disposable database.
        sqlx::query(&format!(
            "DELETE FROM {} WHERE id=$1",
            schema.table("game_records")
        ))
        .bind(&record.game_id)
        .execute(&pool)
        .await
        .unwrap();
        for owner in &owners {
            sqlx::query("DELETE FROM shared.users WHERE id=$1")
                .bind(owner)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
}
