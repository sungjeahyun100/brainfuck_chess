use axum::{
    body::to_bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use brainfuck_chess_engine::{
    rules::can_end_turn,
    types::{
        ActivateAbilityAction, GameEndReason, GamePhase, GameResult, GameState, MoveAction, Square,
        TurnAction, TurnMode,
    },
};

use crate::app_state::AppState;
use crate::dto::game::{
    BotTurnRequest, PieceOptionsQuery, PlayerDeckSpec, StartingPieceSpec, SubmitActionRequest,
};
use crate::dto::lab::{LabPieceOptionsRequest, LabPieceRequest};
use crate::handlers::games::{bot_turn, get_piece_options, submit_action};
use crate::handlers::lab::piece_lab_options;
use crate::services::game_builder::build_game_state;

fn test_app_with_game() -> (AppState, String) {
    let game_id = "test-game".to_string();
    let white_deck = PlayerDeckSpec {
        starting: vec![
            StartingPieceSpec {
                piece_type: "king".into(),
                square: Square::new(4, 0),
            },
            StartingPieceSpec {
                piece_type: "rook".into(),
                square: Square::new(0, 0),
            },
        ],
        pocket: vec![],
    };
    let black_deck = PlayerDeckSpec {
        starting: vec![StartingPieceSpec {
            piece_type: "king".into(),
            square: Square::new(4, 7),
        }],
        pocket: vec![],
    };
    let state = build_game_state(game_id.clone(), 8, &white_deck, &black_deck).unwrap();
    let app = AppState::new();
    app.games.insert(game_id.clone(), state);
    (app, game_id)
}

fn test_app_with_ability_bishop() -> (AppState, String) {
    let game_id = "ability-game".to_string();
    let white_deck = PlayerDeckSpec {
        starting: vec![
            StartingPieceSpec {
                piece_type: "king".into(),
                square: Square::new(4, 0),
            },
            StartingPieceSpec {
                piece_type: "bishop".into(),
                square: Square::new(2, 0),
            },
        ],
        pocket: vec![],
    };
    let black_deck = PlayerDeckSpec {
        starting: vec![StartingPieceSpec {
            piece_type: "king".into(),
            square: Square::new(4, 7),
        }],
        pocket: vec![],
    };
    let state = build_game_state(game_id.clone(), 8, &white_deck, &black_deck).unwrap();
    let app = AppState::new();
    app.games.insert(game_id.clone(), state);
    (app, game_id)
}

async fn response_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn submit_action_state(response: impl IntoResponse) -> GameState {
    let response = response.into_response();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn submit_action_error(response: impl IntoResponse) -> (StatusCode, String) {
    let response = response.into_response();
    let status = response.status();
    let body: serde_json::Value = response_json(response).await;
    let message = body
        .get("error")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    (status, message)
}

#[tokio::test]
async fn piece_options_returns_only_selected_piece_moves() {
    let (app, game_id) = test_app_with_game();
    let piece_id = "white_rook_1".to_string();

    let response = match get_piece_options(
        State(app),
        Path((game_id, piece_id.clone())),
        Query(PieceOptionsQuery::default()),
    )
    .await
    {
        Ok(Json(response)) => response,
        Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
    };

    assert!(!response.moves.is_empty());
    assert!(response.moves.iter().all(|m| m.piece_id == piece_id));
}

#[tokio::test]
async fn lab_piece_options_uses_temporary_state_without_storing_game() {
    let app = AppState::new();
    let req = LabPieceOptionsRequest {
        board_size: 8,
        selected_piece_id: "lab_white_rook_1".into(),
        ability_id: None,
        pieces: vec![
            LabPieceRequest {
                id: "lab_white_rook_1".into(),
                piece_type: "rook".into(),
                owner: "white".into(),
                square: Square::new(3, 3),
            },
            LabPieceRequest {
                id: "lab_black_knight_1".into(),
                piece_type: "knight".into(),
                owner: "black".into(),
                square: Square::new(3, 6),
            },
        ],
    };

    let response = match piece_lab_options(Json(req)).await {
        Ok(Json(response)) => response,
        Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
    };

    assert!(response.moves.contains(&Square::new(3, 6)));
    assert!(response.attacks.contains(&Square::new(3, 6)));
    assert!(app.games.is_empty());
}

#[tokio::test]
async fn submit_move_action_automatically_ends_turn() {
    let (app, game_id) = test_app_with_game();

    let response = submit_action_state(
        submit_action(
            State(app.clone()),
            Path(game_id.clone()),
            Json(SubmitActionRequest {
                action: TurnAction::Move(MoveAction {
                    player_id: "white".into(),
                    piece_id: "white_rook_1".into(),
                    from: Square::new(0, 0),
                    to: Square::new(0, 1),
                    captured_piece_id: None,
                    promotion: None,
                    ability_id: None,
                }),
            }),
        )
        .await,
    )
    .await;

    assert_eq!(response.current_player, "black");
    assert_eq!(response.turn_number, 2);
    assert!(response.turn_state.actions.is_empty());
    let stored = app.games.get(&game_id).unwrap();
    assert_eq!(stored.current_player, "black");
}

#[tokio::test]
async fn bot_turn_api_runs_and_persists_a_complete_turn() {
    let (app, game_id) = test_app_with_game();

    let response = match bot_turn(
        State(app.clone()),
        Path(game_id.clone()),
        Json(BotTurnRequest {
            bot_player_id: "white".into(),
            difficulty: Some("easy".into()),
        }),
    )
    .await
    {
        Ok(Json(response)) => response,
        Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
    };

    assert!(response.ok);
    assert!(!response.actions.is_empty());
    assert_eq!(response.timeline.len(), response.actions.len());
    assert!(response
        .timeline
        .iter()
        .all(|frame| !frame.effects.is_empty()));
    assert!(response
        .timeline
        .iter()
        .zip(&response.actions)
        .all(|(frame, action)| &frame.action == action));
    assert!(
        response.game_state.phase == GamePhase::Ended
            || response.game_state.current_player == "black"
    );
    let stored = app.games.get(&game_id).unwrap();
    assert_eq!(stored.current_player, response.game_state.current_player);
    assert_eq!(stored.turn_number, response.game_state.turn_number);
}

#[tokio::test]
async fn bot_turn_api_rejects_an_ended_game_without_mutating_it() {
    let (app, game_id) = test_app_with_game();
    {
        let mut state = app.games.get_mut(&game_id).unwrap();
        state.phase = GamePhase::Ended;
        state.result = Some(GameResult {
            winner: Some("black".into()),
            reason: GameEndReason::KingCapture,
        });
    }

    let error = bot_turn(
        State(app.clone()),
        Path(game_id.clone()),
        Json(BotTurnRequest {
            bot_player_id: "white".into(),
            difficulty: Some("normal".into()),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    let stored = app.games.get(&game_id).unwrap();
    assert_eq!(stored.phase, GamePhase::Ended);
    assert_eq!(
        stored.result.as_ref().unwrap().winner.as_deref(),
        Some("black")
    );
}

#[tokio::test]
async fn bot_turn_api_rejects_an_unknown_difficulty() {
    let (app, game_id) = test_app_with_game();
    let error = bot_turn(
        State(app),
        Path(game_id),
        Json(BotTurnRequest {
            bot_player_id: "white".into(),
            difficulty: Some("impossible".into()),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert!(error.1.error.contains("difficulty"));
}

#[tokio::test]
async fn submit_action_activates_ability_and_records_action() {
    let (app, game_id) = test_app_with_ability_bishop();

    let response = submit_action_state(
        submit_action(
            State(app.clone()),
            Path(game_id.clone()),
            Json(SubmitActionRequest {
                action: TurnAction::ActivateAbility(ActivateAbilityAction {
                    player_id: "white".into(),
                    piece_id: "white_bishop_1".into(),
                    ability_id: "bounce_mode".into(),
                }),
            }),
        )
        .await,
    )
    .await;

    let bishop = response.pieces.get("white_bishop_1").unwrap();
    assert_eq!(bishop.type_id, "bishop");
    assert_eq!(
        bishop
            .active_ability
            .as_ref()
            .map(|active| active.ability_id.as_str()),
        Some("bounce_mode")
    );
    assert_eq!(response.turn_state.mode, TurnMode::Move);
    assert!(can_end_turn(&response));
    assert!(matches!(
        response.turn_state.actions.as_slice(),
        [TurnAction::ActivateAbility(_)]
    ));
}

#[tokio::test]
async fn submit_action_rejects_ability_during_drop_mode() {
    let (app, game_id) = test_app_with_ability_bishop();
    {
        let mut state = app.games.get_mut(&game_id).unwrap();
        state.turn_state.mode = TurnMode::Drop;
    }

    let (status, message) = submit_action_error(
        submit_action(
            State(app),
            Path(game_id),
            Json(SubmitActionRequest {
                action: TurnAction::ActivateAbility(ActivateAbilityAction {
                    player_id: "white".into(),
                    piece_id: "white_bishop_1".into(),
                    ability_id: "bounce_mode".into(),
                }),
            }),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("능력"));
}

#[tokio::test]
async fn submit_action_rejects_reactivating_active_ability() {
    let (app, game_id) = test_app_with_ability_bishop();
    let request = || SubmitActionRequest {
        action: TurnAction::ActivateAbility(ActivateAbilityAction {
            player_id: "white".into(),
            piece_id: "white_bishop_1".into(),
            ability_id: "bounce_mode".into(),
        }),
    };

    let _ = submit_action_state(
        submit_action(State(app.clone()), Path(game_id.clone()), Json(request())).await,
    )
    .await;
    let (status, message) =
        submit_action_error(submit_action(State(app), Path(game_id), Json(request())).await).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("사용할 수 없는 능력"));
}

#[tokio::test]
async fn submit_action_rejects_once_per_turn_repeat_even_after_manual_clear() {
    let (app, game_id) = test_app_with_ability_bishop();
    let request = || SubmitActionRequest {
        action: TurnAction::ActivateAbility(ActivateAbilityAction {
            player_id: "white".into(),
            piece_id: "white_bishop_1".into(),
            ability_id: "bounce_mode".into(),
        }),
    };

    let _ = submit_action_state(
        submit_action(State(app.clone()), Path(game_id.clone()), Json(request())).await,
    )
    .await;
    {
        let mut state = app.games.get_mut(&game_id).unwrap();
        state
            .pieces
            .get_mut("white_bishop_1")
            .unwrap()
            .active_ability = None;
    }
    let (status, message) =
        submit_action_error(submit_action(State(app), Path(game_id), Json(request())).await).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("사용할 수 없는 능력"));
}
