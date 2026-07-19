use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{
    extract::{Path, Query, State},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use brainfuck_chess_engine::{
    ai::{play_bot_turn_detailed, AiAction, BotDifficulty},
    endgame::submit_action as submit_engine_action,
    legal_moves::{
        generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares,
        generate_piece_legal_drop_actions, generate_piece_legal_move_actions_with_options,
        MoveGenerationOptions,
    },
    pieces::default_pieces::all_default_definitions,
    rules::{
        calculate_deck_score, calculate_score_limit, create_board, get_base_zone_squares,
        validate_deck,
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
    host_deck: Option<PlayerDeckSpec>,
    guest_deck: Option<PlayerDeckSpec>,
    host_ready: bool,
    guest_ready: bool,
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
struct SelectDeckRequest {
    client_id: String,
    deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
struct RoomReadyRequest {
    client_id: String,
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
    action: SubmitAction,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SubmitAction {
    Move(SubmitMoveRequest),
    Drop(SubmitDropRequest),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitMoveRequest {
    piece_id: PieceId,
    to: Square,
    #[serde(default)]
    promotion: Option<PieceTypeId>,
    #[serde(default)]
    move_option_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitDropRequest {
    piece_id: PieceId,
    to: Square,
}

#[derive(Deserialize)]
struct BotTurnRequest {
    bot_player_id: PlayerId,
    #[serde(default)]
    difficulty: Option<String>,
}

#[derive(Debug, Serialize)]
struct BotTurnStats {
    searched_nodes: u64,
    depth_reached: u8,
    elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
struct BotTurnResponse {
    ok: bool,
    game_state: GameState,
    actions: Vec<AiAction>,
    stats: BotTurnStats,
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
struct PieceOptionsResponse {
    moves: Vec<MoveAction>,
    attacks: Vec<Square>,
}

#[derive(Default, Deserialize)]
struct PieceOptionsQuery {
    #[serde(alias = "ability_id")]
    move_option_id: Option<String>,
}

#[derive(Clone, Deserialize)]
struct LabPieceSpec {
    id: String,
    piece_type: String,
    owner: PlayerId,
    square: Square,
    #[serde(default)]
    state: HashMap<String, PieceStateValue>,
    #[serde(default)]
    move_option_cooldowns: HashMap<String, CooldownState>,
}

#[derive(Deserialize)]
struct LabPieceOptionsRequest {
    board_size: i32,
    pieces: Vec<LabPieceSpec>,
    selected_piece_id: String,
    #[serde(alias = "ability_id")]
    move_option_id: Option<String>,
    #[serde(default)]
    global_state: HashMap<String, i32>,
}

#[derive(Serialize)]
struct LabMoveOption {
    id: String,
    name: String,
    description: String,
    available: bool,
    kind: MoveOptionKind,
    execution_mode: MoveOptionExecutionMode,
    cooldown_remaining: u32,
}

#[derive(Serialize)]
struct LabPieceOptionsResponse {
    moves: Vec<Square>,
    legal_moves: Vec<MoveAction>,
    attacks: Vec<Square>,
    move_options: Vec<LabMoveOption>,
    piece_definitions: HashMap<PieceTypeId, PieceDefinition>,
    piece_states: HashMap<PieceId, HashMap<String, PieceStateValue>>,
    piece_cooldowns: HashMap<PieceId, HashMap<String, CooldownState>>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

fn resolve_piece_type(player_id: &str, raw_piece_type: &str) -> Option<String> {
    match raw_piece_type {
        "king" | "queen" | "rook" | "bishop" | "knight" | "amazon" | "cannon-rook"
        | "cannon_rook" | "tempest-rook" | "tempest-queen" | "tempest-knight"
        | "bouncing-bishop" | "tempest-bishop" | "windmill" => {
            Some(raw_piece_type.replace('_', "-"))
        }
        "pawn" | "pawn-white" | "pawn-black" => Some(if player_id == "white" {
            "pawn-white".into()
        } else {
            "pawn-black".into()
        }),
        "tempest-pawn" | "tempest-pawn-white" | "tempest-pawn-black" => {
            Some(if player_id == "white" {
                "tempest-pawn-white".into()
            } else {
                "tempest-pawn-black".into()
            })
        }
        _ => None,
    }
}

fn make_piece_id(
    player_id: &str,
    piece_type: &str,
    counters: &mut HashMap<String, u32>,
) -> PieceId {
    let next = counters.entry(piece_type.into()).or_insert(0);
    *next += 1;
    format!("{}_{}_{}", player_id, piece_type.replace('-', "_"), next).into()
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
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
            state: definitions
                .get(&type_id)
                .map(PieceDefinition::initial_state)
                .unwrap_or_default(),
            move_option_cooldowns: HashMap::new(),
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
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
            state: definitions
                .get(&type_id)
                .map(PieceDefinition::initial_state)
                .unwrap_or_default(),
            move_option_cooldowns: HashMap::new(),
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
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);
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

    let state = GameState {
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
        global_state: HashMap::new(),
        turn_state: TurnState::new(),
        history: Vec::new(),
        result: None,
        chessembly_program_cache,
    };

    Ok(state)
}

fn build_lab_game_state(req: &LabPieceOptionsRequest) -> Result<GameState, String> {
    if !(8..=12).contains(&req.board_size) {
        return Err("보드 크기는 8부터 12까지 선택할 수 있습니다.".into());
    }

    let mut board = create_board(req.board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);
    let mut pieces = HashMap::new();
    let mut white_starting = Vec::new();
    let mut black_starting = Vec::new();
    let mut seen_piece_ids = HashSet::new();

    for lab_piece in &req.pieces {
        if lab_piece.owner != "white" && lab_piece.owner != "black" {
            return Err("기물 owner는 white 또는 black이어야 합니다.".into());
        }
        if !seen_piece_ids.insert(lab_piece.id.clone()) {
            return Err(format!("중복된 테스트 기물 id입니다: {}", lab_piece.id));
        }
        if !board.is_in_bounds(&lab_piece.square) {
            return Err(format!(
                "{} 배치가 보드 밖입니다.",
                lab_piece.square.to_id()
            ));
        }
        if !board.is_empty(&lab_piece.square) {
            return Err(format!(
                "{} 칸에 이미 기물이 있습니다.",
                lab_piece.square.to_id()
            ));
        }

        let type_id = resolve_piece_type(&lab_piece.owner, &lab_piece.piece_type)
            .ok_or_else(|| format!("알 수 없는 기물 타입입니다: {}", lab_piece.piece_type))?;
        let definition = defs
            .get(&type_id)
            .ok_or_else(|| format!("기물 정의를 찾을 수 없습니다: {type_id}"))?;
        let mut piece_state = definition.initial_state();
        for (key, value) in &lab_piece.state {
            let Some(state_definition) = definition
                .state_schema
                .iter()
                .find(|state| state.key == *key)
            else {
                return Err(format!("{type_id}: 알 수 없는 기물 상태 키입니다: {key}"));
            };
            if std::mem::discriminant(&state_definition.default_value)
                != std::mem::discriminant(value)
            {
                return Err(format!(
                    "{type_id}: 기물 상태 `{key}`의 값 타입이 올바르지 않습니다."
                ));
            }
            piece_state.insert(key.clone(), value.clone());
        }
        if let Some(option_id) = lab_piece.move_option_cooldowns.keys().find(|option_id| {
            !definition
                .move_options
                .iter()
                .any(|option| option.id.as_str() == option_id.as_str())
        }) {
            return Err(format!(
                "{type_id}: 알 수 없는 이동 옵션 쿨타임입니다: {option_id}"
            ));
        }
        let piece_id = PieceId::from(lab_piece.id.clone());
        let piece = Piece {
            id: piece_id.clone(),
            owner: lab_piece.owner.clone(),
            type_id: type_id.clone(),
            current_square: Some(lab_piece.square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
            state: piece_state,
            move_option_cooldowns: lab_piece.move_option_cooldowns.clone(),
        };

        board
            .squares
            .insert(lab_piece.square.to_id(), Some(piece_id.clone()));
        if lab_piece.owner == "white" {
            white_starting.push(piece_id.clone());
        } else {
            black_starting.push(piece_id.clone());
        }
        pieces.insert(piece_id, piece);
    }

    let selected_piece_id = PieceId::from(req.selected_piece_id.clone());
    let selected_piece = pieces
        .get(&selected_piece_id)
        .ok_or_else(|| "선택한 테스트 기물을 찾을 수 없습니다.".to_string())?;
    let current_player = selected_piece.owner.clone();

    let white_deck = Deck {
        player_id: "white".into(),
        starting_pieces: white_starting,
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(req.board_size),
        total_score: 0,
    };
    let black_deck = Deck {
        player_id: "black".into(),
        starting_pieces: black_starting,
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(req.board_size),
        total_score: 0,
    };

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

    Ok(GameState {
        id: "piece-lab".into(),
        board,
        pieces,
        piece_definitions: defs,
        players,
        current_player,
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: req.global_state.clone(),
        turn_state: TurnState::new(),
        history: Vec::new(),
        result: None,
        chessembly_program_cache,
    })
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
        .route("/games/:id/bot-turn", post(run_bot_turn))
        .route("/games/:id/resign", post(resign_game))
        .route("/games/:id/legal-moves", get(get_legal_moves))
        .route("/games/:id/piece-attacks/:piece_id", get(get_piece_attacks))
        .route(
            "/games/:id/pieces/:piece_id/options",
            get(get_piece_options),
        )
        .route("/lab/piece-options", post(get_lab_piece_options))
        .route("/games/:id/legal-drops", get(get_legal_drops))
        .route("/rooms", post(create_room))
        .route("/rooms/:id", get(get_room))
        .route("/rooms/:id/join", post(join_room))
        .route("/rooms/:id/select-deck", post(select_room_deck))
        .route("/rooms/:id/ready", post(ready_room))
        .route("/rooms/:id/unready", post(unready_room))
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
        host_deck: Some(req.deck),
        guest_deck: None,
        host_ready: true,
        guest_ready: false,
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

async fn select_room_deck(
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

async fn ready_room(
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

async fn unready_room(
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
        SubmitAction::Move(request) => {
            let piece = state.pieces.get(&request.piece_id).ok_or_else(|| {
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
            // The request contains selection data only. Capture and effects are
            // regenerated from authoritative state and never trusted from JSON.
            let move_options = MoveGenerationOptions {
                move_option_id: request.move_option_id.clone(),
            };
            let matching_actions = generate_piece_legal_move_actions_with_options(
                state,
                &request.piece_id,
                &move_options,
            )
            .into_iter()
            .filter(|m| {
                m.to == request.to
                    && m.promotion == request.promotion
                    && request
                        .move_option_id
                        .as_ref()
                        .is_none_or(|option_id| m.move_option_id == *option_id)
            })
            .collect::<Vec<_>>();
            let [legal_action] = matching_actions.as_slice() else {
                let error = if matching_actions.len() > 1 {
                    "동일 조건의 이동이 여러 개여서 선택이 모호합니다."
                } else {
                    "합법적이지 않은 이동입니다."
                };
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: error.into(),
                    }),
                ));
            };
            *state = submit_engine_action(state.clone(), TurnAction::Move(legal_action.clone()))
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
        }
        SubmitAction::Drop(request) => {
            let legal_action = generate_piece_legal_drop_actions(state, &request.piece_id)
                .into_iter()
                .find(|action| action.to == request.to);
            let Some(legal_action) = legal_action else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "착수 가능한 칸이 아닙니다.".into(),
                    }),
                ));
            };
            *state = submit_engine_action(state.clone(), TurnAction::Drop(legal_action))
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
        }
    }

    Ok(Json(state.clone()))
}

async fn run_bot_turn(
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

async fn get_legal_moves(
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

async fn get_legal_drops(
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

async fn get_piece_attacks(
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

async fn get_piece_options(
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
                    move_option_id: query.move_option_id,
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

async fn get_lab_piece_options(
    Json(req): Json<LabPieceOptionsRequest>,
) -> Result<Json<LabPieceOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let state = build_lab_game_state(&req)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let piece_id = PieceId::from(req.selected_piece_id.clone());
    let legal_moves = generate_piece_legal_move_actions_with_options(
        &state,
        &piece_id,
        &MoveGenerationOptions {
            move_option_id: req.move_option_id.clone(),
        },
    );
    let mut seen_moves = HashSet::new();
    let moves = legal_moves
        .iter()
        .map(|action| action.to)
        .filter(|square| seen_moves.insert(square.to_id()))
        .collect();
    let mut seen_attacks = HashSet::new();
    let attacks = generate_piece_attack_squares(&state, &piece_id)
        .into_iter()
        .filter(|square| seen_attacks.insert(square.to_id()))
        .collect();
    let move_options = state
        .pieces
        .get(&piece_id)
        .and_then(|piece| {
            state
                .piece_definitions
                .get(&piece.type_id)
                .map(|definition| (piece, definition))
        })
        .map(|(piece, definition)| {
            definition
                .move_options
                .iter()
                .map(|option| {
                    let cooldown_remaining = piece
                        .move_option_cooldowns
                        .get(&option.id)
                        .map_or(0, |cooldown| cooldown.remaining);
                    LabMoveOption {
                        id: option.id.clone(),
                        name: option.name.clone(),
                        description: option.description.clone(),
                        available: option.execution_mode == MoveOptionExecutionMode::MoveModifier
                            && cooldown_remaining == 0,
                        kind: option.kind,
                        execution_mode: option.execution_mode,
                        cooldown_remaining,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(LabPieceOptionsResponse {
        moves,
        legal_moves,
        attacks,
        move_options,
        piece_definitions: state.piece_definitions.clone(),
        piece_states: state
            .pieces
            .iter()
            .map(|(piece_id, piece)| (piece_id.clone(), piece.state.clone()))
            .collect(),
        piece_cooldowns: state
            .pieces
            .iter()
            .map(|(piece_id, piece)| (piece_id.clone(), piece.move_option_cooldowns.clone()))
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let app = AppState {
            games: Arc::new(DashMap::new()),
            rooms: Arc::new(DashMap::new()),
        };
        app.games.insert(game_id.clone(), state);
        (app, game_id)
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
        let app = AppState {
            games: Arc::new(DashMap::new()),
            rooms: Arc::new(DashMap::new()),
        };
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_white_rook_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pieces: vec![
                LabPieceSpec {
                    id: "lab_white_rook_1".into(),
                    piece_type: "rook".into(),
                    owner: "white".into(),
                    square: Square::new(3, 3),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                },
                LabPieceSpec {
                    id: "lab_black_knight_1".into(),
                    piece_type: "knight".into(),
                    owner: "black".into(),
                    square: Square::new(3, 6),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                },
            ],
        };

        let response = match get_lab_piece_options(Json(req)).await {
            Ok(Json(response)) => response,
            Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
        };

        assert!(response.moves.contains(&Square::new(3, 6)));
        assert!(response.attacks.contains(&Square::new(3, 6)));
        assert!(app.games.is_empty());
    }

    #[tokio::test]
    async fn lab_piece_options_respects_submitted_piece_state() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_white_windmill_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pieces: vec![LabPieceSpec {
                id: "lab_white_windmill_1".into(),
                piece_type: "windmill".into(),
                owner: "white".into(),
                square: Square::new(3, 3),
                state: HashMap::from([("mode".into(), PieceStateValue::Text("rook".into()))]),
                move_option_cooldowns: HashMap::new(),
            }],
        };

        let response = match get_lab_piece_options(Json(req)).await {
            Ok(Json(response)) => response,
            Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
        };

        assert!(response.moves.contains(&Square::new(4, 3)));
        assert!(!response.moves.contains(&Square::new(4, 4)));
    }

    #[tokio::test]
    async fn submit_move_action_automatically_ends_turn() {
        let (app, game_id) = test_app_with_game();

        let response = match submit_action(
            State(app.clone()),
            Path(game_id.clone()),
            Json(SubmitActionRequest {
                action: SubmitAction::Move(SubmitMoveRequest {
                    piece_id: "white_rook_1".into(),
                    to: Square::new(0, 1),
                    promotion: None,
                    move_option_id: None,
                }),
            }),
        )
        .await
        {
            Ok(Json(state)) => state,
            Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
        };

        assert_eq!(response.current_player, "black");
        assert_eq!(response.turn_number, 2);
        assert!(response.turn_state.actions.is_empty());
        let stored = app.games.get(&game_id).unwrap();
        assert_eq!(stored.current_player, "black");
    }

    #[tokio::test]
    async fn submit_move_rejects_unknown_option_and_noncanonical_destination() {
        let (app, game_id) = test_app_with_game();
        let unknown_option = submit_action(
            State(app.clone()),
            Path(game_id.clone()),
            Json(SubmitActionRequest {
                action: SubmitAction::Move(SubmitMoveRequest {
                    piece_id: "white_rook_1".into(),
                    to: Square::new(0, 1),
                    promotion: None,
                    move_option_id: Some("missing".into()),
                }),
            }),
        )
        .await;
        assert!(matches!(unknown_option, Err((StatusCode::BAD_REQUEST, _))));

        let illegal_destination = submit_action(
            State(app),
            Path(game_id),
            Json(SubmitActionRequest {
                action: SubmitAction::Move(SubmitMoveRequest {
                    piece_id: "white_rook_1".into(),
                    to: Square::new(1, 1),
                    promotion: None,
                    move_option_id: None,
                }),
            }),
        )
        .await;
        assert!(matches!(
            illegal_destination,
            Err((StatusCode::BAD_REQUEST, _))
        ));
    }

    #[tokio::test]
    async fn bot_turn_api_runs_and_persists_a_complete_turn() {
        let (app, game_id) = test_app_with_game();

        let response = match run_bot_turn(
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
        assert!(
            response.game_state.phase == GamePhase::Ended
                || response.game_state.current_player == "black"
        );
        let stored = app.games.get(&game_id).unwrap();
        assert_eq!(stored.current_player, response.game_state.current_player);
        assert_eq!(stored.turn_number, response.game_state.turn_number);
    }

    #[tokio::test]
    async fn submit_move_action_applies_canonical_piece_state_effect() {
        let game_id = "windmill-game".to_string();
        let white_deck = PlayerDeckSpec {
            starting: vec![
                StartingPieceSpec {
                    piece_type: "king".into(),
                    square: Square::new(4, 0),
                },
                StartingPieceSpec {
                    piece_type: "windmill".into(),
                    square: Square::new(3, 1),
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
        let app = AppState {
            games: Arc::new(DashMap::new()),
            rooms: Arc::new(DashMap::new()),
        };
        app.games.insert(game_id.clone(), state);

        let response = match submit_action(
            State(app),
            Path(game_id),
            Json(SubmitActionRequest {
                action: SubmitAction::Move(SubmitMoveRequest {
                    piece_id: "white_windmill_1".into(),
                    to: Square::new(4, 2),
                    promotion: None,
                    move_option_id: None,
                }),
            }),
        )
        .await
        {
            Ok(Json(state)) => state,
            Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
        };

        assert_eq!(
            response.pieces["white_windmill_1"].state.get("mode"),
            Some(&PieceStateValue::Text("rook".into()))
        );
        assert!(!response.global_state.contains_key("mode"));
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

        let error = run_bot_turn(
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
        let error = run_bot_turn(
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
}
