use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{
    extract::{Path, State},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use brainfuck_chess_engine::{
    endgame::{apply_drop_action, apply_move_action},
    legal_moves::{
        generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares,
    },
    pieces::default_pieces::all_default_definitions,
    placement::validate_drop_action,
    rules::{
        calculate_deck_score, calculate_score_limit, can_end_turn, create_board, end_turn,
        get_base_zone_squares, grant_move_stacks, validate_deck,
    },
    types::*,
};

// ─── App state ────────────────────────────────────────────────────────────────

type GameStore = Arc<DashMap<String, GameState>>;
type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;

#[derive(Clone)]
struct AppState {
    games: GameStore,
    rooms: RoomStore,
}

// ─── API types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateGameRequest {
    board_size: i32,
    white_deck: PlayerDeckSpec,
    black_deck: PlayerDeckSpec,
}

#[derive(Clone, Serialize, Deserialize)]
struct MultiplayerRoom {
    id: String,
    board_size: i32,
    host_side: PlayerId,
    guest_side: PlayerId,
    #[serde(skip_serializing)]
    host_client_id: String,
    #[serde(skip_serializing)]
    guest_client_id: Option<String>,
    host_deck: PlayerDeckSpec,
    guest_deck: Option<PlayerDeckSpec>,
    game_id: Option<String>,
}

#[derive(Deserialize)]
struct CreateRoomRequest {
    board_size: i32,
    host_side: PlayerId,
    client_id: String,
    deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
struct JoinRoomRequest {
    client_id: String,
    deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
struct ResignRoomRequest {
    client_id: String,
    player_id: PlayerId,
}

#[derive(Deserialize)]
struct ResignGameRequest {
    player_id: PlayerId,
}

#[derive(Clone, Deserialize, Serialize)]
struct PlayerDeckSpec {
    starting: Vec<StartingPieceSpec>,
    pocket: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
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
        "king" | "queen" | "rook" | "bishop" | "knight" | "amazon" | "tempest-rook"
        | "bouncing-bishop" => Some(raw_piece_type.into()),
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
            return Err(format!(
                "{} 시작 기물은 기본 진영에만 배치할 수 있습니다.",
                player_id
            ));
        }
        if !board.is_empty(&placement.square) {
            return Err(format!(
                "{} 배치 칸이 이미 사용 중입니다.",
                placement.square.to_id()
            ));
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

        board
            .squares
            .insert(placement.square.to_id(), Some(piece_id.clone()));
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

fn build_game_state(
    id: String,
    board_size: i32,
    white_spec: &PlayerDeckSpec,
    black_spec: &PlayerDeckSpec,
) -> Result<GameState, String> {
    if board_size < 8 {
        return Err("보드 크기는 최소 8이어야 합니다.".into());
    }

    let mut board = create_board(board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let mut pieces = HashMap::new();

    let white_deck = build_player_deck(
        "white",
        white_spec,
        board_size,
        &mut board,
        &mut pieces,
        &defs,
    )?;
    let black_deck = build_player_deck(
        "black",
        black_spec,
        board_size,
        &mut board,
        &mut pieces,
        &defs,
    )?;

    let mut players = HashMap::new();
    players.insert(
        "white".into(),
        Player {
            id: "white".into(),
            deck: white_deck,
            captured_pieces: Vec::new(),
        },
    );
    players.insert(
        "black".into(),
        Player {
            id: "black".into(),
            deck: black_deck,
            captured_pieces: Vec::new(),
        },
    );

    let mut state = GameState {
        id,
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
    Ok(state)
}

fn opponent_side(side: &PlayerId) -> PlayerId {
    if side == "white" {
        "black".into()
    } else {
        "white".into()
    }
}

fn materialize_neutral_deck(
    spec: &PlayerDeckSpec,
    player_id: &str,
    board_size: i32,
) -> PlayerDeckSpec {
    if player_id == "white" {
        return spec.clone();
    }

    PlayerDeckSpec {
        starting: spec
            .starting
            .iter()
            .map(|piece| StartingPieceSpec {
                piece_type: piece.piece_type.clone(),
                square: Square {
                    file: piece.square.file,
                    rank: board_size - 1 - piece.square.rank,
                },
            })
            .collect(),
        pocket: spec.pocket.clone(),
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

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let state = AppState {
        games: Arc::new(DashMap::new()),
        rooms: Arc::new(DashMap::new()),
    };

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
        .route("/games/:id/resign", post(resign_game))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/piece-attacks/:piece_id", get(get_piece_attacks))
        .route("/games/:id/legal-drops", get(get_legal_drops))
        .route("/rooms", post(create_room))
        .route("/rooms/:id", get(get_room))
        .route("/rooms/:id/join", post(join_room))
        .route("/rooms/:id/resign", post(resign_room))
        .with_state(state);

    // SPA fallback: unknown paths → index.html so Vue Router handles them.
    let spa = ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_fallback));

    let app = Router::new()
        .route_service("/", ServeFile::new(&index_fallback))
        .route("/config.js", get(config_js))
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

fn app_env() -> &'static str {
    match std::env::var("APP_ENV").as_deref() {
        Ok("local") => "local",
        Ok("test") => "test",
        Ok("prod") => "prod",
        _ => "prod",
    }
}

async fn config_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        format!(
            "window.APP_CONFIG = Object.freeze({{ appEnv: '{}' }});\n",
            app_env()
        ),
    )
}

async fn create_game(
    State(app): State<AppState>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = Uuid::new_v4().to_string();
    let state = build_game_state(id.clone(), req.board_size, &req.white_deck, &req.black_deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    app.games.insert(id.clone(), state.clone());
    Ok(Json(GameResponse { id, state }))
}

async fn create_room(
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
        host_deck: req.deck,
        guest_deck: None,
        game_id: None,
    };

    app.rooms.insert(id, room.clone());
    Ok(Json(room))
}

async fn get_room(
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

async fn join_room(
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

    let game_id = Uuid::new_v4().to_string();
    let host_deck = materialize_neutral_deck(&room.host_deck, &room.host_side, room.board_size);
    let guest_deck = materialize_neutral_deck(&req.deck, &room.guest_side, room.board_size);
    let (white_deck, black_deck) = if room.host_side == "white" {
        (&host_deck, &guest_deck)
    } else {
        (&guest_deck, &host_deck)
    };
    let state = build_game_state(game_id.clone(), room.board_size, white_deck, black_deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    room.guest_deck = Some(req.deck);
    room.guest_client_id = Some(req.client_id);
    room.game_id = Some(game_id.clone());
    app.games.insert(game_id.clone(), state.clone());
    Ok(Json(GameResponse { id: game_id, state }))
}

async fn resign_room(
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

async fn resign_game(
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

async fn get_game(
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

async fn submit_action(
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
            // Validate turn mode
            if state.turn_state.mode == TurnMode::Drop {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "착수 턴에는 이동할 수 없습니다.".into(),
                    }),
                ));
            }
            // Validate it's this player's piece
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
            if piece.move_stack == 0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "이동 스택이 없습니다.".into(),
                    }),
                ));
            }
            // Validate it's a legal move
            let legal_moves = generate_legal_move_actions(state);
            let is_legal = legal_moves
                .iter()
                .any(|m| m.piece_id == action.piece_id && m.to == action.to);
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
            *state = new_state;
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

async fn get_legal_moves(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LegalMovesResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => {
            let moves = generate_legal_move_actions(&state);
            Ok(Json(LegalMovesResponse { moves }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

async fn get_legal_drops(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LegalDropsResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => {
            let drops = generate_legal_drop_actions(&state);
            Ok(Json(LegalDropsResponse { drops }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

async fn get_piece_attacks(
    State(app): State<AppState>,
    Path((id, piece_id)): Path<(String, String)>,
) -> Result<Json<PieceAttacksResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => {
            let squares = generate_piece_attack_squares(&state, &piece_id);
            Ok(Json(PieceAttacksResponse { squares }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}
