use super::tests::{add_board, state_with_pockets};
use super::*;
use crate::*;
use brainfuck_chess_engine::ai::{generate_ai_actions, AiAction};
use serde_json::json;

fn headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-game-client-id", "g8a-controller".parse().unwrap());
    h
}
fn request(action: AiAction) -> SubmitActionRequest {
    let mut value = serde_json::to_value(action).unwrap();
    value.as_object_mut().unwrap().remove("player_id");
    serde_json::from_value(json!({"action": value})).unwrap()
}
fn game(bot: &str, winning_summon: bool) -> StoredGame {
    let mut state = state_with_pockets(8, 8);
    if winning_summon {
        let extra = state.players[bot].deck.extra_deck_pieces[0].clone();
        state.pieces.get_mut(&extra).unwrap().type_id = "guhang".into();
        // Actual built-in values: deployed paratroopers have score 3 but
        // ai_board_value 0. Trading nine of them for Guhang is profitable.
        for n in 0..9 {
            add_board(
                &mut state,
                &format!("sacrifice{n}"),
                bot,
                "paratrooper",
                Square::new(n % 5, if bot == "white" { 2 + n / 5 } else { 5 - n / 5 }),
            );
        }
    }
    let mut game = StoredGame::new(state, TimeControlId::Unlimited, false, now_ms());
    // Retain v1 bot/timeline replay regression coverage.
    game.record.initial_draws =
        super::tests::initialize_automatic(&mut game.state, &mut |_| Ok(0)).unwrap();
    game.record.initial_state = game.state.clone();
    game.record.ruleset_version = "deck-chess-standard-1".into();
    game.access = GameAccess::Local {
        client: "g8a-controller".into(),
        human: Some(opponent_player(bot)),
    };
    game
}

#[tokio::test]
async fn both_bot_sides_commit_hand_drop_draw_and_exact_timeline_without_stats() {
    for bot in ["white", "black"] {
        let app = AppState::in_memory();
        let game = game(bot, false);
        let id = game.id.clone();
        app.games.insert(id.clone(), game);
        if bot == "black" {
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
        }
        let before = app.games.get(&id).unwrap().state.clone();
        assert_eq!(before.players[bot].deck.hand_pieces.len(), 4);
        let human = opponent_player(bot);
        let response = run_bot_turn(
            State(app.clone()),
            headers(),
            Path(id.clone()),
            Json(BotTurnRequest {
                bot_player_id: bot.into(),
                difficulty: Some("hard".into()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(response
            .actions
            .iter()
            .any(|a| matches!(a, AiAction::Drop(_))));
        assert!(response.stats.is_none());
        let stored = app.games.get(&id).unwrap();
        assert_eq!(
            stored.state.players[&human].deck.hand_pieces.len(),
            before.players[&human].deck.hand_pieces.len() + 1
        );
        assert_eq!(stored.record.actions.last().unwrap().draws.len(), 1);
        assert_eq!(
            stored.record.actions.last().unwrap().draws[0]
                .piece_ids
                .len(),
            1
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
        let wire = serde_json::to_value(&response).unwrap();
        assert!(wire.get("stats").is_none());
        for hidden in stored.state.players[bot]
            .deck
            .hand_pieces
            .iter()
            .chain(&stored.state.players[bot].deck.pocket_pieces)
        {
            assert!(!wire.to_string().contains(hidden.as_str()));
        }
        let final_frame = &wire["timeline"].as_array().unwrap().last().unwrap()["state"];
        for key in [
            "pieces",
            "players",
            "board",
            "current_player",
            "hand_counts",
        ] {
            assert_eq!(final_frame[key], wire["game_state"][key], "{key}");
        }
    }
}

#[tokio::test]
async fn both_bot_sides_choose_profitable_extra_and_replay_exact_ids() {
    for bot in ["white", "black"] {
        let app = AppState::in_memory();
        let game = game(bot, true);
        let id = game.id.clone();
        app.games.insert(id.clone(), game);
        if bot == "black" {
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
        }
        let response = run_bot_turn(
            State(app.clone()),
            headers(),
            Path(id.clone()),
            Json(BotTurnRequest {
                bot_player_id: bot.into(),
                difficulty: Some("hard".into()),
            }),
        )
        .await
        .unwrap()
        .0;
        let AiAction::ExtraSummon(summon) = &response.actions[0] else {
            panic!("expected profitable summon: {:?}", response.actions)
        };
        assert_eq!(summon.sacrifice_piece_ids.len(), 9);
        let stored = app.games.get(&id).unwrap();
        assert!(summon
            .sacrifice_piece_ids
            .iter()
            .all(|id| stored.state.pieces[id].captured));
        assert!(!stored.state.players[bot]
            .deck
            .extra_deck_pieces
            .contains(&summon.extra_piece_id));
        assert_eq!(stored.record.ruleset_version, "deck-chess-standard-1");
        let recorded = &stored.record.actions.last().unwrap();
        assert_eq!(
            serde_json::to_value(&recorded.action).unwrap(),
            serde_json::to_value(&response.actions[0]).unwrap()
        );
        assert_eq!(recorded.draws.len(), 1);
        assert_eq!(
            analysis::state_hash(
                &stored
                    .record
                    .state_at_ply(stored.record.actions.len().try_into().unwrap())
                    .unwrap()
            ),
            analysis::state_hash(&stored.state)
        );
    }
}

#[test]
fn actual_summon_advances_one_draw_and_search_leaves_reserves_unchanged() {
    let mut game = game("white", false);
    for n in 0..3 {
        add_board(
            &mut game.state,
            &format!("queen{n}"),
            "white",
            "queen",
            Square::new(n, 2),
        );
    }
    let before = game.state.clone();
    let original = analysis::state_hash(&before);
    let action = generate_ai_actions(&before)
        .into_iter()
        .find(|a| matches!(a, AiAction::ExtraSummon(_)))
        .unwrap();
    let mut after = brainfuck_chess_engine::ai::apply_ai_action(before.clone(), &action).unwrap();
    assert_eq!(
        after.players["black"].deck.hand_pieces,
        before.players["black"].deck.hand_pieces
    );
    let mut calls = 0;
    let draws = turn_start(&before, &mut after, &mut |_| {
        calls += 1;
        Ok(0)
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(draws.len(), 1);
    assert_eq!(analysis::state_hash(&before), original);
}

#[tokio::test]
async fn bot_forced_landing_keeps_two_frames_and_only_one_turn_draw() {
    let app = AppState::in_memory();
    let mut game = game("white", false);
    add_board(&mut game.state, "air", "white", "bomber", Square::new(0, 4));
    game.state
        .board
        .set_piece_at_layer(Square::new(0, 4), PieceLayer::Ground, None);
    game.state
        .board
        .set_piece_at_layer(Square::new(0, 4), PieceLayer::Air, Some("air".into()));
    let p = game.state.pieces.get_mut("air").unwrap();
    p.layer = PieceLayer::Air;
    p.remaining_flight_turns = 1;
    p.state
        .insert("airborne".into(), PieceStateValue::Boolean(true));
    game.record.initial_state = game.state.clone();
    let id = game.id.clone();
    app.games.insert(id.clone(), game);
    let response = run_bot_turn(
        State(app.clone()),
        headers(),
        Path(id.clone()),
        Json(BotTurnRequest {
            bot_player_id: "white".into(),
            difficulty: Some("easy".into()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(response.actions.len(), 2);
    assert!(
        matches!(&response.actions[1], AiAction::Ability(a) if a.ability_id == "forced-landing")
    );
    let stored = app.games.get(&id).unwrap();
    assert!(stored.record.actions[0].draws.is_empty());
    assert_eq!(stored.record.actions[1].draws.len(), 1);
    assert_eq!(
        analysis::state_hash(&stored.record.state_at_ply(2).unwrap()),
        analysis::state_hash(&stored.state)
    );
}
