use async_trait::async_trait;
use brainfuck_chess_engine::types::{GameResult, GameState, PlayerId, TurnAction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::database::DataSchema;
use crate::time_control::{ClockSnapshot, TimeControlId};

pub(crate) const GAME_RECORD_FORMAT_VERSION: u32 = 1;
pub(crate) const RULESET_VERSION: &str = "deck-chess-1";
pub(crate) const CHESSEMBLY_VERSION: &str = "chessembly-1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameRecordPlayer {
    pub(crate) public_id: String,
    pub(crate) nickname: String,
    pub(crate) side: PlayerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecordedAction {
    pub(crate) ply: u32,
    pub(crate) piece_index: u32,
    pub(crate) player_id: PlayerId,
    pub(crate) action: TurnAction,
    pub(crate) elapsed_ms: i64,
    pub(crate) clock_before_ms: Option<i64>,
    pub(crate) clock_after_ms: Option<i64>,
    pub(crate) clock: ClockSnapshot,
    pub(crate) state_hash: String,
    pub(crate) state_after: GameState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GameRecordOwnership {
    pub(crate) white_user_id: Option<String>,
    pub(crate) black_user_id: Option<String>,
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
    pub(crate) piece_id_map: HashMap<String, u32>,
    pub(crate) actions: Vec<RecordedAction>,
    pub(crate) final_state: Option<GameState>,
    pub(crate) final_clock: Option<ClockSnapshot>,
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
        let piece_id_map = initial_state
            .pieces
            .keys()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .enumerate()
            .map(|(index, id)| (id, index as u32))
            .collect();
        let white = players
            .get("white")
            .map(|p| p.public_id.as_str())
            .unwrap_or("white");
        let black = players
            .get("black")
            .map(|p| p.public_id.as_str())
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
            piece_id_map,
            actions: Vec::new(),
            final_state: None,
            final_clock: None,
        }
    }

    pub(crate) fn push_action(
        &mut self,
        player_id: PlayerId,
        action: TurnAction,
        elapsed_ms: i64,
        clock_before_ms: Option<i64>,
        clock_after_ms: Option<i64>,
        clock: ClockSnapshot,
        state_after: GameState,
    ) {
        let piece_id = match &action {
            TurnAction::Move(action) => action.piece_id.to_string(),
            TurnAction::Drop(action) => action.piece_id.to_string(),
            TurnAction::Ability(action) => action.piece_id.to_string(),
        };
        let next_piece_index = self.piece_id_map.len() as u32;
        let piece_index = *self
            .piece_id_map
            .entry(piece_id)
            .or_insert(next_piece_index);
        self.actions.push(RecordedAction {
            ply: self.actions.len() as u32 + 1,
            piece_index,
            player_id,
            action,
            elapsed_ms,
            clock_before_ms,
            clock_after_ms,
            clock,
            state_hash: state_hash(&state_after),
            state_after,
        });
    }

    pub(crate) fn finalize(&mut self, state: &GameState, clock: ClockSnapshot, ended_at_ms: i64) {
        if self.ended_at_ms.is_some() {
            return;
        }
        self.ended_at_ms = Some(ended_at_ms);
        self.result = state.result.clone();
        self.final_state = Some(state.clone());
        self.final_clock = Some(clock);
    }
}

fn state_hash(state: &GameState) -> String {
    let bytes = serde_json::to_vec(state).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
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
    async fn list_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecord>, &'static str>;
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
            .cloned())
    }
    async fn list_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecord>, &'static str> {
        let mut records = self
            .0
            .read()
            .map_err(|_| "unavailable")?
            .values()
            .filter(|record| record.ownership.contains(user_id))
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.started_at_ms));
        records.truncate(limit.max(0) as usize);
        Ok(records)
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
        sqlx::query(&format!("INSERT INTO {} AS target (id, white_public_id, black_public_id, white_user_id, black_user_id, started_at_ms, ended_at_ms, result_reason, display_name, record_version, record) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (id) DO UPDATE SET white_user_id=COALESCE(EXCLUDED.white_user_id, target.white_user_id), black_user_id=COALESCE(EXCLUDED.black_user_id, target.black_user_id), ended_at_ms=EXCLUDED.ended_at_ms, result_reason=EXCLUDED.result_reason, record=EXCLUDED.record", self.table))
            .bind(&record.game_id)
            .bind(record.players.get("white").map(|p| p.public_id.as_str()))
            .bind(record.players.get("black").map(|p| p.public_id.as_str()))
            .bind(record.ownership.white_user_id.as_deref())
            .bind(record.ownership.black_user_id.as_deref())
            .bind(record.started_at_ms)
            .bind(record.ended_at_ms)
            .bind(record.result.as_ref().map(|result| format!("{:?}", result.reason)))
            .bind(&record.display_name)
            .bind(record.format_version as i32)
            .bind(value)
            .execute(&self.pool).await.map_err(|_| "unavailable")?;
        Ok(())
    }
    async fn get(&self, game_id: &str) -> Result<Option<GameRecord>, &'static str> {
        let row = sqlx::query(&format!(
            "SELECT record, white_user_id, black_user_id FROM {} WHERE id=$1",
            self.table
        ))
        .bind(game_id)
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
        };
        Ok(Some(record))
    }
    async fn list_for_user_id(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<GameRecord>, &'static str> {
        let rows = sqlx::query(&format!("SELECT record, white_user_id, black_user_id FROM {} WHERE white_user_id=$1 OR black_user_id=$1 ORDER BY started_at_ms DESC LIMIT $2", self.table))
            .bind(user_id).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await.map_err(|_| "unavailable")?;
        rows.into_iter()
            .map(|row| {
                let mut record: GameRecord =
                    serde_json::from_value(row.try_get("record").map_err(|_| "unavailable")?)
                        .map_err(|_| "unavailable")?;
                record.ownership = GameRecordOwnership {
                    white_user_id: row.try_get("white_user_id").map_err(|_| "unavailable")?,
                    black_user_id: row.try_get("black_user_id").map_err(|_| "unavailable")?,
                };
                Ok(record)
            })
            .collect()
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
                        public_id: "white-public".into(),
                        nickname: "White".into(),
                        side: "white".into(),
                    },
                ),
                (
                    "black".into(),
                    GameRecordPlayer {
                        public_id: "black-public".into(),
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
                .list_for_user_id(&black_user_id, 50)
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
