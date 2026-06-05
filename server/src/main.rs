use axum::{Router, Json, extract::{State, Path}};
use axum::routing::{get, post};
use axum::http::StatusCode;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use brainfuck_chess_engine::{
    endgame::{apply_drop_action, apply_move_action},
    legal_moves::{generate_legal_drop_actions, generate_legal_move_actions},
    pieces::default_pieces::all_default_definitions,
    placement::validate_drop_action,
    rules::{calculate_score_limit, create_board, end_turn, grant_move_stacks, can_end_turn},
    types::*,
};

// ─── App state ────────────────────────────────────────────────────────────────

type GameStore = Arc<DashMap<String, GameState>>;

// ─── API types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateGameRequest {
    board_size: i32,
}

#[derive(Serialize)]
struct GameResponse {
    id: String,
    state: GameState,
}

#[derive(Deserialize)]
struct SubmitActionRequest {
    action: TurnAction,
}

#[derive(Serialize)]
struct LegalMovesResponse {
    moves: Vec<MoveAction>,
}

#[derive(Serialize)]
struct LegalDropsResponse {
    drops: Vec<DropAction>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let store: GameStore = Arc::new(DashMap::new());

    let app = Router::new()
        .route("/health", get(health))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/actions", post(submit_action))
        .route("/games/:id/end-turn", post(end_game_turn))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/legal-drops", get(get_legal_drops))
        .with_state(store);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{}", port);
    println!("Server running on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ─── Handlers ────────────────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn create_game(
    State(store): State<GameStore>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.board_size < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "보드 크기는 최소 8이어야 합니다.".into() }),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let board = create_board(req.board_size);
    let score_limit = calculate_score_limit(req.board_size);

    let defs: std::collections::HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    let make_deck = |player_id: &str| Deck {
        player_id: player_id.into(),
        starting_pieces: Vec::new(),
        pocket_pieces: Vec::new(),
        score_limit,
        total_score: 0,
    };

    let mut players = std::collections::HashMap::new();
    players.insert(
        "white".into(),
        Player { id: "white".into(), deck: make_deck("white"), captured_pieces: Vec::new() },
    );
    players.insert(
        "black".into(),
        Player { id: "black".into(), deck: make_deck("black"), captured_pieces: Vec::new() },
    );

    let mut state = GameState {
        id: id.clone(),
        board,
        pieces: std::collections::HashMap::new(),
        piece_definitions: defs,
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Setup,
        turn_state: TurnState::new(),
        result: None,
    };

    grant_move_stacks(&mut state);

    store.insert(id.clone(), state.clone());
    Ok(Json(GameResponse { id, state }))
}

async fn get_game(
    State(store): State<GameStore>,
    Path(id): Path<String>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    match store.get(&id) {
        Some(state) => Ok(Json(state.clone())),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() }),
        )),
    }
}

async fn submit_action(
    State(store): State<GameStore>,
    Path(id): Path<String>,
    Json(req): Json<SubmitActionRequest>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    let mut entry = store
        .get_mut(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() })))?;

    let state = entry.value_mut();

    if state.phase == GamePhase::Ended {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "게임이 이미 종료되었습니다.".into() })));
    }

    match req.action {
        TurnAction::Move(action) => {
            // Validate turn mode
            if state.turn_state.mode == TurnMode::Drop {
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "착수 턴에는 이동할 수 없습니다.".into() })));
            }
            // Validate it's this player's piece
            let piece = state.pieces.get(&action.piece_id)
                .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "기물을 찾을 수 없습니다.".into() })))?;
            if piece.owner != state.current_player {
                return Err((StatusCode::FORBIDDEN, Json(ErrorResponse { error: "자신의 기물만 이동할 수 있습니다.".into() })));
            }
            if piece.move_stack == 0 {
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "이동 스택이 없습니다.".into() })));
            }
            // Validate it's a legal move
            let legal_moves = generate_legal_move_actions(state);
            let is_legal = legal_moves.iter().any(|m| m.piece_id == action.piece_id && m.to == action.to);
            if !is_legal {
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "합법적이지 않은 이동입니다.".into() })));
            }

            state.turn_state.mode = TurnMode::Move;
            let new_state = apply_move_action(state.clone(), action);
            *state = new_state;
        }
        TurnAction::Drop(action) => {
            if state.turn_state.mode == TurnMode::Move {
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "이동 턴에는 착수할 수 없습니다.".into() })));
            }
            validate_drop_action(state, &action)
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

            state.turn_state.mode = TurnMode::Drop;
            let new_state = apply_drop_action(state.clone(), action);
            *state = new_state;
        }
    }

    Ok(Json(state.clone()))
}

async fn end_game_turn(
    State(store): State<GameStore>,
    Path(id): Path<String>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    let mut entry = store
        .get_mut(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() })))?;

    let state = entry.value_mut();

    if !can_end_turn(state) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "턴을 종료하려면 최소 1개의 행동이 필요합니다.".into() })));
    }

    let new_state = end_turn(state.clone());
    *state = new_state;
    Ok(Json(state.clone()))
}

async fn get_legal_moves(
    State(store): State<GameStore>,
    Path(id): Path<String>,
) -> Result<Json<LegalMovesResponse>, (StatusCode, Json<ErrorResponse>)> {
    match store.get(&id) {
        Some(state) => {
            let moves = generate_legal_move_actions(&state);
            Ok(Json(LegalMovesResponse { moves }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() }))),
    }
}

async fn get_legal_drops(
    State(store): State<GameStore>,
    Path(id): Path<String>,
) -> Result<Json<LegalDropsResponse>, (StatusCode, Json<ErrorResponse>)> {
    match store.get(&id) {
        Some(state) => {
            let drops = generate_legal_drop_actions(&state);
            Ok(Json(LegalDropsResponse { drops }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() }))),
    }
}

