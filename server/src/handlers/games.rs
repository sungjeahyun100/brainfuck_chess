use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use brainfuck_chess_engine::{
    actions::service::submit_turn_action,
    ai::{play_bot_turn_detailed, BotDifficulty},
    legal_moves::{
        generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares,
        generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
    },
    rules::{can_end_turn, end_turn},
    types::*,
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dto::error::{bad_request, not_found, ErrorBody as ErrorResponse};
use crate::dto::game::{
    snapshot_for_state, BotTurnRequest, BotTurnResponse, BotTurnStats, CreateGameRequest,
    GameResponse, LegalDropsResponse, LegalMovesResponse, PieceAttacksResponse, PieceOptionsQuery,
    PieceOptionsResponse, ResignGameRequest, SubmitActionRequest,
};
use crate::services::game_builder::build_game_state;

fn opponent_side(side: &PlayerId) -> PlayerId {
    if side == "white" {
        "black".into()
    } else {
        "white".into()
    }
}

pub async fn create_game(
    State(app): State<AppState>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = Uuid::new_v4().to_string();
    let state = build_game_state(id.clone(), req.board_size, &req.white_deck, &req.black_deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    app.games.insert(id.clone(), state.clone());
    Ok(Json(GameResponse {
        id,
        state: snapshot_for_state(state),
    }))
}

pub async fn get_game(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GameSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => Ok(Json(snapshot_for_state(state.clone()))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

pub async fn resign_game(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ResignGameRequest>,
) -> Result<Json<GameSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    if req.player_id != "white" && req.player_id != "black" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "진영은 white 또는 black이어야 합니다.".into(),
            }),
        ));
    }

    let mut entry = app.games.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
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

    Ok(Json(snapshot_for_state(state.clone())))
}

pub async fn submit_action(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SubmitActionRequest>,
) -> impl IntoResponse {
    let Some(mut entry) = app.games.get_mut(&id) else {
        return not_found("게임을 찾을 수 없습니다.");
    };

    match submit_turn_action(entry.clone(), req.action) {
        Ok(next_state) => {
            *entry = next_state.clone();
            Json(snapshot_for_state(next_state)).into_response()
        }
        Err(error) => bad_request(error.to_string()),
    }
}

pub async fn bot_turn(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BotTurnRequest>,
) -> Result<Json<BotTurnResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.bot_player_id != "white" && req.bot_player_id != "black" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "bot_player_id는 white 또는 black이어야 합니다.".into(),
            }),
        ));
    }
    let difficulty = match req.difficulty.as_deref().unwrap_or("normal") {
        "easy" => BotDifficulty::Easy,
        "normal" => BotDifficulty::Normal,
        "hard" => BotDifficulty::Hard,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "difficulty는 easy, normal, hard 중 하나여야 합니다.".into(),
                }),
            ));
        }
    };

    let mut entry = app.games.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;
    if entry.phase == GamePhase::Ended || entry.result.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "게임이 이미 종료되었습니다.".into(),
            }),
        ));
    }
    if entry.current_player != req.bot_player_id {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "현재 턴 플레이어와 bot_player_id가 일치하지 않습니다.".into(),
            }),
        ));
    }

    let result = play_bot_turn_detailed(entry.clone(), &req.bot_player_id, difficulty)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    *entry = result.state.clone();

    Ok(Json(BotTurnResponse {
        ok: true,
        game_state: snapshot_for_state(result.state),
        actions: result.actions,
        stats: BotTurnStats {
            searched_nodes: result.searched_nodes,
            depth_reached: result.depth_reached,
            elapsed_ms: result.elapsed_ms,
        },
    }))
}

pub async fn end_game_turn(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GameSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    let mut entry = app.games.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    let state = entry.value_mut();

    if !can_end_turn(state) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "턴을 종료하려면 최소 1개의 행동이 필요합니다.".into(),
            }),
        ));
    }

    let new_state = end_turn(state.clone());
    *state = new_state;
    Ok(Json(snapshot_for_state(state.clone())))
}

pub async fn get_legal_moves(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LegalMovesResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => Ok(Json(LegalMovesResponse {
            moves: generate_legal_move_actions(&state),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

pub async fn get_legal_drops(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LegalDropsResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => Ok(Json(LegalDropsResponse {
            drops: generate_legal_drop_actions(&state),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

pub async fn get_piece_attacks(
    State(app): State<AppState>,
    Path((id, piece_id)): Path<(String, String)>,
) -> Result<Json<PieceAttacksResponse>, (StatusCode, Json<ErrorResponse>)> {
    let piece_id = PieceId::from(piece_id);
    match app.games.get(&id) {
        Some(state) => Ok(Json(PieceAttacksResponse {
            squares: generate_piece_attack_squares(&state, &piece_id),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

pub async fn get_piece_options(
    State(app): State<AppState>,
    Path((id, piece_id)): Path<(String, String)>,
    Query(query): Query<PieceOptionsQuery>,
) -> Result<Json<PieceOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let piece_id = PieceId::from(piece_id);
    match app.games.get(&id) {
        Some(state) => {
            let moves = generate_piece_legal_move_actions_with_options(
                &state,
                &piece_id,
                &MoveGenerationOptions {
                    ability_id: query.ability_id,
                },
            );
            let attacks = generate_piece_attack_squares(&state, &piece_id);
            Ok(Json(PieceOptionsResponse { moves, attacks }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}
