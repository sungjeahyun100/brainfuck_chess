use axum::{Router, Json, extract::{State, Path}};
use axum::routing::{get, post};
use axum::http::StatusCode;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use brainfuck_chess_engine::{
    endgame::{apply_drop_action, apply_move_action},
    legal_moves::{generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares},
    pieces::default_pieces::all_default_definitions,
    placement::validate_drop_action,
    rules::{calculate_deck_score, calculate_score_limit, can_end_turn, create_board, end_turn, get_base_zone_squares, grant_move_stacks, validate_deck},
    types::*,
};

// ─── App state ────────────────────────────────────────────────────────────────

type GameStore = Arc<DashMap<String, GameState>>;

// ─── API types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateGameRequest {
    board_size: i32,
    white_deck: PlayerDeckSpec,
    black_deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
struct PlayerDeckSpec {
    starting: Vec<StartingPieceSpec>,
    pocket: Vec<String>,
}

#[derive(Deserialize)]
struct StartingPieceSpec {
    piece_type: String,
    square: Square,
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
struct PieceAttacksResponse {
    squares: Vec<Square>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn resolve_piece_type(player_id: &str, raw_piece_type: &str) -> Option<String> {
    match raw_piece_type {
        "king" | "queen" | "rook" | "bishop" | "knight" => Some(raw_piece_type.into()),
        "pawn" | "pawn-white" | "pawn-black" => Some(if player_id == "white" {
            "pawn-white".into()
        } else {
            "pawn-black".into()
        }),
        _ => None,
    }
}

fn make_piece_id(player_id: &str, piece_type: &str, counters: &mut HashMap<String, u32>) -> String {
    let next = counters.entry(piece_type.into()).or_insert(0);
    *next += 1;
    format!("{}_{}_{}", player_id, piece_type.replace('-', "_"), next)
}

fn build_player_deck(
    player_id: &str,
    spec: &PlayerDeckSpec,
    board_size: i32,
    board: &mut Board,
    pieces: &mut HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> Result<Deck, String> {
    let base_zone: HashSet<SquareId> = get_base_zone_squares(&player_id.to_string(), board_size)
        .into_iter()
        .map(|sq| sq.to_id())
        .collect();

    let mut counters = HashMap::new();
    let mut starting_pieces = Vec::new();
    let mut pocket_pieces = Vec::new();

    for placement in &spec.starting {
        if !board.is_in_bounds(&placement.square) {
            return Err(format!("{} 시작 기물 배치가 보드 밖입니다.", player_id));
        }
        if !base_zone.contains(&placement.square.to_id()) {
            return Err(format!("{} 시작 기물은 기본 진영에만 배치할 수 있습니다.", player_id));
        }
        if !board.is_empty(&placement.square) {
            return Err(format!("{} 배치 칸이 이미 사용 중입니다.", placement.square.to_id()));
        }

        let type_id = resolve_piece_type(player_id, &placement.piece_type)
            .ok_or_else(|| format!("알 수 없는 기물 타입입니다: {}", placement.piece_type))?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);

        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: Some(placement.square),
            in_pocket: false,
            captured: false,
            move_stack: 0,
            has_moved: false,
        };

        board.squares.insert(placement.square.to_id(), Some(piece_id.clone()));
        pieces.insert(piece_id.clone(), piece);
        starting_pieces.push(piece_id);
    }

    for pocket_piece in &spec.pocket {
        let type_id = resolve_piece_type(player_id, pocket_piece)
            .ok_or_else(|| format!("알 수 없는 포켓 기물 타입입니다: {}", pocket_piece))?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);
        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: None,
            in_pocket: true,
            captured: false,
            move_stack: 0,
            has_moved: false,
        };

        pieces.insert(piece_id.clone(), piece);
        pocket_pieces.push(piece_id);
    }

    let mut deck = Deck {
        player_id: player_id.into(),
        starting_pieces,
        pocket_pieces,
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };

    deck.total_score = calculate_deck_score(&deck, pieces, definitions);

    let validation = validate_deck(&deck, board_size, pieces, definitions);
    if !validation.valid {
        return Err(validation.errors.join(" "));
    }

    Ok(deck)
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let store: GameStore = Arc::new(DashMap::new());

    // Static frontend directory — populated at Docker build time.
    // Falls back gracefully if the directory doesn't exist (dev mode).
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "/app/dist".into());
    let index_fallback = format!("{}/index.html", static_dir);

    let api = Router::new()
        .route("/health", get(health))
        .route("/games", post(create_game))
        .route("/games/:id", get(get_game))
        .route("/games/:id/actions", post(submit_action))
        .route("/games/:id/end-turn", post(end_game_turn))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/piece-attacks/:piece_id", get(get_piece_attacks))
        .route("/games/:id/legal-drops", get(get_legal_drops))
        .with_state(store);

    // SPA fallback: unknown paths → index.html so Vue Router handles them.
    let spa = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(&index_fallback));

    let app = Router::new()
        .route_service("/", ServeFile::new(&index_fallback))
        .nest("/api", api)
        .fallback_service(spa);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{}", port);
    println!("Server running on {} | static dir: {}", addr, static_dir);
    println!(
        "index.html exists: {}",
        std::path::Path::new(&index_fallback).exists()
    );
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
    let mut board = create_board(req.board_size);

    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    let mut pieces = HashMap::new();

    let white_deck = build_player_deck(
        "white",
        &req.white_deck,
        req.board_size,
        &mut board,
        &mut pieces,
        &defs,
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    let black_deck = build_player_deck(
        "black",
        &req.black_deck,
        req.board_size,
        &mut board,
        &mut pieces,
        &defs,
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    let mut players = HashMap::new();
    players.insert(
        "white".into(),
        Player { id: "white".into(), deck: white_deck, captured_pieces: Vec::new() },
    );
    players.insert(
        "black".into(),
        Player { id: "black".into(), deck: black_deck, captured_pieces: Vec::new() },
    );

    let mut state = GameState {
        id: id.clone(),
        board,
        pieces,
        piece_definitions: defs,
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
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

async fn get_piece_attacks(
    State(store): State<GameStore>,
    Path((id, piece_id)): Path<(String, String)>,
) -> Result<Json<PieceAttacksResponse>, (StatusCode, Json<ErrorResponse>)> {
    match store.get(&id) {
        Some(state) => {
            let squares = generate_piece_attack_squares(&state, &piece_id);
            Ok(Json(PieceAttacksResponse { squares }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "게임을 찾을 수 없습니다.".into() }))),
    }
}

