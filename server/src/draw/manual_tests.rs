use super::*;
use crate::*;
use brainfuck_chess_engine::actions::{draw_required, prepare_draw};
use serde_json::json;

fn game() -> StoredGame {
    let mut game = StoredGame::new(
        super::tests::state_with_pockets(8, 8),
        TimeControlId::FiveThree,
        false,
        now_ms(),
    );
    game.initialize_draws().unwrap();
    game.access = GameAccess::Multiplayer {
        clients: [
            ("white-client".into(), "white".into()),
            ("black-client".into(), "black".into()),
        ]
        .into(),
    };
    game
}
fn headers(side: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-game-client-id",
        format!("{side}-client").parse().unwrap(),
    );
    headers
}
fn request(value: serde_json::Value) -> SubmitActionRequest {
    serde_json::from_value(json!({"action": value})).unwrap()
}
async fn send(
    app: &AppState,
    side: &str,
    value: serde_json::Value,
) -> Result<Json<time_control::GameView>, (StatusCode, Json<ErrorResponse>)> {
    submit_action(
        State(app.clone()),
        headers(side),
        Path("draw-test".into()),
        Json(request(value)),
    )
    .await
}

#[tokio::test]
async fn manual_draw_authority_duplicates_phase_clock_and_exact_replay() {
    let app = AppState::in_memory();
    let game = game();
    assert!(draw_required(&game.state));
    assert_eq!(game.players["white"].deck.hand_pieces.len(), 3);
    let before = game.state.clone();
    app.games.insert(game.id.clone(), game);
    let move_action = generate_legal_move_actions(&before)[0].clone();
    for intent in [
        json!({"type":"move","piece_id":move_action.piece_id,"to":move_action.to}),
        json!({"type":"drop","piece_id":before.players["white"].deck.hand_pieces[0],"to":{"file":0,"rank":0}}),
        json!({"type":"ability","piece_id":move_action.piece_id,"ability_id":"takeoff"}),
        json!({"type":"extra_summon","extra_piece_id":before.players["white"].deck.extra_deck_pieces[0],"sacrifice_piece_ids":[],"target_square":{"file":0,"rank":0}}),
    ] {
        assert_eq!(
            send(&app, "white", intent).await.unwrap_err().1.error,
            "DRAW_REQUIRED"
        );
    }
    assert!(
        serde_json::from_value::<SubmitActionRequest>(json!({"action":{"type":"end_turn"}}))
            .is_err()
    );
    for field in ["piece_id", "piece_ids", "player_id", "draw_required"] {
        let mut intent = json!({"type":"draw","turn_number":1});
        intent[field] = json!("forged");
        assert!(serde_json::from_value::<SubmitActionRequest>(json!({"action":intent})).is_err());
    }
    let intent = json!({"type":"draw","turn_number":1});
    assert_eq!(
        send(&app, "black", intent.clone()).await.unwrap_err().0,
        StatusCode::FORBIDDEN
    );
    let (a, b) = tokio::join!(
        send(&app, "white", intent.clone()),
        send(&app, "white", intent.clone())
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let state = {
        let game = app.games.get("draw-test").unwrap();
        assert_eq!(game.players["white"].deck.hand_pieces.len(), 4);
        assert_eq!(game.current_player, "white");
        assert_eq!(game.turn_number, 1);
        assert!(!draw_required(&game.state));
        assert_eq!(game.history.len(), 1);
        assert_eq!(game.record.actions.len(), 1);
        assert_eq!(game.record.actions[0].draws[0].piece_ids.len(), 1);
        let replay = game.record.state_at_ply(1).unwrap();
        assert_eq!(
            analysis::state_hash(&replay),
            analysis::state_hash(&game.state)
        );
        assert!(
            game.clock
                .snapshot(now_ms(), true)
                .white_remaining_ms
                .unwrap()
                <= 300_000
        );
        let hidden = game_view::project_state(&game.state, game_view::Audience::Player("black"));
        for id in game.players["white"]
            .deck
            .hand_pieces
            .iter()
            .chain(&game.players["white"].deck.pocket_pieces)
        {
            assert!(!hidden.pieces.contains_key(id));
        }
        game.state.clone()
    };
    let mut replay = submit_engine_action(
        before.clone(),
        TurnAction::Draw(DrawAction {
            player_id: "white".into(),
            turn_number: 1,
        }),
    )
    .unwrap();
    assert!(replay_turn(&before, &mut replay, &[]).is_err());
    let move_action = generate_legal_move_actions(&state)[0].clone();
    send(&app,"white",json!({"type":"move","piece_id":move_action.piece_id,"to":move_action.to,"move_option_id":move_action.move_option_id})).await.unwrap();
    let game = app.games.get("draw-test").unwrap();
    assert!(draw_required(&game.state));
    assert_eq!(game.players["black"].deck.hand_pieces.len(), 3);
    assert!(game.record.actions[1].draws.is_empty());
    assert_eq!(
        analysis::state_hash(&game.record.state_at_ply(2).unwrap()),
        analysis::state_hash(&game.state)
    );
    drop(game);
    assert!(send(&app, "black", intent).await.is_err()); // stale turn number
    let mut game = app.games.get_mut("draw-test").unwrap();
    game.state.phase = GamePhase::Ended;
    drop(game);
    assert!(send(&app, "black", json!({"type":"draw","turn_number":2}))
        .await
        .is_err());
}

#[test]
fn manual_draw_empty_deck_rng_failure_and_analysis_resolution() {
    for count in [0, 1, 3] {
        let mut state = super::tests::state_with_pockets(count, count);
        initialize(&mut state, &mut |_| Ok(0)).unwrap();
        assert!(!draw_required(&state));
        let action = generate_legal_move_actions(&state)[0].clone();
        assert!(submit_engine_action(state, TurnAction::Move(action)).is_ok());
    }
    let state = game().state;
    let action = DrawAction {
        player_id: "white".into(),
        turn_number: 1,
    };
    let hash = analysis::state_hash(&state);
    assert!(submit(state.clone(), action.clone(), &mut |_| Err(
        "entropy failure".into()
    ))
    .is_err());
    assert_eq!(analysis::state_hash(&state), hash);
    let after = submit_engine_action(state.clone(), TurnAction::Draw(action.clone())).unwrap();
    let mut node = analysis::AnalysisNode {
        id: "draw".into(),
        parent_node_id: None,
        request_id: String::new(),
        action: TurnAction::Draw(action),
        state_after: analysis::normalized_state(after),
        state_hash: String::new(),
        created_at_ms: 0,
        draws: vec![],
    };
    analysis::resolve_node(&mut node, Some(&state), &mut |_| Ok(0)).unwrap();
    assert_eq!(node.draws.len(), 1);
    let replay = replay_analysis_node(state, &node).unwrap();
    assert_eq!(
        analysis::state_hash(&replay),
        analysis::state_hash(&node.state_after)
    );
}

#[tokio::test]
async fn manual_draw_bot_uses_action_before_search_and_records_order() {
    for bot in ["white", "black"] {
        let mut game = game();
        if bot == "black" {
            // Fixture starts at black turn, preserving initial record through a real white turn.
            let action = DrawAction {
                player_id: "white".into(),
                turn_number: 1,
            };
            let (state, _) = submit(game.state.clone(), action, &mut |_| Ok(0)).unwrap();
            let mv = generate_legal_move_actions(&state)[0].clone();
            game.state = submit_engine_action(state, TurnAction::Move(mv)).unwrap();
        }
        game.access = GameAccess::Local {
            client: format!("{}-client", opponent_player(bot)),
            human: Some(opponent_player(bot)),
        };
        let before = game.players[bot].deck.pocket_pieces.len();
        prepare_draw(&mut game.state);
        let app = AppState::in_memory();
        app.games.insert(game.id.clone(), game);
        let result = run_bot_turn(
            State(app.clone()),
            headers(&opponent_player(bot)),
            Path("draw-test".into()),
            Json(BotTurnRequest {
                bot_player_id: bot.into(),
                difficulty: Some("easy".into()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(matches!(result.actions.first(), Some(AiAction::Draw(_))));
        assert!(result.actions.len() >= 2);
        let game = app.games.get("draw-test").unwrap();
        assert_eq!(game.players[bot].deck.pocket_pieces.len(), before - 1);
        assert!(matches!(game.record.actions[0].action, TurnAction::Draw(_)));
        assert_eq!(game.record.actions[0].draws[0].piece_ids.len(), 1);
        assert_ne!(game.current_player, bot);
        if bot == "white" {
            assert_eq!(
                analysis::state_hash(
                    &game
                        .record
                        .state_at_ply(game.record.actions.len() as u32)
                        .unwrap()
                ),
                analysis::state_hash(&game.state)
            );
        }
    }
}

#[test]
fn manual_draw_lifecycle_cannot_be_overwritten_by_piece_effects() {
    let before = game().state;
    let (state, _) = submit(
        before,
        DrawAction {
            player_id: "white".into(),
            turn_number: 1,
        },
        &mut |_| Ok(0),
    )
    .unwrap();
    let mut action = generate_legal_move_actions(&state)[0].clone();
    action.effects.global_state_updates = vec![
        GlobalStateUpdate {
            key: brainfuck_chess_engine::actions::MANUAL_DRAW.into(),
            value: 0,
        },
        GlobalStateUpdate {
            key: brainfuck_chess_engine::actions::DRAW_REQUIRED.into(),
            value: 0,
        },
    ];
    let next =
        brainfuck_chess_engine::endgame::apply_and_advance_turn(state, TurnAction::Move(action));
    assert_eq!(
        next.global_state
            .get(brainfuck_chess_engine::actions::MANUAL_DRAW),
        Some(&1)
    );
    assert!(draw_required(&next));
    let action = generate_legal_move_actions(&next)[0].clone();
    assert_eq!(
        submit_engine_action(next, TurnAction::Move(action)).unwrap_err(),
        "DRAW_REQUIRED"
    );
}
