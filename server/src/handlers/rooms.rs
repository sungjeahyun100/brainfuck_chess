use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use brainfuck_chess_engine::types::{GameEndReason, GamePhase, GameResult, GameState, PlayerId};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dto::error::ErrorBody as ErrorResponse;
use crate::dto::game::GameResponse;
use crate::dto::room::{
    CreateRoomRequest, JoinRoomRequest, MultiplayerRoom, ResignRoomRequest, RoomReadyRequest,
    SelectDeckRequest,
};
use crate::mappers::deck_spec::materialize_neutral_deck;
use crate::services::game_builder::build_game_state;
use crate::stores::game_store::GameStore;
use crate::stores::room_store::RoomStore;

fn opponent_side(side: &PlayerId) -> PlayerId {
    if side == "white" {
        "black".into()
    } else {
        "white".into()
    }
}

fn generate_room_id(rooms: &RoomStore) -> String {
    for _ in 0..16 {
        let id = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(6)
            .collect::<String>()
            .to_uppercase();
        if !rooms.contains_key(&id) {
            return id;
        }
    }

    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(12)
        .collect::<String>()
        .to_uppercase()
}

fn start_room_game(
    room: &mut MultiplayerRoom,
    games: &GameStore,
) -> Result<Option<GameResponse>, String> {
    if let Some(game_id) = &room.game_id {
        let state = games
            .get(game_id)
            .ok_or_else(|| "방의 게임을 찾을 수 없습니다.".to_string())?;
        return Ok(Some(GameResponse {
            id: game_id.clone(),
            state: state.clone(),
        }));
    }

    if !room.host_ready || !room.guest_ready {
        return Ok(None);
    }

    let host_spec = room
        .host_deck
        .as_ref()
        .ok_or_else(|| "방장 덱이 선택되지 않았습니다.".to_string())?;
    let guest_spec = room
        .guest_deck
        .as_ref()
        .ok_or_else(|| "참가자 덱이 선택되지 않았습니다.".to_string())?;
    let game_id = Uuid::new_v4().to_string();
    let host_deck = materialize_neutral_deck(host_spec, &room.host_side, room.board_size);
    let guest_deck = materialize_neutral_deck(guest_spec, &room.guest_side, room.board_size);
    let (white_deck, black_deck) = if room.host_side == "white" {
        (&host_deck, &guest_deck)
    } else {
        (&guest_deck, &host_deck)
    };
    let state = build_game_state(game_id.clone(), room.board_size, white_deck, black_deck)?;

    room.game_id = Some(game_id.clone());
    games.insert(game_id.clone(), state.clone());
    Ok(Some(GameResponse { id: game_id, state }))
}

pub async fn create_room(
    State(app): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    if req.board_size < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "보드 크기는 최소 8이어야 합니다.".into(),
            }),
        ));
    }
    if req.host_side != "white" && req.host_side != "black" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "진영은 white 또는 black이어야 합니다.".into(),
            }),
        ));
    }

    let id = generate_room_id(&app.rooms);
    let room = MultiplayerRoom {
        id: id.clone(),
        board_size: req.board_size,
        guest_side: opponent_side(&req.host_side),
        host_client_id: req.client_id,
        guest_client_id: None,
        host_side: req.host_side,
        host_deck: Some(req.deck),
        guest_deck: None,
        host_ready: true,
        guest_ready: false,
        game_id: None,
    };

    app.rooms.insert(id, room.clone());
    Ok(Json(room))
}

pub async fn get_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    app.rooms
        .get(&id.to_uppercase())
        .map(|room| Json(room.clone()))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "방을 찾을 수 없습니다.".into(),
                }),
            )
        })
}

pub async fn join_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    if req.client_id == room.host_client_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "자신이 만든 방에는 참가자로 입장할 수 없습니다.".into(),
            }),
        ));
    }

    if let Some(game_id) = &room.game_id {
        let state = app.games.get(game_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "방의 게임을 찾을 수 없습니다.".into(),
                }),
            )
        })?;
        return Ok(Json(GameResponse {
            id: game_id.clone(),
            state: state.clone(),
        }));
    }

    room.guest_deck = Some(req.deck);
    room.guest_client_id = Some(req.client_id);
    room.guest_ready = true;
    let response = start_room_game(room.value_mut(), &app.games)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "양쪽 플레이어가 아직 준비되지 않았습니다.".into(),
                }),
            )
        })?;
    Ok(Json(response))
}

pub async fn select_room_deck(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SelectDeckRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    if room.game_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "이미 게임이 시작된 방에서는 덱을 변경할 수 없습니다.".into(),
            }),
        ));
    }

    if req.client_id == room.host_client_id {
        room.host_deck = Some(req.deck);
        room.host_ready = false;
        return Ok(Json(room.clone()));
    }

    if room.guest_client_id.is_none() {
        room.guest_client_id = Some(req.client_id.clone());
    }

    if room.guest_client_id.as_deref() == Some(req.client_id.as_str()) {
        room.guest_deck = Some(req.deck);
        room.guest_ready = false;
        return Ok(Json(room.clone()));
    }

    Err((
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "이 방의 플레이어만 덱을 변경할 수 있습니다.".into(),
        }),
    ))
}

pub async fn ready_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RoomReadyRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    if room.game_id.is_some() {
        return Ok(Json(room.clone()));
    }

    if req.client_id == room.host_client_id {
        if room.host_deck.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "방장 덱이 선택되지 않았습니다.".into(),
                }),
            ));
        }
        room.host_ready = true;
    } else if room.guest_client_id.as_deref() == Some(req.client_id.as_str()) {
        if room.guest_deck.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "참가자 덱이 선택되지 않았습니다.".into(),
                }),
            ));
        }
        room.guest_ready = true;
    } else {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "이 방의 플레이어만 준비할 수 있습니다.".into(),
            }),
        ));
    }

    start_room_game(room.value_mut(), &app.games)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    Ok(Json(room.clone()))
}

pub async fn unready_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RoomReadyRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    if room.game_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "이미 게임이 시작된 방에서는 준비를 해제할 수 없습니다.".into(),
            }),
        ));
    }

    if req.client_id == room.host_client_id {
        room.host_ready = false;
    } else if room.guest_client_id.as_deref() == Some(req.client_id.as_str()) {
        room.guest_ready = false;
    } else {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "이 방의 플레이어만 준비를 해제할 수 있습니다.".into(),
            }),
        ));
    }

    Ok(Json(room.clone()))
}

pub async fn resign_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ResignRoomRequest>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    let room = app
        .rooms
        .get(&id.to_uppercase())
        .map(|room| room.clone())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "방을 찾을 수 없습니다.".into(),
                }),
            )
        })?;

    let is_host = req.player_id == room.host_side && req.client_id == room.host_client_id;
    let is_guest = req.player_id == room.guest_side
        && room.guest_client_id.as_deref() == Some(req.client_id.as_str());
    if !is_host && !is_guest {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "이 방의 플레이어만 기권할 수 있습니다.".into(),
            }),
        ));
    }

    let game_id = room.game_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "아직 시작되지 않은 방입니다.".into(),
            }),
        )
    })?;
    let mut entry = app.games.get_mut(&game_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방의 게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    let state = entry.value_mut();
    if state.phase != GamePhase::Ended {
        state.phase = GamePhase::Ended;
        state.result = Some(GameResult {
            winner: Some(opponent_side(&req.player_id)),
            reason: GameEndReason::Resignation,
        });
    }

    Ok(Json(state.clone()))
}
