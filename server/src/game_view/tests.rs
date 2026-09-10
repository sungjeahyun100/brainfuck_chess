use super::*;
use crate::*;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use serde_json::{json, Value};
use tower::Service;

fn spec() -> PlayerDeckSpec {
    let mut starting = vec![StartingPieceSpec {
        piece: DeckPieceRef::BuiltIn {
            piece_type: "king".into(),
        },
        square: Square::new(3, 0),
    }];
    starting.extend(
        brainfuck_chess_engine::rules::get_front_zone_squares_with_ruleset(
            &"white".into(),
            8,
            DeckRuleset::Standard,
        )
        .into_iter()
        .map(|square| StartingPieceSpec {
            piece: DeckPieceRef::BuiltIn {
                piece_type: "pawn".into(),
            },
            square,
        }),
    );
    PlayerDeckSpec {
        ruleset: DeckRuleset::Standard,
        name: None,
        starting,
        pocket: [
            "knight", "bishop", "rook", "knight", "knight", "knight", "knight", "knight",
        ]
        .into_iter()
        .map(|piece_type| DeckPieceRef::BuiltIn {
            piece_type: piece_type.into(),
        })
        .collect(),
        extra: vec![DeckPieceRef::BuiltIn {
            piece_type: "bomber".into(),
        }],
    }
}

fn add_hands(state: &mut GameState) -> HashMap<PlayerId, Vec<PieceId>> {
    let mut hands = HashMap::new();
    for side in ["white", "black"] {
        let ids = state.players[side].deck.pocket_pieces[..2].to_vec();
        for id in &ids {
            brainfuck_chess_engine::hand::move_pocket_piece_to_hand(state, &side.into(), id)
                .unwrap();
        }
        hands.insert(side.into(), ids);
    }
    hands
}

async fn http(
    app: &AppState,
    method: &str,
    path: &str,
    client: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(client) = client {
        req = req.header("x-game-client-id", client);
    }
    let response = routes::api(app.clone())
        .call(
            req.body(if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.headers()["vary"]
        .to_str()
        .unwrap()
        .contains("x-game-client-id"));
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn assert_private(value: &Value, state: &GameState, own: Option<&str>) {
    let wire = value.to_string();
    assert!(!wire.contains("\"initial_draws\""));
    assert!(!wire.contains("\"draws\""));
    for (side, player) in &state.players {
        if own == Some(side.as_str()) {
            continue;
        }
        for id in player
            .deck
            .hand_pieces
            .iter()
            .chain(&player.deck.pocket_pieces)
            .chain(&player.deck.extra_deck_pieces)
        {
            assert!(!wire.contains(id.as_str()), "hidden ID {id} in wire {wire}");
        }
    }
}

#[tokio::test]
async fn multiplayer_hand_privacy_covers_full_sync_legal_submit_rejoin_and_public_routes() {
    let app = AppState::in_memory();
    let deck = spec();
    let (status, room) = http(&app, "POST", "/rooms", Some("host-token"), json!({"ruleset":"standard", "board_size":8, "host_side":"white", "client_id":"host-token", "deck":deck})).await;
    assert_eq!(status, StatusCode::OK, "{room}");
    assert!(room["host_deck"].is_null());
    assert_eq!(room["host_has_deck"], true);
    let room_id = room["id"].as_str().unwrap();
    let (status, created) = http(
        &app,
        "POST",
        &format!("/rooms/{room_id}/join"),
        Some("guest-token"),
        json!({"client_id":"guest-token", "deck":deck}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().unwrap();
    let (hands, state) = {
        let game = app.games.get(id).unwrap();
        assert_eq!(game.players["white"].deck.hand_pieces.len(), 4);
        assert_eq!(game.players["black"].deck.hand_pieces.len(), 3);
        let hands: HashMap<_, _> = game
            .players
            .iter()
            .map(|(side, player)| (side.clone(), player.deck.hand_pieces.clone()))
            .collect();
        (hands, game.state.clone())
    };
    for (client, own) in [
        (Some("host-token"), Some("white")),
        (Some("guest-token"), Some("black")),
        (None, None),
        (Some("stranger"), None),
    ] {
        let (status, view) = http(&app, "GET", &format!("/games/{id}"), client, Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_private(&view, &state, own);
        for side in ["white", "black"] {
            assert_eq!(view["hand_counts"][side], hands[side].len());
        }
        if let Some(own) = own {
            assert_eq!(
                view["players"][own]["deck"]["hand_pieces"],
                json!(hands[own])
            );
            for piece_id in &hands[own] {
                assert_eq!(view["pieces"][piece_id.as_str()]["owner"], own);
            }
        }
    }
    for (client, own) in [("host-token", "white"), ("guest-token", "black")] {
        let mut revision = Value::Null;
        for round in 0..2 {
            let (status, sync) = http(&app, "POST", &format!("/rooms/{room_id}/heartbeat"), None, json!({"client_id":client, "player_id":own, "latest_ply":0, "catalog_revision":revision})).await;
            assert_eq!(status, StatusCode::OK, "{sync}");
            assert_private(&sync, &state, Some(own));
            assert_eq!(sync["dynamic"]["hand_counts"]["white"], 4);
            assert_eq!(
                sync["dynamic"]["players"][own]["deck"]["hand_pieces"],
                json!(hands[own])
            );
            assert_eq!(sync.get("catalog").is_some(), round == 0);
            revision = sync["catalog_revision"].clone();
        }
    }
    let (status, _) = http(
        &app,
        "POST",
        &format!("/rooms/{room_id}/heartbeat"),
        None,
        json!({"client_id":"host-token", "player_id":"black", "latest_ply":0}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for path in [
        format!("/games/{id}/legal-drops"),
        format!("/games/{id}/legal-moves"),
        format!("/games/{id}/pieces/{}/options", hands["white"][0]),
    ] {
        for client in [None, Some("guest-token")] {
            let (status, response) = http(&app, "GET", &path, client, Value::Null).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_private(&response, &state, None);
        }
    }
    let (status, legal) = http(
        &app,
        "GET",
        &format!("/games/{id}/legal-drops"),
        Some("host-token"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!legal["drops"].as_array().unwrap().is_empty());
    assert_private(&legal, &state, Some("white"));
    let (status, rejoin) = http(
        &app,
        "POST",
        &format!("/rooms/{room_id}/join"),
        None,
        json!({"client_id":"guest-token", "deck":deck}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_private(&rejoin, &state, Some("black"));
    assert_eq!(
        rejoin["state"]["players"]["black"]["deck"]["hand_pieces"],
        json!(hands["black"])
    );
    let (_, stranger) = http(
        &app,
        "POST",
        &format!("/rooms/{room_id}/join"),
        None,
        json!({"client_id":"stranger", "deck":deck}),
    )
    .await;
    assert_private(&stranger, &state, None);
    let (_, public_room) = http(&app, "GET", &format!("/rooms/{room_id}"), None, Value::Null).await;
    assert!(public_room["host_deck"].is_null() && public_room["guest_deck"].is_null());
    assert!(!public_room.to_string().contains("host-token"));
    let (_, record) = http(
        &app,
        "GET",
        &format!("/games/{id}/record"),
        None,
        Value::Null,
    )
    .await;
    assert_private(&record, &state, None);
    let (_, bot) = http(
        &app,
        "POST",
        &format!("/games/{id}/bot-turn"),
        Some("host-token"),
        json!({"bot_player_id":"white", "difficulty":"easy"}),
    )
    .await;
    assert_private(&bot, &state, None);
    let before = serde_json::to_value(&state).unwrap();
    let destination = legal["drops"][0]["to"].clone();
    for piece_id in [
        &state.players["white"].deck.pocket_pieces[0],
        &state.players["white"].deck.extra_deck_pieces[0],
        &hands["black"][0],
    ] {
        let (status, _) = http(
            &app,
            "POST",
            &format!("/games/{id}/actions"),
            Some("host-token"),
            json!({"action":{"type":"drop", "piece_id":piece_id, "to":destination}}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            serde_json::to_value(&app.games.get(id).unwrap().state).unwrap(),
            before
        );
    }
    let (status, after) = http(
        &app,
        "POST",
        &format!("/games/{id}/actions"),
        Some("host-token"),
        json!({"action":{"type":"drop", "piece_id":hands["white"][0], "to":destination}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_private(&after, &state, Some("white"));
    assert_eq!(after["hand_counts"]["white"], 3);
    assert_eq!(after["hand_counts"]["black"], 4);
    let after_white = app.games.get(id).unwrap().state.clone();
    for (client, own) in [("host-token", "white"), ("guest-token", "black")] {
        let (status, sync) = http(
            &app,
            "POST",
            &format!("/rooms/{room_id}/heartbeat"),
            None,
            json!({"client_id":client,"player_id":own,"latest_ply":0}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_private(&sync, &after_white, Some(own));
        assert_eq!(sync["dynamic"]["hand_counts"]["black"], 4);
        assert_eq!(
            sync["dynamic"]["players"][own]["deck"]["hand_pieces"],
            json!(after_white.players[own].deck.hand_pieces)
        );
    }
    let black_action = generate_legal_move_actions(&after_white)[0].clone();
    let (status, after_black) = http(&app, "POST", &format!("/games/{id}/actions"), Some("guest-token"),
        json!({"action":{"type":"move","piece_id":black_action.piece_id,"to":black_action.to,"move_option_id":black_action.move_option_id}})).await;
    assert_eq!(status, StatusCode::OK, "{after_black}");
    let authority = app.games.get(id).unwrap().state.clone();
    assert_eq!(authority.current_player, "white");
    assert_eq!(authority.players["white"].deck.hand_pieces.len(), 4);
    assert_eq!(authority.players["black"].deck.hand_pieces.len(), 4);
    assert_private(&after_black, &authority, Some("black"));
}

#[tokio::test]
async fn local_and_bot_creation_bind_distinct_view_capabilities_and_bot_frames_are_projected() {
    for human in [None, Some("white"), Some("black")] {
        let app = AppState::in_memory();
        let white = spec();
        let black = materialize_neutral_deck(&white, "black", 8);
        let mut request =
            json!({"ruleset":"standard", "board_size":8, "white_deck":white, "black_deck":black});
        if let Some(human) = human {
            request["local_side"] = human.into();
            request["bot_player_id"] = opponent_player(human).into();
        }
        let (status, response) = http(&app, "POST", "/games", Some("controller"), request).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let id = response["id"].as_str().unwrap();
        let state = {
            let game = app.games.get(id).unwrap();
            assert_eq!(game.players["white"].deck.hand_pieces.len(), 4);
            assert_eq!(game.players["black"].deck.hand_pieces.len(), 3);
            game.state.clone()
        };
        let (_, own_view) = http(
            &app,
            "GET",
            &format!("/games/{id}"),
            Some("controller"),
            Value::Null,
        )
        .await;
        for side in ["white", "black"] {
            let visible = human.is_none() || human == Some(side);
            assert_eq!(
                own_view["players"][side]["deck"]
                    .get("hand_pieces")
                    .is_some(),
                visible
            );
        }
        let (_, outsider) = http(
            &app,
            "GET",
            &format!("/games/{id}"),
            Some("outsider"),
            Value::Null,
        )
        .await;
        assert_private(&outsider, &state, None);
        if let Some(human) = human {
            assert_private(&own_view, &state, Some(human));
            let bot = opponent_player(human);
            if bot == "black" {
                let action = generate_legal_move_actions(&state)[0].clone();
                let (status, moved) = http(&app, "POST", &format!("/games/{id}/actions"), Some("controller"),
                    json!({"action":{"type":"move", "piece_id":action.piece_id, "to":action.to, "move_option_id":action.move_option_id}})).await;
                assert_eq!(status, StatusCode::OK, "{moved}");
            }
            let before_bot = app.games.get(id).unwrap().state.clone();
            assert_eq!(before_bot.current_player, bot);
            assert_eq!(before_bot.players[&bot].deck.hand_pieces.len(), 4);
            let body = json!({"bot_player_id":bot, "difficulty":"easy"});
            let (status, _) = http(
                &app,
                "POST",
                &format!("/games/{id}/bot-turn"),
                Some("outsider"),
                body.clone(),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            let (status, response) = http(
                &app,
                "POST",
                &format!("/games/{id}/bot-turn"),
                Some("controller"),
                body,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{response}");
            assert!(response.get("stats").is_none());
            assert!(!response["timeline"].as_array().unwrap().is_empty());
            let after = app.games.get(id).unwrap().state.clone();
            assert_private(&response, &after, Some(human));
            assert_eq!(after.current_player, human);
            assert_eq!(
                after.players[human].deck.hand_pieces.len(),
                before_bot.players[human].deck.hand_pieces.len() + 1
            );
            let last_frame = response["timeline"].as_array().unwrap().last().unwrap();
            assert_eq!(
                last_frame["state"]["hand_counts"][human],
                after.players[human].deck.hand_pieces.len()
            );
            let record = app.games.get(id).unwrap().record.clone();
            assert_eq!(
                crate::analysis::state_hash(
                    &record.state_at_ply(record.actions.len() as u32).unwrap()
                )
                .unwrap(),
                crate::analysis::state_hash(&after).unwrap()
            );
            let (retry_status, _) = http(
                &app,
                "POST",
                &format!("/games/{id}/bot-turn"),
                Some("controller"),
                json!({"bot_player_id":bot, "difficulty":"easy"}),
            )
            .await;
            assert_eq!(retry_status, StatusCode::CONFLICT);
            assert_eq!(
                crate::analysis::state_hash(&app.games.get(id).unwrap().state).unwrap(),
                crate::analysis::state_hash(&after).unwrap()
            );
            for frame in response["timeline"].as_array().unwrap() {
                assert!(frame["state"]["hand_counts"][bot.as_str()].is_number());
            }
        }
    }
}

#[test]
fn hand_hash_and_custom_catalog_projection_follow_membership_without_mutating_authority() {
    let deck = spec();
    let black = materialize_neutral_deck(&deck, "black", 8);
    let mut state = build_game_state("hand-hash".into(), 8, &deck, &black, vec![]).unwrap();
    let initial_hash = analysis::state_hash(&state).unwrap();
    let hands = add_hands(&mut state);
    assert_ne!(initial_hash, analysis::state_hash(&state).unwrap());
    let mut other = state.clone();
    let id = hands["black"][0].clone();
    other
        .players
        .get_mut("black")
        .unwrap()
        .deck
        .hand_pieces
        .retain(|p| p != &id);
    assert_ne!(
        analysis::state_hash(&state).unwrap(),
        analysis::state_hash(&other).unwrap()
    );
    // Custom package helper definitions must not leak a private package either.
    let mut definition = state.piece_definitions["knight"].clone();
    definition.id = "private-custom-type".into();
    state
        .piece_definitions
        .insert(definition.id.clone(), definition);
    state.pieces.get_mut(&id).unwrap().type_id = "private-custom-type".into();
    state.custom_piece_manifest.push(
        brainfuck_chess_engine::custom_pieces::CustomPieceManifestEntry {
            package_id: "private-package".into(),
            version: 1,
            content_hash: "hash".into(),
            definition_snapshot_hash: "definition-hash".into(),
            exposed_type_id: "private-custom-type".into(),
            runtime_type_ids: vec!["private-custom-type".into()],
        },
    );
    let before = serde_json::to_value(&state).unwrap();
    let public = project_state(&state, Audience::Player("white"));
    let private = project_state(&state, Audience::Player("black"));
    assert!(!serde_json::to_string(&public)
        .unwrap()
        .contains("private-custom"));
    assert!(!serde_json::to_string(&public)
        .unwrap()
        .contains("private-package"));
    assert!(private
        .piece_definitions
        .contains_key("private-custom-type"));
    assert_ne!(
        catalog_revision(&public, 1, Audience::Player("white")),
        catalog_revision(&private, 1, Audience::Player("black"))
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}

#[test]
fn opponent_pocket_to_hand_transfer_exposes_only_count_not_membership_by_subtraction() {
    let deck = spec();
    let black = materialize_neutral_deck(&deck, "black", 8);
    let mut state = build_game_state("future-transfer".into(), 8, &deck, &black, vec![]).unwrap();
    let before = project_state(&state, Audience::Player("white"));
    let id = state.players["black"].deck.pocket_pieces[0].clone();
    assert_eq!(hand_counts(&state)["black"], 0);
    brainfuck_chess_engine::hand::move_pocket_piece_to_hand(&mut state, &"black".into(), &id)
        .unwrap();
    let after = project_state(&state, Audience::Player("white"));
    assert_eq!(hand_counts(&state)["black"], 1);
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
    assert_eq!(
        catalog_revision(&before, 1, Audience::Player("white")),
        catalog_revision(&after, 1, Audience::Player("white"))
    );
}

#[tokio::test]
async fn g5_room_both_players_summon_and_rejoin_heartbeat_preserve_private_extra() {
    let app = AppState::in_memory();
    let mut deck = spec();
    deck.pocket = (0..3)
        .map(|_| DeckPieceRef::BuiltIn {
            piece_type: "queen".into(),
        })
        .collect();
    deck.extra = vec![DeckPieceRef::BuiltIn {
        piece_type: "guhang".into(),
    }];
    let (status,room)=http(&app,"POST","/rooms",Some("g5-host"),json!({"ruleset":"standard","board_size":8,"host_side":"white","client_id":"g5-host","deck":deck})).await;
    assert_eq!(status, StatusCode::OK, "{room}");
    let room_id = room["id"].as_str().unwrap();
    let (status, created) = http(
        &app,
        "POST",
        &format!("/rooms/{room_id}/join"),
        Some("g5-guest"),
        json!({"client_id":"g5-guest","deck":deck}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().unwrap();
    for (side, client) in [("white", "g5-host"), ("black", "g5-guest")] {
        let before = app.games.get(id).unwrap().state.clone();
        let extra = &before.players[side].deck.extra_deck_pieces[0];
        let sacrifices = &before.players[side].deck.hand_pieces;
        let (_, public) = http(&app, "GET", &format!("/games/{id}"), None, Value::Null).await;
        assert!(public["pieces"].get(extra.as_str()).is_none());
        assert_private(&public, &before, None);
        let (status, options) = http(
            &app,
            "POST",
            &format!("/games/{id}/summon-options"),
            Some(client),
            json!({"extra_piece_id":extra,"sacrifice_piece_ids":sacrifices}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{options}");
        assert_eq!(options["cost"], 25);
        assert!(!options["actions"].as_array().unwrap().is_empty());
        let (status, _) = http(
            &app,
            "POST",
            &format!("/games/{id}/summon-options"),
            Some("unknown"),
            json!({"extra_piece_id":extra}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let mut action = options["actions"][0].clone();
        action["type"] = json!("extra_summon");
        action.as_object_mut().unwrap().remove("player_id");
        let (status, view) = http(
            &app,
            "POST",
            &format!("/games/{id}/actions"),
            Some(client),
            json!({"action":action}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{view}");
        assert!(view["players"][side]["deck"]
            .get("extra_deck_pieces")
            .is_none());
        let after = app.games.get(id).unwrap().state.clone();
        assert_private(&view, &after, Some(side));
        let (status, sync) = http(
            &app,
            "POST",
            &format!("/rooms/{room_id}/heartbeat"),
            None,
            json!({"client_id":client,"player_id":side,"latest_ply":0}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{sync}");
        assert!(sync["dynamic"]["players"][side]["deck"]
            .get("extra_deck_pieces")
            .is_none());
        assert_private(&sync, &after, Some(side));
        // The existing host cannot join its own room; its reconnect path is GET.
        let (status, rejoin) = if side == "white" {
            http(
                &app,
                "GET",
                &format!("/games/{id}"),
                Some(client),
                Value::Null,
            )
            .await
        } else {
            http(
                &app,
                "POST",
                &format!("/rooms/{room_id}/join"),
                Some(client),
                json!({"client_id":client,"deck":deck}),
            )
            .await
        };
        assert_eq!(status, StatusCode::OK, "{rejoin}");
        assert!(rejoin["players"][side]["deck"]
            .get("extra_deck_pieces")
            .is_none());
        assert_private(&rejoin, &after, Some(side));
    }
}
