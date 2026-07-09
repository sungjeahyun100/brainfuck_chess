use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use brainfuck_chess_engine::{
    ai::{play_bot_turn_detailed, BotDifficulty},
    endgame::{apply_activate_ability_action, apply_drop_action, apply_move_action},
    legal_moves::{
        generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares,
        generate_piece_legal_drop_actions, generate_piece_legal_move_actions_with_options,
        MoveGenerationOptions,
    },
    rules::{can_end_turn, end_turn},
    types::*,
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dto::error::ErrorBody as ErrorResponse;
use crate::dto::game::{
    BotTurnRequest, BotTurnResponse, BotTurnStats, CreateGameRequest, GameResponse,
    LegalDropsResponse, LegalMovesResponse, PieceAttacksResponse, PieceOptionsQuery,
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

fn has_move_or_drop_action(turn_state: &TurnState) -> bool {
    turn_state
        .actions
        .iter()
        .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
}

fn end_turn_after_action(state: GameState) -> GameState {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        state
    } else {
        end_turn(state)
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
    Ok(Json(GameResponse { id, state }))
}

pub async fn get_game(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => Ok(Json(state.clone())),
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
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
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

    Ok(Json(state.clone()))
}

pub async fn submit_action(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SubmitActionRequest>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    let mut entry = app.games.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    let state = entry.value_mut();

    if state.phase == GamePhase::Ended {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "게임이 이미 종료되었습니다.".into(),
            }),
        ));
    }

    if has_move_or_drop_action(&state.turn_state) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "이번 턴에는 이미 행동했습니다. 턴을 종료하세요.".into(),
            }),
        ));
    }

    match req.action {
        TurnAction::Move(action) => {
            if action.player_id != state.current_player {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "현재 턴 플레이어만 행동할 수 있습니다.".into(),
                    }),
                ));
            }
            if state.turn_state.mode == TurnMode::Drop {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "착수 턴에는 이동할 수 없습니다.".into(),
                    }),
                ));
            }
            let piece = state.pieces.get(&action.piece_id).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "기물을 찾을 수 없습니다.".into(),
                    }),
                )
            })?;
            if piece.owner != state.current_player {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "자신의 기물만 이동할 수 있습니다.".into(),
                    }),
                ));
            }
            let move_options = MoveGenerationOptions {
                ability_id: action.ability_id.clone(),
            };
            let is_legal = generate_piece_legal_move_actions_with_options(
                state,
                &action.piece_id,
                &move_options,
            )
            .iter()
            .any(|m| {
                m.from == action.from
                    && m.to == action.to
                    && m.promotion == action.promotion
                    && m.ability_id == action.ability_id
            });
            if !is_legal {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "합법적이지 않은 이동입니다.".into(),
                    }),
                ));
            }

            state.turn_state.mode = TurnMode::Move;
            let new_state = apply_move_action(state.clone(), action);
            *state = end_turn_after_action(new_state);
        }
        TurnAction::Drop(action) => {
            if action.player_id != state.current_player {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "현재 턴 플레이어만 행동할 수 있습니다.".into(),
                    }),
                ));
            }
            if state.turn_state.mode == TurnMode::Move {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "이동 턴에는 착수할 수 없습니다.".into(),
                    }),
                ));
            }
            let is_legal = generate_piece_legal_drop_actions(state, &action.piece_id)
                .iter()
                .any(|d| d.player_id == action.player_id && d.to == action.to);
            if !is_legal {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "착수 가능한 칸이 아닙니다.".into(),
                    }),
                ));
            }

            state.turn_state.mode = TurnMode::Drop;
            let new_state = apply_drop_action(state.clone(), action);
            *state = end_turn_after_action(new_state);
        }
        TurnAction::ActivateAbility(action) => {
            if action.player_id != state.current_player {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "현재 턴 플레이어만 행동할 수 있습니다.".into(),
                    }),
                ));
            }
            if state.turn_state.mode == TurnMode::Drop {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "착수 턴에는 능력을 발동할 수 없습니다.".into(),
                    }),
                ));
            }

            let piece = state.pieces.get(&action.piece_id).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "기물을 찾을 수 없습니다.".into(),
                    }),
                )
            })?;
            if piece.owner != state.current_player {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "자신의 기물 능력만 발동할 수 있습니다.".into(),
                    }),
                ));
            }
            if !piece.is_on_board() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "보드 위의 기물만 능력을 발동할 수 있습니다.".into(),
                    }),
                ));
            }
            if piece.active_ability.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "이미 활성화된 능력이 있습니다.".into(),
                    }),
                ));
            }

            let definition = state.piece_definitions.get(&piece.type_id).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "기물 정의를 찾을 수 없습니다.".into(),
                    }),
                )
            })?;
            let ability = definition
                .abilities
                .iter()
                .find(|ability| ability.id == action.ability_id)
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "해당 기물에 없는 능력입니다.".into(),
                        }),
                    )
                })?;
            if ability.once_per_turn
                && state.turn_state.actions.iter().any(|existing| {
                    matches!(
                        existing,
                        TurnAction::ActivateAbility(previous)
                            if previous.piece_id == action.piece_id
                                && previous.ability_id == action.ability_id
                    )
                })
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "이 능력은 같은 턴에 한 번만 발동할 수 있습니다.".into(),
                    }),
                ));
            }

            state.turn_state.mode = TurnMode::Move;
            let new_state = apply_activate_ability_action(state.clone(), action);
            *state = new_state;
        }
    }

    Ok(Json(state.clone()))
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
        game_state: result.state,
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
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
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
    Ok(Json(state.clone()))
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
