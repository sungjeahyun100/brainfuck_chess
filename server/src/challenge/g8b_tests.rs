use super::*;
use crate::*;
use brainfuck_chess_engine::ai::{generate_ai_actions, AiAction};
use brainfuck_chess_engine::rules::get_front_zone_squares_with_ruleset;
use serde_json::json;

fn fixture() -> ChallengeDefinition {
    let mut starting = vec![placement("king", 4, 0)];
    // Official placement changes classification only; every square is in Base.
    starting.extend(
        get_base_zone_squares_with_ruleset(&"white".into(), 9, DeckRuleset::Standard)
            .into_iter()
            .filter(|sq| *sq != Square::new(4, 0))
            .map(|sq| nonstandard_placement("paratrooper", sq.file, sq.rank)),
    );
    ChallengeDefinition {
        id: "test_only_standard",
        name: "Test fixture",
        description: "Test only",
        ruleset: DeckRuleset::Standard,
        map_id: "standard-9x9",
        board_size: 9,
        opponent_starting: starting,
        opponent_pocket: vec![OfficialPocket {
            piece_type: "knight",
            count: 8,
        }],
        opponent_extra: vec!["guhang", "bomber", "guhang"],
        bot_difficulty: BotDifficulty::Hard,
        time_control: TimeControlId::Unlimited,
        enabled: true,
    }
}
fn player(def: &ChallengeDefinition) -> serde_json::Value {
    let mut starting =
        vec![json!({"piece_type":"king", "square":{"file":def.board_size / 2,"rank":0}})];
    starting.extend(
        get_front_zone_squares_with_ruleset(&"white".into(), def.board_size, def.ruleset)
            .into_iter()
            .map(|sq| json!({"piece_type":"pawn", "square":sq})),
    );
    json!({"ruleset":def.ruleset,"map_id":def.map_id,"board_size":def.board_size,
        "starting":starting,"pocket":vec![json!({"piece_type":"knight"});8],"extra":if def.ruleset == DeckRuleset::Standard { vec!["guhang"] } else { vec![] }
            .into_iter().map(|id| json!({"piece_type":id})).collect::<Vec<_>>()})
}
fn headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-game-client-id", "g8b-controller".parse().unwrap());
    h
}
async fn create(
    app: &AppState,
    def: &ChallengeDefinition,
    deck: serde_json::Value,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    create_challenge_game_from_definition(
        app.clone(),
        headers(),
        serde_json::from_value(json!({"player_deck":deck})).unwrap(),
        def,
    )
    .await
}
fn request(action: AiAction) -> SubmitActionRequest {
    let mut value = serde_json::to_value(action).unwrap();
    value.as_object_mut().unwrap().remove("player_id");
    serde_json::from_value(json!({"action":value})).unwrap()
}

#[test]
fn public_legacy_content_golden() {
    let defs = definitions();
    assert_eq!(
        defs.iter().map(|d| d.id).collect::<Vec<_>>(),
        ["tempest_horde", "raining_men", "tempest_set"]
    );
    let expected = [
        (
            "템페스트 호드",
            12,
            "standard-12x12",
            BotDifficulty::Normal,
            vec![("king", 5, 0, false)]
                .into_iter()
                .chain((0..12).map(|f| ("tempest-pawn", f, 2, false)))
                .collect::<Vec<_>>(),
            vec![("tempest-pawn", 47)],
        ),
        (
            "사람비가 내려와",
            12,
            "standard-12x12",
            BotDifficulty::Normal,
            vec![("king", 5, 0, false), ("guhang", 6, 0, false)],
            vec![("paratrooper", 31)],
        ),
        (
            "템페스트 셋",
            10,
            "standard-10x10",
            BotDifficulty::Hard,
            vec![
                ("tempest-rook", 1, 0, false),
                ("tempest-knight", 2, 0, false),
                ("tempest-bishop", 3, 0, false),
                ("tempest-queen", 4, 0, false),
                ("king", 5, 0, false),
                ("tempest-bishop", 6, 0, false),
                ("tempest-knight", 7, 0, false),
                ("tempest-rook", 8, 0, false),
            ]
            .into_iter()
            .chain((0..10).map(|f| ("tempest-pawn", f, 1, true)))
            .collect(),
            vec![],
        ),
    ];
    for (d, (name, size, map, difficulty, starting, pocket)) in defs.iter().zip(expected) {
        assert_eq!(
            (d.name, d.board_size, d.map_id, d.bot_difficulty),
            (name, size, map, difficulty)
        );
        assert_eq!(d.ruleset, DeckRuleset::Legacy);
        assert_eq!(d.time_control, TimeControlId::Unlimited);
        assert!(d.enabled);
        assert!(d.opponent_extra.is_empty());
        assert_eq!(
            d.opponent_starting
                .iter()
                .map(|p| (
                    p.piece_type,
                    p.square.file,
                    p.square.rank,
                    p.allow_nonstandard_zone
                ))
                .collect::<Vec<_>>(),
            starting
        );
        assert_eq!(
            d.opponent_pocket
                .iter()
                .map(|p| (p.piece_type, p.count))
                .collect::<Vec<_>>(),
            pocket
        );
    }
    assert!(find("test_only_standard").is_none());
}

#[test]
fn registry_rejects_invalid_format_zones_and_official_placement() {
    let valid = fixture();
    validate_registry(&[valid.clone()]).unwrap();
    for change in 0..12 {
        let mut d = valid.clone();
        match change {
            0 => d.map_id = "missing",
            1 => d.board_size = 12,
            2 => d.id = "",
            3 => d.opponent_starting[1].square = d.opponent_starting[0].square,
            4 => d.opponent_starting[1].square = Square::new(0, 0),
            5 => d.opponent_starting[1].allow_nonstandard_zone = false,
            6 => d.opponent_starting[0].piece_type = "guhang",
            7 => d.opponent_pocket[0].piece_type = "king",
            8 => d.opponent_pocket[0].count = u32::MAX,
            9 => d.opponent_extra.push("bomber"),
            10 => d.opponent_extra = vec!["king"],
            _ => d.opponent_pocket[0].piece_type = "guhang",
        }
        assert!(validate_registry(&[d]).is_err(), "case {change}");
    }
    for value in ["unknown", "", "Standard"] {
        assert!(serde_json::from_value::<DeckRuleset>(json!(value)).is_err());
    }
}

#[tokio::test]
async fn player_format_validation_summary_and_legacy_initial_state() {
    let app = AppState::in_memory();
    let def = fixture();
    for field in ["ruleset", "map_id", "board_size", "starting"] {
        let mut deck = player(&def);
        deck[field] = match field {
            "ruleset" => json!("legacy"),
            "map_id" => json!("standard-12x12"),
            "board_size" => json!(12),
            _ => json!([]),
        };
        assert!(create(&app, &def, deck).await.is_err());
        assert!(app.games.is_empty());
    }
    let mut deck = player(&def);
    deck.as_object_mut().unwrap().remove("map_id");
    assert!(create(&app, &def, deck).await.is_err());
    for key in [
        "ruleset",
        "map_id",
        "board_size",
        "opponent_deck",
        "opponent_extra",
        "bot_difficulty",
        "result",
        "winner",
        "challenge_id",
    ] {
        let mut payload = json!({"player_deck":player(&def)});
        payload[key] = json!("override");
        assert!(
            serde_json::from_value::<CreateChallengeGameRequest>(payload).is_err(),
            "{key}"
        );
    }
    let mut payload = json!({"player_deck":player(&def)});
    payload["player_deck"]["hand"] = json!([]);
    assert!(serde_json::from_value::<CreateChallengeGameRequest>(payload).is_err());
    for def in definitions() {
        let mut deck = player(&def);
        for key in ["ruleset", "map_id", "board_size"] {
            deck.as_object_mut().unwrap().remove(key);
        }
        let response = create(&app, &def, deck).await.unwrap().0;
        let stored = app.games.get(&response.id).unwrap();
        assert_eq!(stored.state.ruleset, DeckRuleset::Legacy);
        assert_eq!(stored.record.ruleset_version, "deck-chess-1");
        assert!(stored.record.initial_draws.is_empty());
        for p in stored.state.players.values() {
            assert!(p.deck.hand_pieces.is_empty());
            assert!(p.deck.extra_deck_pieces.is_empty());
        }
    }
    let summaries = list_challenges(State(app), HeaderMap::new())
        .await
        .unwrap()
        .0;
    assert_eq!(summaries.len(), 3);
    for s in summaries {
        let d = find(s.id).unwrap();
        assert_eq!(s.ruleset, d.ruleset);
        assert_eq!(s.map_id, d.map_id);
    }
}

#[tokio::test]
async fn standard_production_creation_bot_summon_draw_privacy_and_exact_replay() {
    let app = AppState::in_memory();
    let def = fixture();
    let response = create(&app, &def, player(&def)).await.unwrap().0;
    let id = response.id;
    {
        let stored = app.games.get(&id).unwrap();
        assert_eq!(stored.record.game_mode, game_record::GameMode::Challenge);
        assert_eq!(stored.record.challenge_id.as_deref(), Some(def.id));
        assert_eq!(stored.record.ruleset_version, "deck-chess-standard-1");
        assert_eq!(
            stored
                .record
                .initial_draws
                .iter()
                .map(|d| d.piece_ids.len())
                .collect::<Vec<_>>(),
            [3, 3, 1]
        );
        assert_eq!(stored.state.players["white"].deck.hand_pieces.len(), 4);
        assert_eq!(stored.state.players["black"].deck.hand_pieces.len(), 3);
        let wire = serde_json::to_value(&response.state).unwrap().to_string();
        for hidden in stored.state.players["black"]
            .deck
            .hand_pieces
            .iter()
            .chain(&stored.state.players["black"].deck.pocket_pieces)
        {
            assert!(!wire.contains(hidden.as_str()));
        }
        for (side, p) in &stored.state.players {
            for extra in &p.deck.extra_deck_pieces {
                assert_eq!(wire.contains(extra.as_str()), side == "white");
            }
        }
        assert_eq!(stored.record.decks["black"].extra.len(), 3);
    }
    let action = generate_ai_actions(&app.games.get(&id).unwrap().state)
        .into_iter()
        .find(|a| matches!(a, AiAction::Drop(_)))
        .unwrap();
    let _ = submit_action(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(request(action)),
    )
    .await
    .unwrap();
    {
        let state = app.games.get(&id).unwrap().state.clone();
        assert_eq!(state.players["black"].deck.hand_pieces.len(), 4);
        let mut hidden = state.clone();
        for id in hidden.players["white"]
            .deck
            .hand_pieces
            .iter()
            .chain(&hidden.players["white"].deck.pocket_pieces)
        {
            hidden.pieces.get_mut(id).unwrap().type_id = "queen".into();
        }
        let actions = |s: &GameState| {
            let mut a = generate_ai_actions(s)
                .iter()
                .map(|a| serde_json::to_string(a).unwrap())
                .collect::<Vec<_>>();
            a.sort();
            a
        };
        assert_eq!(actions(&state), actions(&hidden));
        use brainfuck_chess_engine::ai::{
            choose_bot_action_with_limits_and_options, SearchLimits, SearchOptions,
        };
        let limits = SearchLimits {
            max_depth_actions: 1,
            max_nodes: 0,
            soft_time_ms: 60_000,
            hard_time_ms: 60_000,
        };
        let choose = |s: &GameState| {
            choose_bot_action_with_limits_and_options(
                s,
                &"black".into(),
                BotDifficulty::Hard,
                limits,
                SearchOptions::default(),
            )
            .unwrap()
        };
        let a = choose(&state);
        let b = choose(&hidden);
        assert_eq!(a.action, b.action);
        assert_eq!(a.score, b.score);
    }
    let bot = run_bot_turn(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(BotTurnRequest {
            bot_player_id: "black".into(),
            difficulty: Some("easy".into()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(bot.stats.is_none());
    assert!(
        bot.actions
            .iter()
            .any(|a| matches!(a, AiAction::ExtraSummon(_))),
        "{:?}",
        bot.actions
    );
    let final_action_state = app.games.get(&id).unwrap().state.clone();
    let _ = resign_game(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(ResignGameRequest {
            player_id: "white".into(),
        }),
    )
    .await
    .unwrap();
    let stored = app.games.get(&id).unwrap();
    let replay = completed_record_view(stored.record.clone()).unwrap().0;
    let ply = replay.actions.len() as u32;
    let analyzed = analysis_state(
        &replay,
        &[],
        &AnalysisPosition {
            base_ply: ply,
            tree_id: None,
            node_id: None,
            pending_actions: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        analysis::state_hash(&analyzed),
        analysis::state_hash(&replay.state_at_ply(ply).unwrap())
    );
    assert_eq!(stored.record.actions.last().unwrap().draws.len(), 1);
    assert_eq!(
        analysis::state_hash(
            &stored
                .record
                .state_at_ply(stored.record.actions.len() as u32)
                .unwrap()
        ),
        analysis::state_hash(&final_action_state)
    );
    let wire = serde_json::to_value(&bot).unwrap().to_string();
    for hidden in stored.state.players["black"]
        .deck
        .hand_pieces
        .iter()
        .chain(&stored.state.players["black"].deck.pocket_pieces)
    {
        assert!(!wire.contains(hidden.as_str()));
    }
}

#[tokio::test]
async fn completion_analysis_and_clear_keep_server_authority_for_both_formats() {
    for def in [fixture(), find("raining_men").unwrap()] {
        for winner in ["white", "black"] {
            let app = AppState::in_memory();
            let response = create(&app, &def, player(&def)).await.unwrap().0;
            let id = response.id;
            // Registered identity normally comes from the existing auth/profile resolver.
            app.games
                .get_mut(&id)
                .unwrap()
                .challenge
                .as_mut()
                .unwrap()
                .registered_user_id = Some("registered".into());
            let forged = resign_game(
                State(app.clone()),
                headers(),
                Path(id.clone()),
                Json(ResignGameRequest {
                    player_id: "black".into(),
                }),
            )
            .await;
            assert_eq!(forged.unwrap_err().0, StatusCode::FORBIDDEN);
            assert!(app.games.get(&id).unwrap().state.result.is_none());
            if winner == "black" {
                let _ = resign_game(
                    State(app.clone()),
                    headers(),
                    Path(id.clone()),
                    Json(ResignGameRequest {
                        player_id: "white".into(),
                    }),
                )
                .await
                .unwrap();
            } else {
                // Model a server adjudicated opponent loss, never a client-supplied result.
                app.games
                    .get_mut(&id)
                    .unwrap()
                    .end_with_loss("black", GameEndReason::Timeout);
            }
            let record = app.games.get(&id).unwrap().record.clone();
            let replay = completed_record_view(record.clone()).unwrap().0;
            assert_eq!(replay.challenge_id.as_deref(), Some(def.id));
            let restored = analysis_state(
                &replay,
                &[],
                &AnalysisPosition {
                    base_ply: 0,
                    tree_id: None,
                    node_id: None,
                    pending_actions: vec![],
                },
            )
            .unwrap();
            assert_eq!(
                analysis::state_hash(&restored),
                analysis::state_hash(&replay.initial_state)
            );
            if def.ruleset == DeckRuleset::Standard {
                assert_eq!(
                    replay.initial_state.players["black"].deck.hand_pieces.len(),
                    3
                );
                let wire = serde_json::to_value(&replay).unwrap().to_string();
                for id in &replay.initial_state.players["black"].deck.hand_pieces {
                    assert!(wire.contains(id.as_str()));
                }
            }
            persist_completed_record(&app.games, &app.game_records, &app.challenge_progress, &id)
                .await
                .unwrap();
            persist_completed_record(&app.games, &app.game_records, &app.challenge_progress, &id)
                .await
                .unwrap();
            let clears = app
                .challenge_progress
                .list_clears("registered")
                .await
                .unwrap();
            assert_eq!(clears.len(), usize::from(winner == "white"));
            if winner == "white" {
                assert_eq!(clears[0].challenge_id, def.id);
            }
        }
    }
}

#[tokio::test]
async fn explicit_high_ground_map_materializes_in_standard_challenge() {
    let mut def = fixture();
    def.map_id = "central-high-ground-12x12";
    def.board_size = 12;
    def.opponent_starting = vec![placement("king", 5, 0)];
    def.opponent_starting.extend(
        get_front_zone_squares_with_ruleset(&"white".into(), 12, DeckRuleset::Standard)
            .into_iter()
            .map(|sq| placement("pawn", sq.file, sq.rank)),
    );
    let app = AppState::in_memory();
    let response = create(&app, &def, player(&def)).await.unwrap().0;
    let game = app.games.get(&response.id).unwrap();
    assert!(!game.state.board.terrain.is_empty());
    for snapshot in game.record.decks.values() {
        assert_eq!(snapshot.map_id, def.map_id);
    }
}

#[tokio::test]
async fn standard_challenge_bot_can_drop_from_hand() {
    let mut def = fixture();
    def.opponent_starting = vec![placement("king", 4, 0)];
    def.opponent_starting.extend(
        get_front_zone_squares_with_ruleset(&"white".into(), 9, DeckRuleset::Standard)
            .into_iter()
            .map(|sq| placement("pawn", sq.file, sq.rank)),
    );
    let app = AppState::in_memory();
    let response = create(&app, &def, player(&def)).await.unwrap().0;
    let id = response.id;
    let action = generate_ai_actions(&app.games.get(&id).unwrap().state)
        .into_iter()
        .find(|a| matches!(a, AiAction::Drop(_)))
        .unwrap();
    let _ = submit_action(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(request(action)),
    )
    .await
    .unwrap();
    let before = app.games.get(&id).unwrap().state.clone();
    let bot = run_bot_turn(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(BotTurnRequest {
            bot_player_id: "black".into(),
            difficulty: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(bot.actions.iter().any(|a|matches!(a,AiAction::Drop(drop) if before.players["black"].deck.hand_pieces.contains(&drop.piece_id))));
}

#[tokio::test]
#[ignore = "requires TEST_ANALYSIS_DATABASE_URL for disposable G9 PostgreSQL"]
async fn postgres_g9_completed_challenge_persists_clear() {
    let url = std::env::var("TEST_ANALYSIS_DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    for def in [fixture(), find("raining_men").unwrap()] {
        let owner = format!("g9-clear-{}", Uuid::new_v4());
        sqlx::query("INSERT INTO shared.users (id) VALUES ($1)")
            .bind(&owner)
            .execute(&pool)
            .await
            .unwrap();
        let mut app = AppState::in_memory();
        app.challenge_progress = std::sync::Arc::new(PostgresChallengeProgressRepository::new(
            pool.clone(),
            crate::database::DataSchema::Test,
        ));
        let id = create(&app, &def, player(&def)).await.unwrap().0.id;
        {
            let mut game = app.games.get_mut(&id).unwrap();
            game.challenge.as_mut().unwrap().registered_user_id = Some(owner.clone());
            game.end_with_loss("black", GameEndReason::Timeout);
        }
        for _ in 0..2 {
            persist_completed_record(&app.games, &app.game_records, &app.challenge_progress, &id)
                .await
                .unwrap();
        }
        let reconnected = PostgresChallengeProgressRepository::new(
            sqlx::PgPool::connect(&url).await.unwrap(),
            crate::database::DataSchema::Test,
        );
        let clears = reconnected.list_clears(&owner).await.unwrap();
        assert_eq!(clears.len(), 1);
        assert_eq!(clears[0].challenge_id, def.id);
    }
}
