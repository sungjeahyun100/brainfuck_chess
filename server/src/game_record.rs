use async_trait::async_trait;
use brainfuck_chess_engine::types::{
    GameResult, GameState, MoveOptionKind, Piece, PieceId, PieceLayer, PlayerId, Square, TurnAction,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::database::DataSchema;
use crate::time_control::{ClockSnapshot, TimeControlId};

pub(crate) const GAME_RECORD_FORMAT_VERSION: u32 = 2;
pub(crate) const RULESET_VERSION: &str = "deck-chess-1";
pub(crate) const CHESSEMBLY_VERSION: &str = "chessembly-1";
pub(crate) const AUTO_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RetentionMode {
    Auto,
    #[default]
    Permanent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GameMode {
    #[default]
    Standard,
    Challenge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameRecordPlayer {
    pub(crate) public_id: Option<String>,
    pub(crate) nickname: String,
    pub(crate) side: PlayerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CustomDeckPieceSnapshot {
    pub(crate) custom_piece_id: String,
    pub(crate) version: u32,
    pub(crate) content_hash: String,
    pub(crate) exposed_piece_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeckDeploymentSnapshot {
    #[serde(default)]
    pub(crate) piece_type_id: String,
    pub(crate) piece_name: String,
    #[serde(default)]
    pub(crate) custom_piece: Option<CustomDeckPieceSnapshot>,
    pub(crate) square: Square,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeckPocketSnapshot {
    #[serde(default)]
    pub(crate) piece_type_id: String,
    pub(crate) piece_name: String,
    #[serde(default)]
    pub(crate) custom_piece: Option<CustomDeckPieceSnapshot>,
    pub(crate) count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeckSnapshot {
    #[serde(default)]
    pub(crate) snapshot_version: u32,
    pub(crate) side: PlayerId,
    pub(crate) deck_name: String,
    #[serde(default)]
    pub(crate) map_id: String,
    #[serde(default)]
    pub(crate) board_size: i32,
    pub(crate) deployments: Vec<DeckDeploymentSnapshot>,
    pub(crate) pocket: Vec<DeckPocketSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NotationActionKind {
    Move,
    MoveWithAbility,
    Ability,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ActorSnapshot {
    pub(crate) piece_id: String,
    pub(crate) piece_type_id: String,
    pub(crate) piece_name: String,
    pub(crate) from: Option<Square>,
    pub(crate) layer: PieceLayer,
    pub(crate) current_ammo: Option<u32>,
    pub(crate) state: HashMap<String, brainfuck_chess_engine::types::PieceStateValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AbilityEventSnapshot {
    pub(crate) ability_id: String,
    pub(crate) ability_name: String,
    pub(crate) target: Option<Square>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecordedNotationAction {
    pub(crate) turn_number: u32,
    pub(crate) move_number: u32,
    pub(crate) side: PlayerId,
    pub(crate) actor: ActorSnapshot,
    pub(crate) kind: NotationActionKind,
    pub(crate) ability_id: Option<String>,
    pub(crate) ability_name: Option<String>,
    pub(crate) from: Option<Square>,
    pub(crate) to: Option<Square>,
    pub(crate) target: Option<Square>,
    pub(crate) ability_events: Vec<AbilityEventSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum StateDeltaOperation {
    Set { path: Vec<String>, value: Value },
    Remove { path: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecordedAction {
    pub(crate) ply: u32,
    pub(crate) player_id: PlayerId,
    pub(crate) action: TurnAction,
    pub(crate) notation: RecordedNotationAction,
    pub(crate) state_delta: Vec<StateDeltaOperation>,
    pub(crate) elapsed_ms: i64,
    pub(crate) clock_before_ms: Option<i64>,
    pub(crate) clock_after_ms: Option<i64>,
    pub(crate) clock: ClockSnapshot,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GameRecordOwnership {
    pub(crate) white_user_id: Option<String>,
    pub(crate) black_user_id: Option<String>,
    pub(crate) persist: bool,
}

impl GameRecordOwnership {
    pub(crate) fn contains(&self, user_id: &str) -> bool {
        self.white_user_id.as_deref() == Some(user_id)
            || self.black_user_id.as_deref() == Some(user_id)
    }

    pub(crate) fn both_user_ids(&self) -> Option<(&str, &str)> {
        Some((
            self.white_user_id.as_deref()?,
            self.black_user_id.as_deref()?,
        ))
    }

    pub(crate) fn has_registered_owner(&self) -> bool {
        self.persist
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameRecordSummary {
    pub(crate) game_id: String,
    pub(crate) display_name: String,
    pub(crate) started_at_ms: i64,
    pub(crate) ended_at_ms: Option<i64>,
    pub(crate) result: Option<GameResult>,
    pub(crate) players: HashMap<PlayerId, GameRecordPlayer>,
    pub(crate) time_control: TimeControlId,
    pub(crate) owner_side: PlayerId,
    pub(crate) game_mode: GameMode,
    pub(crate) challenge_id: Option<String>,
    pub(crate) retention_mode: RetentionMode,
    pub(crate) expires_at_ms: Option<i64>,
    pub(crate) analysis_count: i64,
}

impl GameRecordSummary {
    fn from_record(record: &GameRecord, user_id: &str) -> Self {
        let owner_side = if record.ownership.black_user_id.as_deref() == Some(user_id) {
            "black"
        } else {
            "white"
        };
        Self {
            game_id: record.game_id.clone(),
            display_name: record.display_name.clone(),
            started_at_ms: record.started_at_ms,
            ended_at_ms: record.ended_at_ms,
            result: record.result.clone(),
            players: record.players.clone(),
            time_control: record.time_control,
            owner_side: owner_side.into(),
            game_mode: record.game_mode,
            challenge_id: record.challenge_id.clone(),
            retention_mode: record.retention_mode,
            expires_at_ms: record.expires_at_ms,
            analysis_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameRecord {
    #[serde(skip)]
    pub(crate) ownership: GameRecordOwnership,
    pub(crate) format_version: u32,
    pub(crate) game_id: String,
    pub(crate) display_name: String,
    pub(crate) ruleset_version: String,
    pub(crate) chessembly_version: String,
    pub(crate) started_at_ms: i64,
    pub(crate) ended_at_ms: Option<i64>,
    pub(crate) result: Option<GameResult>,
    pub(crate) players: HashMap<PlayerId, GameRecordPlayer>,
    pub(crate) time_control: TimeControlId,
    pub(crate) initial_state: GameState,
    pub(crate) initial_clock: ClockSnapshot,
    pub(crate) decks: HashMap<PlayerId, DeckSnapshot>,
    pub(crate) actions: Vec<RecordedAction>,
    pub(crate) final_clock: Option<ClockSnapshot>,
    #[serde(default)]
    pub(crate) game_mode: GameMode,
    #[serde(default)]
    pub(crate) challenge_id: Option<String>,
    #[serde(default)]
    pub(crate) retention_mode: RetentionMode,
    #[serde(default)]
    pub(crate) expires_at_ms: Option<i64>,
}

impl GameRecord {
    pub(crate) fn new(
        initial_state: GameState,
        players: HashMap<PlayerId, GameRecordPlayer>,
        time_control: TimeControlId,
        started_at_ms: i64,
        initial_clock: ClockSnapshot,
        ownership: GameRecordOwnership,
    ) -> Self {
        let map_id = format!(
            "standard-{}x{}",
            initial_state.board.size, initial_state.board.size
        );
        Self::new_with_deck_names(
            initial_state,
            players,
            time_control,
            started_at_ms,
            initial_clock,
            ownership,
            HashMap::new(),
            map_id,
        )
    }

    pub(crate) fn new_with_deck_names(
        mut initial_state: GameState,
        players: HashMap<PlayerId, GameRecordPlayer>,
        time_control: TimeControlId,
        started_at_ms: i64,
        initial_clock: ClockSnapshot,
        ownership: GameRecordOwnership,
        deck_names: HashMap<PlayerId, String>,
        map_id: String,
    ) -> Self {
        initial_state.history.clear();
        let decks = build_deck_snapshots(&initial_state, &deck_names, &map_id);
        let white = players
            .get("white")
            .and_then(|p| p.public_id.as_deref())
            .or_else(|| players.get("white").map(|p| p.nickname.as_str()))
            .unwrap_or("white");
        let black = players
            .get("black")
            .and_then(|p| p.public_id.as_deref())
            .or_else(|| players.get("black").map(|p| p.nickname.as_str()))
            .unwrap_or("black");
        Self {
            ownership,
            format_version: GAME_RECORD_FORMAT_VERSION,
            game_id: initial_state.id.clone(),
            display_name: replay_display_name(white, black, started_at_ms),
            ruleset_version: RULESET_VERSION.into(),
            chessembly_version: CHESSEMBLY_VERSION.into(),
            started_at_ms,
            ended_at_ms: None,
            result: None,
            players,
            time_control,
            initial_state,
            initial_clock,
            decks,
            actions: Vec::new(),
            final_clock: None,
            game_mode: GameMode::Standard,
            challenge_id: None,
            retention_mode: RetentionMode::Permanent,
            expires_at_ms: None,
        }
    }

    pub(crate) fn push_action(
        &mut self,
        _player_id: PlayerId,
        action: TurnAction,
        elapsed_ms: i64,
        clock_before_ms: Option<i64>,
        clock_after_ms: Option<i64>,
        clock: ClockSnapshot,
        state_before: &GameState,
        state_after: GameState,
    ) {
        let player_id = action_player_id(&action).clone();
        let notation = build_notation(&action, state_before);
        let state_delta = build_state_delta(state_before, &state_after);
        self.actions.push(RecordedAction {
            ply: self.actions.len() as u32 + 1,
            player_id,
            action,
            notation,
            state_delta,
            elapsed_ms,
            clock_before_ms,
            clock_after_ms,
            clock,
        });
    }

    pub(crate) fn finalize(&mut self, state: &GameState, clock: ClockSnapshot, ended_at_ms: i64) {
        if self.ended_at_ms.is_some() {
            return;
        }
        self.ended_at_ms = Some(ended_at_ms);
        self.retention_mode = RetentionMode::Auto;
        self.expires_at_ms = Some(ended_at_ms.saturating_add(AUTO_RETENTION_MS));
        self.result = state.result.clone();
        self.final_clock = Some(clock);
    }

    pub(crate) fn is_expired_at(&self, now_ms: i64) -> bool {
        self.retention_mode == RetentionMode::Auto
            && self.expires_at_ms.is_some_and(|expires| expires <= now_ms)
    }

    pub(crate) fn state_at_ply(&self, ply: u32) -> Result<GameState, &'static str> {
        if ply as usize > self.actions.len() {
            return Err("invalid_ply");
        }
        let mut value = serde_json::to_value(&self.initial_state).map_err(|_| "invalid_record")?;
        for recorded in self.actions.iter().take(ply as usize) {
            for operation in &recorded.state_delta {
                apply_delta_operation(&mut value, operation)?;
            }
        }
        let mut state: GameState = serde_json::from_value(value).map_err(|_| "invalid_record")?;
        state.history.clear();
        Ok(state)
    }
}

fn apply_delta_operation(
    root: &mut Value,
    operation: &StateDeltaOperation,
) -> Result<(), &'static str> {
    let (path, replacement) = match operation {
        StateDeltaOperation::Set { path, value } => (path, Some(value.clone())),
        StateDeltaOperation::Remove { path } => (path, None),
    };
    if path.is_empty()
        || path
            .iter()
            .any(|part| matches!(part.as_str(), "__proto__" | "prototype" | "constructor"))
    {
        return Err("invalid_record");
    }
    let mut parent = root;
    for segment in &path[..path.len() - 1] {
        parent = parent
            .as_object_mut()
            .and_then(|object| object.get_mut(segment))
            .ok_or("invalid_record")?;
    }
    let object = parent.as_object_mut().ok_or("invalid_record")?;
    let key = path.last().ok_or("invalid_record")?;
    if let Some(value) = replacement {
        object.insert(key.clone(), value);
    } else {
        object.remove(key);
    }
    Ok(())
}

fn piece_name(state: &GameState, piece: &Piece) -> String {
    state
        .piece_definitions
        .get(&piece.type_id)
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|| piece.type_id.clone())
}

fn custom_piece_snapshot(
    state: &GameState,
    piece_type_id: &str,
) -> Option<CustomDeckPieceSnapshot> {
    let rest = piece_type_id.strip_prefix("custom:")?;
    let (custom_piece_id, version_and_key) = rest.rsplit_once(":v")?;
    let (version, exposed_piece_key) = version_and_key.split_once(':')?;
    let version = version.parse().ok()?;
    let manifest = state
        .custom_piece_manifest
        .iter()
        .find(|entry| entry.exposed_type_id == piece_type_id)?;
    Some(CustomDeckPieceSnapshot {
        custom_piece_id: custom_piece_id.into(),
        version,
        content_hash: manifest.content_hash.clone(),
        exposed_piece_key: exposed_piece_key.into(),
    })
}

fn build_deck_snapshots(
    state: &GameState,
    deck_names: &HashMap<PlayerId, String>,
    map_id: &str,
) -> HashMap<PlayerId, DeckSnapshot> {
    state
        .players
        .iter()
        .map(|(side, player)| {
            let mut deployments = player
                .deck
                .starting_pieces
                .iter()
                .filter_map(|id| state.pieces.get(id))
                .filter_map(|piece| {
                    Some(DeckDeploymentSnapshot {
                        piece_type_id: piece.type_id.clone(),
                        piece_name: piece_name(state, piece),
                        custom_piece: custom_piece_snapshot(state, &piece.type_id),
                        square: piece.current_square?,
                    })
                })
                .collect::<Vec<_>>();
            deployments.sort_by_key(|entry| {
                (
                    entry.square.rank,
                    entry.square.file,
                    entry.piece_name.clone(),
                )
            });

            let mut counts = HashMap::<(String, String), u32>::new();
            for id in &player.deck.pocket_pieces {
                if let Some(piece) = state.pieces.get(id) {
                    *counts
                        .entry((piece.type_id.clone(), piece_name(state, piece)))
                        .or_default() += 1;
                }
            }
            let mut pocket = counts
                .into_iter()
                .map(|((piece_type_id, piece_name), count)| DeckPocketSnapshot {
                    custom_piece: custom_piece_snapshot(state, &piece_type_id),
                    piece_type_id,
                    piece_name,
                    count,
                })
                .collect::<Vec<_>>();
            pocket.sort_by(|left, right| left.piece_name.cmp(&right.piece_name));
            let deck_name = deck_names
                .get(side)
                .filter(|name| !name.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("{} deck", side));
            (
                side.clone(),
                DeckSnapshot {
                    snapshot_version: 1,
                    side: side.clone(),
                    deck_name,
                    map_id: map_id.into(),
                    board_size: state.board.size,
                    deployments,
                    pocket,
                },
            )
        })
        .collect()
}

fn actor_piece_id(action: &TurnAction) -> &PieceId {
    match action {
        TurnAction::Move(action) => &action.piece_id,
        TurnAction::Drop(action) => &action.piece_id,
        TurnAction::Ability(action) => &action.piece_id,
    }
}

fn action_player_id(action: &TurnAction) -> &PlayerId {
    match action {
        TurnAction::Move(action) => &action.player_id,
        TurnAction::Drop(action) => &action.player_id,
        TurnAction::Ability(action) => &action.player_id,
    }
}

/// Engine turn numbers advance after each completed player-turn. Forced
/// follow-up actions keep the current engine turn number.
fn full_move_number(engine_turn_number: u32) -> u32 {
    engine_turn_number.saturating_add(1) / 2
}

fn ability_name(state: &GameState, piece: &Piece, ability_id: &str) -> String {
    state
        .piece_definitions
        .get(&piece.type_id)
        .and_then(|definition| {
            definition
                .move_options
                .iter()
                .find(|option| option.id == ability_id)
        })
        .map(|option| option.name.clone())
        .unwrap_or_else(|| ability_id.to_string())
}

fn build_notation(action: &TurnAction, state_before: &GameState) -> RecordedNotationAction {
    let piece = state_before.pieces.get(actor_piece_id(action));
    let actor = piece
        .map(|piece| ActorSnapshot {
            piece_id: piece.id.to_string(),
            piece_type_id: piece.type_id.clone(),
            piece_name: piece_name(state_before, piece),
            from: piece.current_square,
            layer: piece.layer.clone(),
            current_ammo: Some(piece.current_ammo),
            state: piece.state.clone(),
        })
        .unwrap_or_else(|| ActorSnapshot {
            piece_id: actor_piece_id(action).to_string(),
            piece_type_id: "unknown".into(),
            piece_name: "unknown".into(),
            from: None,
            layer: PieceLayer::Ground,
            current_ammo: None,
            state: HashMap::new(),
        });

    let (kind, ability_id, ability_name_value, to, target) = match action {
        TurnAction::Move(move_action) => {
            let is_ability = piece
                .and_then(|piece| state_before.piece_definitions.get(&piece.type_id))
                .and_then(|definition| {
                    definition
                        .move_options
                        .iter()
                        .find(|option| option.id == move_action.move_option_id)
                })
                .is_some_and(|option| option.kind == MoveOptionKind::Ability);
            let id = is_ability.then(|| move_action.move_option_id.clone());
            let name = id.as_deref().map(|id| {
                piece
                    .map(|piece| ability_name(state_before, piece, id))
                    .unwrap_or_else(|| id.into())
            });
            (
                if is_ability {
                    NotationActionKind::MoveWithAbility
                } else {
                    NotationActionKind::Move
                },
                id,
                name,
                Some(move_action.to),
                None,
            )
        }
        TurnAction::Drop(drop_action) => (
            NotationActionKind::Drop,
            None,
            None,
            Some(drop_action.to),
            None,
        ),
        TurnAction::Ability(ability_action) => {
            let name = piece
                .map(|piece| ability_name(state_before, piece, &ability_action.ability_id))
                .unwrap_or_else(|| ability_action.ability_id.clone());
            (
                NotationActionKind::Ability,
                Some(ability_action.ability_id.clone()),
                Some(name),
                ability_action.to,
                ability_action.to,
            )
        }
    };
    let ability_events = ability_id
        .as_ref()
        .zip(ability_name_value.as_ref())
        .map(|(ability_id, ability_name)| {
            vec![AbilityEventSnapshot {
                ability_id: ability_id.clone(),
                ability_name: ability_name.clone(),
                target,
            }]
        })
        .unwrap_or_default();
    RecordedNotationAction {
        turn_number: state_before.turn_number,
        move_number: full_move_number(state_before.turn_number),
        side: action_player_id(action).clone(),
        from: actor.from,
        actor,
        kind,
        ability_id,
        ability_name: ability_name_value,
        to,
        target,
        ability_events,
    }
}

fn replay_state_value(state: &GameState) -> Value {
    let mut value = serde_json::to_value(state).unwrap_or(Value::Null);
    if let Value::Object(root) = &mut value {
        root.remove("piece_definitions");
        root.remove("custom_piece_manifest");
        root.remove("history");
        root.remove("id");
        if let Some(Value::Object(board)) = root.get_mut("board") {
            board.remove("size");
            board.remove("terrain");
        }
    }
    value
}

fn build_state_delta(before: &GameState, after: &GameState) -> Vec<StateDeltaOperation> {
    let mut operations = Vec::new();
    diff_values(
        &replay_state_value(before),
        &replay_state_value(after),
        &mut Vec::new(),
        &mut operations,
    );
    operations
}

fn diff_values(
    before: &Value,
    after: &Value,
    path: &mut Vec<String>,
    output: &mut Vec<StateDeltaOperation>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            for key in before.keys().filter(|key| !after.contains_key(*key)) {
                let mut removed_path = path.clone();
                removed_path.push(key.clone());
                output.push(StateDeltaOperation::Remove { path: removed_path });
            }
            for (key, value) in after {
                path.push(key.clone());
                if let Some(previous) = before.get(key) {
                    diff_values(previous, value, path, output);
                } else {
                    output.push(StateDeltaOperation::Set {
                        path: path.clone(),
                        value: value.clone(),
                    });
                }
                path.pop();
            }
        }
        _ => output.push(StateDeltaOperation::Set {
            path: path.clone(),
            value: after.clone(),
        }),
    }
}

fn safe_name_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "player".into()
    } else {
        trimmed.chars().take(32).collect()
    }
}

fn replay_display_name(white: &str, black: &str, started_at_ms: i64) -> String {
    // UTC makes exported names deterministic across clients and servers.
    let seconds = started_at_ms.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    format!(
        "{}-{}-{year:04}-{month:02}-{day:02}-{hour:02}{minute:02}",
        safe_name_part(white),
        safe_name_part(black)
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[async_trait]
pub(crate) trait GameRecordRepository: Send + Sync {
    async fn save(&self, record: &GameRecord) -> Result<(), &'static str>;
    async fn get(&self, game_id: &str) -> Result<Option<GameRecord>, &'static str>;
    async fn list_summaries_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecordSummary>, &'static str>;
    async fn set_retention(
        &self,
        _game_id: &str,
        _user_id: &str,
        _permanent: bool,
        _now_ms: i64,
    ) -> Result<Option<GameRecord>, &'static str> {
        Err("unavailable")
    }
    async fn cleanup_expired(&self, _now_ms: i64) -> Result<u64, &'static str> {
        Ok(0)
    }
}

pub(crate) type GameRecordStore = Arc<dyn GameRecordRepository>;

#[derive(Default)]
pub(crate) struct InMemoryGameRecordRepository(RwLock<HashMap<String, GameRecord>>);

#[async_trait]
impl GameRecordRepository for InMemoryGameRecordRepository {
    async fn save(&self, record: &GameRecord) -> Result<(), &'static str> {
        self.0
            .write()
            .map_err(|_| "unavailable")?
            .insert(record.game_id.clone(), record.clone());
        Ok(())
    }
    async fn get(&self, game_id: &str) -> Result<Option<GameRecord>, &'static str> {
        Ok(self
            .0
            .read()
            .map_err(|_| "unavailable")?
            .get(game_id)
            .filter(|record| !record.is_expired_at(crate::time_control::now_ms()))
            .cloned())
    }
    async fn list_summaries_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecordSummary>, &'static str> {
        let mut records = self
            .0
            .read()
            .map_err(|_| "unavailable")?
            .values()
            .filter(|record| record.ownership.contains(user_id))
            .filter(|record| !record.is_expired_at(crate::time_control::now_ms()))
            .map(|record| GameRecordSummary::from_record(record, user_id))
            .collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.started_at_ms));
        records.truncate(limit.max(0) as usize);
        Ok(records)
    }
    async fn set_retention(
        &self,
        game_id: &str,
        user_id: &str,
        permanent: bool,
        now_ms: i64,
    ) -> Result<Option<GameRecord>, &'static str> {
        let mut records = self.0.write().map_err(|_| "unavailable")?;
        let Some(record) = records.get_mut(game_id) else {
            return Ok(None);
        };
        if !record.ownership.contains(user_id) {
            return Err("forbidden");
        }
        record.retention_mode = if permanent {
            RetentionMode::Permanent
        } else {
            RetentionMode::Auto
        };
        record.expires_at_ms = if permanent {
            None
        } else {
            record
                .ended_at_ms
                .map(|ended| ended.saturating_add(AUTO_RETENTION_MS))
        };
        if record.is_expired_at(now_ms) {
            records.remove(game_id);
            return Ok(None);
        }
        Ok(Some(record.clone()))
    }
    async fn cleanup_expired(&self, now_ms: i64) -> Result<u64, &'static str> {
        let mut records = self.0.write().map_err(|_| "unavailable")?;
        let before = records.len();
        records.retain(|_, record| !record.is_expired_at(now_ms));
        Ok((before - records.len()) as u64)
    }
}

pub(crate) struct PostgresGameRecordRepository {
    pool: PgPool,
    table: String,
}

impl PostgresGameRecordRepository {
    pub(crate) fn new(pool: PgPool, schema: DataSchema) -> Self {
        Self {
            pool,
            table: format!("{}.game_records", schema.name()),
        }
    }
}

#[async_trait]
impl GameRecordRepository for PostgresGameRecordRepository {
    async fn save(&self, record: &GameRecord) -> Result<(), &'static str> {
        let value = serde_json::to_value(record).map_err(|_| "unavailable")?;
        sqlx::query(&format!("INSERT INTO {} AS target (id, white_public_id, black_public_id, white_user_id, black_user_id, started_at_ms, ended_at_ms, result_reason, display_name, record_version, record, retention_mode, expires_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) ON CONFLICT (id) DO UPDATE SET white_user_id=COALESCE(EXCLUDED.white_user_id, target.white_user_id), black_user_id=COALESCE(EXCLUDED.black_user_id, target.black_user_id), ended_at_ms=EXCLUDED.ended_at_ms, result_reason=EXCLUDED.result_reason, record=EXCLUDED.record, retention_mode=EXCLUDED.retention_mode, expires_at_ms=EXCLUDED.expires_at_ms", self.table))
            .bind(&record.game_id)
            .bind(record.players.get("white").and_then(|p| p.public_id.as_deref()))
            .bind(record.players.get("black").and_then(|p| p.public_id.as_deref()))
            .bind(record.ownership.white_user_id.as_deref())
            .bind(record.ownership.black_user_id.as_deref())
            .bind(record.started_at_ms)
            .bind(record.ended_at_ms)
            .bind(record.result.as_ref().map(|result| format!("{:?}", result.reason)))
            .bind(&record.display_name)
            .bind(record.format_version as i32)
            .bind(value)
            .bind(match record.retention_mode { RetentionMode::Auto => "auto", RetentionMode::Permanent => "permanent" })
            .bind(record.expires_at_ms)
            .execute(&self.pool).await.map_err(|_| "unavailable")?;
        Ok(())
    }
    async fn get(&self, game_id: &str) -> Result<Option<GameRecord>, &'static str> {
        let row = sqlx::query(&format!(
            "SELECT record, white_user_id, black_user_id, retention_mode, expires_at_ms FROM {} WHERE id=$1 AND NOT (retention_mode='auto' AND expires_at_ms <= $2)",
            self.table
        ))
        .bind(game_id)
        .bind(crate::time_control::now_ms())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        let Some(row) = row else { return Ok(None) };
        let mut record: GameRecord =
            serde_json::from_value(row.try_get("record").map_err(|_| "unavailable")?)
                .map_err(|_| "unavailable")?;
        record.ownership = GameRecordOwnership {
            white_user_id: row.try_get("white_user_id").map_err(|_| "unavailable")?,
            black_user_id: row.try_get("black_user_id").map_err(|_| "unavailable")?,
            persist: true,
        };
        record.retention_mode = match row
            .try_get::<String, _>("retention_mode")
            .map_err(|_| "unavailable")?
            .as_str()
        {
            "auto" => RetentionMode::Auto,
            _ => RetentionMode::Permanent,
        };
        record.expires_at_ms = row.try_get("expires_at_ms").map_err(|_| "unavailable")?;
        Ok(Some(record))
    }
    async fn list_summaries_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecordSummary>, &'static str> {
        let rows = sqlx::query(&format!("SELECT records.id, display_name, started_at_ms, ended_at_ms, record->'result' AS result, record->'players' AS players, record->'time_control' AS time_control, COALESCE(record->'game_mode', '\"standard\"'::jsonb) AS game_mode, record->>'challenge_id' AS challenge_id, CASE WHEN black_user_id=$1 THEN 'black' ELSE 'white' END AS owner_side, retention_mode, expires_at_ms, (SELECT COUNT(*) FROM {schema}.game_analysis_trees trees WHERE trees.game_id=records.id AND trees.owner_user_id=$1) AS analysis_count FROM {table} records WHERE (white_user_id=$1 OR black_user_id=$1) AND NOT (retention_mode='auto' AND expires_at_ms <= $2) ORDER BY started_at_ms DESC LIMIT $3", schema=self.table.split('.').next().unwrap_or("test"), table=self.table))
            .bind(user_id).bind(crate::time_control::now_ms()).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await.map_err(|_| "unavailable")?;
        rows.into_iter()
            .map(|row| {
                Ok(GameRecordSummary {
                    game_id: row.try_get("id").map_err(|_| "unavailable")?,
                    display_name: row.try_get("display_name").map_err(|_| "unavailable")?,
                    started_at_ms: row.try_get("started_at_ms").map_err(|_| "unavailable")?,
                    ended_at_ms: row.try_get("ended_at_ms").map_err(|_| "unavailable")?,
                    result: serde_json::from_value(
                        row.try_get("result").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    players: serde_json::from_value(
                        row.try_get("players").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    time_control: serde_json::from_value(
                        row.try_get("time_control").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    owner_side: row.try_get("owner_side").map_err(|_| "unavailable")?,
                    game_mode: serde_json::from_value(
                        row.try_get("game_mode").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    challenge_id: row.try_get("challenge_id").map_err(|_| "unavailable")?,
                    retention_mode: match row
                        .try_get::<String, _>("retention_mode")
                        .map_err(|_| "unavailable")?
                        .as_str()
                    {
                        "auto" => RetentionMode::Auto,
                        _ => RetentionMode::Permanent,
                    },
                    expires_at_ms: row.try_get("expires_at_ms").map_err(|_| "unavailable")?,
                    analysis_count: row.try_get("analysis_count").map_err(|_| "unavailable")?,
                })
            })
            .collect()
    }
    async fn set_retention(
        &self,
        game_id: &str,
        user_id: &str,
        permanent: bool,
        now_ms: i64,
    ) -> Result<Option<GameRecord>, &'static str> {
        let mode = if permanent { "permanent" } else { "auto" };
        let row = sqlx::query(&format!("UPDATE {} SET retention_mode=$3, expires_at_ms=CASE WHEN $3='permanent' THEN NULL ELSE ended_at_ms + $4 END, record=jsonb_set(jsonb_set(record, '{{retention_mode}}', to_jsonb($3::text)), '{{expires_at_ms}}', CASE WHEN $3='permanent' THEN 'null'::jsonb ELSE to_jsonb(ended_at_ms + $4) END) WHERE id=$1 AND (white_user_id=$2 OR black_user_id=$2) RETURNING record, white_user_id, black_user_id, retention_mode, expires_at_ms", self.table))
            .bind(game_id).bind(user_id).bind(mode).bind(AUTO_RETENTION_MS).fetch_optional(&self.pool).await.map_err(|_| "unavailable")?;
        let Some(row) = row else { return Ok(None) };
        let expires: Option<i64> = row.try_get("expires_at_ms").map_err(|_| "unavailable")?;
        if mode == "auto" && expires.is_some_and(|value| value <= now_ms) {
            sqlx::query(&format!("DELETE FROM {} WHERE id=$1", self.table))
                .bind(game_id)
                .execute(&self.pool)
                .await
                .map_err(|_| "unavailable")?;
            return Ok(None);
        }
        let mut record: GameRecord =
            serde_json::from_value(row.try_get("record").map_err(|_| "unavailable")?)
                .map_err(|_| "unavailable")?;
        record.ownership = GameRecordOwnership {
            white_user_id: row.try_get("white_user_id").map_err(|_| "unavailable")?,
            black_user_id: row.try_get("black_user_id").map_err(|_| "unavailable")?,
            persist: true,
        };
        record.retention_mode = if permanent {
            RetentionMode::Permanent
        } else {
            RetentionMode::Auto
        };
        record.expires_at_ms = expires;
        Ok(Some(record))
    }
    async fn cleanup_expired(&self, now_ms: i64) -> Result<u64, &'static str> {
        sqlx::query(&format!(
            "DELETE FROM {} WHERE retention_mode='auto' AND expires_at_ms <= $1",
            self.table
        ))
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map(|result| result.rows_affected())
        .map_err(|_| "unavailable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brainfuck_chess_engine::types::{Board, ChessemblyProgramCache, GamePhase};
    use uuid::Uuid;

    #[test]
    fn display_name_is_white_black_timestamp_and_sanitized() {
        assert_eq!(
            replay_display_name("playerA", "bad/id", 1_777_209_120_000),
            "playerA-bad_id-2026-04-26-1312"
        );
    }

    #[test]
    fn finalized_records_use_thirty_days_from_the_authoritative_end_time() {
        let mut record =
            postgres_test_record("retention-finalize".into(), "owner".into(), "other".into());
        let state = record.initial_state.clone();
        let clock = record.initial_clock.clone();
        record.finalize(&state, clock, 10_000);
        assert_eq!(record.retention_mode, RetentionMode::Auto);
        assert_eq!(record.expires_at_ms, Some(10_000 + AUTO_RETENTION_MS));
    }

    #[tokio::test]
    async fn permanent_toggle_restores_original_expiry_and_cleanup_preserves_permanent() {
        let repository = InMemoryGameRecordRepository::default();
        let mut record =
            postgres_test_record("retention-toggle".into(), "owner".into(), "other".into());
        let state = record.initial_state.clone();
        let clock = record.initial_clock.clone();
        record.finalize(&state, clock, 1_000);
        repository.save(&record).await.unwrap();
        let permanent = repository
            .set_retention(&record.game_id, "owner", true, AUTO_RETENTION_MS + 2_000)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(permanent.retention_mode, RetentionMode::Permanent);
        assert_eq!(permanent.expires_at_ms, None);
        assert_eq!(
            repository
                .cleanup_expired(AUTO_RETENTION_MS + 2_000)
                .await
                .unwrap(),
            0
        );
        let removed = repository
            .set_retention(&record.game_id, "owner", false, AUTO_RETENTION_MS + 2_000)
            .await
            .unwrap();
        assert!(
            removed.is_none(),
            "unpinning after the original deadline deletes immediately"
        );
    }

    #[tokio::test]
    async fn logically_expired_records_are_hidden_before_physical_cleanup() {
        let repository = InMemoryGameRecordRepository::default();
        let mut expired =
            postgres_test_record("expired-hidden".into(), "owner".into(), "other".into());
        expired.retention_mode = RetentionMode::Auto;
        expired.expires_at_ms = Some(0);
        repository.save(&expired).await.unwrap();
        assert!(repository.get(&expired.game_id).await.unwrap().is_none());
        assert!(repository
            .list_summaries_for_user_id("owner", 50)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(repository.cleanup_expired(1).await.unwrap(), 1);
    }

    #[test]
    fn legacy_record_without_retention_fields_is_protected() {
        let record =
            postgres_test_record("legacy-retention".into(), "owner".into(), "other".into());
        let mut value = serde_json::to_value(record).unwrap();
        value.as_object_mut().unwrap().remove("retention_mode");
        value.as_object_mut().unwrap().remove("expires_at_ms");
        let restored: GameRecord = serde_json::from_value(value).unwrap();
        assert_eq!(restored.retention_mode, RetentionMode::Permanent);
        assert_eq!(restored.expires_at_ms, None);
    }

    #[test]
    fn reconstructed_analysis_base_does_not_mutate_the_canonical_record() {
        let record =
            postgres_test_record("immutable-canonical".into(), "owner".into(), "other".into());
        let original_turn = record.initial_state.turn_number;
        let mut analysis_base = record.state_at_ply(0).unwrap();
        analysis_base.turn_number += 10;
        assert_eq!(record.initial_state.turn_number, original_turn);
        assert!(record.actions.is_empty());
    }

    fn postgres_test_record(
        game_id: String,
        white_user_id: String,
        black_user_id: String,
    ) -> GameRecord {
        let state = GameState {
            id: game_id,
            board: Board {
                size: 8,
                squares: HashMap::new(),
                air_squares: HashMap::new(),
                terrain: HashMap::new(),
            },
            pieces: HashMap::new(),
            piece_definitions: HashMap::new(),
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
            chessembly_program_cache: ChessemblyProgramCache::default(),
        };
        let clock = ClockSnapshot {
            time_control: TimeControlId::Unlimited,
            mode: crate::time_control::TimeControlMode::Unlimited,
            initial_time_ms: None,
            increment_ms: 0,
            active_color: "white".into(),
            turn_started_at_ms: Some(1),
            server_now_ms: 1,
            white_remaining_ms: None,
            black_remaining_ms: None,
            white_elapsed_ms: 0,
            black_elapsed_ms: 0,
        };
        GameRecord::new(
            state,
            HashMap::from([
                (
                    "white".into(),
                    GameRecordPlayer {
                        public_id: Some("white-public".into()),
                        nickname: "White".into(),
                        side: "white".into(),
                    },
                ),
                (
                    "black".into(),
                    GameRecordPlayer {
                        public_id: Some("black-public".into()),
                        nickname: "Black".into(),
                        side: "black".into(),
                    },
                ),
            ]),
            TimeControlId::Unlimited,
            1,
            clock,
            GameRecordOwnership {
                white_user_id: Some(white_user_id),
                black_user_id: Some(black_user_id),
                persist: true,
            },
        )
    }

    #[tokio::test]
    #[ignore = "requires TEST_GAME_RECORD_DATABASE_URL for a disposable migrated PostgreSQL database"]
    async fn postgres_repository_inserts_upserts_gets_and_lists_by_internal_user() {
        let database_url = std::env::var("TEST_GAME_RECORD_DATABASE_URL")
            .expect("TEST_GAME_RECORD_DATABASE_URL is required");
        let pool = PgPool::connect(&database_url).await.unwrap();
        let suffix = Uuid::new_v4().to_string();
        let white_user_id = format!("record-white-{suffix}");
        let black_user_id = format!("record-black-{suffix}");
        for user_id in [&white_user_id, &black_user_id] {
            sqlx::query("INSERT INTO shared.users (id, account_kind, status, created_at, updated_at) VALUES ($1, 'registered', 'active', 1, 1)")
                .bind(user_id)
                .execute(&pool)
                .await
                .unwrap();
        }

        for schema in [DataSchema::Prod, DataSchema::Test] {
            let game_id = format!("record-{}-{suffix}", schema.name());
            let repository = PostgresGameRecordRepository::new(pool.clone(), schema);
            let mut record = postgres_test_record(
                game_id.clone(),
                white_user_id.clone(),
                black_user_id.clone(),
            );
            repository.save(&record).await.unwrap();
            record.ended_at_ms = Some(2);
            repository.save(&record).await.unwrap();

            let loaded = repository.get(&game_id).await.unwrap().unwrap();
            assert_eq!(loaded.ended_at_ms, Some(2));
            assert_eq!(
                loaded.ownership.white_user_id.as_deref(),
                Some(white_user_id.as_str())
            );
            let listed = repository
                .list_summaries_for_user_id(&black_user_id, 50)
                .await
                .unwrap();
            assert!(listed.iter().any(|item| item.game_id == game_id));

            sqlx::query(&format!(
                "DELETE FROM {}.game_records WHERE id = $1",
                schema.name()
            ))
            .bind(&game_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query("DELETE FROM shared.users WHERE id = ANY($1)")
            .bind(vec![white_user_id, black_user_id])
            .execute(&pool)
            .await
            .unwrap();
    }
}
