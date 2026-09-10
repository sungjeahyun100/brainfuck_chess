use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{
    extract::{Path, Query, State},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

mod account;
mod analysis;
mod app_state;
mod auth;
mod challenge;
mod custom_piece;
mod database;
mod deck;
mod draw;
mod game_record;
mod game_view;
mod profiling;
mod request_guard;
mod routes;
mod stores;
mod time_control;

use account::ProfileVisibility;
use app_state::AppState;
use game_record::{GameRecordOwnership, GameRecordPlayer};
use game_view::{Audience, GameAccess};
use stores::RoomStore;
use time_control::{now_ms, GameSyncView, GameView, StoredGame, TimeControlId};

use brainfuck_chess_engine::{
    actions::submit_action as submit_engine_action,
    ai::{play_bot_turn_detailed, AiAction, BotDifficulty},
    attack_map::generate_attack_map,
    custom_pieces::{install_runtime_catalog, CustomPiecePackage},
    legal_moves::{
        generate_legal_drop_actions, generate_legal_move_actions, generate_piece_attack_squares,
        generate_piece_legal_ability_actions, generate_piece_legal_drop_actions,
        generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
    },
    pieces::default_pieces::all_default_definitions,
    rules::{
        board_map_definition, calculate_deck_score, calculate_score_limit, create_board,
        create_board_with_variant, get_base_zone_squares_with_ruleset, standard_board_map_id,
        validate_deck_with_ruleset,
    },
    types::*,
};

fn game_client(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-game-client-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
}

fn opponent_player(side: &str) -> PlayerId {
    if side == "white" {
        "black".into()
    } else {
        "white".into()
    }
}

fn require_control(
    game: &StoredGame,
    client: Option<&str>,
    side: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if game
        .challenge
        .as_ref()
        .is_some_and(|context| context.metadata.player_id != side)
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Challenge 상대 진영은 직접 조작할 수 없습니다.".into(),
            }),
        ));
    }
    if game.audience(client).controls(side) {
        return Ok(());
    }
    Err((
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "이 진영의 참가자만 요청할 수 있습니다.".into(),
        }),
    ))
}

// ─── API types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateGameRequest {
    #[serde(default)]
    ruleset: DeckRuleset,
    board_size: i32,
    #[serde(default)]
    map_id: Option<String>,
    #[serde(default)]
    board_variant: BoardVariant,
    white_deck: PlayerDeckSpec,
    black_deck: PlayerDeckSpec,
    #[serde(default)]
    time_control: TimeControlId,
    #[serde(default)]
    local_side: Option<PlayerId>,
    #[serde(default)]
    bot_player_id: Option<PlayerId>,
    #[serde(default)]
    local_nickname: Option<String>,
    #[serde(default)]
    guest_nickname: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateChallengeGameRequest {
    player_deck: ChallengePlayerDeckSpec,
    #[serde(default)]
    local_nickname: Option<String>,
}

// Format belongs to the submitted player deck, never to the server challenge.
// Old Legacy clients omitted map/size; only that historical request defaults.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengePlayerDeckSpec {
    #[serde(flatten)]
    deck: PlayerDeckSpec,
    map_id: Option<String>,
    board_size: Option<i32>,
}

#[derive(Clone, Deserialize)]
struct MultiplayerRoom {
    #[serde(default)]
    ruleset: DeckRuleset,
    id: String,
    board_size: i32,
    map_id: String,
    #[serde(default)]
    board_variant: BoardVariant,
    host_side: PlayerId,
    guest_side: PlayerId,
    host_client_id: String,
    guest_client_id: Option<String>,
    host_owner_id: String,
    guest_owner_id: Option<String>,
    host_deck: Option<PlayerDeckSpec>,
    guest_deck: Option<PlayerDeckSpec>,
    host_ready: bool,
    guest_ready: bool,
    game_id: Option<String>,
    #[serde(default)]
    time_control: TimeControlId,
}

impl Serialize for MultiplayerRoom {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::json!({
            "ruleset": self.ruleset, "id": self.id, "board_size": self.board_size,
            "map_id": self.map_id, "board_variant": self.board_variant,
            "host_side": self.host_side, "guest_side": self.guest_side,
            "host_ready": self.host_ready, "guest_ready": self.guest_ready,
            "game_id": self.game_id, "time_control": self.time_control,
        });
        // The lobby only needs readiness for Standard; reserve contents are private.
        value["host_deck"] = if self.ruleset == DeckRuleset::Legacy {
            serde_json::to_value(&self.host_deck).map_err(serde::ser::Error::custom)?
        } else {
            serde_json::Value::Null
        };
        value["guest_deck"] = if self.ruleset == DeckRuleset::Legacy {
            serde_json::to_value(&self.guest_deck).map_err(serde::ser::Error::custom)?
        } else {
            serde_json::Value::Null
        };
        value["host_has_deck"] = self.host_deck.is_some().into();
        value["guest_has_deck"] = self.guest_deck.is_some().into();
        value.serialize(serializer)
    }
}

#[derive(Deserialize)]
struct CreateRoomRequest {
    #[serde(default)]
    ruleset: DeckRuleset,
    board_size: i32,
    #[serde(default)]
    map_id: Option<String>,
    #[serde(default)]
    board_variant: BoardVariant,
    host_side: PlayerId,
    client_id: String,
    deck: PlayerDeckSpec,
    #[serde(default)]
    time_control: TimeControlId,
}

fn ensure_ruleset(expected: DeckRuleset, actual: DeckRuleset) -> Result<(), String> {
    if expected != actual {
        return Err("덱과 게임의 룰이 다릅니다. 같은 룰의 덱을 선택하세요.".into());
    }
    Ok(())
}

fn resolve_board_map(
    map_id: Option<&str>,
    board_size: i32,
    legacy_variant: BoardVariant,
) -> Result<(String, BoardVariant), String> {
    if let Some(map_id) = map_id {
        let map = board_map_definition(map_id)
            .ok_or_else(|| format!("지원하지 않는 맵입니다: {map_id}"))?;
        if map.board_size != board_size {
            return Err("맵과 덱의 보드 크기가 다릅니다.".into());
        }
        return Ok((map.id.into(), map.variant));
    }
    if legacy_variant == BoardVariant::CentralHighGround {
        if board_size != 12 {
            return Err("중앙 고지 보드는 12x12에서만 사용할 수 있습니다.".into());
        }
        return Ok(("central-high-ground-12x12".into(), legacy_variant));
    }
    let id = standard_board_map_id(board_size)
        .ok_or_else(|| "지원하지 않는 보드 크기입니다.".to_string())?;
    Ok((id.into(), BoardVariant::Plain))
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

#[derive(Deserialize)]
struct HeartbeatRequest {
    client_id: String,
    player_id: PlayerId,
    #[serde(default)]
    catalog_revision: Option<u64>,
    #[serde(default)]
    latest_ply: usize,
}

#[derive(Clone, Deserialize, Serialize)]
struct PlayerDeckSpec {
    #[serde(default)]
    ruleset: DeckRuleset,
    #[serde(default)]
    name: Option<String>,
    starting: Vec<StartingPieceSpec>,
    pocket: Vec<DeckPieceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extra: Vec<DeckPieceRef>,
}

#[derive(Clone, Deserialize, Serialize)]
struct StartingPieceSpec {
    #[serde(flatten)]
    piece: DeckPieceRef,
    square: Square,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum DeckPieceRef {
    BuiltIn {
        piece_type: String,
    },
    Custom {
        custom_piece_id: String,
        version: u32,
        content_hash: String,
        exposed_piece_key: String,
    },
}

#[derive(Serialize)]
struct GameResponse {
    id: String,
    state: GameView,
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
    Ability(SubmitAbilityRequest),
    ExtraSummon(SubmitExtraSummonRequest),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitExtraSummonRequest {
    extra_piece_id: PieceId,
    sacrifice_piece_ids: Vec<PieceId>,
    target_square: Square,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SummonOptionsRequest {
    extra_piece_id: PieceId,
    #[serde(default)]
    sacrifice_piece_ids: Vec<PieceId>,
}
#[derive(Serialize)]
struct SummonOptionsResponse {
    sacrifice_piece_ids: Vec<PieceId>,
    policy: brainfuck_chess_engine::summon::SummonPolicy,
    cost: u32,
    actions: Vec<brainfuck_chess_engine::types::ExtraSummonAction>,
}
fn summon_options(
    state: &GameState,
    request: &SummonOptionsRequest,
) -> Result<SummonOptionsResponse, String> {
    use brainfuck_chess_engine::summon::*;
    let sacrifice_piece_ids = sacrifice_candidates(state, &request.extra_piece_id)?;
    let piece = &state.pieces[&request.extra_piece_id];
    let cost = state.piece_definitions[&piece.type_id].score;
    let mut seen = std::collections::HashSet::new();
    let mut score = 0u64;
    for id in &request.sacrifice_piece_ids {
        if !seen.insert(id) || !sacrifice_piece_ids.contains(id) {
            return Err("제물 선택이 유효하지 않습니다.".into());
        }
        score += u64::from(state.piece_definitions[&state.pieces[id].type_id].score);
    }
    let actions = if score >= u64::from(cost) {
        generate_extra_summon_actions(state, &request.extra_piece_id, &request.sacrifice_piece_ids)?
    } else {
        Vec::new()
    };
    Ok(SummonOptionsResponse {
        sacrifice_piece_ids,
        policy: summon_policy(state.ruleset, &piece.type_id).ok_or("소환 정책이 없습니다.")?,
        cost,
        actions,
    })
}
async fn get_summon_options(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<SummonOptionsRequest>,
) -> Result<Json<SummonOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let game = app.games.get(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;
    require_control(&game, game_client(&headers), &game.current_player)?;
    summon_options(&game.state, &request)
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitAbilityRequest {
    piece_id: PieceId,
    ability_id: String,
    #[serde(default)]
    target_piece_id: Option<PieceId>,
    #[serde(default)]
    target_piece_ids: Vec<PieceId>,
    #[serde(default)]
    pocket_piece_id: Option<PieceId>,
    #[serde(default)]
    to: Option<Square>,
    #[serde(default)]
    deployments: Vec<AbilityDeployment>,
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
    score: i32,
    searched_nodes: u64,
    depth_reached: u8,
    completed_depth: u8,
    iterations_started: u8,
    iterations_completed: u8,
    qnodes: u64,
    beta_cutoffs: u64,
    tt_probes: u64,
    tt_hits: u64,
    tt_cutoffs: u64,
    tt_stores: u64,
    aspiration_searches: u64,
    aspiration_researches: u64,
    aspiration_fail_lows: u64,
    aspiration_fail_highs: u64,
    generated_legal_actions: u64,
    unique_canonical_actions: u64,
    beam_selected_actions: u64,
    mandatory_tactical_actions: u64,
    drop_actions_generated: u64,
    drop_actions_selected: u64,
    board_optional_actions_generated: u64,
    board_optional_actions_selected: u64,
    quiet_drop_actions_generated: u64,
    quiet_drop_actions_selected: u64,
    normal_nodes: u64,
    move_generation_nanos: u64,
    canonical_deduplication_nanos: u64,
    move_ordering_nanos: u64,
    root_generated_legal_actions: u64,
    root_unique_canonical_actions: u64,
    root_beam_selected_actions: u64,
    root_mandatory_tactical_actions: u64,
    root_drop_actions_generated: u64,
    root_drop_actions_selected: u64,
    root_board_optional_actions_generated: u64,
    root_board_optional_actions_selected: u64,
    root_quiet_drop_actions_generated: u64,
    root_quiet_drop_actions_selected: u64,
    elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
struct BotTurnResponse {
    ok: bool,
    game_state: GameView,
    actions: Vec<AiAction>,
    timeline: Vec<game_view::TimelineFrameView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<BotTurnStats>,
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
    ability_actions: Vec<AbilityAction>,
}

#[derive(Default, Deserialize)]
struct PieceOptionsQuery {
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
    #[serde(default)]
    current_ammo: Option<u32>,
    #[serde(default)]
    layer: PieceLayer,
    #[serde(default)]
    remaining_flight_turns: u32,
}

#[derive(Clone, Deserialize)]
struct LabPocketPieceSpec {
    id: String,
    piece_type: String,
    owner: PlayerId,
    #[serde(default)]
    state: HashMap<String, PieceStateValue>,
    #[serde(default)]
    current_ammo: Option<u32>,
}

#[derive(Deserialize)]
struct LabPieceOptionsRequest {
    board_size: i32,
    pieces: Vec<LabPieceSpec>,
    #[serde(default)]
    pocket_pieces: Vec<LabPocketPieceSpec>,
    #[serde(default)]
    custom_pieces: Vec<LabCustomPieceRef>,
    selected_piece_id: String,
    move_option_id: Option<String>,
    #[serde(default)]
    global_state: HashMap<String, i32>,
}

#[derive(Deserialize)]
struct LabApplyActionRequest {
    lab: LabPieceOptionsRequest,
    action: TurnAction,
}

#[derive(Clone, Deserialize)]
struct LabCustomPieceRef {
    custom_piece_id: String,
    version: u32,
    content_hash: String,
    exposed_piece_key: String,
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
    legal_drops: Vec<DropAction>,
    legal_ability_actions: Vec<AbilityAction>,
    attacks: Vec<Square>,
    move_options: Vec<LabMoveOption>,
    piece_definitions: HashMap<PieceTypeId, PieceDefinition>,
    piece_states: HashMap<PieceId, HashMap<String, PieceStateValue>>,
    piece_cooldowns: HashMap<PieceId, HashMap<String, CooldownState>>,
    piece_runtime: HashMap<PieceId, Piece>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

fn resolve_piece_type(player_id: &str, raw_piece_type: &str) -> Option<String> {
    match raw_piece_type {
        "king"
        | "queen"
        | "rook"
        | "bishop"
        | "knight"
        | "nightrider"
        | "amazon"
        | "guhang"
        | "prime-minister"
        | "prime_minister"
        | "cannon-rook"
        | "cannon_rook"
        | "tempest-rook"
        | "tempest-queen"
        | "tempest-knight"
        | "bouncing-bishop"
        | "bouncing-rook"
        | "bouncing-queen"
        | "tempest-bishop"
        | "windmill"
        | "paratrooper"
        | "alternating-soldier"
        | "airborne"
        | "green-camp"
        | "mortar"
        | "tank"
        | "bomber"
        | "machine-gunner"
        | "machine_gunner"
        | "shell"
        | "sacrificial-shrine"
        | "sacrificial_shrine"
        | "sacrificial-lamb"
        | "sacrificial_lamb"
        | "fanatic"
        | "wall"
        | "repairman" => Some(raw_piece_type.replace('_', "-")),
        "surface-to-air-missile"
        | "surface-to-air-missile-white"
        | "surface-to-air-missile-black" => Some(if player_id == "white" {
            "surface-to-air-missile-white".into()
        } else {
            "surface-to-air-missile-black".into()
        }),
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
        "bouncing-pawn" | "bouncing-pawn-white" | "bouncing-pawn-black" => {
            Some(if player_id == "white" {
                "bouncing-pawn-white".into()
            } else {
                "bouncing-pawn-black".into()
            })
        }
        "dozer" | "dozer-white" | "dozer-black" => Some(if player_id == "white" {
            "dozer-white".into()
        } else {
            "dozer-black".into()
        }),
        _ => None,
    }
}

fn resolve_deck_piece_type(
    player_id: &str,
    piece: &DeckPieceRef,
    packages: &HashMap<(String, u32), CustomPiecePackage>,
) -> Result<String, String> {
    match piece {
        DeckPieceRef::BuiltIn { piece_type } => resolve_piece_type(player_id, piece_type)
            .ok_or_else(|| format!("알 수 없는 기물 타입입니다: {piece_type}")),
        DeckPieceRef::Custom {
            custom_piece_id,
            version,
            content_hash,
            exposed_piece_key,
        } => {
            let package = packages
                .get(&(custom_piece_id.clone(), *version))
                .ok_or_else(|| "승인되지 않은 커스텀 기물 참조입니다.".to_string())?;
            if package.content_hash != *content_hash
                || package.exposed_piece_key != *exposed_piece_key
            {
                return Err("커스텀 기물 버전 정보가 일치하지 않습니다.".into());
            }
            Ok(package.exposed_type_id.clone())
        }
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

// Admission checks for Extra/Main placement also run before room mutation.
// Full geometry, score and package authorization remain in the game factory.
fn validate_spec_deck_zones(spec: &PlayerDeckSpec) -> Result<(), String> {
    use brainfuck_chess_engine::rules::{piece_deck_zone, DeckZone, MAX_EXTRA_DECK_PIECES};
    if spec.ruleset == DeckRuleset::Legacy && !spec.extra.is_empty() {
        return Err("Legacy에서는 Extra Deck을 사용할 수 없습니다.".into());
    }
    if spec.extra.len() > MAX_EXTRA_DECK_PIECES {
        return Err("Extra Deck은 최대 3기까지 사용할 수 있습니다.".into());
    }
    let zone = |piece: &DeckPieceRef| -> Result<DeckZone, String> {
        match piece {
            DeckPieceRef::BuiltIn { piece_type } => {
                let id = resolve_piece_type("white", piece_type)
                    .ok_or_else(|| "알 수 없는 기물입니다.".to_string())?;
                Ok(piece_deck_zone(&id, spec.ruleset))
            }
            // Custom definitions currently have no Extra eligibility metadata.
            DeckPieceRef::Custom { .. } => Ok(DeckZone::Main),
        }
    };
    for piece in spec.starting.iter().map(|p| &p.piece).chain(&spec.pocket) {
        if zone(piece)? != DeckZone::Main {
            return Err(
                "Standard Extra Deck 전용 기물은 Starting/Pocket에 넣을 수 없습니다.".into(),
            );
        }
    }
    for piece in &spec.extra {
        if zone(piece)? != DeckZone::Extra {
            return Err("Extra Deck에 허용되지 않는 기물입니다.".into());
        }
    }
    Ok(())
}

fn build_player_deck(
    player_id: &str,
    spec: &PlayerDeckSpec,
    board_size: i32,
    board: &mut Board,
    pieces: &mut HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
    packages: &HashMap<(String, u32), CustomPiecePackage>,
    enforce_user_validation: bool,
) -> Result<Deck, String> {
    validate_spec_deck_zones(spec)?;
    let base_zone: HashSet<SquareId> =
        get_base_zone_squares_with_ruleset(&player_id.to_string(), board_size, spec.ruleset)
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

        let type_id = resolve_deck_piece_type(player_id, &placement.piece, packages)?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);

        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: Some(placement.square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            current_ammo: definitions
                .get(&type_id)
                .map_or(0, |definition| definition.max_ammo),
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
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
        let type_id = resolve_deck_piece_type(player_id, pocket_piece, packages)?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);
        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: None,
            in_pocket: true,
            captured: false,
            has_moved: false,
            current_ammo: definitions
                .get(&type_id)
                .map_or(0, |definition| definition.max_ammo),
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: definitions
                .get(&type_id)
                .map(PieceDefinition::initial_state)
                .unwrap_or_default(),
            move_option_cooldowns: HashMap::new(),
        };

        pieces.insert(piece_id.clone(), piece);
        pocket_pieces.push(piece_id);
    }

    let mut extra_deck_pieces = Vec::new();
    for reference in &spec.extra {
        let type_id = resolve_deck_piece_type(player_id, reference, packages)?;
        let definition = definitions
            .get(&type_id)
            .ok_or("알 수 없는 Extra 기물입니다.")?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);
        let mut piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id,
            current_square: None,
            in_pocket: false,
            captured: false,
            has_moved: false,
            current_ammo: 0,
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: HashMap::new(),
            move_option_cooldowns: HashMap::new(),
        };
        piece.initialize_from_definition(definition);
        pieces.insert(piece_id.clone(), piece);
        extra_deck_pieces.push(piece_id);
    }

    let mut deck = Deck {
        player_id: player_id.into(),
        starting_pieces,
        pocket_pieces,
        hand_pieces: Vec::new(),
        extra_deck_pieces,
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };

    deck.total_score = calculate_deck_score(&deck, pieces, definitions);

    if enforce_user_validation {
        let validation =
            validate_deck_with_ruleset(&deck, board_size, pieces, definitions, spec.ruleset);
        if !validation.valid {
            return Err(validation.errors.join(" "));
        }
    }

    Ok(deck)
}

#[cfg(test)]
fn build_game_state(
    id: String,
    board_size: i32,
    white_spec: &PlayerDeckSpec,
    black_spec: &PlayerDeckSpec,
    packages: Vec<CustomPiecePackage>,
) -> Result<GameState, String> {
    build_game_state_with_variant(
        id,
        board_size,
        BoardVariant::Plain,
        white_spec,
        black_spec,
        packages,
        true,
        true,
    )
}

fn build_game_state_with_variant(
    id: String,
    board_size: i32,
    board_variant: BoardVariant,
    white_spec: &PlayerDeckSpec,
    black_spec: &PlayerDeckSpec,
    packages: Vec<CustomPiecePackage>,
    validate_white_as_user_deck: bool,
    validate_black_as_user_deck: bool,
) -> Result<GameState, String> {
    ensure_ruleset(white_spec.ruleset, black_spec.ruleset)?;
    if board_size < 8 {
        return Err("보드 크기는 최소 8이어야 합니다.".into());
    }

    let board = create_board_with_variant(board_size, board_variant)?;
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);
    let pieces = HashMap::new();

    let mut state = GameState {
        ruleset: white_spec.ruleset,
        id,
        board,
        pieces,
        piece_definitions: defs,
        custom_piece_manifest: Vec::new(),
        players: HashMap::new(),
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: HashMap::new(),
        history: Vec::new(),
        result: None,
        chessembly_program_cache,
    };
    install_runtime_catalog(&mut state, &packages).map_err(|error| error.to_string())?;
    let package_index = packages
        .into_iter()
        .map(|package| ((package.package_id.clone(), package.version), package))
        .collect::<HashMap<_, _>>();

    let white_deck = build_player_deck(
        "white",
        white_spec,
        board_size,
        &mut state.board,
        &mut state.pieces,
        &state.piece_definitions,
        &package_index,
        validate_white_as_user_deck,
    )?;
    let black_deck = build_player_deck(
        "black",
        black_spec,
        board_size,
        &mut state.board,
        &mut state.pieces,
        &state.piece_definitions,
        &package_index,
        validate_black_as_user_deck,
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

    state.players = players;
    Ok(state)
}

fn build_lab_game_state(
    req: &LabPieceOptionsRequest,
    packages: &[CustomPiecePackage],
) -> Result<GameState, String> {
    if !(8..=12).contains(&req.board_size) {
        return Err("보드 크기는 8부터 12까지 선택할 수 있습니다.".into());
    }

    let board = create_board(req.board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);
    let mut catalog_state = GameState {
        ruleset: Default::default(),
        id: "piece-lab-catalog".into(),
        board,
        pieces: HashMap::new(),
        piece_definitions: defs,
        custom_piece_manifest: Vec::new(),
        players: HashMap::new(),
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: HashMap::new(),
        history: Vec::new(),
        result: None,
        chessembly_program_cache,
    };
    install_runtime_catalog(&mut catalog_state, packages).map_err(|error| error.to_string())?;
    let mut board = catalog_state.board;
    let defs = catalog_state.piece_definitions;
    let chessembly_program_cache = catalog_state.chessembly_program_cache;
    let custom_piece_manifest = catalog_state.custom_piece_manifest;
    let mut pieces = HashMap::new();
    let mut white_starting = Vec::new();
    let mut black_starting = Vec::new();
    let mut white_pocket = Vec::new();
    let mut black_pocket = Vec::new();
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
        if !board.is_empty_at_layer(&lab_piece.square, lab_piece.layer) {
            return Err(format!(
                "{} 칸에 이미 기물이 있습니다.",
                lab_piece.square.to_id()
            ));
        }

        let type_id = resolve_piece_type(&lab_piece.owner, &lab_piece.piece_type)
            .or_else(|| {
                defs.contains_key(&lab_piece.piece_type)
                    .then(|| lab_piece.piece_type.clone())
            })
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
            current_ammo: lab_piece.current_ammo.unwrap_or(definition.max_ammo),
            layer: lab_piece.layer,
            remaining_flight_turns: lab_piece.remaining_flight_turns,
            state: piece_state,
            move_option_cooldowns: lab_piece.move_option_cooldowns.clone(),
        };

        board.set_piece_at_layer(lab_piece.square, lab_piece.layer, Some(piece_id.clone()));
        if lab_piece.owner == "white" {
            white_starting.push(piece_id.clone());
        } else {
            black_starting.push(piece_id.clone());
        }
        pieces.insert(piece_id, piece);
    }

    for lab_piece in &req.pocket_pieces {
        if lab_piece.owner != "white" && lab_piece.owner != "black" {
            return Err("포켓 기물 owner는 white 또는 black이어야 합니다.".into());
        }
        if !seen_piece_ids.insert(lab_piece.id.clone()) {
            return Err(format!("중복된 테스트 기물 id입니다: {}", lab_piece.id));
        }
        let type_id = resolve_piece_type(&lab_piece.owner, &lab_piece.piece_type)
            .or_else(|| {
                defs.contains_key(&lab_piece.piece_type)
                    .then(|| lab_piece.piece_type.clone())
            })
            .ok_or_else(|| format!("알 수 없는 포켓 기물 타입입니다: {}", lab_piece.piece_type))?;
        let definition = defs
            .get(&type_id)
            .ok_or_else(|| format!("기물 정의를 찾을 수 없습니다: {type_id}"))?;
        let mut piece_state = definition.initial_state();
        for (key, value) in &lab_piece.state {
            let state_definition = definition
                .state_schema
                .iter()
                .find(|state| state.key == *key)
                .ok_or_else(|| format!("{type_id}: 알 수 없는 기물 상태 키입니다: {key}"))?;
            if std::mem::discriminant(&state_definition.default_value)
                != std::mem::discriminant(value)
            {
                return Err(format!(
                    "{type_id}: 기물 상태 `{key}`의 값 타입이 올바르지 않습니다."
                ));
            }
            piece_state.insert(key.clone(), value.clone());
        }
        let piece_id = PieceId::from(lab_piece.id.clone());
        pieces.insert(
            piece_id.clone(),
            Piece {
                id: piece_id.clone(),
                owner: lab_piece.owner.clone(),
                type_id,
                current_square: None,
                in_pocket: true,
                captured: false,
                has_moved: false,
                current_ammo: lab_piece.current_ammo.unwrap_or(definition.max_ammo),
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
                state: piece_state,
                move_option_cooldowns: HashMap::new(),
            },
        );
        if lab_piece.owner == "white" {
            white_pocket.push(piece_id);
        } else {
            black_pocket.push(piece_id);
        }
    }

    let selected_piece_id = PieceId::from(req.selected_piece_id.clone());
    let selected_piece = pieces
        .get(&selected_piece_id)
        .ok_or_else(|| "선택한 테스트 기물을 찾을 수 없습니다.".to_string())?;
    let current_player = selected_piece.owner.clone();

    let white_deck = Deck {
        hand_pieces: Vec::new(),
        extra_deck_pieces: Vec::new(),
        player_id: "white".into(),
        starting_pieces: white_starting,
        pocket_pieces: white_pocket,
        score_limit: calculate_score_limit(req.board_size),
        total_score: 0,
    };
    let black_deck = Deck {
        player_id: "black".into(),
        starting_pieces: black_starting,
        hand_pieces: Vec::new(),
        extra_deck_pieces: Vec::new(),
        pocket_pieces: black_pocket,
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
        ruleset: Default::default(),
        id: "piece-lab".into(),
        board,
        pieces,
        piece_definitions: defs,
        custom_piece_manifest,
        players,
        current_player,
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: req.global_state.clone(),
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
        ruleset: spec.ruleset,
        name: spec.name.clone(),
        starting: spec
            .starting
            .iter()
            .map(|piece| StartingPieceSpec {
                piece: piece.piece.clone(),
                square: Square {
                    file: piece.square.file,
                    rank: board_size - 1 - piece.square.rank,
                },
            })
            .collect(),
        pocket: spec.pocket.clone(),
        extra: spec.extra.clone(),
    }
}

async fn resolve_custom_packages(
    app: &AppState,
    decks: &[(&str, &PlayerDeckSpec)],
) -> Result<Vec<CustomPiecePackage>, String> {
    let mut packages = HashMap::<(String, u32), CustomPiecePackage>::new();
    for (owner, deck) in decks {
        let refs = deck
            .starting
            .iter()
            .map(|placement| &placement.piece)
            .chain(deck.pocket.iter())
            .chain(deck.extra.iter());
        for piece in refs {
            let DeckPieceRef::Custom {
                custom_piece_id,
                version,
                content_hash,
                exposed_piece_key,
            } = piece
            else {
                continue;
            };
            let key = (custom_piece_id.clone(), *version);
            if let Some(existing) = packages.get(&key) {
                if existing.content_hash != *content_hash
                    || existing.exposed_piece_key != *exposed_piece_key
                {
                    return Err("서로 다른 커스텀 기물 버전 정보가 충돌합니다.".into());
                }
                continue;
            }
            let package = app
                .custom_pieces
                .runtime_package(owner, custom_piece_id, *version)
                .await
                .map_err(|_| "커스텀 기물 저장소를 사용할 수 없습니다.".to_string())?
                .ok_or_else(|| "커스텀 기물이 없거나 사용할 권한이 없습니다.".to_string())?;
            if package.content_hash != *content_hash
                || package.exposed_piece_key != *exposed_piece_key
            {
                return Err("커스텀 기물 버전 정보가 일치하지 않습니다.".into());
            }
            packages.insert(key, package);
        }
    }
    Ok(packages.into_values().collect())
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

async fn start_room_game(
    room: &mut MultiplayerRoom,
    app: &AppState,
) -> Result<Option<GameResponse>, String> {
    if let Some(game_id) = &room.game_id {
        let mut state = app
            .games
            .get_mut(game_id)
            .ok_or_else(|| "방의 게임을 찾을 수 없습니다.".to_string())?;
        let now = now_ms();
        state.adjudicate(now);
        return Ok(Some(GameResponse {
            id: game_id.clone(),
            state: state.view(now),
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
    ensure_ruleset(room.ruleset, host_spec.ruleset)?;
    ensure_ruleset(room.ruleset, guest_spec.ruleset)?;
    let game_id = Uuid::new_v4().to_string();
    let host_deck = materialize_neutral_deck(host_spec, &room.host_side, room.board_size);
    let guest_deck = materialize_neutral_deck(guest_spec, &room.guest_side, room.board_size);
    let (white_deck, black_deck) = if room.host_side == "white" {
        (&host_deck, &guest_deck)
    } else {
        (&guest_deck, &host_deck)
    };
    let host_owner = room.host_owner_id.as_str();
    let guest_owner = room
        .guest_owner_id
        .as_deref()
        .ok_or_else(|| "참가자 인증 정보가 없습니다.".to_string())?;
    let packages =
        resolve_custom_packages(app, &[(host_owner, host_spec), (guest_owner, guest_spec)]).await?;
    let state = build_game_state_with_variant(
        game_id.clone(),
        room.board_size,
        room.board_variant,
        white_deck,
        black_deck,
        packages,
        true,
        true,
    )?;
    let (white_owner, black_owner) = if room.host_side == "white" {
        (host_owner, guest_owner)
    } else {
        (guest_owner, host_owner)
    };
    let (record_players, record_ownership) =
        game_record_players(app, white_owner, black_owner).await;

    let now = now_ms();
    let mut stored = StoredGame::new_with_players_and_deck_names(
        state,
        room.time_control,
        true,
        now,
        record_players,
        record_ownership,
        HashMap::from([
            (
                "white".into(),
                white_deck
                    .name
                    .clone()
                    .unwrap_or_else(|| "white deck".into()),
            ),
            (
                "black".into(),
                black_deck
                    .name
                    .clone()
                    .unwrap_or_else(|| "black deck".into()),
            ),
        ]),
        room.map_id.clone(),
    );
    stored.initialize_draws()?;
    room.game_id = Some(game_id.clone());
    stored.access = GameAccess::Multiplayer {
        clients: HashMap::from([
            (room.host_client_id.clone(), room.host_side.clone()),
            (
                room.guest_client_id
                    .clone()
                    .ok_or("참가자 식별자가 없습니다.")?,
                room.guest_side.clone(),
            ),
        ]),
    };
    let view = stored.view(now);
    app.games.insert(game_id.clone(), stored);
    Ok(Some(GameResponse {
        id: game_id,
        state: view,
    }))
}

async fn game_record_players(
    app: &AppState,
    white_owner: &str,
    black_owner: &str,
) -> (HashMap<PlayerId, GameRecordPlayer>, GameRecordOwnership) {
    let white_registered = app
        .accounts
        .authenticated_user(white_owner)
        .await
        .ok()
        .flatten()
        .is_some();
    let black_registered = app
        .accounts
        .authenticated_user(black_owner)
        .await
        .ok()
        .flatten()
        .is_some();
    let white = game_record_player(app, white_owner, "white").await;
    let black = game_record_player(app, black_owner, "black").await;
    let persist = white_registered || black_registered;
    (
        HashMap::from([("white".into(), white), ("black".into(), black)]),
        GameRecordOwnership {
            white_user_id: (!white_owner.is_empty()).then(|| white_owner.to_owned()),
            black_user_id: (!black_owner.is_empty()).then(|| black_owner.to_owned()),
            persist,
        },
    )
}

async fn game_record_player(app: &AppState, owner: &str, side: &str) -> GameRecordPlayer {
    let profile = app.accounts.authenticated_user(owner).await.ok().flatten();
    GameRecordPlayer {
        public_id: profile
            .as_ref()
            .and_then(|profile| profile.public_id.clone()),
        nickname: profile
            .and_then(|profile| profile.display_name)
            .unwrap_or_else(|| {
                if side == "white" {
                    "White Player".into()
                } else {
                    "Black Player".into()
                }
            }),
        side: side.into(),
    }
}

fn normalize_game_nickname(value: Option<&str>, fallback: &str) -> Result<String, String> {
    let nickname = value.unwrap_or(fallback).trim();
    if nickname.is_empty()
        || nickname.chars().count() > 30
        || nickname.chars().any(char::is_control)
    {
        return Err("닉네임은 1~30자의 표시 가능한 문자여야 합니다.".into());
    }
    Ok(nickname.to_owned())
}

async fn singleplayer_record_players(
    app: &AppState,
    owner: &str,
    local_side: &str,
    local_nickname: Option<&str>,
    guest_nickname: Option<&str>,
) -> Result<(HashMap<PlayerId, GameRecordPlayer>, GameRecordOwnership), String> {
    if local_side != "white" && local_side != "black" {
        return Err("로컬 플레이어 진영이 올바르지 않습니다.".into());
    }
    let opponent_side = if local_side == "white" {
        "black"
    } else {
        "white"
    };
    let mut local = game_record_player(app, owner, local_side).await;
    local.nickname = normalize_game_nickname(local_nickname, &local.nickname)?;
    let guest = GameRecordPlayer {
        public_id: None,
        nickname: normalize_game_nickname(guest_nickname, "Guest")?,
        side: opponent_side.into(),
    };
    let registered = app
        .accounts
        .authenticated_user(owner)
        .await
        .ok()
        .flatten()
        .is_some();
    let mut players = HashMap::new();
    players.insert(local_side.into(), local);
    players.insert(opponent_side.into(), guest);
    Ok((
        players,
        GameRecordOwnership {
            white_user_id: (local_side == "white" && !owner.is_empty()).then(|| owner.to_owned()),
            black_user_id: (local_side == "black" && !owner.is_empty()).then(|| owner.to_owned()),
            persist: registered,
        },
    ))
}

async fn run_game_time_adjudicator(
    games: stores::GameStore,
    records: game_record::GameRecordStore,
    challenge_progress: challenge::ChallengeProgressStore,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
    loop {
        interval.tick().await;
        let ids = games
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        let now = now_ms();
        for id in ids {
            if let Some(mut game) = games.get_mut(&id) {
                game.adjudicate(now);
            }
            if let Err(error) =
                persist_completed_record(&games, &records, &challenge_progress, &id).await
            {
                eprintln!("failed to persist completed game record {id}: {error}");
            }
        }
    }
}

async fn run_game_record_cleanup(records: game_record::GameRecordStore) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
    loop {
        interval.tick().await;
        if let Err(error) = records.cleanup_expired(now_ms()).await {
            eprintln!("failed to clean expired game records: {error}");
        }
    }
}

async fn persist_completed_record(
    games: &stores::GameStore,
    records: &game_record::GameRecordStore,
    challenge_progress: &challenge::ChallengeProgressStore,
    game_id: &str,
) -> Result<(), &'static str> {
    let Some((record, challenge_context)) = games.get(game_id).and_then(|game| {
        game.completed_record()
            .map(|record| (record, game.challenge.clone()))
    }) else {
        return Ok(());
    };
    if record.ownership.has_registered_owner() {
        records.save(&record).await?;
    }
    if let Some(context) = challenge_context {
        let player_won = record
            .result
            .as_ref()
            .and_then(|result| result.winner.as_deref())
            == Some(context.metadata.player_id.as_str());
        if player_won {
            if let Some(user_id) = context.registered_user_id.as_deref() {
                challenge_progress
                    .record_clear(
                        user_id,
                        &context.metadata.id,
                        record.ended_at_ms.unwrap_or_else(now_ms),
                    )
                    .await?;
            }
        }
    }
    if let Some(mut game) = games.get_mut(game_id) {
        game.mark_record_persisted();
    }
    Ok(())
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    challenge::validate_registry(&challenge::definitions()).unwrap_or_else(|error| {
        panic!("server startup blocked by invalid Challenge definitions: {error}")
    });
    let environment = app_env();
    let state = AppState::from_env(environment)
        .await
        .unwrap_or_else(|error| panic!("server startup blocked: {error}"));
    tokio::spawn(run_game_time_adjudicator(
        state.games.clone(),
        state.game_records.clone(),
        state.challenge_progress.clone(),
    ));
    tokio::spawn(run_game_record_cleanup(state.game_records.clone()));

    // Static frontend directory — populated at Docker build time.
    // Falls back gracefully if the directory doesn't exist (dev mode).
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "/app/dist".into());
    let index_fallback = format!("{}/index.html", static_dir);

    let api = routes::api(state);

    // SPA fallback: unknown paths → index.html so Vue Router handles them.
    let spa = ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_fallback));

    let app = Router::new()
        .route_service("/", ServeFile::new(&index_fallback))
        .route("/config.js", get(config_js))
        .nest("/api", api)
        .fallback_service(spa)
        .layer(axum::middleware::from_fn(
            request_guard::block_sensitive_paths,
        ));

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

async fn get_piece_scores() -> Json<HashMap<PieceTypeId, u32>> {
    Json(
        default_piece_catalog()
            .into_iter()
            .map(|(piece_type, metadata)| (piece_type, metadata.score))
            .collect(),
    )
}

#[derive(Clone, Copy, Serialize)]
struct PieceCatalogMetadata {
    score: u32,
    max_ammo: u32,
    deployment_zone: DeploymentZone,
    standard_deck_zone: brainfuck_chess_engine::rules::DeckZone,
}

fn default_piece_catalog() -> HashMap<PieceTypeId, PieceCatalogMetadata> {
    let mut catalog: HashMap<PieceTypeId, PieceCatalogMetadata> = all_default_definitions()
        .into_iter()
        .map(|definition| {
            (
                definition.id.clone(),
                PieceCatalogMetadata {
                    standard_deck_zone: brainfuck_chess_engine::rules::piece_deck_zone(
                        &definition.id,
                        DeckRuleset::Standard,
                    ),
                    score: definition.score,
                    max_ammo: definition.max_ammo,
                    deployment_zone: definition.deployment_zone,
                },
            )
        })
        .collect();

    // The deck builder uses color-neutral pawn IDs while the engine keeps
    // direction-specific definitions.
    for (neutral, white) in [
        ("pawn", "pawn-white"),
        ("tempest-pawn", "tempest-pawn-white"),
        ("bouncing-pawn", "bouncing-pawn-white"),
        ("dozer", "dozer-white"),
        ("surface-to-air-missile", "surface-to-air-missile-white"),
    ] {
        if let Some(metadata) = catalog.get(white).copied() {
            catalog.insert(neutral.into(), metadata);
        }
    }
    catalog
}

async fn get_piece_catalog() -> Json<HashMap<PieceTypeId, PieceCatalogMetadata>> {
    Json(default_piece_catalog())
}

async fn list_challenges(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<challenge::ChallengeSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let owner = custom_piece::authenticated_owner(&app, &headers).ok();
    let cleared = if let Some(owner) = owner.as_deref() {
        app.challenge_progress
            .list_clears(owner)
            .await
            .map_err(|_| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "Challenge 클리어 기록을 불러오지 못했습니다.".into(),
                    }),
                )
            })?
            .into_iter()
            .map(|clear| clear.challenge_id)
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    Ok(Json(
        challenge::definitions()
            .into_iter()
            .filter(|definition| definition.enabled)
            .map(|definition| challenge::ChallengeSummary {
                id: definition.id,
                name: definition.name,
                description: definition.description,
                ruleset: definition.ruleset,
                board_size: definition.board_size,
                map_id: definition.map_id.into(),
                bot_difficulty: definition.bot_difficulty,
                time_control: definition.time_control,
                cleared: cleared.contains(definition.id),
            })
            .collect(),
    ))
}

async fn create_challenge_game(
    State(app): State<AppState>,
    Path(challenge_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<CreateChallengeGameRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let definition = challenge::find(&challenge_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Challenge를 찾을 수 없습니다.".into(),
            }),
        )
    })?;
    create_challenge_game_from_definition(app, headers, req, &definition).await
}

async fn create_challenge_game_from_definition(
    app: AppState,
    headers: HeaderMap,
    req: CreateChallengeGameRequest,
    definition: &challenge::ChallengeDefinition,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let bad_request = |error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error }));
    challenge::validate_registry(std::slice::from_ref(definition)).map_err(bad_request)?;
    ensure_ruleset(definition.ruleset, req.player_deck.deck.ruleset).map_err(bad_request)?;
    let legacy = definition.ruleset == DeckRuleset::Legacy;
    if req
        .player_deck
        .map_id
        .as_deref()
        .unwrap_or(if legacy { definition.map_id } else { "" })
        != definition.map_id
        || req
            .player_deck
            .board_size
            .unwrap_or(if legacy { definition.board_size } else { 0 })
            != definition.board_size
    {
        return Err(bad_request(
            "플레이어 덱의 map/board가 Challenge와 일치해야 합니다.".into(),
        ));
    }
    let player_deck = req.player_deck.deck;
    let owner = custom_piece::authenticated_owner(&app, &headers).unwrap_or_default();
    let packages = resolve_custom_packages(&app, &[(&owner, &player_deck)])
        .await
        .map_err(|error| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse { error }),
            )
        })?;
    let opponent_deck = challenge::opponent_deck(&definition, "black");
    let id = Uuid::new_v4().to_string();
    let state = build_game_state_with_variant(
        id.clone(),
        definition.board_size,
        brainfuck_chess_engine::rules::board_map_definition(definition.map_id)
            .unwrap()
            .variant,
        &player_deck,
        &opponent_deck,
        packages,
        true,
        false,
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let now = now_ms();
    let (record_players, record_ownership) = singleplayer_record_players(
        &app,
        &owner,
        "white",
        req.local_nickname.as_deref(),
        Some("Challenge Bot"),
    )
    .await
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let registered_user_id = record_ownership
        .white_user_id
        .clone()
        .filter(|_| record_ownership.persist);
    let mut stored = StoredGame::new_with_players_and_deck_names(
        state,
        definition.time_control,
        false,
        now,
        record_players,
        record_ownership,
        HashMap::from([
            (
                "white".into(),
                player_deck
                    .name
                    .clone()
                    .unwrap_or_else(|| "Player Deck".into()),
            ),
            ("black".into(), definition.name.into()),
        ]),
        definition.map_id.into(),
    );
    stored.initialize_draws().map_err(draw_error)?;
    stored.access = GameAccess::Local {
        client: game_client(&headers).unwrap_or_default().to_owned(),
        human: Some("white".into()),
    };
    stored.set_challenge(challenge::ChallengeGameContext {
        metadata: challenge::ChallengeGameMetadata {
            id: definition.id.into(),
            name: definition.name.into(),
            player_id: "white".into(),
            bot_player_id: "black".into(),
            bot_difficulty: definition.bot_difficulty,
        },
        registered_user_id,
    });
    let view = stored.view_for(now, stored.audience(game_client(&headers)));
    app.games.insert(id.clone(), stored);
    Ok(Json(GameResponse { id, state: view }))
}

fn app_env() -> &'static str {
    let configured = std::env::var("APP_ENV").ok();
    resolve_app_env(configured.as_deref(), cfg!(debug_assertions))
}

fn resolve_app_env(configured: Option<&str>, debug_build: bool) -> &'static str {
    match configured {
        Some("local") => "local",
        Some("test") => "test",
        Some("prod") => "prod",
        None if debug_build => "local",
        _ => "prod",
    }
}

async fn config_js() -> impl IntoResponse {
    let config = serde_json::json!({
        "appEnv": app_env(),
        "firebase": {
            "apiKey": std::env::var("FIREBASE_API_KEY").unwrap_or_default(),
            "authDomain": std::env::var("FIREBASE_AUTH_DOMAIN").unwrap_or_default(),
            "projectId": std::env::var("IDENTITY_PLATFORM_PROJECT_ID").unwrap_or_default(),
            "appId": std::env::var("FIREBASE_APP_ID").unwrap_or_default(),
        }
    });
    let serialized = serde_json::to_string(&config)
        .unwrap_or_else(|_| "{\"appEnv\":\"prod\"}".into())
        .replace('<', "\\u003c");
    (
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        format!("window.APP_CONFIG = Object.freeze({serialized});\n"),
    )
}

async fn create_game(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    ensure_ruleset(req.ruleset, req.white_deck.ruleset)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    ensure_ruleset(req.ruleset, req.black_deck.ruleset)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let owner = custom_piece::authenticated_owner(&app, &headers).unwrap_or_default();
    let packages = resolve_custom_packages(
        &app,
        &[(&owner, &req.white_deck), (&owner, &req.black_deck)],
    )
    .await
    .map_err(|error| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse { error }),
        )
    })?;
    let (map_id, board_variant) =
        resolve_board_map(req.map_id.as_deref(), req.board_size, req.board_variant)
            .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let id = Uuid::new_v4().to_string();
    let state = build_game_state_with_variant(
        id.clone(),
        req.board_size,
        board_variant,
        &req.white_deck,
        &req.black_deck,
        packages,
        true,
        true,
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let now = now_ms();
    let (record_players, record_ownership) = if let Some(local_side) = req.local_side.as_deref() {
        singleplayer_record_players(
            &app,
            &owner,
            local_side,
            req.local_nickname.as_deref(),
            req.guest_nickname.as_deref(),
        )
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?
    } else {
        game_record_players(&app, &owner, &owner).await
    };
    let mut stored = StoredGame::new_with_players_and_deck_names(
        state,
        req.time_control,
        false,
        now,
        record_players,
        record_ownership,
        HashMap::from([
            (
                "white".into(),
                req.white_deck
                    .name
                    .clone()
                    .unwrap_or_else(|| "white deck".into()),
            ),
            (
                "black".into(),
                req.black_deck
                    .name
                    .clone()
                    .unwrap_or_else(|| "black deck".into()),
            ),
        ]),
        map_id,
    );
    if let Some(bot) = req.bot_player_id.as_deref() {
        if !matches!(bot, "white" | "black")
            || req.local_side.as_deref() != Some(opponent_player(bot).as_str())
        {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Bot과 인간 진영이 올바르지 않습니다.".into(),
                }),
            ));
        }
    }
    stored.initialize_draws().map_err(draw_error)?;
    stored.access = GameAccess::Local {
        client: game_client(&headers).unwrap_or_default().to_owned(),
        human: req.bot_player_id.as_deref().map(opponent_player),
    };
    let view = stored.view_for(now, stored.audience(game_client(&headers)));
    app.games.insert(id.clone(), stored);
    Ok(Json(GameResponse { id, state: view }))
}

async fn create_room(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    ensure_ruleset(req.ruleset, req.deck.ruleset)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    validate_spec_deck_zones(&req.deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let owner = custom_piece::authenticated_owner(&app, &headers).unwrap_or_default();
    if req.board_size < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "보드 크기는 최소 8이어야 합니다.".into(),
            }),
        ));
    }
    let (map_id, board_variant) =
        resolve_board_map(req.map_id.as_deref(), req.board_size, req.board_variant)
            .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    create_board_with_variant(req.board_size, board_variant)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
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
        ruleset: req.ruleset,
        id: id.clone(),
        board_size: req.board_size,
        map_id,
        board_variant,
        guest_side: opponent_side(&req.host_side),
        host_client_id: req.client_id,
        guest_client_id: None,
        host_owner_id: owner,
        guest_owner_id: None,
        host_side: req.host_side,
        host_deck: Some(req.deck),
        guest_deck: None,
        host_ready: true,
        guest_ready: false,
        game_id: None,
        time_control: req.time_control,
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
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<Json<GameResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_spec_deck_zones(&req.deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let owner = custom_piece::authenticated_owner(&app, &headers).unwrap_or_default();
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    ensure_ruleset(room.ruleset, req.deck.ruleset)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    if req.client_id == room.host_client_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "자신이 만든 방에는 참가자로 입장할 수 없습니다.".into(),
            }),
        ));
    }

    if let Some(game_id) = &room.game_id {
        let mut state = app.games.get_mut(game_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "방의 게임을 찾을 수 없습니다.".into(),
                }),
            )
        })?;
        let now = now_ms();
        state.adjudicate(now);
        return Ok(Json(GameResponse {
            id: game_id.clone(),
            state: state.view_for(now, state.audience(Some(&req.client_id))),
        }));
    }

    room.guest_deck = Some(req.deck);
    room.guest_client_id = Some(req.client_id.clone());
    room.guest_owner_id = Some(owner);
    room.guest_ready = true;
    let mut response = start_room_game(room.value_mut(), &app)
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "양쪽 플레이어가 아직 준비되지 않았습니다.".into(),
                }),
            )
        })?;
    if let Some(game) = app.games.get(&response.id) {
        response.state = game.view_for(now_ms(), game.audience(Some(&req.client_id)));
    }
    Ok(Json(response))
}

async fn select_room_deck(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SelectDeckRequest>,
) -> Result<Json<MultiplayerRoom>, (StatusCode, Json<ErrorResponse>)> {
    validate_spec_deck_zones(&req.deck)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let owner = custom_piece::authenticated_owner(&app, &headers).unwrap_or_default();
    let room_id = id.to_uppercase();
    let mut room = app.rooms.get_mut(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    ensure_ruleset(room.ruleset, req.deck.ruleset)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    if room.game_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "이미 게임이 시작된 방에서는 덱을 변경할 수 없습니다.".into(),
            }),
        ));
    }

    if req.client_id == room.host_client_id {
        if owner != room.host_owner_id {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "방장 덱을 변경할 권한이 없습니다.".into(),
                }),
            ));
        }
        room.host_deck = Some(req.deck);
        room.host_ready = false;
        return Ok(Json(room.clone()));
    }

    if room.guest_client_id.is_none() {
        room.guest_client_id = Some(req.client_id.clone());
    }

    if room.guest_client_id.as_deref() == Some(req.client_id.as_str()) {
        room.guest_deck = Some(req.deck);
        room.guest_owner_id = Some(owner);
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

    start_room_game(room.value_mut(), &app)
        .await
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
) -> Result<Json<GameView>, (StatusCode, Json<ErrorResponse>)> {
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

    let game = entry.value_mut();
    if game.phase != GamePhase::Ended {
        game.clock.stop(now_ms());
        game.end_with_loss(&req.player_id, GameEndReason::Resignation);
    }

    Ok(Json(
        game.view_for(now_ms(), game.audience(Some(&req.client_id))),
    ))
}

async fn resign_game(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ResignGameRequest>,
) -> Result<Json<GameView>, (StatusCode, Json<ErrorResponse>)> {
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

    let game = entry.value_mut();
    require_control(game, game_client(&headers), &req.player_id)?;
    if game.phase != GamePhase::Ended {
        game.clock.stop(now_ms());
        game.end_with_loss(&req.player_id, GameEndReason::Resignation);
    }

    Ok(Json(
        game.view_for(now_ms(), game.audience(game_client(&headers))),
    ))
}

async fn heartbeat_room(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<GameSyncView>, (StatusCode, Json<ErrorResponse>)> {
    #[cfg(feature = "profiling")]
    let handler_started = std::time::Instant::now();
    let room = app.rooms.get(&id.to_uppercase()).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방을 찾을 수 없습니다.".into(),
            }),
        )
    })?;
    let authorized = (req.player_id == room.host_side && req.client_id == room.host_client_id)
        || (req.player_id == room.guest_side
            && room.guest_client_id.as_deref() == Some(req.client_id.as_str()));
    if !authorized {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "이 방의 플레이어만 heartbeat를 보낼 수 있습니다.".into(),
            }),
        ));
    }
    let game_id = room.game_id.clone().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "아직 시작되지 않은 방입니다.".into(),
            }),
        )
    })?;
    drop(room);
    #[cfg(feature = "profiling")]
    let guard_started = std::time::Instant::now();
    let mut game = app.games.get_mut(&game_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "방의 게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;
    let now = now_ms();
    game.adjudicate(now);
    if game.phase != GamePhase::Ended {
        game.heartbeat(&req.player_id, now);
    }
    #[cfg(feature = "profiling")]
    let view_started = std::time::Instant::now();
    let view = game.sync_view_for(
        now,
        req.catalog_revision,
        req.latest_ply,
        Audience::Player(&req.player_id),
    );
    #[cfg(feature = "profiling")]
    let view_nanos = view_started.elapsed().as_nanos() as u64;
    drop(game);
    #[cfg(feature = "profiling")]
    {
        let guard_nanos = guard_started.elapsed().as_nanos() as u64;
        let handler_nanos = handler_started.elapsed().as_nanos() as u64;
        let serialization_started = std::time::Instant::now();
        let response_bytes = serde_json::to_vec(&view).map_or(0, |json| json.len() as u64);
        let serialization_nanos = serialization_started.elapsed().as_nanos() as u64;
        profiling::record_heartbeat(
            handler_nanos,
            guard_nanos,
            view_nanos,
            serialization_nanos,
            response_bytes,
        );
    }
    Ok(Json(view))
}

async fn get_game(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<GameView>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get_mut(&id) {
        Some(mut game) => {
            let now = now_ms();
            game.adjudicate(now);
            Ok(Json(
                game.view_for(now, game.audience(game_client(&headers))),
            ))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    }
}

async fn get_game_record(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<game_record::GameRecord>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(mut game) = app.games.get_mut(&id) {
        let now = now_ms();
        game.adjudicate(now);
        let ended = game.phase == GamePhase::Ended;
        let record = game.record.clone();
        drop(game);
        ensure_game_record_access(&app, &headers, &record).await?;
        if !ended {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "진행 중인 게임은 아직 기보를 내보낼 수 없습니다.".into(),
                }),
            ));
        }
        if let Err(error) =
            persist_completed_record(&app.games, &app.game_records, &app.challenge_progress, &id)
                .await
        {
            eprintln!("failed to persist completed game record {id}: {error}");
        }
        if record.ownership.has_registered_owner() {
            return match app.game_records.get(&id).await {
                Ok(Some(stored)) => {
                    ensure_game_record_access(&app, &headers, &stored).await?;
                    completed_record_view(stored)
                }
                Ok(None) => Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "게임 기록을 찾을 수 없습니다.".into(),
                    }),
                )),
                Err(_) => Err(repository_error("unavailable")),
            };
        }
        return completed_record_view(record);
    }
    match app.game_records.get(&id).await {
        Ok(Some(record)) => {
            ensure_game_record_access(&app, &headers, &record).await?;
            completed_record_view(record)
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임 기록을 찾을 수 없습니다.".into(),
            }),
        )),
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "게임 기록 저장소를 사용할 수 없습니다.".into(),
            }),
        )),
    }
}

/// Completed-record boundary: authoritative Hand/Draw identities are available only
/// after existing access checks. This never changes live GameView projection.
fn completed_record_view(
    record: game_record::GameRecord,
) -> Result<Json<game_record::GameRecord>, (StatusCode, Json<ErrorResponse>)> {
    validate_completed_standard_record(&record)?;
    Ok(Json(record))
}

fn validate_record_rules_version(
    record: &game_record::GameRecord,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    record.validate_rules_version().map(|_| ()).map_err(|code| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse { error: code.into() }),
        )
    })
}

fn validate_completed_standard_record(
    record: &game_record::GameRecord,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    validate_record_rules_version(record)?;
    if record.initial_state.ruleset == DeckRuleset::Standard {
        if record.ended_at_ms.is_none() {
            return Err(private_game_record_error());
        }
        record
            .state_at_ply(record.actions.len() as u32)
            .map_err(|_| {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "완료 기록의 Draw 무결성 검증에 실패했습니다.".into(),
                    }),
                )
            })?;
    }
    Ok(())
}

async fn ensure_game_record_access(
    app: &AppState,
    headers: &HeaderMap,
    record: &game_record::GameRecord,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if app
        .auth
        .authenticate(headers)
        .ok()
        .is_some_and(|user_id| record.ownership.contains(&user_id))
    {
        return Ok(());
    }
    let Some((white_user_id, black_user_id)) = record.ownership.both_user_ids() else {
        return Err(private_game_record_error());
    };
    let white = app
        .accounts
        .authenticated_user(white_user_id)
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "계정 공개 설정을 확인할 수 없습니다.".into(),
                }),
            )
        })?;
    let black = if black_user_id == white_user_id {
        white.clone()
    } else {
        app.accounts
            .authenticated_user(black_user_id)
            .await
            .map_err(|_| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "계정 공개 설정을 확인할 수 없습니다.".into(),
                    }),
                )
            })?
    };
    if third_party_game_record_is_public(
        white.map(|profile| profile.profile_visibility),
        black.map(|profile| profile.profile_visibility),
    ) {
        Ok(())
    } else {
        Err(private_game_record_error())
    }
}

fn third_party_game_record_is_public(
    white: Option<ProfileVisibility>,
    black: Option<ProfileVisibility>,
) -> bool {
    white == Some(ProfileVisibility::Public) && black == Some(ProfileVisibility::Public)
}

fn private_game_record_error() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "게임 기록을 찾을 수 없습니다.".into(),
        }),
    )
}

async fn list_game_records(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<game_record::GameRecordSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let owner = app
        .auth
        .authenticate(&headers)
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    app.accounts
        .authenticated_user(&owner)
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "계정 정보를 불러올 수 없습니다.".into(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "로그인 계정의 기록만 조회할 수 있습니다.".into(),
                }),
            )
        })?;
    let mut summaries = app
        .game_records
        .list_summaries_for_user_id(&owner, 50)
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "게임 기록 저장소를 사용할 수 없습니다.".into(),
                }),
            )
        })?;
    for summary in &mut summaries {
        summary.analysis_count = app
            .analyses
            .list(&summary.game_id, &owner)
            .await
            .map_err(repository_error)?
            .len() as i64;
    }
    Ok(Json(summaries))
}

#[derive(Deserialize)]
struct UpdateRetentionRequest {
    permanent: bool,
}

async fn update_game_retention(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<UpdateRetentionRequest>,
) -> Result<Json<game_record::GameRecord>, (StatusCode, Json<ErrorResponse>)> {
    let owner = authenticated_record_owner(&app, &headers, &id).await?;
    app.game_records
        .set_retention(&id, &owner, request.permanent, now_ms())
        .await
        .map_err(repository_error)?
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "게임 기록을 찾을 수 없습니다.".into(),
                }),
            )
        })
}

#[derive(Deserialize)]
struct AnalysisPosition {
    base_ply: u32,
    tree_id: Option<String>,
    node_id: Option<String>,
    #[serde(default)]
    pending_actions: Vec<TurnAction>,
}
#[derive(Deserialize)]
struct AnalysisOptionsRequest {
    #[serde(flatten)]
    position: AnalysisPosition,
    piece_id: String,
    move_option_id: Option<String>,
    #[serde(default)]
    sacrifice_piece_ids: Vec<PieceId>,
}
#[derive(Serialize)]
struct AnalysisOptionsResponse {
    moves: Vec<MoveAction>,
    drops: Vec<DropAction>,
    ability_actions: Vec<AbilityAction>,
    previews: Vec<AnalysisActionPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summon: Option<SummonOptionsResponse>,
}
#[derive(Serialize)]
struct AnalysisActionPreview {
    action: TurnAction,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    draw_pending: bool,
    state_delta: Vec<game_record::StateDeltaOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_hash: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAnalysisRequest {
    base_ply: u32,
    name: Option<String>,
    action: TurnAction,
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppendAnalysisRequest {
    parent_node_id: String,
    action: TurnAction,
    expected_version: i64,
    request_id: String,
}
#[derive(Deserialize)]
struct RenameAnalysisRequest {
    name: String,
    expected_version: i64,
}
#[derive(Deserialize)]
struct DeleteAnalysisRequest {
    expected_version: i64,
}

async fn authenticated_record_owner(
    app: &AppState,
    headers: &HeaderMap,
    game_id: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let owner = app
        .auth
        .authenticate(headers)
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let record = app
        .game_records
        .get(game_id)
        .await
        .map_err(repository_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "게임 기록을 찾을 수 없습니다.".into(),
                }),
            )
        })?;
    validate_completed_standard_record(&record)?;
    if !record.ownership.contains(&owner) {
        return Err(private_game_record_error());
    }
    Ok(owner)
}

fn repository_error(error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        "forbidden" => (StatusCode::FORBIDDEN, "수정 권한이 없습니다."),
        "conflict" => (
            StatusCode::CONFLICT,
            "다른 탭에서 분석 라인이 변경되었습니다. 새로고침 후 다시 시도하세요.",
        ),
        "invalid_parent" => (StatusCode::BAD_REQUEST, "유효하지 않은 분석 분기입니다."),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            "저장소를 사용할 수 없습니다.",
        ),
    };
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

async fn owned_record_and_trees(
    app: &AppState,
    headers: &HeaderMap,
    game_id: &str,
) -> Result<
    (String, game_record::GameRecord, Vec<analysis::AnalysisTree>),
    (StatusCode, Json<ErrorResponse>),
> {
    let owner = authenticated_record_owner(app, headers, game_id).await?;
    let record = app
        .game_records
        .get(game_id)
        .await
        .map_err(repository_error)?
        .ok_or_else(|| private_game_record_error())?;
    let trees = app
        .analyses
        .list(game_id, &owner)
        .await
        .map_err(repository_error)?;
    validate_analysis_trees(&record, &trees)?;
    Ok((owner, record, trees))
}

fn draw_error(_error: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "Draw 전이를 확정하지 못했습니다. 게임 상태는 변경되지 않았습니다.".into(),
        }),
    )
}

fn replay_analysis_node(
    parent: GameState,
    node: &analysis::AnalysisNode,
) -> Result<GameState, String> {
    if parent.ruleset == DeckRuleset::Legacy && node.draws.is_empty() {
        return submit_engine_action(parent, node.action.clone());
    }
    let mut next = submit_engine_action(parent.clone(), node.action.clone())?;
    draw::replay_turn(&parent, &mut next, &node.draws)?;
    Ok(next)
}

fn validate_analysis_trees(
    record: &game_record::GameRecord,
    trees: &[analysis::AnalysisTree],
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    validate_record_rules_version(record)?;
    for tree in trees {
        if record.initial_state.ruleset == DeckRuleset::Standard
            && tree
                .nodes
                .iter()
                .map(|node| &node.id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != tree.nodes.len()
        {
            return Err(repository_error("conflict"));
        }
        let base = record.state_at_ply(tree.base_ply).map_err(|_| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "분석 기준 상태의 무결성 검증에 실패했습니다.".into(),
                }),
            )
        })?;
        let mut verified = HashMap::<String, GameState>::new();
        let mut pending = tree.nodes.iter().collect::<Vec<_>>();
        while !pending.is_empty() {
            let Some(index) = pending.iter().position(|node| {
                node.parent_node_id.is_none()
                    || node
                        .parent_node_id
                        .as_ref()
                        .is_some_and(|parent| verified.contains_key(parent))
            }) else {
                return Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "분석 부모 노드의 무결성 검증에 실패했습니다.".into(),
                    }),
                ));
            };
            let node = pending.swap_remove(index);
            let parent = node
                .parent_node_id
                .as_ref()
                .and_then(|parent_id| verified.get(parent_id).cloned())
                .unwrap_or_else(|| base.clone());
            let expected =
                analysis::normalized_state(replay_analysis_node(parent, node).map_err(|_| {
                    (
                        StatusCode::CONFLICT,
                        Json(ErrorResponse {
                            error: "분석 수의 합법성 재검증에 실패했습니다.".into(),
                        }),
                    )
                })?);
            let expected_hash = analysis::state_hash(&expected).map_err(repository_error)?;
            let actual_hash = analysis::state_hash(&node.state_after).map_err(repository_error)?;
            let legacy_hash = expected.ruleset == DeckRuleset::Legacy
                && analysis::is_legacy_state_hash(&node.state_hash);
            if expected_hash != actual_hash || (!legacy_hash && actual_hash != node.state_hash) {
                eprintln!(
                    "analysis_integrity_failure game_id={} variation_id={} node_id={} parent_node_id={} base_ply={} expected_hash={} actual_hash={} stored_hash={}",
                    record.game_id,
                    tree.id,
                    node.id,
                    node.parent_node_id.as_deref().unwrap_or("<root>"),
                    tree.base_ply,
                    expected_hash,
                    actual_hash,
                    node.state_hash,
                );
                return Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "분석 상태 무결성 검증에 실패했습니다.".into(),
                    }),
                ));
            }
            if legacy_hash {
                #[cfg(debug_assertions)]
                eprintln!("analysis_legacy_hash_verified game_id={} variation_id={} node_id={} parent_node_id={} base_ply={} canonical_hash={}", record.game_id, tree.id, node.id, node.parent_node_id.as_deref().unwrap_or("<root>"), tree.base_ply, actual_hash);
            }
            verified.insert(node.id.clone(), expected);
        }
    }
    Ok(())
}

fn analysis_state(
    record: &game_record::GameRecord,
    trees: &[analysis::AnalysisTree],
    position: &AnalysisPosition,
) -> Result<GameState, (StatusCode, Json<ErrorResponse>)> {
    validate_record_rules_version(record)?;
    if record.initial_state.ruleset == DeckRuleset::Standard {
        validate_analysis_trees(record, trees)?;
    }
    if record.initial_state.ruleset == DeckRuleset::Standard && !position.pending_actions.is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Standard 분석은 분기 저장 완료 후 다음 수를 선택하세요.".into(),
            }),
        ));
    }
    if position.pending_actions.len() > 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "저장 대기 중인 분석 수가 너무 많습니다.".into(),
            }),
        ));
    }
    let state = match (&position.tree_id, &position.node_id) {
        (None, None) => record.state_at_ply(position.base_ply).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "유효하지 않은 기준 ply입니다.".into(),
                }),
            )
        }),
        (Some(tree_id), Some(node_id)) => {
            let tree = trees
                .iter()
                .find(|tree| &tree.id == tree_id && tree.base_ply == position.base_ply)
                .ok_or_else(|| private_game_record_error())?;
            let node = tree
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "분석 노드를 찾을 수 없습니다.".into(),
                        }),
                    )
                })?;
            let parent = if let Some(parent_id) = node.parent_node_id.as_deref() {
                let parent = tree
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == parent_id)
                    .ok_or_else(|| {
                        (
                            StatusCode::CONFLICT,
                            Json(ErrorResponse {
                                error: "분석 부모 노드의 무결성 검증에 실패했습니다.".into(),
                            }),
                        )
                    })?;
                let parent_actual_hash =
                    analysis::state_hash(&parent.state_after).map_err(repository_error)?;
                if !analysis::is_legacy_state_hash(&parent.state_hash)
                    && parent_actual_hash != parent.state_hash
                {
                    return Err((
                        StatusCode::CONFLICT,
                        Json(ErrorResponse {
                            error: "분석 부모 노드의 무결성 검증에 실패했습니다.".into(),
                        }),
                    ));
                }
                parent.state_after.clone()
            } else {
                record.state_at_ply(tree.base_ply).map_err(|_| {
                    (
                        StatusCode::CONFLICT,
                        Json(ErrorResponse {
                            error: "분석 기준 상태의 무결성 검증에 실패했습니다.".into(),
                        }),
                    )
                })?
            };
            let expected =
                analysis::normalized_state(replay_analysis_node(parent, node).map_err(|_| {
                    (
                        StatusCode::CONFLICT,
                        Json(ErrorResponse {
                            error: "분석 수의 합법성 재검증에 실패했습니다.".into(),
                        }),
                    )
                })?);
            let expected_hash = analysis::state_hash(&expected).map_err(repository_error)?;
            let actual_hash = analysis::state_hash(&node.state_after).map_err(repository_error)?;
            let legacy_hash = expected.ruleset == DeckRuleset::Legacy
                && analysis::is_legacy_state_hash(&node.state_hash);
            if expected_hash != actual_hash || (!legacy_hash && actual_hash != node.state_hash) {
                eprintln!("analysis_integrity_failure game_id={} variation_id={} node_id={} parent_node_id={} base_ply={} expected_hash={} actual_hash={} stored_hash={}", record.game_id, tree.id, node.id, node.parent_node_id.as_deref().unwrap_or("<root>"), tree.base_ply, expected_hash, actual_hash, node.state_hash);
                return Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "분석 상태 무결성 검증에 실패했습니다.".into(),
                    }),
                ));
            }
            if legacy_hash {
                #[cfg(debug_assertions)]
                eprintln!("analysis_legacy_hash_verified game_id={} variation_id={} node_id={} parent_node_id={} base_ply={} canonical_hash={}", record.game_id, tree.id, node.id, node.parent_node_id.as_deref().unwrap_or("<root>"), tree.base_ply, actual_hash);
            }
            Ok(expected)
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "분석 위치가 올바르지 않습니다.".into(),
            }),
        )),
    }?;
    position
        .pending_actions
        .iter()
        .cloned()
        .try_fold(state, |state, action| {
            submit_engine_action(state, action)
                .map(analysis::normalized_state)
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))
        })
}

async fn list_analysis_trees(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<analysis::AnalysisTree>>, (StatusCode, Json<ErrorResponse>)> {
    let (_, _, trees) = owned_record_and_trees(&app, &headers, &id).await?;
    Ok(Json(trees))
}

async fn get_analysis_options(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AnalysisOptionsRequest>,
) -> Result<Json<AnalysisOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let started = std::time::Instant::now();
    let owner = authenticated_record_owner(&app, &headers, &id).await?;
    let record = app
        .game_records
        .get(&id)
        .await
        .map_err(repository_error)?
        .ok_or_else(private_game_record_error)?;
    let trees = if request.position.tree_id.is_some() {
        app.analyses
            .list(&id, &owner)
            .await
            .map_err(repository_error)?
    } else {
        Vec::new()
    };
    let state = analysis_state(&record, &trees, &request.position)?;
    let state_ready = started.elapsed();
    let piece_id: PieceId = request.piece_id.into();
    let moves = generate_piece_legal_move_actions_with_options(
        &state,
        &piece_id,
        &MoveGenerationOptions {
            move_option_id: request.move_option_id,
        },
    );
    let drops = generate_piece_legal_drop_actions(&state, &piece_id);
    let ability_actions: Vec<AbilityAction> = state
        .pieces
        .get(&piece_id)
        .and_then(|piece| state.piece_definitions.get(&piece.type_id))
        .map(|definition| {
            definition
                .move_options
                .iter()
                .flat_map(|option| {
                    generate_piece_legal_ability_actions(&state, &piece_id, &option.id)
                })
                .collect()
        })
        .unwrap_or_default();
    let summon = if state
        .players
        .get(&state.current_player)
        .is_some_and(|p| p.deck.extra_deck_pieces.contains(&piece_id))
    {
        Some(
            summon_options(
                &state,
                &SummonOptionsRequest {
                    extra_piece_id: piece_id.clone(),
                    sacrifice_piece_ids: request.sacrifice_piece_ids,
                },
            )
            .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?,
        )
    } else {
        None
    };
    let legal_ready = started.elapsed();
    let actions = moves
        .iter()
        .cloned()
        .map(TurnAction::Move)
        .chain(drops.iter().cloned().map(TurnAction::Drop))
        .chain(ability_actions.iter().cloned().map(TurnAction::Ability))
        .chain(
            summon
                .iter()
                .flat_map(|options| options.actions.iter().cloned())
                .map(TurnAction::ExtraSummon),
        );
    let previews = actions
        .filter_map(|action| {
            let next = submit_engine_action(state.clone(), action.clone()).ok()?;
            let state_after = analysis::normalized_state(next);
            let draw_pending = draw::starts_turn(&state, &state_after);
            let state_hash = if draw_pending {
                None
            } else {
                Some(analysis::state_hash(&state_after).ok()?)
            };
            let state_delta = game_record::build_state_delta(&state, &state_after);
            Some(AnalysisActionPreview {
                action,
                draw_pending,
                state_delta,
                state_hash,
            })
        })
        .collect();
    #[cfg(debug_assertions)]
    eprintln!(
        "analysis_options_timing game_id={} state_us={} legal_us={} preview_us={} total_us={}",
        id,
        state_ready.as_micros(),
        (legal_ready - state_ready).as_micros(),
        (started.elapsed() - legal_ready).as_micros(),
        started.elapsed().as_micros(),
    );
    Ok(Json(AnalysisOptionsResponse {
        moves,
        drops,
        ability_actions,
        previews,
        summon,
    }))
}

async fn create_analysis_tree(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateAnalysisRequest>,
) -> Result<Json<analysis::AnalysisTree>, (StatusCode, Json<ErrorResponse>)> {
    let owner = authenticated_record_owner(&app, &headers, &id).await?;
    let record = app
        .game_records
        .get(&id)
        .await
        .map_err(repository_error)?
        .ok_or_else(private_game_record_error)?;
    if request.request_id.len() > 100 || request.request_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "잘못된 요청 식별자입니다.".into(),
            }),
        ));
    }
    let state = record.state_at_ply(request.base_ply).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "유효하지 않은 기준 ply입니다.".into(),
            }),
        )
    })?;
    let draw_parent = (state.ruleset == DeckRuleset::Standard).then(|| state.clone());
    let next = submit_engine_action(state, request.action.clone())
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let count = app
        .analyses
        .list(&id, &owner)
        .await
        .map_err(repository_error)?
        .len();
    let name = normalized_analysis_name(
        request
            .name
            .as_deref()
            .unwrap_or(&format!("Variation {}", count + 1)),
    )?;
    let tree = analysis::new_tree(
        id,
        owner,
        name,
        request.base_ply,
        request.action,
        next,
        now_ms(),
        request.request_id.clone(),
    )
    .map_err(repository_error)?;
    let saved = app
        .analyses
        .create(tree, &request.request_id, draw_parent.as_ref())
        .await
        .map_err(repository_error)?;
    if draw_parent.is_some() {
        validate_analysis_trees(&record, std::slice::from_ref(&saved))?;
    }
    Ok(Json(saved))
}

async fn append_analysis_node(
    State(app): State<AppState>,
    Path((id, tree_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AppendAnalysisRequest>,
) -> Result<Json<analysis::AnalysisAppendResult>, (StatusCode, Json<ErrorResponse>)> {
    let started = std::time::Instant::now();
    if request.request_id.is_empty() || request.request_id.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "잘못된 요청 식별자입니다.".into(),
            }),
        ));
    }
    let owner = authenticated_record_owner(&app, &headers, &id).await?;
    let record = app
        .game_records
        .get(&id)
        .await
        .map_err(repository_error)?
        .ok_or_else(private_game_record_error)?;
    let trees = app
        .analyses
        .list(&id, &owner)
        .await
        .map_err(repository_error)?;
    let validation_ready = started.elapsed();
    let tree = trees
        .iter()
        .find(|tree| tree.id == tree_id)
        .ok_or_else(|| private_game_record_error())?;
    let position = AnalysisPosition {
        base_ply: tree.base_ply,
        tree_id: Some(tree_id.clone()),
        node_id: Some(request.parent_node_id.clone()),
        pending_actions: Vec::new(),
    };
    let state = analysis_state(&record, &trees, &position)?;
    let draw_parent = (state.ruleset == DeckRuleset::Standard).then(|| state.clone());
    let next = analysis::normalized_state(
        submit_engine_action(state, request.action.clone())
            .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?,
    );
    let node = analysis::AnalysisNode {
        id: Uuid::new_v4().to_string(),
        parent_node_id: Some(request.parent_node_id),
        action: request.action,
        draws: Vec::new(),
        state_hash: analysis::state_hash(&next).map_err(repository_error)?,
        state_after: next,
        created_at_ms: now_ms(),
        request_id: request.request_id.clone(),
    };
    let transition_ready = started.elapsed();
    let result = app
        .analyses
        .append(
            &tree_id,
            &owner,
            node,
            request.expected_version,
            &request.request_id,
            draw_parent.as_ref(),
        )
        .await
        .map_err(repository_error)?
        .ok_or_else(|| private_game_record_error())?;
    #[cfg(debug_assertions)]
    eprintln!(
        "analysis_append_timing game_id={} variation_id={} node_id={} parent_node_id={} validation_us={} transition_us={} persistence_us={} total_us={}",
        id,
        tree_id,
        result.node.id,
        result.node.parent_node_id.as_deref().unwrap_or("<root>"),
        validation_ready.as_micros(),
        (transition_ready - validation_ready).as_micros(),
        (started.elapsed() - transition_ready).as_micros(),
        started.elapsed().as_micros(),
    );
    Ok(Json(result))
}

fn normalized_analysis_name(name: &str) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "분석 이름은 1~80자로 입력하세요.".into(),
            }),
        ))
    } else {
        Ok(name.into())
    }
}

async fn rename_analysis_tree(
    State(app): State<AppState>,
    Path((id, tree_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RenameAnalysisRequest>,
) -> Result<Json<analysis::AnalysisTree>, (StatusCode, Json<ErrorResponse>)> {
    let (owner, _, trees) = owned_record_and_trees(&app, &headers, &id).await?;
    if !trees.iter().any(|tree| tree.id == tree_id) {
        return Err(private_game_record_error());
    }
    let name = normalized_analysis_name(&request.name)?;
    app.analyses
        .rename(&tree_id, &owner, &name, request.expected_version, now_ms())
        .await
        .map_err(repository_error)?
        .map(Json)
        .ok_or_else(private_game_record_error)
}
async fn delete_analysis_tree(
    State(app): State<AppState>,
    Path((id, tree_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let (owner, _, trees) = owned_record_and_trees(&app, &headers, &id).await?;
    if !trees.iter().any(|tree| tree.id == tree_id) {
        return Err(private_game_record_error());
    }
    if app
        .analyses
        .delete_tree(&tree_id, &owner)
        .await
        .map_err(repository_error)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(private_game_record_error())
    }
}
async fn delete_analysis_subtree(
    State(app): State<AppState>,
    Path((id, tree_id, node_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Json(request): Json<DeleteAnalysisRequest>,
) -> Result<Json<analysis::AnalysisTree>, (StatusCode, Json<ErrorResponse>)> {
    let (owner, _, trees) = owned_record_and_trees(&app, &headers, &id).await?;
    if !trees.iter().any(|tree| tree.id == tree_id) {
        return Err(private_game_record_error());
    }
    app.analyses
        .delete_subtree(
            &tree_id,
            &owner,
            &node_id,
            request.expected_version,
            now_ms(),
        )
        .await
        .map_err(repository_error)?
        .map(Json)
        .ok_or_else(private_game_record_error)
}

async fn submit_action(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SubmitActionRequest>,
) -> Result<Json<GameView>, (StatusCode, Json<ErrorResponse>)> {
    let mut entry = app.games.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )
    })?;

    let game = entry.value_mut();
    require_control(game, game_client(&headers), &game.current_player)?;
    let now = now_ms();
    game.adjudicate(now);

    if game.phase == GamePhase::Ended {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "게임이 이미 종료되었습니다.".into(),
            }),
        ));
    }

    let moving_player = game.current_player.clone();
    let clock_before = game.clock.snapshot(now, true);
    let state_before = game.state.clone();
    let (mut next_state, recorded_action) = match req.action {
        SubmitAction::Move(request) => {
            let piece = game.pieces.get(&request.piece_id).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "기물을 찾을 수 없습니다.".into(),
                    }),
                )
            })?;
            if piece.owner != game.current_player {
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
                &game.state,
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
            let action = TurnAction::Move(legal_action.clone());
            let state = submit_engine_action(game.state.clone(), action.clone())
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
            (state, action)
        }
        SubmitAction::Drop(request) => {
            let legal_action = generate_piece_legal_drop_actions(&game.state, &request.piece_id)
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
            let action = TurnAction::Drop(legal_action);
            let state = submit_engine_action(game.state.clone(), action.clone())
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
            (state, action)
        }
        SubmitAction::ExtraSummon(request) => {
            let action =
                TurnAction::ExtraSummon(brainfuck_chess_engine::types::ExtraSummonAction {
                    player_id: game.current_player.clone(),
                    extra_piece_id: request.extra_piece_id,
                    sacrifice_piece_ids: request.sacrifice_piece_ids,
                    target_square: request.target_square,
                });
            let state = submit_engine_action(game.state.clone(), action.clone())
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
            (state, action)
        }
        SubmitAction::Ability(request) => {
            let legal_action = AbilityAction {
                player_id: game.current_player.clone(),
                piece_id: request.piece_id,
                ability_id: request.ability_id,
                target_piece_id: request.target_piece_id,
                target_piece_ids: request.target_piece_ids,
                pocket_piece_id: request.pocket_piece_id,
                to: request.to,
                deployments: request.deployments,
            };
            let action = TurnAction::Ability(legal_action);
            let state = submit_engine_action(game.state.clone(), action.clone())
                .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
            (state, action)
        }
    };

    let confirmed_at = now_ms();
    game.adjudicate(confirmed_at);
    if game.phase == GamePhase::Ended {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "착수 확정 전에 시간이 만료되었습니다.".into(),
            }),
        ));
    }
    let draws = draw::turn_start(&state_before, &mut next_state, &mut draw::random_index)
        .map_err(draw_error)?;
    game.state = next_state;
    let next_player = game.current_player.clone();
    let ended = game.phase == GamePhase::Ended;
    game.clock
        .finish_turn(&moving_player, &next_player, confirmed_at, ended);
    let clock_after = game.clock.snapshot(confirmed_at, !ended);
    let elapsed_ms = clock_before
        .turn_started_at_ms
        .map(|started| confirmed_at.saturating_sub(started))
        .unwrap_or(0);
    game.record
        .push_action(
            moving_player.clone(),
            recorded_action,
            elapsed_ms,
            player_clock_value(&clock_before, &moving_player),
            player_clock_value(&clock_after, &moving_player),
            clock_after,
            &state_before,
            game.state.clone(),
        )
        .draws = draws;
    if ended {
        let final_state = game.state.clone();
        let final_clock = game.clock.snapshot(confirmed_at, false);
        game.record
            .finalize(&final_state, final_clock, confirmed_at);
    }
    Ok(Json(game.view_for(
        confirmed_at,
        game.audience(game_client(&headers)),
    )))
}

fn player_clock_value(clock: &time_control::ClockSnapshot, player: &str) -> Option<i64> {
    if clock.mode == time_control::TimeControlMode::Countdown {
        if player == "white" {
            clock.white_remaining_ms
        } else {
            clock.black_remaining_ms
        }
    } else if player == "white" {
        Some(clock.white_elapsed_ms)
    } else {
        Some(clock.black_elapsed_ms)
    }
}

async fn run_bot_turn(
    State(app): State<AppState>,
    headers: HeaderMap,
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
    let requested_difficulty = match req.difficulty.as_deref().unwrap_or("normal") {
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
    if entry.ruleset == DeckRuleset::Standard {
        match &entry.access {
            GameAccess::Local {
                human: Some(human), ..
            } if *human == opponent_player(&req.bot_player_id) => {
                require_control(&entry, game_client(&headers), human)?;
            }
            _ => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "설정된 Bot 게임에서만 실행할 수 있습니다.".into(),
                    }),
                ))
            }
        }
    }
    let started_at = now_ms();
    let difficulty = if let Some(context) = entry.challenge.as_ref() {
        if req.bot_player_id != context.metadata.bot_player_id {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Challenge 봇 진영은 서버 정의로 고정됩니다.".into(),
                }),
            ));
        }
        context.metadata.bot_difficulty
    } else {
        requested_difficulty
    };
    entry.adjudicate(started_at);
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

    let moving_player = entry.current_player.clone();
    let clock_before = entry.clock.snapshot(started_at, true);
    let replay_initial_state = entry.state.clone();
    let mut result = play_bot_turn_detailed(entry.state.clone(), &req.bot_player_id, difficulty)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let finished_at = now_ms();
    entry.adjudicate(finished_at);
    if entry.phase == GamePhase::Ended {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "봇 착수 확정 전에 시간이 만료되었습니다.".into(),
            }),
        ));
    }
    // Search has finished and the clock accepted the action. Only the final frame
    // can start the next player's turn; intermediate forced landings never draw.
    let draws = draw::turn_start(
        &replay_initial_state,
        &mut result.state,
        &mut draw::random_index,
    )
    .map_err(draw_error)?;
    if let Some(frame) = result.timeline.last_mut() {
        frame.state = result.state.clone();
    }
    entry.state = result.state.clone();
    let next_player = entry.current_player.clone();
    let ended = entry.phase == GamePhase::Ended;
    entry
        .clock
        .finish_turn(&moving_player, &next_player, finished_at, ended);
    let clock_after = entry.clock.snapshot(finished_at, !ended);
    let mut frame_before = replay_initial_state;
    for (index, frame) in result.timeline.iter().enumerate() {
        entry
            .record
            .push_action(
                moving_player.clone(),
                ai_action_to_turn_action(frame.action.clone()),
                if index == 0 {
                    finished_at
                        .saturating_sub(clock_before.turn_started_at_ms.unwrap_or(started_at))
                } else {
                    0
                },
                player_clock_value(&clock_before, &moving_player),
                player_clock_value(&clock_after, &moving_player),
                clock_after.clone(),
                &frame_before,
                frame.state.clone(),
            )
            .draws = if index + 1 == result.timeline.len() {
            draws.clone()
        } else {
            Vec::new()
        };
        frame_before = frame.state.clone();
    }
    if ended {
        let final_state = entry.state.clone();
        let final_clock = entry.clock.snapshot(finished_at, false);
        entry
            .record
            .finalize(&final_state, final_clock, finished_at);
    }

    Ok(Json(BotTurnResponse {
        ok: true,
        game_state: entry.view_for(finished_at, entry.audience(game_client(&headers))),
        actions: result.actions,
        timeline: result
            .timeline
            .into_iter()
            .map(|frame| game_view::TimelineFrameView {
                action: frame.action,
                state: game_view::TimelineStateView {
                    state: game_view::project_state(
                        &frame.state,
                        entry.audience(game_client(&headers)),
                    ),
                    hand_counts: game_view::hand_counts(&frame.state),
                },
            })
            .collect(),
        stats: (entry.ruleset == DeckRuleset::Legacy).then_some(BotTurnStats {
            score: result.score,
            searched_nodes: result.searched_nodes,
            depth_reached: result.depth_reached,
            completed_depth: result.completed_depth,
            iterations_started: result.stats.iterations_started,
            iterations_completed: result.stats.iterations_completed,
            qnodes: result.stats.qnodes,
            beta_cutoffs: result.stats.beta_cutoffs,
            tt_probes: result.stats.tt_probes,
            tt_hits: result.stats.tt_hits,
            tt_cutoffs: result.stats.tt_cutoffs,
            tt_stores: result.stats.tt_stores,
            aspiration_searches: result.stats.aspiration_searches,
            aspiration_researches: result.stats.aspiration_researches,
            aspiration_fail_lows: result.stats.aspiration_fail_lows,
            aspiration_fail_highs: result.stats.aspiration_fail_highs,
            generated_legal_actions: result.stats.generated_legal_actions,
            unique_canonical_actions: result.stats.unique_canonical_actions,
            beam_selected_actions: result.stats.beam_selected_actions,
            mandatory_tactical_actions: result.stats.mandatory_tactical_actions,
            drop_actions_generated: result.stats.drop_actions_generated,
            drop_actions_selected: result.stats.drop_actions_selected,
            board_optional_actions_generated: result.stats.board_optional_actions_generated,
            board_optional_actions_selected: result.stats.board_optional_actions_selected,
            quiet_drop_actions_generated: result.stats.quiet_drop_actions_generated,
            quiet_drop_actions_selected: result.stats.quiet_drop_actions_selected,
            normal_nodes: result.stats.normal_nodes,
            move_generation_nanos: result.stats.move_generation_nanos,
            canonical_deduplication_nanos: result.stats.canonical_deduplication_nanos,
            move_ordering_nanos: result.stats.move_ordering_nanos,
            root_generated_legal_actions: result.stats.root_generated_legal_actions,
            root_unique_canonical_actions: result.stats.root_unique_canonical_actions,
            root_beam_selected_actions: result.stats.root_beam_selected_actions,
            root_mandatory_tactical_actions: result.stats.root_mandatory_tactical_actions,
            root_drop_actions_generated: result.stats.root_drop_actions_generated,
            root_drop_actions_selected: result.stats.root_drop_actions_selected,
            root_board_optional_actions_generated: result
                .stats
                .root_board_optional_actions_generated,
            root_board_optional_actions_selected: result.stats.root_board_optional_actions_selected,
            root_quiet_drop_actions_generated: result.stats.root_quiet_drop_actions_generated,
            root_quiet_drop_actions_selected: result.stats.root_quiet_drop_actions_selected,
            elapsed_ms: result.elapsed_ms,
        }),
    }))
}

fn ai_action_to_turn_action(action: AiAction) -> TurnAction {
    match action {
        AiAction::Move(action) => TurnAction::Move(action),
        AiAction::Drop(action) => TurnAction::Drop(action),
        AiAction::Ability(action) => TurnAction::Ability(action),
        AiAction::ExtraSummon(action) => TurnAction::ExtraSummon(action),
    }
}

async fn get_legal_moves(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LegalMovesResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => {
            require_control(&state, game_client(&headers), &state.current_player)?;
            Ok(Json(LegalMovesResponse {
                moves: generate_legal_move_actions(&state),
            }))
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
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LegalDropsResponse>, (StatusCode, Json<ErrorResponse>)> {
    match app.games.get(&id) {
        Some(state) => {
            require_control(&state, game_client(&headers), &state.current_player)?;
            Ok(Json(LegalDropsResponse {
                drops: generate_legal_drop_actions(&state),
            }))
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

async fn get_player_attacks(
    State(app): State<AppState>,
    Path((id, player_id)): Path<(String, String)>,
) -> Result<Json<PieceAttacksResponse>, (StatusCode, Json<ErrorResponse>)> {
    if player_id != "white" && player_id != "black" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "플레이어는 white 또는 black이어야 합니다.".into(),
            }),
        ));
    }

    match app.games.get(&id) {
        Some(state) => {
            let attack_map = generate_attack_map(&state, &player_id, &HashMap::new());
            let mut squares = attack_map
                .attacked_squares
                .into_iter()
                .map(|square_id| square_id.to_square())
                .collect::<Vec<_>>();
            squares.sort_by_key(|square| (square.rank, square.file));
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

async fn get_piece_options(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((id, piece_id)): Path<(String, String)>,
    Query(query): Query<PieceOptionsQuery>,
) -> Result<Json<PieceOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    #[cfg(feature = "profiling")]
    let started = std::time::Instant::now();
    let piece_id = PieceId::from(piece_id);
    let response = match app.games.get(&id) {
        Some(state) => {
            require_control(&state, game_client(&headers), &state.current_player)?;
            let ability_actions = query
                .move_option_id
                .as_deref()
                .map(|id| generate_piece_legal_ability_actions(&state, &piece_id, id))
                .unwrap_or_default();
            let moves = if ability_actions.is_empty() {
                generate_piece_legal_move_actions_with_options(
                    &state,
                    &piece_id,
                    &MoveGenerationOptions {
                        move_option_id: query.move_option_id,
                    },
                )
            } else {
                Vec::new()
            };
            Ok(Json(PieceOptionsResponse {
                moves,
                ability_actions,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "게임을 찾을 수 없습니다.".into(),
            }),
        )),
    };
    #[cfg(feature = "profiling")]
    brainfuck_chess_engine::profiling::record_piece_options(started.elapsed());
    #[cfg(feature = "profiling")]
    if let Ok(Json(body)) = &response {
        let serialization_started = std::time::Instant::now();
        let _ = serde_json::to_vec(body);
        profiling::record_piece_options_serialization(
            serialization_started.elapsed().as_nanos() as u64
        );
    }
    response
}

async fn get_lab_piece_options(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LabPieceOptionsRequest>,
) -> Result<Json<LabPieceOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let packages = resolve_lab_packages(&app, &headers, &req.custom_pieces).await?;
    let state = build_lab_game_state(&req, &packages)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let piece_id = PieceId::from(req.selected_piece_id.clone());
    let selected_in_pocket = state
        .pieces
        .get(&piece_id)
        .is_some_and(|piece| piece.in_pocket);
    let legal_moves = if selected_in_pocket {
        Vec::new()
    } else {
        generate_piece_legal_move_actions_with_options(
            &state,
            &piece_id,
            &MoveGenerationOptions {
                move_option_id: req.move_option_id.clone(),
            },
        )
    };
    let legal_drops = if selected_in_pocket {
        generate_piece_legal_drop_actions(&state, &piece_id)
    } else {
        Vec::new()
    };
    let legal_ability_actions = if selected_in_pocket {
        Vec::new()
    } else {
        req.move_option_id
            .as_deref()
            .map(|ability_id| generate_piece_legal_ability_actions(&state, &piece_id, ability_id))
            .unwrap_or_default()
    };
    let mut seen_moves = HashSet::new();
    let moves = legal_moves
        .iter()
        .map(|action| action.to)
        .chain(legal_drops.iter().map(|action| action.to))
        .chain(legal_ability_actions.iter().filter_map(|action| action.to))
        .filter(|square| seen_moves.insert(square.to_id()))
        .collect();
    let mut seen_attacks = HashSet::new();
    let attacks = if selected_in_pocket {
        Vec::new()
    } else {
        generate_piece_attack_squares(&state, &piece_id)
    }
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
                        available: cooldown_remaining == 0
                            && option.is_enabled_for(piece)
                            && if option.execution_mode == MoveOptionExecutionMode::MoveModifier {
                                !generate_piece_legal_move_actions_with_options(
                                    &state,
                                    &piece_id,
                                    &MoveGenerationOptions {
                                        move_option_id: Some(option.id.clone()),
                                    },
                                )
                                .is_empty()
                            } else {
                                !generate_piece_legal_ability_actions(&state, &piece_id, &option.id)
                                    .is_empty()
                            },
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
        legal_drops,
        legal_ability_actions,
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
        piece_runtime: state.pieces.clone(),
    }))
}

async fn apply_lab_action(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LabApplyActionRequest>,
) -> Result<Json<GameState>, (StatusCode, Json<ErrorResponse>)> {
    let packages = resolve_lab_packages(&app, &headers, &req.lab.custom_pieces).await?;
    let state = build_lab_game_state(&req.lab, &packages)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let state = submit_engine_action(state, req.action)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    Ok(Json(state))
}

async fn resolve_lab_packages(
    app: &AppState,
    headers: &HeaderMap,
    custom_pieces: &[LabCustomPieceRef],
) -> Result<Vec<CustomPiecePackage>, (StatusCode, Json<ErrorResponse>)> {
    if custom_pieces.is_empty() {
        return Ok(Vec::new());
    }
    let owner = custom_piece::authenticated_owner(app, headers)
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let deck = PlayerDeckSpec {
        extra: Vec::new(),
        ruleset: Default::default(),
        name: None,
        starting: custom_pieces
            .iter()
            .map(|piece| StartingPieceSpec {
                piece: DeckPieceRef::Custom {
                    custom_piece_id: piece.custom_piece_id.clone(),
                    version: piece.version,
                    content_hash: piece.content_hash.clone(),
                    exposed_piece_key: piece.exposed_piece_key.clone(),
                },
                square: Square::new(0, 0),
            })
            .collect(),
        pocket: Vec::new(),
    };
    resolve_custom_packages(app, &[(owner.as_str(), &deck)])
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions;

    struct FailOnceGameRecordRepository {
        fail_next: std::sync::atomic::AtomicBool,
        saves: std::sync::Mutex<Vec<game_record::GameRecord>>,
    }

    #[async_trait::async_trait]
    impl game_record::GameRecordRepository for FailOnceGameRecordRepository {
        async fn save(&self, record: &game_record::GameRecord) -> Result<(), &'static str> {
            if self
                .fail_next
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err("injected failure");
            }
            self.saves.lock().unwrap().push(record.clone());
            Ok(())
        }
        async fn get(
            &self,
            _game_id: &str,
        ) -> Result<Option<game_record::GameRecord>, &'static str> {
            Ok(None)
        }
        async fn list_summaries_for_user_id(
            &self,
            _user_id: &str,
            _limit: i64,
        ) -> Result<Vec<game_record::GameRecordSummary>, &'static str> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn missing_app_env_is_local_only_for_debug_builds() {
        assert_eq!(resolve_app_env(None, true), "local");
        assert_eq!(resolve_app_env(None, false), "prod");
        assert_eq!(resolve_app_env(Some("prod"), true), "prod");
        assert_eq!(resolve_app_env(Some("typo"), true), "prod");
    }

    #[test]
    fn third_party_record_visibility_requires_two_public_accounts() {
        assert!(third_party_game_record_is_public(
            Some(ProfileVisibility::Public),
            Some(ProfileVisibility::Public),
        ));
        assert!(!third_party_game_record_is_public(
            Some(ProfileVisibility::Public),
            Some(ProfileVisibility::Private),
        ));
        assert!(!third_party_game_record_is_public(
            Some(ProfileVisibility::Private),
            Some(ProfileVisibility::Private),
        ));
        assert!(!third_party_game_record_is_public(
            Some(ProfileVisibility::Public),
            None,
        ));
    }

    #[test]
    fn guest_nickname_is_trimmed_and_rejects_empty_controls_and_excess_length() {
        assert_eq!(
            normalize_game_nickname(Some("  상대  "), "Guest").unwrap(),
            "상대"
        );
        assert!(normalize_game_nickname(Some(""), "Guest").is_err());
        assert!(normalize_game_nickname(Some("bad\nname"), "Guest").is_err());
        assert!(normalize_game_nickname(Some(&"x".repeat(31)), "Guest").is_err());
    }

    #[tokio::test]
    async fn singleplayer_metadata_follows_the_resolved_local_side_and_account_profile() {
        let (app, _) = test_app_with_game();
        let identity = account::VerifiedIdentity {
            issuer: "issuer".into(),
            subject: "single-subject".into(),
            provider: "google".into(),
            email: None,
            email_verified: true,
            display_name: Some("계정 닉네임".into()),
            avatar_url: None,
        };
        app.accounts
            .complete_google_login("single-user", &identity, None)
            .await
            .unwrap();
        app.accounts
            .update_profile("single-user", Some("single_public"), None, None)
            .await
            .unwrap();
        let (players, ownership) = singleplayer_record_players(
            &app,
            "single-user",
            "black",
            Some("임시 닉네임"),
            Some("로컬 상대"),
        )
        .await
        .unwrap();
        assert_eq!(players["black"].nickname, "임시 닉네임");
        assert_eq!(players["black"].public_id.as_deref(), Some("single_public"));
        assert_eq!(players["white"].nickname, "로컬 상대");
        assert_eq!(players["white"].public_id, None);
        assert_eq!(ownership.black_user_id.as_deref(), Some("single-user"));
        assert_eq!(ownership.white_user_id, None);
        assert!(ownership.has_registered_owner());
        assert_eq!(
            app.accounts
                .authenticated_user("single-user")
                .await
                .unwrap()
                .unwrap()
                .display_name
                .as_deref(),
            Some("계정 닉네임")
        );

        let (fallback, _) =
            singleplayer_record_players(&app, "single-user", "white", None, Some("Guest"))
                .await
                .unwrap();
        assert_eq!(fallback["white"].nickname, "계정 닉네임");

        app.accounts.ensure_guest("guest-session").await.unwrap();
        let (guest_players, guest_ownership) = singleplayer_record_players(
            &app,
            "guest-session",
            "white",
            Some("Local Guest"),
            Some("Guest"),
        )
        .await
        .unwrap();
        assert_eq!(guest_players["white"].nickname, "Local Guest");
        assert_eq!(
            guest_ownership.white_user_id.as_deref(),
            Some("guest-session")
        );
        assert_eq!(guest_ownership.black_user_id, None);
        assert!(!guest_ownership.has_registered_owner());

        let (_, black_guest_ownership) = singleplayer_record_players(
            &app,
            "guest-session",
            "black",
            Some("Local Guest"),
            Some("Guest"),
        )
        .await
        .unwrap();
        assert_eq!(black_guest_ownership.white_user_id, None);
        assert_eq!(
            black_guest_ownership.black_user_id.as_deref(),
            Some("guest-session")
        );
        assert!(!black_guest_ownership.persist);
    }

    #[tokio::test]
    async fn multiplayer_ownership_preserves_all_participant_sessions() {
        let (app, _) = test_app_with_game();
        for (user_id, subject, public_id) in [
            ("account-a", "subject-a", "public_a"),
            ("account-b", "subject-b", "public_b"),
        ] {
            let identity = account::VerifiedIdentity {
                issuer: "issuer".into(),
                subject: subject.into(),
                provider: "google".into(),
                email: None,
                email_verified: true,
                display_name: Some(public_id.into()),
                avatar_url: None,
            };
            app.accounts
                .complete_google_login(user_id, &identity, None)
                .await
                .unwrap();
            app.accounts
                .update_profile(user_id, Some(public_id), None, None)
                .await
                .unwrap();
        }
        app.accounts.ensure_guest("guest-a").await.unwrap();
        app.accounts.ensure_guest("guest-b").await.unwrap();
        assert!(app
            .accounts
            .authenticated_user("guest-a")
            .await
            .unwrap()
            .is_none());

        let (players, both) = game_record_players(&app, "account-a", "account-b").await;
        assert_eq!(players["white"].public_id.as_deref(), Some("public_a"));
        assert_eq!(players["black"].public_id.as_deref(), Some("public_b"));
        assert_eq!(both.white_user_id.as_deref(), Some("account-a"));
        assert_eq!(both.black_user_id.as_deref(), Some("account-b"));
        assert!(both.persist);

        let (players, account_guest) = game_record_players(&app, "account-a", "guest-b").await;
        assert_eq!(players["black"].public_id, None);
        assert_eq!(account_guest.white_user_id.as_deref(), Some("account-a"));
        assert_eq!(account_guest.black_user_id.as_deref(), Some("guest-b"));
        assert!(account_guest.persist);

        let (players, guest_account) = game_record_players(&app, "guest-a", "account-b").await;
        assert_eq!(players["white"].public_id, None);
        assert_eq!(guest_account.white_user_id.as_deref(), Some("guest-a"));
        assert_eq!(guest_account.black_user_id.as_deref(), Some("account-b"));
        assert!(guest_account.persist);

        let (players, neither) = game_record_players(&app, "guest-a", "guest-b").await;
        assert_eq!(players["white"].public_id, None);
        assert_eq!(players["black"].public_id, None);
        assert_eq!(neither.white_user_id.as_deref(), Some("guest-a"));
        assert_eq!(neither.black_user_id.as_deref(), Some("guest-b"));
        assert!(!neither.persist);
    }

    #[tokio::test]
    async fn participant_sessions_keep_replay_access_without_granting_guest_history() {
        let (app, game_id) = test_app_with_game();
        let identity = account::VerifiedIdentity {
            issuer: "issuer".into(),
            subject: "access-subject".into(),
            provider: "google".into(),
            email: None,
            email_verified: true,
            display_name: Some("Account".into()),
            avatar_url: None,
        };
        app.accounts
            .complete_google_login("account", &identity, None)
            .await
            .unwrap();
        app.accounts.ensure_guest("guest-a").await.unwrap();
        app.accounts.ensure_guest("guest-b").await.unwrap();
        app.accounts.ensure_guest("unrelated").await.unwrap();
        let headers = |user_id: &str| {
            let mut headers = HeaderMap::new();
            headers.insert("x-user-id", user_id.parse().unwrap());
            headers
        };
        let mut record = app.games.get(&game_id).unwrap().record.clone();

        record.ownership = GameRecordOwnership {
            white_user_id: Some("account".into()),
            black_user_id: Some("guest-a".into()),
            persist: true,
        };
        assert!(
            ensure_game_record_access(&app, &headers("account"), &record)
                .await
                .is_ok()
        );
        assert!(
            ensure_game_record_access(&app, &headers("guest-a"), &record)
                .await
                .is_ok()
        );
        assert_eq!(
            ensure_game_record_access(&app, &headers("unrelated"), &record)
                .await
                .unwrap_err()
                .0,
            StatusCode::NOT_FOUND
        );

        record.ownership = GameRecordOwnership {
            white_user_id: Some("guest-a".into()),
            black_user_id: Some("guest-b".into()),
            persist: false,
        };
        assert!(
            ensure_game_record_access(&app, &headers("guest-a"), &record)
                .await
                .is_ok()
        );
        assert!(
            ensure_game_record_access(&app, &headers("guest-b"), &record)
                .await
                .is_ok()
        );
        assert_eq!(
            ensure_game_record_access(&app, &headers("unrelated"), &record)
                .await
                .unwrap_err()
                .0,
            StatusCode::NOT_FOUND
        );

        record.ownership = GameRecordOwnership {
            white_user_id: Some("guest-a".into()),
            black_user_id: None,
            persist: false,
        };
        assert!(
            ensure_game_record_access(&app, &headers("guest-a"), &record)
                .await
                .is_ok()
        );
        assert_eq!(
            ensure_game_record_access(&app, &headers("unrelated"), &record)
                .await
                .unwrap_err()
                .0,
            StatusCode::NOT_FOUND
        );

        assert_eq!(
            list_game_records(State(app), headers("guest-a"))
                .await
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn completed_record_is_marked_only_after_a_successful_idempotent_save() {
        let (app, game_id) = test_app_with_game();
        {
            let mut game = app.games.get_mut(&game_id).unwrap();
            game.record.ownership = GameRecordOwnership {
                white_user_id: Some("registered-user".into()),
                black_user_id: None,
                persist: true,
            };
            game.end_with_loss("black", GameEndReason::Resignation);
            assert!(game.completed_record().is_some());
        }
        let repository = std::sync::Arc::new(FailOnceGameRecordRepository {
            fail_next: std::sync::atomic::AtomicBool::new(true),
            saves: std::sync::Mutex::new(Vec::new()),
        });
        let store: game_record::GameRecordStore = repository.clone();

        assert!(
            persist_completed_record(&app.games, &store, &app.challenge_progress, &game_id)
                .await
                .is_err()
        );
        assert!(app
            .games
            .get(&game_id)
            .unwrap()
            .completed_record()
            .is_some());

        persist_completed_record(&app.games, &store, &app.challenge_progress, &game_id)
            .await
            .unwrap();
        assert!(app
            .games
            .get(&game_id)
            .unwrap()
            .completed_record()
            .is_none());
        assert_eq!(repository.saves.lock().unwrap().len(), 1);

        persist_completed_record(&app.games, &store, &app.challenge_progress, &game_id)
            .await
            .unwrap();
        assert_eq!(repository.saves.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn guest_only_completed_record_is_not_sent_to_the_repository() {
        let (app, game_id) = test_app_with_game();
        {
            let mut game = app.games.get_mut(&game_id).unwrap();
            game.record.ownership = GameRecordOwnership::default();
            game.end_with_loss("black", GameEndReason::Resignation);
        }
        let repository = std::sync::Arc::new(FailOnceGameRecordRepository {
            fail_next: std::sync::atomic::AtomicBool::new(true),
            saves: std::sync::Mutex::new(Vec::new()),
        });
        let store: game_record::GameRecordStore = repository.clone();
        persist_completed_record(&app.games, &store, &app.challenge_progress, &game_id)
            .await
            .unwrap();
        assert!(app
            .games
            .get(&game_id)
            .unwrap()
            .completed_record()
            .is_none());
        assert!(repository.saves.lock().unwrap().is_empty());
        assert!(repository
            .fail_next
            .load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn record_listing_uses_internal_ownership_and_never_serializes_it() {
        let (app, game_id) = test_app_with_game();
        let mut record = app.games.get(&game_id).unwrap().record.clone();
        record.players.get_mut("white").unwrap().public_id = Some("old_public_id".into());
        record.ownership = GameRecordOwnership {
            white_user_id: Some("stable-internal-user".into()),
            black_user_id: Some("other-internal-user".into()),
            persist: true,
        };
        app.game_records.save(&record).await.unwrap();

        let listed = app
            .game_records
            .list_summaries_for_user_id("stable-internal-user", 50)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].game_id, game_id);
        let exported = serde_json::to_string(&listed[0]).unwrap();
        assert!(exported.contains("old_public_id"));
        assert!(!exported.contains("stable-internal-user"));
        assert!(!exported.contains("other-internal-user"));
        assert!(!exported.contains("actions"));
        assert!(!exported.contains("initial_state"));
        assert!(!exported.contains("state_delta"));
    }

    #[tokio::test]
    async fn record_access_uses_current_privacy_but_keeps_identity_snapshots() {
        let (app, game_id) = test_app_with_game();
        for (user_id, subject, public_id) in [
            ("white-user", "white-subject", "white_old"),
            ("black-user", "black-subject", "black_id"),
        ] {
            let identity = account::VerifiedIdentity {
                issuer: "issuer".into(),
                subject: subject.into(),
                provider: "google".into(),
                email: None,
                email_verified: true,
                display_name: Some(format!("{public_id} name")),
                avatar_url: None,
            };
            app.accounts
                .complete_google_login(user_id, &identity, None)
                .await
                .unwrap();
            app.accounts
                .update_profile(user_id, Some(public_id), None, None)
                .await
                .unwrap();
        }
        let mut record = app.games.get(&game_id).unwrap().record.clone();
        record.players.get_mut("white").unwrap().public_id = Some("white_old".into());
        record.players.get_mut("black").unwrap().public_id = Some("black_id".into());
        record.ownership = GameRecordOwnership {
            white_user_id: Some("white-user".into()),
            black_user_id: Some("black-user".into()),
            persist: true,
        };
        app.game_records.save(&record).await.unwrap();
        app.games.remove(&game_id);

        let mut third_party = HeaderMap::new();
        third_party.insert("x-user-id", "third-user".parse().unwrap());
        assert!(get_game_record(
            State(app.clone()),
            Path(game_id.clone()),
            third_party.clone(),
        )
        .await
        .is_ok());

        app.accounts
            .update_profile("black-user", None, None, Some(ProfileVisibility::Private))
            .await
            .unwrap();
        let private_player_info = game_record_player(&app, "black-user", "black").await;
        assert_eq!(private_player_info.public_id.as_deref(), Some("black_id"));
        assert_eq!(private_player_info.nickname, "black_id name");
        assert_eq!(
            get_game_record(State(app.clone()), Path(game_id.clone()), third_party,)
                .await
                .unwrap_err()
                .0,
            StatusCode::NOT_FOUND
        );

        let mut owner = HeaderMap::new();
        owner.insert("x-user-id", "white-user".parse().unwrap());
        let owned = get_game_record(State(app.clone()), Path(game_id), owner)
            .await
            .unwrap()
            .0;
        assert_eq!(
            owned.players["white"].public_id.as_deref(),
            Some("white_old")
        );

        app.accounts
            .update_profile("white-user", Some("white_new"), None, None)
            .await
            .unwrap();
        let listed = app
            .game_records
            .list_summaries_for_user_id("white-user", 50)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].players["white"].public_id.as_deref(),
            Some("white_old")
        );
    }

    #[test]
    fn map_id_selects_variant_and_rejects_size_mismatch() {
        assert_eq!(
            resolve_board_map(Some("central-high-ground-12x12"), 12, BoardVariant::Plain),
            Ok((
                "central-high-ground-12x12".into(),
                BoardVariant::CentralHighGround
            )),
        );
        assert!(
            resolve_board_map(Some("central-high-ground-12x12"), 8, BoardVariant::Plain,).is_err()
        );
    }

    #[test]
    fn legacy_requests_resolve_to_the_equivalent_map() {
        assert_eq!(
            resolve_board_map(None, 8, BoardVariant::Plain),
            Ok(("standard-8x8".into(), BoardVariant::Plain)),
        );
        assert_eq!(
            resolve_board_map(None, 12, BoardVariant::CentralHighGround),
            Ok((
                "central-high-ground-12x12".into(),
                BoardVariant::CentralHighGround
            )),
        );
    }

    fn built_in(piece_type: &str) -> DeckPieceRef {
        DeckPieceRef::BuiltIn {
            piece_type: piece_type.into(),
        }
    }

    fn starting_with_front_line(
        side: &str,
        mut back_rank: Vec<StartingPieceSpec>,
    ) -> Vec<StartingPieceSpec> {
        let rank = if side == "white" { 1 } else { 6 };
        back_rank.extend((0..8).map(|file| StartingPieceSpec {
            piece: built_in("pawn"),
            square: Square::new(file, rank),
        }));
        back_rank
    }

    fn valid_player_deck(board_size: i32) -> PlayerDeckSpec {
        let front_rank = if board_size >= 10 { 2 } else { 1 };
        let mut starting = vec![StartingPieceSpec {
            piece: built_in("king"),
            square: Square::new(board_size / 2, 0),
        }];
        starting.extend((0..board_size).map(|file| StartingPieceSpec {
            piece: built_in("pawn"),
            square: Square::new(file, front_rank),
        }));
        PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: Some("Player Deck".into()),
            starting,
            pocket: vec![],
        }
    }

    #[test]
    fn challenge_factory_reuses_game_state_with_authoritative_official_decks() {
        for definition in challenge::definitions() {
            let player = valid_player_deck(definition.board_size);
            let opponent = challenge::opponent_deck(&definition, "black");
            let state = build_game_state_with_variant(
                format!("challenge-{}", definition.id),
                definition.board_size,
                BoardVariant::Plain,
                &player,
                &opponent,
                vec![],
                true,
                false,
            )
            .unwrap();
            assert_eq!(state.board.size, definition.board_size);
            assert_eq!(
                state.players["black"].deck.total_score,
                match definition.id {
                    "tempest_horde" => 118,
                    "raining_men" => 118,
                    "tempest_set" => 66,
                    _ => unreachable!(),
                }
            );
        }
    }

    #[test]
    fn challenge_request_rejects_client_supplied_opponent_deck() {
        let payload = serde_json::json!({
            "player_deck": { "name": "Player", "starting": [], "pocket": [] },
            "opponent_deck": { "starting": [], "pocket": [] }
        });
        assert!(serde_json::from_value::<CreateChallengeGameRequest>(payload).is_err());
    }

    #[test]
    fn challenge_bot_state_includes_paratrooper_drops_and_tempest_moves() {
        let raining = challenge::find("raining_men").unwrap();
        let mut raining_state = build_game_state_with_variant(
            "raining".into(),
            12,
            BoardVariant::Plain,
            &valid_player_deck(12),
            &challenge::opponent_deck(&raining, "black"),
            vec![],
            true,
            false,
        )
        .unwrap();
        raining_state.current_player = "black".into();
        assert!(generate_legal_drop_actions(&raining_state)
            .iter()
            .any(|action| { raining_state.pieces[&action.piece_id].type_id == "paratrooper" }));

        let horde = challenge::find("tempest_horde").unwrap();
        let mut horde_state = build_game_state_with_variant(
            "horde".into(),
            12,
            BoardVariant::Plain,
            &valid_player_deck(12),
            &challenge::opponent_deck(&horde, "black"),
            vec![],
            true,
            false,
        )
        .unwrap();
        horde_state.current_player = "black".into();
        let pawn_id = horde_state
            .pieces
            .values()
            .find(|piece| {
                piece.owner == "black" && piece.type_id == "tempest-pawn-black" && !piece.in_pocket
            })
            .unwrap()
            .id
            .clone();
        assert!(
            !brainfuck_chess_engine::legal_moves::generate_piece_legal_move_actions(
                &horde_state,
                &pawn_id,
            )
            .is_empty()
        );
    }

    #[tokio::test]
    async fn only_an_authoritative_player_win_records_a_challenge_clear() {
        for (loser, should_clear) in [("black", true), ("white", false)] {
            let (app, game_id) = test_app_with_game();
            {
                let mut game = app.games.get_mut(&game_id).unwrap();
                game.record.ownership = GameRecordOwnership {
                    white_user_id: Some("registered-user".into()),
                    black_user_id: None,
                    persist: true,
                };
                game.set_challenge(challenge::ChallengeGameContext {
                    metadata: challenge::ChallengeGameMetadata {
                        id: "tempest_horde".into(),
                        name: "템페스트 호드".into(),
                        player_id: "white".into(),
                        bot_player_id: "black".into(),
                        bot_difficulty: BotDifficulty::Normal,
                    },
                    registered_user_id: Some("registered-user".into()),
                });
                game.end_with_loss(loser, GameEndReason::Resignation);
            }
            persist_completed_record(
                &app.games,
                &app.game_records,
                &app.challenge_progress,
                &game_id,
            )
            .await
            .unwrap();
            let clears = app
                .challenge_progress
                .list_clears("registered-user")
                .await
                .unwrap();
            assert_eq!(!clears.is_empty(), should_clear);
        }
    }

    fn remove_front_line_after_validation(state: &mut GameState) {
        let front_piece_ids = state
            .pieces
            .values()
            .filter(|piece| {
                piece.type_id.starts_with("pawn-")
                    && piece.current_square.is_some_and(|square| {
                        square.rank == if piece.owner == "white" { 1 } else { 6 }
                    })
            })
            .map(|piece| piece.id.clone())
            .collect::<HashSet<_>>();
        for piece_id in &front_piece_ids {
            if let Some(square) = state.pieces[piece_id].current_square {
                state.board.squares.insert(square.to_id(), None);
            }
            state.pieces.remove(piece_id);
        }
        for player in state.players.values_mut() {
            player
                .deck
                .starting_pieces
                .retain(|piece_id| !front_piece_ids.contains(piece_id));
        }
    }

    fn test_app_with_game() -> (AppState, String) {
        let game_id = "test-game".to_string();
        let white_deck = PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "white",
                vec![
                    StartingPieceSpec {
                        piece: built_in("king"),
                        square: Square::new(4, 0),
                    },
                    StartingPieceSpec {
                        piece: built_in("rook"),
                        square: Square::new(0, 0),
                    },
                ],
            ),
            pocket: vec![],
        };
        let black_deck = PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "black",
                vec![StartingPieceSpec {
                    piece: built_in("king"),
                    square: Square::new(4, 7),
                }],
            ),
            pocket: vec![],
        };
        let mut state =
            build_game_state(game_id.clone(), 8, &white_deck, &black_deck, vec![]).unwrap();
        remove_front_line_after_validation(&mut state);
        let app = AppState::in_memory();
        app.games.insert(
            game_id.clone(),
            StoredGame::new(state, TimeControlId::Unlimited, false, now_ms()),
        );
        (app, game_id)
    }

    #[test]
    #[ignore = "payload diagnostic; run explicitly with --ignored --nocapture"]
    fn game_view_payload_profile() {
        const ITERATIONS: u32 = 200;

        fn measure(name: &str, state: GameState) {
            let full_store = AppState::in_memory();
            full_store.games.insert(
                "profile".into(),
                StoredGame::new(state.clone(), TimeControlId::Unlimited, true, now_ms()),
            );
            let full_guard_started = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                let game = full_store.games.get_mut("profile").unwrap();
                std::hint::black_box(game.view(now_ms()));
            }
            let full_guard_us = (full_guard_started.elapsed() / ITERATIONS).as_micros();

            let sync_store = AppState::in_memory();
            sync_store.games.insert(
                "profile".into(),
                StoredGame::new(state.clone(), TimeControlId::Unlimited, true, now_ms()),
            );
            let sync_since_ply = state.history.len();
            let sync_guard_started = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                let mut game = sync_store.games.get_mut("profile").unwrap();
                std::hint::black_box(game.sync_view(now_ms(), Some(1), sync_since_ply));
            }
            let sync_guard_us = (sync_guard_started.elapsed() / ITERATIONS).as_micros();

            let mut game = StoredGame::new(state, TimeControlId::Unlimited, true, now_ms());
            let clone_started = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(game.state.clone());
            }
            let clone_us = (clone_started.elapsed() / ITERATIONS).as_micros();

            let full_view_started = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(game.view(now_ms()));
            }
            let full_view_us = (full_view_started.elapsed() / ITERATIONS).as_micros();
            let full_sample = game.view(now_ms());
            let full_serialization_started = std::time::Instant::now();
            let mut full_bytes = 0;
            for _ in 0..ITERATIONS {
                full_bytes = serde_json::to_vec(&full_sample).unwrap().len();
            }
            let full_serialization_us =
                (full_serialization_started.elapsed() / ITERATIONS).as_micros();

            let since_ply = game.state.history.len();
            let sync_view_started = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(game.sync_view(now_ms(), Some(1), since_ply));
            }
            let sync_view_us = (sync_view_started.elapsed() / ITERATIONS).as_micros();
            let sync_sample = game.sync_view(now_ms(), Some(1), since_ply);
            let sync_serialization_started = std::time::Instant::now();
            let mut sync_bytes = 0;
            for _ in 0..ITERATIONS {
                sync_bytes = serde_json::to_vec(&sync_sample).unwrap().len();
            }
            let sync_serialization_us =
                (sync_serialization_started.elapsed() / ITERATIONS).as_micros();

            let full_value = serde_json::to_value(game.view(now_ms())).unwrap();
            let catalog_bytes = serde_json::to_vec(&full_value["piece_definitions"])
                .unwrap()
                .len();
            let history_bytes = serde_json::to_vec(&full_value["history"]).unwrap().len();
            println!(
                "{name},before,{full_bytes},{full_view_us},{clone_us},{full_serialization_us},{catalog_bytes},{history_bytes},{full_guard_us}"
            );
            println!("{name},after,{sync_bytes},{sync_view_us},0,{sync_serialization_us},0,0,{sync_guard_us}");
        }

        let (app, game_id) = test_app_with_game();
        let initial = app.games.get(&game_id).unwrap().state.clone();
        let sample_action =
            generate_piece_legal_move_actions(&initial, &PieceId::from("white_rook_1"))
                .into_iter()
                .next()
                .unwrap();
        let sample_record = ActionRecord {
            turn_number: 1,
            player_id: "white".into(),
            action: TurnAction::Move(sample_action),
        };

        println!("scenario,variant,bytes,view_construction_us,state_clone_us,serialization_us,catalog_bytes,history_bytes,dashmap_guard_us");
        measure("initial-8x8", initial.clone());

        let mut fifty_ply = initial.clone();
        fifty_ply.history = vec![sample_record.clone(); 50];
        measure("50-ply-8x8", fifty_ply);

        let mut two_hundred_ply = initial.clone();
        two_hundred_ply.history = vec![sample_record.clone(); 200];
        measure("200-ply-8x8", two_hundred_ply);

        let mut large = initial.clone();
        large.board = create_board(12);
        for piece in large.pieces.values() {
            if let Some(square) = piece.current_square {
                large
                    .board
                    .squares
                    .insert(square.to_id(), Some(piece.id.clone()));
            }
        }
        measure("12x12", large);

        let mut custom = initial;
        let mut definition = custom.piece_definitions["rook"].clone();
        definition.id = "custom-profile-rook".into();
        custom
            .piece_definitions
            .insert(definition.id.clone(), definition);
        custom.pieces.get_mut("white_rook_1").unwrap().type_id = "custom-profile-rook".into();
        custom.rebuild_chessembly_cache();
        measure("custom-piece", custom);
    }

    #[test]
    fn heartbeat_sync_omits_unchanged_catalog_runtime_and_full_history() {
        let (app, game_id) = test_app_with_game();
        let mut game = app.games.get_mut(&game_id).unwrap();

        let initial_json = serde_json::to_value(game.view(now_ms())).unwrap();
        assert!(initial_json.get("piece_definitions").is_some());
        assert!(initial_json.get("chessembly_program_cache").is_none());

        let reconnect = game.sync_view(now_ms(), None, 0);
        let catalog_revision = reconnect.catalog_revision;
        assert!(reconnect.catalog.is_some());
        let reconnect_json = serde_json::to_value(&reconnect).unwrap();
        assert_eq!(reconnect_json["dynamic"]["board"], initial_json["board"]);
        assert_eq!(
            reconnect_json["catalog"]["piece_definitions"],
            initial_json["piece_definitions"]
        );

        let steady = game.sync_view(now_ms(), Some(catalog_revision), 0);
        let steady_json = serde_json::to_value(&steady).unwrap();
        assert!(steady.catalog.is_none());
        assert!(steady_json["dynamic"].get("piece_definitions").is_none());
        assert!(steady_json["dynamic"].get("history").is_none());
        assert!(steady_json.get("chessembly_program_cache").is_none());
    }

    #[test]
    fn heartbeat_history_is_incremental_and_mismatch_has_full_recovery() {
        let (app, game_id) = test_app_with_game();
        let mut game = app.games.get_mut(&game_id).unwrap();
        let action = generate_piece_legal_move_actions(&game.state, &PieceId::from("white_rook_1"))
            .into_iter()
            .next()
            .unwrap();
        game.state.history.push(ActionRecord {
            turn_number: 1,
            player_id: "white".into(),
            action: TurnAction::Move(action),
        });

        let incremental = game.sync_view(now_ms(), Some(1), 0);
        assert_eq!(incremental.latest_ply, 1);
        assert_eq!(incremental.new_history.len(), 1);
        assert!(!incremental.resync_required);

        let caught_up = game.sync_view(now_ms(), Some(1), 1);
        assert!(caught_up.new_history.is_empty());

        let recovered = game.sync_view(now_ms(), Some(1), 99);
        assert!(recovered.resync_required);
        assert_eq!(recovered.new_history.len(), 1);
    }

    #[tokio::test]
    async fn replay_analysis_persists_four_plies_and_branches_from_the_selected_parent() {
        let (app, game_id) = test_app_with_game();
        let mut record = app.games.get(&game_id).unwrap().record.clone();
        record.ownership = GameRecordOwnership {
            white_user_id: Some("analysis-owner".into()),
            black_user_id: None,
            persist: true,
        };
        app.game_records.save(&record).await.unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "analysis-owner".parse().unwrap());

        let first = generate_legal_move_actions(&record.initial_state)
            .into_iter()
            .next()
            .expect("white has a legal analysis move");
        let tree = create_analysis_tree(
            State(app.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Json(CreateAnalysisRequest {
                base_ply: 0,
                name: None,
                action: TurnAction::Move(first),
                request_id: "create-four-ply".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let mut parent = tree.nodes[0].clone();
        let reply_piece = parent
            .state_after
            .pieces
            .values()
            .find(|piece| {
                piece.owner == parent.state_after.current_player && piece.current_square.is_some()
            })
            .expect("reply side has a board piece")
            .id
            .to_string();
        let options = get_analysis_options(
            State(app.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Json(AnalysisOptionsRequest {
                sacrifice_piece_ids: Vec::new(),
                position: AnalysisPosition {
                    base_ply: 0,
                    tree_id: Some(tree.id.clone()),
                    node_id: Some(parent.id.clone()),
                    pending_actions: Vec::new(),
                },
                piece_id: reply_piece,
                move_option_id: None,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(!options.moves.is_empty());
        assert_eq!(
            options.previews.len(),
            options.moves.len() + options.drops.len() + options.ability_actions.len()
        );
        let mut version = tree.version;
        let mut second_node = None;
        for index in 1..4 {
            let action = generate_legal_move_actions(&parent.state_after)
                .into_iter()
                .next()
                .expect("analysis line has a legal continuation");
            let appended = append_analysis_node(
                State(app.clone()),
                Path((game_id.clone(), tree.id.clone())),
                headers.clone(),
                Json(AppendAnalysisRequest {
                    parent_node_id: parent.id.clone(),
                    action: TurnAction::Move(action),
                    expected_version: version,
                    request_id: format!("append-{index}"),
                }),
            )
            .await
            .unwrap()
            .0;
            assert_eq!(
                appended.node.parent_node_id.as_deref(),
                Some(parent.id.as_str())
            );
            if index == 1 {
                second_node = Some(appended.node.clone());
            }
            parent = appended.node;
            version = appended.version;
        }

        let branch_parent = second_node.expect("second analysis ply exists");
        let existing_child = app.analyses.list(&game_id, "analysis-owner").await.unwrap()[0]
            .nodes
            .iter()
            .find(|node| node.parent_node_id.as_deref() == Some(branch_parent.id.as_str()))
            .unwrap()
            .action
            .clone();
        let alternative = generate_legal_move_actions(&branch_parent.state_after)
            .into_iter()
            .map(TurnAction::Move)
            .find(|action| {
                serde_json::to_value(action).unwrap()
                    != serde_json::to_value(&existing_child).unwrap()
            })
            .expect("branch parent has an alternative move");
        let branch = append_analysis_node(
            State(app.clone()),
            Path((game_id.clone(), tree.id.clone())),
            headers,
            Json(AppendAnalysisRequest {
                parent_node_id: branch_parent.id.clone(),
                action: alternative,
                expected_version: version,
                request_id: "branch-d".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(
            branch.node.parent_node_id.as_deref(),
            Some(branch_parent.id.as_str())
        );

        let stored = app.analyses.list(&game_id, "analysis-owner").await.unwrap();
        validate_analysis_trees(&record, &stored).unwrap();
        assert_eq!(stored[0].nodes.len(), 5);
        assert_eq!(
            stored[0]
                .nodes
                .iter()
                .filter(|node| node.parent_node_id.as_deref() == Some(branch_parent.id.as_str()))
                .count(),
            2
        );
        let mut legacy = stored.clone();
        for node in &mut legacy[0].nodes {
            node.state_hash = "sha256:legacy-stream-order-hash".into();
        }
        validate_analysis_trees(&record, &legacy)
            .expect("legacy hashes retain semantic validation");
    }

    #[tokio::test]
    async fn piece_scores_are_served_from_engine_definitions() {
        let Json(scores) = get_piece_scores().await;

        assert_eq!(scores.get("tempest-queen"), Some(&10));
        assert_eq!(scores.get("tempest-rook"), Some(&8));
        assert_eq!(scores.get("tempest-bishop"), Some(&5));
        assert_eq!(scores.get("tempest-knight"), Some(&5));
        assert_eq!(scores.get("bouncing-rook"), Some(&6));
        assert_eq!(scores.get("bouncing-queen"), Some(&13));
        assert_eq!(scores.get("prime-minister"), Some(&7));
        assert_eq!(
            resolve_piece_type("white", "prime_minister").as_deref(),
            Some("prime-minister")
        );
        assert_eq!(scores.get("pawn"), scores.get("pawn-white"));
        assert_eq!(scores.get("tempest-pawn"), Some(&2));
        assert_eq!(scores.get("tempest-pawn"), scores.get("tempest-pawn-white"));
        assert_eq!(scores.get("bouncing-pawn"), Some(&2));
        assert_eq!(
            scores.get("bouncing-pawn"),
            scores.get("bouncing-pawn-white")
        );
        assert_eq!(
            resolve_piece_type("white", "bouncing-pawn").as_deref(),
            Some("bouncing-pawn-white")
        );
        assert_eq!(
            resolve_piece_type("black", "bouncing-pawn").as_deref(),
            Some("bouncing-pawn-black")
        );
        assert_eq!(scores.get("dozer"), Some(&2));
        assert_eq!(scores.get("dozer"), scores.get("dozer-white"));
        assert_eq!(scores.get("mortar"), Some(&8));
        assert_eq!(scores.get("machine-gunner"), Some(&8));
        for (piece_type, score) in [
            ("shell", 3),
            ("sacrificial-shrine", 8),
            ("sacrificial-lamb", 1),
            ("fanatic", 2),
            ("wall", 1),
            ("repairman", 4),
        ] {
            assert_eq!(scores.get(piece_type), Some(&score));
            assert_eq!(
                resolve_piece_type("white", piece_type).as_deref(),
                Some(piece_type)
            );
        }
        assert_eq!(
            resolve_piece_type("black", "sacrificial_shrine").as_deref(),
            Some("sacrificial-shrine")
        );
        assert_eq!(
            resolve_piece_type("black", "sacrificial_lamb").as_deref(),
            Some("sacrificial-lamb")
        );
        assert_eq!(scores.get("surface-to-air-missile"), Some(&2));
        assert_eq!(
            resolve_piece_type("white", "surface-to-air-missile").as_deref(),
            Some("surface-to-air-missile-white")
        );
        assert_eq!(
            resolve_piece_type("black", "surface-to-air-missile").as_deref(),
            Some("surface-to-air-missile-black")
        );
        assert_eq!(
            resolve_piece_type("white", "dozer").as_deref(),
            Some("dozer-white")
        );
        assert_eq!(
            resolve_piece_type("black", "dozer").as_deref(),
            Some("dozer-black")
        );
    }

    #[tokio::test]
    async fn piece_catalog_serves_deployment_zones_from_engine_definitions() {
        let Json(catalog) = get_piece_catalog().await;

        for piece_type in [
            "pawn",
            "tempest-pawn",
            "bouncing-pawn",
            "dozer",
            "surface-to-air-missile",
        ] {
            assert_eq!(catalog[piece_type].deployment_zone, DeploymentZone::Front);
        }
        for piece_type in ["knight", "bishop", "rook", "queen", "king", "paratrooper"] {
            assert_eq!(catalog[piece_type].deployment_zone, DeploymentZone::Back);
        }
    }

    #[test]
    fn game_creation_rejects_deployment_zone_mismatches_for_both_players() {
        let valid_white = PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "white",
                vec![StartingPieceSpec {
                    piece: built_in("king"),
                    square: Square::new(4, 0),
                }],
            ),
            pocket: vec![],
        };
        let valid_black = PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "black",
                vec![StartingPieceSpec {
                    piece: built_in("king"),
                    square: Square::new(4, 7),
                }],
            ),
            pocket: vec![],
        };

        let mut back_on_white_front = valid_white.clone();
        back_on_white_front.starting[4].piece = built_in("paratrooper");
        let error = build_game_state(
            "invalid-white".into(),
            8,
            &back_on_white_front,
            &valid_black,
            vec![],
        )
        .unwrap_err();
        assert!(error.contains("뒷줄"));

        let mut back_on_black_front = valid_black.clone();
        back_on_black_front.starting[4].piece = built_in("paratrooper");
        let error = build_game_state(
            "invalid-black".into(),
            8,
            &valid_white,
            &back_on_black_front,
            vec![],
        )
        .unwrap_err();
        assert!(error.contains("뒷줄"));

        let mut front_on_back = valid_white.clone();
        front_on_back.starting.push(StartingPieceSpec {
            piece: built_in("dozer"),
            square: Square::new(3, 0),
        });
        let error = build_game_state(
            "front-on-back".into(),
            8,
            &front_on_back,
            &valid_black,
            vec![],
        )
        .unwrap_err();
        assert!(error.contains("앞줄"));
    }

    #[tokio::test]
    async fn piece_options_returns_only_selected_piece_moves() {
        let (app, game_id) = test_app_with_game();
        let piece_id = "white_rook_1".to_string();

        let response = match get_piece_options(
            State(app),
            HeaderMap::new(),
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
        let json = serde_json::to_value(response).unwrap();
        assert!(json.get("attacks").is_none());
    }

    #[tokio::test]
    async fn player_attacks_returns_the_requested_players_full_attack_map() {
        let (app, game_id) = test_app_with_game();

        let response =
            match get_player_attacks(State(app), Path((game_id, "black".to_string()))).await {
                Ok(Json(response)) => response,
                Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
            };

        assert_eq!(
            response.squares,
            vec![
                Square::new(3, 6),
                Square::new(4, 6),
                Square::new(5, 6),
                Square::new(3, 7),
                Square::new(5, 7),
            ]
        );
    }

    #[tokio::test]
    async fn lab_piece_options_uses_temporary_state_without_storing_game() {
        let app = AppState::in_memory();
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_white_rook_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![
                LabPieceSpec {
                    id: "lab_white_rook_1".into(),
                    piece_type: "rook".into(),
                    owner: "white".into(),
                    square: Square::new(3, 3),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
                LabPieceSpec {
                    id: "lab_black_knight_1".into(),
                    piece_type: "knight".into(),
                    owner: "black".into(),
                    square: Square::new(3, 6),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
            ],
        };

        let response =
            match get_lab_piece_options(State(app.clone()), HeaderMap::new(), Json(req)).await {
                Ok(Json(response)) => response,
                Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
            };

        assert!(response.moves.contains(&Square::new(3, 6)));
        assert!(response.attacks.contains(&Square::new(3, 6)));
        assert!(app.games.is_empty());
    }

    #[test]
    fn lab_game_installs_custom_piece_movement_and_visual() {
        let mut definition = all_default_definitions()
            .into_iter()
            .find(|definition| definition.id == "knight")
            .unwrap();
        definition.id = "main".into();
        definition.name = "Lab Custom".into();
        definition.is_king = false;
        definition.visual.default_asset_key = "data:image/svg+xml;base64,PHN2Zy8+".into();
        let raw_script = serde_json::to_string(&serde_json::json!({
            "format": brainfuck_chess_engine::CUSTOM_PIECE_SCRIPT_FORMAT,
            "definitions": [definition],
        }))
        .unwrap();
        let package = brainfuck_chess_engine::validate_and_build_custom_piece_package(
            brainfuck_chess_engine::CustomPiecePackageInput {
                package_id: "lab-piece".into(),
                version: 1,
                expected_content_hash: None,
                raw_script,
                exposed_piece_key: "main".into(),
                score: 3,
            },
        )
        .unwrap();
        let runtime_type = package.exposed_type_id.clone();
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_custom_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![LabPieceSpec {
                id: "lab_custom_1".into(),
                piece_type: runtime_type.clone(),
                owner: "white".into(),
                square: Square::new(3, 3),
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
                current_ammo: None,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
            }],
        };

        let state = build_lab_game_state(&req, &[package]).unwrap();
        assert_eq!(
            state.piece_definitions[&runtime_type]
                .visual
                .default_asset_key,
            "data:image/svg+xml;base64,PHN2Zy8+"
        );
        assert!(!generate_piece_legal_move_actions_with_options(
            &state,
            &PieceId::from("lab_custom_1"),
            &MoveGenerationOptions::default(),
        )
        .is_empty());
    }

    #[tokio::test]
    async fn lab_piece_options_respects_submitted_piece_state() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_white_windmill_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![LabPieceSpec {
                id: "lab_white_windmill_1".into(),
                piece_type: "windmill".into(),
                owner: "white".into(),
                square: Square::new(3, 3),
                state: HashMap::from([("mode".into(), PieceStateValue::Text("rook".into()))]),
                move_option_cooldowns: HashMap::new(),
                current_ammo: None,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
            }],
        };

        let response =
            match get_lab_piece_options(State(AppState::in_memory()), HeaderMap::new(), Json(req))
                .await
            {
                Ok(Json(response)) => response,
                Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
            };

        assert!(response.moves.contains(&Square::new(4, 3)));
        assert!(!response.moves.contains(&Square::new(4, 4)));
    }

    #[tokio::test]
    async fn lab_piece_options_returns_real_pocket_drop_actions() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_white_pocket_paratrooper_1".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pieces: vec![],
            custom_pieces: vec![],
            pocket_pieces: vec![LabPocketPieceSpec {
                id: "lab_white_pocket_paratrooper_1".into(),
                piece_type: "paratrooper".into(),
                owner: "white".into(),
                state: HashMap::new(),
                current_ammo: None,
            }],
        };

        let response =
            match get_lab_piece_options(State(AppState::in_memory()), HeaderMap::new(), Json(req))
                .await
            {
                Ok(Json(response)) => response,
                Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
            };

        assert!(response.legal_moves.is_empty());
        assert!(!response.legal_drops.is_empty());
        assert!(response.moves.iter().all(|square| square.rank < 2));
        assert_eq!(response.moves.len(), response.legal_drops.len());
    }

    #[test]
    fn lab_game_allows_ground_and_air_pieces_on_the_same_coordinate() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_bomber".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![
                LabPieceSpec {
                    id: "lab_ground_rook".into(),
                    piece_type: "rook".into(),
                    owner: "black".into(),
                    square: Square::new(3, 3),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
                LabPieceSpec {
                    id: "lab_bomber".into(),
                    piece_type: "bomber".into(),
                    owner: "white".into(),
                    square: Square::new(3, 3),
                    state: HashMap::from([("airborne".into(), PieceStateValue::Boolean(true))]),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: Some(2),
                    layer: PieceLayer::Air,
                    remaining_flight_turns: 3,
                },
            ],
        };

        let state = build_lab_game_state(&req, &[]).unwrap();
        assert_eq!(
            state.board.get_piece_at(&Square::new(3, 3)),
            Some(&PieceId::from("lab_ground_rook"))
        );
        assert_eq!(
            state
                .board
                .get_piece_at_layer(&Square::new(3, 3), PieceLayer::Air),
            Some(&PieceId::from("lab_bomber"))
        );
        assert_eq!(state.pieces["lab_bomber"].current_ammo, 2);
        assert_eq!(state.pieces["lab_bomber"].remaining_flight_turns, 3);
    }

    #[tokio::test]
    async fn lab_surface_to_air_missile_exposes_intercept_and_its_air_target() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_sam".into(),
            move_option_id: Some("intercept".into()),
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![
                LabPieceSpec {
                    id: "lab_sam".into(),
                    piece_type: "surface-to-air-missile".into(),
                    owner: "white".into(),
                    square: Square::new(3, 3),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
                LabPieceSpec {
                    id: "lab_enemy_bomber".into(),
                    piece_type: "bomber".into(),
                    owner: "black".into(),
                    square: Square::new(5, 4),
                    state: HashMap::from([("airborne".into(), PieceStateValue::Boolean(true))]),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Air,
                    remaining_flight_turns: 3,
                },
            ],
        };

        let response =
            match get_lab_piece_options(State(AppState::in_memory()), HeaderMap::new(), Json(req))
                .await
            {
                Ok(Json(response)) => response,
                Err((status, Json(error))) => panic!("unexpected error {status}: {}", error.error),
            };

        let intercept = response
            .move_options
            .iter()
            .find(|option| option.id == "intercept")
            .expect("intercept must be exposed by the lab response");
        assert_eq!(intercept.name, "격추");
        assert!(intercept.available);
        assert_eq!(response.legal_ability_actions.len(), 1);
        assert_eq!(
            response.legal_ability_actions[0].target_piece_id,
            Some(PieceId::from("lab_enemy_bomber"))
        );
        assert_eq!(
            response.legal_ability_actions[0].to,
            Some(Square::new(5, 4))
        );
    }

    #[tokio::test]
    async fn lab_apply_action_uses_the_authoritative_engine_transition() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_bomber".into(),
            move_option_id: Some("takeoff".into()),
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![LabPieceSpec {
                id: "lab_bomber".into(),
                piece_type: "bomber".into(),
                owner: "white".into(),
                square: Square::new(1, 1),
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
                current_ammo: None,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
            }],
        };
        let state = build_lab_game_state(&req, &[]).unwrap();
        let takeoff =
            generate_piece_legal_ability_actions(&state, &PieceId::from("lab_bomber"), "takeoff")
                .into_iter()
                .find(|action| action.to == Some(Square::new(6, 1)))
                .unwrap();

        let response = apply_lab_action(
            State(AppState::in_memory()),
            HeaderMap::new(),
            Json(LabApplyActionRequest {
                lab: req,
                action: TurnAction::Ability(takeoff),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.pieces["lab_bomber"].layer, PieceLayer::Air);
        assert_eq!(response.pieces["lab_bomber"].remaining_flight_turns, 5);
        assert_eq!(response.pieces["lab_bomber"].current_ammo, 3);
        assert!(response.board.get_piece_at(&Square::new(1, 1)).is_none());
        assert_eq!(
            response
                .board
                .get_piece_at_layer(&Square::new(6, 1), PieceLayer::Air),
            Some(&PieceId::from("lab_bomber"))
        );
    }

    #[tokio::test]
    async fn lab_apply_action_resolves_fanatic_death_after_capture() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_rook".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![
                LabPieceSpec {
                    id: "lab_rook".into(),
                    piece_type: "rook".into(),
                    owner: "white".into(),
                    square: Square::new(0, 0),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
                LabPieceSpec {
                    id: "lab_fanatic".into(),
                    piece_type: "fanatic".into(),
                    owner: "black".into(),
                    square: Square::new(2, 0),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
            ],
        };
        let state = build_lab_game_state(&req, &[]).unwrap();
        let capture = generate_piece_legal_move_actions(&state, &PieceId::from("lab_rook"))
            .into_iter()
            .find(|action| action.to == Square::new(2, 0))
            .unwrap();

        let response = apply_lab_action(
            State(AppState::in_memory()),
            HeaderMap::new(),
            Json(LabApplyActionRequest {
                lab: req,
                action: TurnAction::Move(capture),
            }),
        )
        .await
        .unwrap()
        .0;

        assert!(response.pieces["lab_fanatic"].captured);
        assert!(response.pieces["lab_rook"].captured);
        assert!(response.board.get_piece_at(&Square::new(2, 0)).is_none());
    }

    #[tokio::test]
    async fn lab_apply_action_replenishes_depleted_ammo_inside_the_home_zone() {
        let req = LabPieceOptionsRequest {
            board_size: 8,
            selected_piece_id: "lab_tank".into(),
            move_option_id: Some("tank-fire".into()),
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![LabPieceSpec {
                id: "lab_tank".into(),
                piece_type: "tank".into(),
                owner: "white".into(),
                square: Square::new(1, 1),
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
                current_ammo: Some(1),
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
            }],
        };
        let state = build_lab_game_state(&req, &[]).unwrap();
        let shot =
            generate_piece_legal_ability_actions(&state, &PieceId::from("lab_tank"), "tank-fire")
                .into_iter()
                .find(|action| action.to == Some(Square::new(1, 4)))
                .unwrap();

        let response = apply_lab_action(
            State(AppState::in_memory()),
            HeaderMap::new(),
            Json(LabApplyActionRequest {
                lab: req,
                action: TurnAction::Ability(shot),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.pieces["lab_tank"].current_ammo, 3);
    }

    #[tokio::test]
    async fn submit_move_action_automatically_ends_turn() {
        let (app, game_id) = test_app_with_game();

        let response = match submit_action(
            State(app.clone()),
            HeaderMap::new(),
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
        assert_eq!(response.history.len(), 1);
        let stored = app.games.get(&game_id).unwrap();
        assert_eq!(stored.current_player, "black");
        assert_eq!(stored.record.actions.len(), 1);
        assert_eq!(
            stored.record.actions[0].notation.actor.piece_id,
            "white_rook_1"
        );
        assert_eq!(
            stored.record.actions[0].notation.actor.from,
            Some(Square::new(0, 0))
        );
        assert!(matches!(
            stored.record.actions[0].notation.kind,
            crate::game_record::NotationActionKind::Move
        ));
        assert!(stored.record.actions[0]
            .state_delta
            .iter()
            .any(|operation| matches!(
                operation,
                crate::game_record::StateDeltaOperation::Set { path, value }
                    if path == &["turn_number".to_string()] && value == &serde_json::json!(2)
            )));
        let serialized = serde_json::to_value(&stored.record.actions[0]).unwrap();
        assert!(serialized.get("state_after").is_none());
        assert!(serialized.get("state_hash").is_none());
        assert!(serialized.get("state_delta").is_some());
        let compact_action_bytes = serde_json::to_vec(&stored.record.actions[0]).unwrap().len();
        let legacy_state_bytes = serde_json::to_vec(&stored.state).unwrap().len();
        assert!(
            compact_action_bytes < legacy_state_bytes,
            "compact={compact_action_bytes}, legacy_state={legacy_state_bytes}"
        );
        assert!(stored.record.initial_state.history.is_empty());
        assert!(stored.record.decks.contains_key("white"));
        assert_eq!(stored.record.actions[0].clock.active_color, "black");
    }

    #[test]
    fn forced_landing_records_two_white_actions_before_the_black_action() {
        let req = LabPieceOptionsRequest {
            board_size: 12,
            selected_piece_id: "bomber".into(),
            move_option_id: None,
            global_state: HashMap::new(),
            pocket_pieces: vec![],
            custom_pieces: vec![],
            pieces: vec![
                LabPieceSpec {
                    id: "bomber".into(),
                    piece_type: "bomber".into(),
                    owner: "white".into(),
                    square: Square::new(4, 10), // e11
                    state: HashMap::from([("airborne".into(), PieceStateValue::Boolean(true))]),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: Some(2),
                    layer: PieceLayer::Air,
                    remaining_flight_turns: 1,
                },
                LabPieceSpec {
                    id: "black-king".into(),
                    piece_type: "king".into(),
                    owner: "black".into(),
                    square: Square::new(0, 11),
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                    current_ammo: None,
                    layer: PieceLayer::Ground,
                    remaining_flight_turns: 0,
                },
            ],
        };
        let state = build_lab_game_state(&req, &[]).unwrap();
        let mut stored = StoredGame::new(state, TimeControlId::FiveThree, false, 1);

        let bomber_move = generate_piece_legal_move_actions(&stored.state, &"bomber".into())
            .into_iter()
            .find(|action| action.to == Square::new(4, 3)) // e4
            .unwrap();
        let before_move = stored.state.clone();
        let after_move =
            submit_engine_action(before_move.clone(), TurnAction::Move(bomber_move.clone()))
                .unwrap();
        assert_eq!(after_move.current_player, "white");
        assert_eq!(after_move.turn_number, 1);
        stored
            .clock
            .finish_turn("white", &after_move.current_player, 1_001, false);
        let clock_after_move = stored.clock.snapshot(1_001, true);
        assert_eq!(clock_after_move.white_remaining_ms, Some(299_000));
        stored.record.push_action(
            "white".into(),
            TurnAction::Move(bomber_move),
            0,
            None,
            None,
            clock_after_move,
            &before_move,
            after_move.clone(),
        );
        stored.state = after_move;

        // The engine's forced-landing runway is four squares, so e4 -> i4 is
        // the legal equivalent of the notation grouping regression scenario.
        let landing =
            generate_piece_legal_ability_actions(&stored.state, &"bomber".into(), "forced-landing")
                .into_iter()
                .find(|action| action.to == Some(Square::new(8, 3))) // i4
                .unwrap();
        let before_landing = stored.state.clone();
        let after_landing =
            submit_engine_action(before_landing.clone(), TurnAction::Ability(landing.clone()))
                .unwrap();
        assert_eq!(after_landing.current_player, "black");
        assert_eq!(after_landing.turn_number, 2);
        stored
            .clock
            .finish_turn("white", &after_landing.current_player, 2_001, false);
        let clock_after_landing = stored.clock.snapshot(2_001, true);
        // Two seconds were consumed across both canonical actions and the
        // three-second increment was added exactly once: 300 - 2 + 3 = 301.
        assert_eq!(clock_after_landing.white_remaining_ms, Some(301_000));
        stored.record.push_action(
            "white".into(),
            TurnAction::Ability(landing),
            0,
            None,
            None,
            clock_after_landing.clone(),
            &before_landing,
            after_landing.clone(),
        );
        stored.state = after_landing;

        let black_move = generate_piece_legal_move_actions(&stored.state, &"black-king".into())
            .into_iter()
            .next()
            .unwrap();
        let before_black = stored.state.clone();
        let after_black =
            submit_engine_action(before_black.clone(), TurnAction::Move(black_move.clone()))
                .unwrap();
        stored.record.push_action(
            "black".into(),
            TurnAction::Move(black_move),
            0,
            None,
            None,
            clock_after_landing,
            &before_black,
            after_black,
        );

        assert_eq!(stored.record.actions.len(), 3);
        assert_eq!(
            stored
                .record
                .actions
                .iter()
                .map(|entry| entry.notation.side.as_str())
                .collect::<Vec<_>>(),
            vec!["white", "white", "black"]
        );
        assert_eq!(
            stored
                .record
                .actions
                .iter()
                .map(|entry| entry.notation.turn_number)
                .collect::<Vec<_>>(),
            vec![1, 1, 2]
        );
        assert_eq!(
            stored
                .record
                .actions
                .iter()
                .map(|entry| entry.notation.move_number)
                .collect::<Vec<_>>(),
            vec![1, 1, 1]
        );
        assert_eq!(
            stored.record.actions[0].notation.from,
            Some(Square::new(4, 10))
        );
        assert_eq!(
            stored.record.actions[0].notation.to,
            Some(Square::new(4, 3))
        );
        assert_eq!(
            stored.record.actions[1].notation.from,
            Some(Square::new(4, 3))
        );
        assert_eq!(
            stored.record.actions[1].notation.to,
            Some(Square::new(8, 3))
        );
        assert_eq!(
            stored.record.actions[1].notation.ability_name.as_deref(),
            Some("강제 착륙")
        );
    }

    #[tokio::test]
    async fn resignation_completes_a_portable_record_with_the_authoritative_result() {
        let (app, game_id) = test_app_with_game();
        app.games
            .get_mut(&game_id)
            .unwrap()
            .record
            .ownership
            .white_user_id = Some("record-owner".into());
        let ended = resign_game(
            State(app.clone()),
            HeaderMap::new(),
            Path(game_id.clone()),
            Json(ResignGameRequest {
                player_id: "white".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "record-owner".parse().unwrap());
        let record = get_game_record(State(app), Path(game_id), headers)
            .await
            .unwrap()
            .0;
        assert_eq!(
            record.result.as_ref().map(|result| &result.reason),
            Some(&GameEndReason::Resignation)
        );
        assert_eq!(
            record
                .result
                .as_ref()
                .and_then(|result| result.winner.as_deref()),
            Some("black")
        );
        assert_eq!(
            ended
                .result
                .as_ref()
                .and_then(|result| result.winner.as_deref()),
            Some("black")
        );
        assert!(record.final_clock.is_some());
    }

    #[tokio::test]
    async fn submit_move_rejects_unknown_option_and_noncanonical_destination() {
        let (app, game_id) = test_app_with_game();
        let unknown_option = submit_action(
            State(app.clone()),
            HeaderMap::new(),
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
            HeaderMap::new(),
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
            HeaderMap::new(),
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
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "white",
                vec![
                    StartingPieceSpec {
                        piece: built_in("king"),
                        square: Square::new(4, 0),
                    },
                    StartingPieceSpec {
                        piece: built_in("windmill"),
                        square: Square::new(3, 0),
                    },
                ],
            ),
            pocket: vec![],
        };
        let black_deck = PlayerDeckSpec {
            extra: Vec::new(),
            ruleset: Default::default(),
            name: None,
            starting: starting_with_front_line(
                "black",
                vec![StartingPieceSpec {
                    piece: built_in("king"),
                    square: Square::new(4, 7),
                }],
            ),
            pocket: vec![],
        };
        let mut state =
            build_game_state(game_id.clone(), 8, &white_deck, &black_deck, vec![]).unwrap();
        remove_front_line_after_validation(&mut state);
        let app = AppState::in_memory();
        app.games.insert(
            game_id.clone(),
            StoredGame::new(state, TimeControlId::Unlimited, false, now_ms()),
        );

        let response = match submit_action(
            State(app),
            HeaderMap::new(),
            Path(game_id),
            Json(SubmitActionRequest {
                action: SubmitAction::Move(SubmitMoveRequest {
                    piece_id: "white_windmill_1".into(),
                    to: Square::new(4, 1),
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
            HeaderMap::new(),
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
            HeaderMap::new(),
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
    async fn ruleset_http(
        app: &AppState,
        path: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        use axum::{
            body::{to_bytes, Body},
            http::Request,
        };
        use tower::Service;
        let response = routes::api(app.clone())
            .call(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 3_000_000).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or_default())
    }

    fn valid_player_deck_with_ruleset(size: i32, ruleset: DeckRuleset) -> PlayerDeckSpec {
        let mut spec = valid_player_deck(size);
        spec.ruleset = ruleset;
        if ruleset == DeckRuleset::Standard {
            spec.starting.truncate(1);
            spec.starting.extend(
                brainfuck_chess_engine::rules::get_front_zone_squares_with_ruleset(
                    &"white".into(),
                    size,
                    ruleset,
                )
                .into_iter()
                .map(|square| StartingPieceSpec {
                    piece: built_in("pawn"),
                    square,
                }),
            );
        }
        spec
    }

    fn ruleset_game_request(size: i32, ruleset: DeckRuleset, map: &str) -> serde_json::Value {
        let white = valid_player_deck_with_ruleset(size, ruleset);
        let black = materialize_neutral_deck(&white, "black", size);
        serde_json::json!({ "board_size": size, "map_id": map, "ruleset": ruleset,
            "white_deck": white, "black_deck": black, "time_control": "unlimited" })
    }

    #[tokio::test]
    async fn ruleset_game_creation_is_independent_of_map_and_preserved_in_views() {
        let app = AppState::in_memory();
        for (size, map) in [
            (8, "standard-8x8"),
            (9, "standard-9x9"),
            (10, "standard-10x10"),
            (11, "standard-11x11"),
            (12, "standard-12x12"),
            (12, "central-high-ground-12x12"),
        ] {
            for ruleset in [DeckRuleset::Legacy, DeckRuleset::Standard] {
                let (status, response) =
                    ruleset_http(&app, "/games", ruleset_game_request(size, ruleset, map)).await;
                assert_eq!(status, StatusCode::OK, "{response}");
                assert_eq!(
                    response["state"]["ruleset"],
                    serde_json::to_value(ruleset).unwrap()
                );
                let id = response["id"].as_str().unwrap();
                let game = app.games.get(id).unwrap();
                assert_eq!(game.state.ruleset, ruleset);
                assert_eq!(game.record.initial_state.ruleset, ruleset);
                assert_eq!(game.record.state_at_ply(0).unwrap().ruleset, ruleset);
                assert_eq!(game.state.board.size, size);
                assert_eq!(game.record.decks["white"].map_id, map);
                assert!(matches!(
                    game.record.game_mode,
                    game_record::GameMode::Standard
                ));
                assert_eq!(
                    game.record.ruleset_version,
                    game_record::current_rules_version_for(ruleset)
                );
                assert!(game.state.history.is_empty());
                let expected_front =
                    brainfuck_chess_engine::rules::get_front_zone_squares_with_ruleset(
                        &"white".into(),
                        size,
                        ruleset,
                    );
                assert_eq!(
                    game.state.players["white"].deck.starting_pieces.len(),
                    expected_front.len() + 1
                );
                assert_eq!(
                    game.state.players["black"].deck.starting_pieces.len(),
                    expected_front.len() + 1
                );
                assert_eq!(
                    game.state.board.terrain,
                    create_board_with_variant(
                        size,
                        if map.starts_with("central-high-ground") {
                            BoardVariant::CentralHighGround
                        } else {
                            BoardVariant::Plain
                        }
                    )
                    .unwrap()
                    .terrain
                );
            }
        }
    }

    #[tokio::test]
    async fn standard_game_creation_rejects_old_geometry_wrong_zone_and_missing_front() {
        let app = AppState::in_memory();
        for side in ["white_deck", "black_deck"] {
            for mode in ["outside", "wrong-zone", "missing-front"] {
                let mut request = ruleset_game_request(8, DeckRuleset::Standard, "standard-8x8");
                let starting = request[side]["starting"].as_array_mut().unwrap();
                match mode {
                    "outside" => starting[0]["square"]["file"] = 0.into(),
                    "wrong-zone" => starting[1]["piece_type"] = "knight".into(),
                    _ => {
                        starting.remove(1);
                    }
                }
                let (status, response) = ruleset_http(&app, "/games", request).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{side} {mode}: {response}");
                assert!(app.games.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn ruleset_old_requests_default_only_missing_values_and_reject_mismatches() {
        let app = AppState::in_memory();
        let mut old = ruleset_game_request(8, DeckRuleset::Legacy, "standard-8x8");
        old.as_object_mut().unwrap().remove("ruleset");
        for side in ["white_deck", "black_deck"] {
            old[side].as_object_mut().unwrap().remove("ruleset");
        }
        let (status, response) = ruleset_http(&app, "/games", old.clone()).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["state"]["ruleset"], "legacy");
        let count = app.games.len();
        for field in ["request", "white_deck", "black_deck"] {
            for value in [
                serde_json::json!("future"),
                serde_json::Value::Null,
                serde_json::json!(1),
            ] {
                let mut bad = old.clone();
                if field == "request" {
                    bad["ruleset"] = value;
                } else {
                    bad[field]["ruleset"] = value;
                }
                assert!(matches!(
                    ruleset_http(&app, "/games", bad).await.0,
                    StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
                ));
            }
        }
        for (request, white, black) in [
            ("legacy", "legacy", "standard"),
            ("legacy", "standard", "standard"),
            ("standard", "legacy", "legacy"),
            ("standard", "standard", "legacy"),
        ] {
            let mut bad = old.clone();
            bad["ruleset"] = request.into();
            bad["white_deck"]["ruleset"] = white.into();
            bad["black_deck"]["ruleset"] = black.into();
            assert_eq!(
                ruleset_http(&app, "/games", bad).await.0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(app.games.len(), count);
    }

    #[tokio::test]
    async fn ruleset_room_create_reselect_ready_join_and_heartbeat_are_authoritative() {
        for ruleset in [DeckRuleset::Legacy, DeckRuleset::Standard] {
            let app = AppState::in_memory();
            let spec = valid_player_deck_with_ruleset(8, ruleset);
            let mut other = spec.clone();
            other.ruleset = if ruleset == DeckRuleset::Legacy {
                DeckRuleset::Standard
            } else {
                DeckRuleset::Legacy
            };
            let payload = serde_json::json!({"board_size":8,"map_id":"standard-8x8", "ruleset":ruleset,
                "host_side":"black","client_id":"host","deck":spec,"time_control":"unlimited"});
            let mut bad = payload.clone();
            bad["deck"] = serde_json::to_value(&other).unwrap();
            assert_eq!(
                ruleset_http(&app, "/rooms", bad).await.0,
                StatusCode::BAD_REQUEST
            );
            assert!(app.rooms.is_empty());
            let (status, room) = ruleset_http(&app, "/rooms", payload).await;
            assert_eq!(status, StatusCode::OK, "{room}");
            assert_eq!(room["ruleset"], serde_json::to_value(ruleset).unwrap());
            let id = room["id"].as_str().unwrap();
            for (route, client) in [
                ("join", "guest"),
                ("select-deck", "guest"),
                ("select-deck", "host"),
            ] {
                assert_eq!(
                    ruleset_http(
                        &app,
                        &format!("/rooms/{id}/{route}"),
                        serde_json::json!({"client_id":client,"deck":other})
                    )
                    .await
                    .0,
                    StatusCode::BAD_REQUEST
                );
                assert!(app.rooms.get(id).unwrap().guest_deck.is_none());
                assert!(app.rooms.get(id).unwrap().guest_client_id.is_none());
            }
            for client in ["host", "guest"] {
                let (status, selected) = ruleset_http(
                    &app,
                    &format!("/rooms/{id}/select-deck"),
                    serde_json::json!({"client_id":client,"deck":spec}),
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{selected}");
                assert_eq!(selected["ruleset"], serde_json::to_value(ruleset).unwrap());
            }
            assert_eq!(
                ruleset_http(
                    &app,
                    &format!("/rooms/{id}/ready"),
                    serde_json::json!({"client_id":"host"})
                )
                .await
                .0,
                StatusCode::OK
            );
            // Final factory validation is independent of the endpoint's earlier check.
            app.rooms.get_mut(id).unwrap().guest_deck = Some(other);
            assert_eq!(
                ruleset_http(
                    &app,
                    &format!("/rooms/{id}/ready"),
                    serde_json::json!({"client_id":"guest"})
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
            assert!(app.games.is_empty());
            app.rooms.get_mut(id).unwrap().guest_deck = Some(spec.clone());
            let (status, ready) = ruleset_http(
                &app,
                &format!("/rooms/{id}/ready"),
                serde_json::json!({"client_id":"guest"}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{ready}");
            let game_id = ready["game_id"].as_str().unwrap();
            assert_eq!(app.games.get(game_id).unwrap().state.ruleset, ruleset);
            let mut revision = serde_json::Value::Null;
            for _ in 0..2 {
                let (status, sync) = ruleset_http(&app, &format!("/rooms/{id}/heartbeat"), serde_json::json!({"client_id":"host","player_id":"black","latest_ply":0,"catalog_revision":revision})).await;
                assert_eq!(status, StatusCode::OK, "{sync}");
                assert_eq!(
                    sync["dynamic"]["ruleset"],
                    serde_json::to_value(ruleset).unwrap()
                );
                revision = sync["catalog_revision"].clone();
            }
            // The immediate join/start path also retains the room format.
            let (_, second) = ruleset_http(&app, "/rooms", serde_json::json!({"board_size":8,"ruleset":ruleset,"host_side":"white","client_id":"host2","deck":spec,"time_control":"unlimited"})).await;
            let second_id = second["id"].as_str().unwrap();
            let (status, joined) = ruleset_http(
                &app,
                &format!("/rooms/{second_id}/join"),
                serde_json::json!({"client_id":"guest2","deck":spec}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{joined}");
            assert_eq!(
                joined["state"]["ruleset"],
                serde_json::to_value(ruleset).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn ruleset_existing_challenges_are_legacy_only() {
        let app = AppState::in_memory();
        let mut spec = valid_player_deck(12);
        spec.ruleset = DeckRuleset::Standard;
        let (status, _) = ruleset_http(
            &app,
            "/challenges/raining_men/games",
            serde_json::json!({"player_deck":spec}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(app.games.is_empty());
        spec.ruleset = DeckRuleset::Legacy;
        let (status, game) = ruleset_http(
            &app,
            "/challenges/raining_men/games",
            serde_json::json!({"player_deck":spec}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{game}");
        assert_eq!(game["state"]["ruleset"], "legacy");
        assert!(app
            .games
            .get(game["id"].as_str().unwrap())
            .unwrap()
            .state
            .pieces
            .values()
            .any(|piece| piece.owner == "black"
                && piece.type_id == "guhang"
                && piece.is_on_board()));
    }
    #[tokio::test]
    async fn ruleset_old_room_requests_default_and_unknown_values_do_not_create_rooms() {
        let app = AppState::in_memory();
        let mut deck = serde_json::to_value(valid_player_deck(8)).unwrap();
        deck.as_object_mut().unwrap().remove("ruleset");
        let old = serde_json::json!({"board_size":8,"host_side":"white","client_id":"old-host","deck":deck});
        let (status, room) = ruleset_http(&app, "/rooms", old.clone()).await;
        assert_eq!(status, StatusCode::OK, "{room}");
        assert_eq!(room["ruleset"], "legacy");
        assert_eq!(room["host_deck"]["ruleset"], "legacy");
        for field in ["request", "deck"] {
            for value in [
                serde_json::json!("future"),
                serde_json::Value::Null,
                serde_json::json!(false),
            ] {
                let mut bad = old.clone();
                if field == "request" {
                    bad["ruleset"] = value;
                } else {
                    bad["deck"]["ruleset"] = value;
                }
                assert!(matches!(
                    ruleset_http(&app, "/rooms", bad).await.0,
                    StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
                ));
            }
        }
        assert_eq!(app.rooms.len(), 1);
        assert!(app.games.is_empty());
    }
    #[test]
    fn extra_factory_preserves_instances_scores_and_inert_zone() {
        use brainfuck_chess_engine::legal_moves::{
            generate_piece_legal_ability_actions, generate_piece_legal_drop_actions,
            generate_piece_legal_move_actions,
        };
        for count in [0, 1, 3, 4] {
            let mut spec = valid_player_deck_with_ruleset(8, DeckRuleset::Standard);
            spec.pocket = vec![built_in("pawn"); 33];
            spec.extra = (0..count)
                .map(|i| built_in(if i == 0 { "guhang" } else { "bomber" }))
                .collect();
            let black = materialize_neutral_deck(&spec, "black", 8);
            let result = build_game_state("extra".into(), 8, &spec, &black, vec![]);
            if count == 4 {
                assert!(result.unwrap_err().contains("최대 3기"));
                continue;
            }
            let state = result.unwrap();
            assert_eq!(state.piece_definitions["guhang"].score, 25);
            assert_eq!(state.piece_definitions["bomber"].score, 13);
            for side in ["white", "black"] {
                let deck = &state.players[side].deck;
                assert_eq!(deck.total_score, 39);
                assert_eq!(deck.extra_deck_pieces.len(), count);
                assert_eq!(
                    deck.extra_deck_pieces.iter().collect::<HashSet<_>>().len(),
                    count
                );
                let mut active = state.clone();
                active.current_player = side.into();
                for id in &deck.extra_deck_pieces {
                    let piece = &state.pieces[id];
                    assert_eq!(piece.owner, side);
                    assert!(!piece.captured && !piece.in_pocket && piece.current_square.is_none());
                    assert!(!deck.pocket_pieces.contains(id) && !deck.starting_pieces.contains(id));
                    assert!(!state
                        .board
                        .squares
                        .values()
                        .chain(state.board.air_squares.values())
                        .any(|p| p.as_ref() == Some(id)));
                    assert!(generate_piece_legal_move_actions(&active, id).is_empty());
                    assert!(generate_piece_legal_drop_actions(&active, id).is_empty());
                    assert!(generate_piece_legal_ability_actions(&active, id, "takeoff").is_empty());
                    let action = TurnAction::Drop(brainfuck_chess_engine::types::DropAction {
                        player_id: side.into(),
                        piece_id: id.clone(),
                        to: Square::new(3, 0),
                        captured_piece_id: None,
                    });
                    assert!(submit_engine_action(active.clone(), action).is_err());
                }
            }
            let value = serde_json::to_value(&state).unwrap();
            let roundtrip: GameState = serde_json::from_value(value).unwrap();
            assert_eq!(
                roundtrip.players["white"].deck.extra_deck_pieces,
                state.players["white"].deck.extra_deck_pieces
            );
            if count > 0 {
                let mut changed = state.clone();
                changed
                    .players
                    .get_mut("white")
                    .unwrap()
                    .deck
                    .extra_deck_pieces
                    .clear();
                assert_ne!(
                    analysis::state_hash(&state).unwrap(),
                    analysis::state_hash(&changed).unwrap()
                );
            }
        }
        for kind in ["guhang", "bomber"] {
            for ruleset in [DeckRuleset::Standard, DeckRuleset::Legacy] {
                for pocket in [false, true] {
                    let mut spec = valid_player_deck_with_ruleset(8, ruleset);
                    if pocket {
                        spec.pocket.push(built_in(kind));
                    } else {
                        spec.starting.push(StartingPieceSpec {
                            piece: built_in(kind),
                            square: Square::new(3, 0),
                        });
                    }
                    let result = build_game_state(
                        "main".into(),
                        8,
                        &spec,
                        &materialize_neutral_deck(&spec, "black", 8),
                        vec![],
                    );
                    assert_eq!(result.is_ok(), ruleset == DeckRuleset::Legacy);
                }
            }
        }
        let mut legacy = valid_player_deck(8);
        legacy.extra.push(built_in("guhang"));
        assert!(build_game_state(
            "legacy".into(),
            8,
            &legacy,
            &materialize_neutral_deck(&legacy, "black", 8),
            vec![]
        )
        .is_err());
    }

    #[test]
    fn extra_does_not_feed_existing_pocket_abilities() {
        use brainfuck_chess_engine::legal_moves::generate_legal_ability_actions;
        for actor in ["alternating-soldier", "airborne"] {
            let mut spec = valid_player_deck_with_ruleset(8, DeckRuleset::Standard);
            spec.starting.push(StartingPieceSpec {
                piece: built_in(actor),
                square: Square::new(3, 0),
            });
            spec.pocket = vec![built_in("paratrooper"), built_in("paratrooper")];
            let base = build_game_state(
                "abilities".into(),
                8,
                &spec,
                &materialize_neutral_deck(&spec, "black", 8),
                vec![],
            )
            .unwrap();
            spec.extra = vec![built_in("guhang"), built_in("bomber")];
            let extra = build_game_state(
                "abilities".into(),
                8,
                &spec,
                &materialize_neutral_deck(&spec, "black", 8),
                vec![],
            )
            .unwrap();
            let actions = generate_legal_ability_actions(&base);
            assert!(!actions.is_empty(), "fixture must exercise {actor}");
            assert_eq!(
                serde_json::to_value(actions).unwrap(),
                serde_json::to_value(generate_legal_ability_actions(&extra)).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn extra_room_validation_reselection_game_and_sync() {
        let app = AppState::in_memory();
        let mut spec = valid_player_deck_with_ruleset(8, DeckRuleset::Standard);
        spec.extra = vec![built_in("guhang"), built_in("bomber"), built_in("bomber")];
        let (status, room) = ruleset_http(&app, "/rooms", serde_json::json!({"board_size":8,"ruleset":"standard","host_side":"black","client_id":"host","deck":spec,"time_control":"unlimited"})).await;
        assert_eq!(status, StatusCode::OK, "{room}");
        let id = room["id"].as_str().unwrap();
        for client in ["host", "guest"] {
            for invalid in [vec![built_in("guhang"); 4], vec![built_in("knight")]] {
                let mut bad = spec.clone();
                bad.extra = invalid;
                let (status, _) = ruleset_http(
                    &app,
                    &format!("/rooms/{id}/select-deck"),
                    serde_json::json!({"client_id":client,"deck":bad}),
                )
                .await;
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert!(app.rooms.get(id).unwrap().guest_deck.is_none());
            }
        }
        let (status, selected) = ruleset_http(
            &app,
            &format!("/rooms/{id}/select-deck"),
            serde_json::json!({"client_id":"host","deck":spec}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(selected["host_deck"].is_null());
        assert_eq!(selected["host_has_deck"], true);
        assert_eq!(
            app.rooms
                .get(id)
                .unwrap()
                .host_deck
                .as_ref()
                .unwrap()
                .extra
                .len(),
            3
        );
        ruleset_http(
            &app,
            &format!("/rooms/{id}/ready"),
            serde_json::json!({"client_id":"host"}),
        )
        .await;
        let (status, game) = ruleset_http(
            &app,
            &format!("/rooms/{id}/join"),
            serde_json::json!({"client_id":"guest","deck":spec}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{game}");
        for side in ["white", "black"] {
            assert_eq!(
                game["state"]["players"][side]["deck"]["extra_deck_pieces"]
                    .as_array()
                    .map_or(0, Vec::len),
                if side == "white" { 3 } else { 0 }
            );
        }
        let (status, sync) = ruleset_http(
            &app,
            &format!("/rooms/{id}/heartbeat"),
            serde_json::json!({"client_id":"host","player_id":"black","latest_ply":0}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{sync}");
        for side in ["white", "black"] {
            assert_eq!(
                sync["dynamic"]["players"][side]["deck"]["extra_deck_pieces"]
                    .as_array()
                    .map_or(0, Vec::len),
                if side == "black" { 3 } else { 0 }
            );
        }
        spec.ruleset = DeckRuleset::Legacy;
        let (status, _) = ruleset_http(&app, "/rooms", serde_json::json!({"board_size":8,"ruleset":"legacy","host_side":"white","client_id":"old","deck":spec})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
