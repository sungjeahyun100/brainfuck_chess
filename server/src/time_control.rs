use crate::challenge::{ChallengeGameContext, ChallengeGameMetadata};
use crate::game_record::{
    GameMode, GameRecord, GameRecordOwnership, GameRecordPlayer, RecordedNotationAction,
};
use brainfuck_chess_engine::types::{GameEndReason, GamePhase, GameResult, GameState, PlayerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const HEARTBEAT_GRACE_MS: i64 = 10_000;
pub(crate) const ABANDONMENT_WARNING_MS: i64 = 20_000;
pub(crate) const ABANDONMENT_FORFEIT_MS: i64 = 120_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimeControlId {
    FiveZero,
    TenZero,
    FiveThree,
    TenFive,
    FifteenTen,
    Unlimited,
}

impl Default for TimeControlId {
    fn default() -> Self {
        Self::Unlimited
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimeControlMode {
    Countdown,
    Unlimited,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TimeControlDefinition {
    pub(crate) initial_time_ms: Option<i64>,
    pub(crate) increment_ms: i64,
    pub(crate) mode: TimeControlMode,
}

impl TimeControlId {
    pub(crate) fn definition(self) -> TimeControlDefinition {
        match self {
            Self::FiveZero => countdown(5, 0),
            Self::TenZero => countdown(10, 0),
            Self::FiveThree => countdown(5, 3),
            Self::TenFive => countdown(10, 5),
            Self::FifteenTen => countdown(15, 10),
            Self::Unlimited => TimeControlDefinition {
                initial_time_ms: None,
                increment_ms: 0,
                mode: TimeControlMode::Unlimited,
            },
        }
    }
}

const fn countdown(minutes: i64, increment_seconds: i64) -> TimeControlDefinition {
    TimeControlDefinition {
        initial_time_ms: Some(minutes * 60_000),
        increment_ms: increment_seconds * 1_000,
        mode: TimeControlMode::Countdown,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClockState {
    time_control: TimeControlId,
    active_color: PlayerId,
    turn_started_at_ms: i64,
    white_remaining_ms: Option<i64>,
    black_remaining_ms: Option<i64>,
    white_elapsed_ms: i64,
    black_elapsed_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClockSnapshot {
    pub(crate) time_control: TimeControlId,
    pub(crate) mode: TimeControlMode,
    pub(crate) initial_time_ms: Option<i64>,
    pub(crate) increment_ms: i64,
    pub(crate) active_color: PlayerId,
    pub(crate) turn_started_at_ms: Option<i64>,
    pub(crate) server_now_ms: i64,
    pub(crate) white_remaining_ms: Option<i64>,
    pub(crate) black_remaining_ms: Option<i64>,
    pub(crate) white_elapsed_ms: i64,
    pub(crate) black_elapsed_ms: i64,
}

impl ClockState {
    pub(crate) fn new(time_control: TimeControlId, now_ms: i64) -> Self {
        let definition = time_control.definition();
        Self {
            time_control,
            active_color: "white".into(),
            turn_started_at_ms: now_ms,
            white_remaining_ms: definition.initial_time_ms,
            black_remaining_ms: definition.initial_time_ms,
            white_elapsed_ms: 0,
            black_elapsed_ms: 0,
        }
    }

    fn active_delta(&self, now_ms: i64) -> i64 {
        now_ms.saturating_sub(self.turn_started_at_ms).max(0)
    }

    pub(crate) fn snapshot(&self, now_ms: i64, running: bool) -> ClockSnapshot {
        let definition = self.time_control.definition();
        let delta = if running {
            self.active_delta(now_ms)
        } else {
            0
        };
        let mut white_remaining = self.white_remaining_ms;
        let mut black_remaining = self.black_remaining_ms;
        let mut white_elapsed = self.white_elapsed_ms;
        let mut black_elapsed = self.black_elapsed_ms;
        if self.active_color == "white" {
            white_remaining = white_remaining.map(|value| value.saturating_sub(delta).max(0));
            if definition.mode == TimeControlMode::Unlimited {
                white_elapsed += delta;
            }
        } else {
            black_remaining = black_remaining.map(|value| value.saturating_sub(delta).max(0));
            if definition.mode == TimeControlMode::Unlimited {
                black_elapsed += delta;
            }
        }
        ClockSnapshot {
            time_control: self.time_control,
            mode: definition.mode,
            initial_time_ms: definition.initial_time_ms,
            increment_ms: definition.increment_ms,
            active_color: self.active_color.clone(),
            turn_started_at_ms: running.then_some(self.turn_started_at_ms),
            server_now_ms: now_ms,
            white_remaining_ms: white_remaining,
            black_remaining_ms: black_remaining,
            white_elapsed_ms: white_elapsed,
            black_elapsed_ms: black_elapsed,
        }
    }

    pub(crate) fn timeout_loser(&self, now_ms: i64) -> Option<PlayerId> {
        let snapshot = self.snapshot(now_ms, true);
        match self.active_color.as_str() {
            "white" if snapshot.white_remaining_ms == Some(0) => Some("white".into()),
            "black" if snapshot.black_remaining_ms == Some(0) => Some("black".into()),
            _ => None,
        }
    }

    pub(crate) fn finish_turn(
        &mut self,
        moving_player: &str,
        next_player: &str,
        now_ms: i64,
        game_ended: bool,
    ) {
        let delta = self.active_delta(now_ms);
        let definition = self.time_control.definition();
        let player_turn_completed = game_ended || moving_player != next_player;
        if definition.mode == TimeControlMode::Countdown {
            let remaining = if moving_player == "white" {
                &mut self.white_remaining_ms
            } else {
                &mut self.black_remaining_ms
            };
            if let Some(value) = remaining {
                *value = value.saturating_sub(delta).max(0);
                if player_turn_completed {
                    *value += definition.increment_ms;
                }
            }
        } else if moving_player == "white" {
            self.white_elapsed_ms += delta;
        } else {
            self.black_elapsed_ms += delta;
        }
        self.active_color = next_player.into();
        self.turn_started_at_ms = now_ms;
        if game_ended {
            // The settled values are retained and snapshots no longer advance.
        }
    }

    pub(crate) fn stop(&mut self, now_ms: i64) {
        let delta = self.active_delta(now_ms);
        if self.time_control.definition().mode == TimeControlMode::Countdown {
            let remaining = if self.active_color == "white" {
                &mut self.white_remaining_ms
            } else {
                &mut self.black_remaining_ms
            };
            if let Some(value) = remaining {
                *value = value.saturating_sub(delta).max(0);
            }
        } else if self.active_color == "white" {
            self.white_elapsed_ms += delta;
        } else {
            self.black_elapsed_ms += delta;
        }
        self.turn_started_at_ms = now_ms;
    }
}

#[derive(Debug, Clone, Default)]
struct PresenceState {
    last_seen_ms: HashMap<PlayerId, i64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlayerPresenceSnapshot {
    pub(crate) connected: bool,
    pub(crate) disconnected_at_ms: Option<i64>,
    pub(crate) warning_at_ms: Option<i64>,
    pub(crate) forfeit_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PresenceSnapshot {
    pub(crate) white: PlayerPresenceSnapshot,
    pub(crate) black: PlayerPresenceSnapshot,
}

#[derive(Clone)]
pub(crate) struct StoredGame {
    pub(crate) state: GameState,
    pub(crate) clock: ClockState,
    presence: Option<PresenceState>,
    pub(crate) record: GameRecord,
    record_persisted: bool,
    pub(crate) challenge: Option<ChallengeGameContext>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GameView {
    #[serde(flatten)]
    pub(crate) state: GameState,
    pub(crate) clock: ClockSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) presence: Option<PresenceSnapshot>,
    pub(crate) player_info: HashMap<PlayerId, GameRecordPlayer>,
    pub(crate) record_notation: Vec<RecordedNotationAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) challenge: Option<ChallengeGameMetadata>,
}

impl Deref for GameView {
    type Target = GameState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl StoredGame {
    #[cfg(test)]
    pub(crate) fn new(
        state: GameState,
        time_control: TimeControlId,
        multiplayer: bool,
        now_ms: i64,
    ) -> Self {
        let players = default_record_players();
        Self::new_with_players(
            state,
            time_control,
            multiplayer,
            now_ms,
            players,
            GameRecordOwnership::default(),
        )
    }

    pub(crate) fn new_with_players(
        state: GameState,
        time_control: TimeControlId,
        multiplayer: bool,
        now_ms: i64,
        players: HashMap<PlayerId, GameRecordPlayer>,
        ownership: GameRecordOwnership,
    ) -> Self {
        let map_id = format!("standard-{}x{}", state.board.size, state.board.size);
        Self::new_with_players_and_deck_names(
            state,
            time_control,
            multiplayer,
            now_ms,
            players,
            ownership,
            HashMap::new(),
            map_id,
        )
    }

    pub(crate) fn new_with_players_and_deck_names(
        state: GameState,
        time_control: TimeControlId,
        multiplayer: bool,
        now_ms: i64,
        players: HashMap<PlayerId, GameRecordPlayer>,
        ownership: GameRecordOwnership,
        deck_names: HashMap<PlayerId, String>,
        map_id: String,
    ) -> Self {
        let presence = multiplayer.then(|| PresenceState {
            last_seen_ms: HashMap::from([("white".into(), now_ms), ("black".into(), now_ms)]),
        });
        let clock = ClockState::new(time_control, now_ms);
        let record = GameRecord::new_with_deck_names(
            state.clone(),
            players,
            time_control,
            now_ms,
            clock.snapshot(now_ms, true),
            ownership,
            deck_names,
            map_id,
        );
        Self {
            state,
            clock,
            presence,
            record,
            record_persisted: false,
            challenge: None,
        }
    }

    pub(crate) fn set_challenge(&mut self, context: ChallengeGameContext) {
        self.record.game_mode = GameMode::Challenge;
        self.record.challenge_id = Some(context.metadata.id.clone());
        self.challenge = Some(context);
    }

    pub(crate) fn heartbeat(&mut self, player: &str, now_ms: i64) {
        if let Some(presence) = &mut self.presence {
            presence.last_seen_ms.insert(player.into(), now_ms);
        }
    }

    pub(crate) fn adjudicate(&mut self, now_ms: i64) {
        if self.state.phase == GamePhase::Ended {
            return;
        }
        if let Some(loser) = self.clock.timeout_loser(now_ms) {
            self.clock.stop(now_ms);
            self.end_with_loss(&loser, GameEndReason::Timeout);
            return;
        }
        if let Some(presence) = &self.presence {
            let loser = abandonment_loser(presence, now_ms);
            if let Some(loser) = loser {
                self.clock.stop(now_ms);
                self.end_with_loss(loser, GameEndReason::Abandonment);
            }
        }
    }

    pub(crate) fn view(&self, now_ms: i64) -> GameView {
        let running = self.state.phase != GamePhase::Ended;
        GameView {
            state: self.state.clone(),
            clock: self.clock.snapshot(now_ms, running),
            presence: self.presence.as_ref().map(|presence| PresenceSnapshot {
                white: presence_for(presence, "white", now_ms),
                black: presence_for(presence, "black", now_ms),
            }),
            player_info: self.record.players.clone(),
            record_notation: self
                .record
                .actions
                .iter()
                .map(|entry| entry.notation.clone())
                .collect(),
            challenge: self
                .challenge
                .as_ref()
                .map(|context| context.metadata.clone()),
        }
    }

    pub(crate) fn end_with_loss(&mut self, loser: &str, reason: GameEndReason) {
        if self.state.phase == GamePhase::Ended {
            return;
        }
        self.state.phase = GamePhase::Ended;
        self.state.result = Some(GameResult {
            winner: Some(opponent(loser)),
            reason,
        });
        let ended_at = now_ms();
        self.record
            .finalize(&self.state, self.clock.snapshot(ended_at, false), ended_at);
    }

    pub(crate) fn completed_record(&self) -> Option<GameRecord> {
        if self.record_persisted || self.record.ended_at_ms.is_none() {
            return None;
        }
        Some(self.record.clone())
    }

    pub(crate) fn mark_record_persisted(&mut self) {
        if self.record.ended_at_ms.is_some() {
            self.record_persisted = true;
        }
    }
}

#[cfg(test)]
fn default_record_players() -> HashMap<PlayerId, GameRecordPlayer> {
    HashMap::from([
        (
            "white".into(),
            GameRecordPlayer {
                public_id: Some("white".into()),
                nickname: "White".into(),
                side: "white".into(),
            },
        ),
        (
            "black".into(),
            GameRecordPlayer {
                public_id: Some("black".into()),
                nickname: "Black".into(),
                side: "black".into(),
            },
        ),
    ])
}

fn abandonment_loser(presence: &PresenceState, now_ms: i64) -> Option<&'static str> {
    ["white", "black"].into_iter().find(|player| {
        presence.last_seen_ms.get(*player).is_some_and(|last_seen| {
            now_ms >= *last_seen + HEARTBEAT_GRACE_MS + ABANDONMENT_FORFEIT_MS
        })
    })
}

fn presence_for(presence: &PresenceState, player: &str, now_ms: i64) -> PlayerPresenceSnapshot {
    let disconnected_at =
        presence.last_seen_ms.get(player).copied().unwrap_or(now_ms) + HEARTBEAT_GRACE_MS;
    let connected = now_ms < disconnected_at;
    PlayerPresenceSnapshot {
        connected,
        disconnected_at_ms: (!connected).then_some(disconnected_at),
        warning_at_ms: (!connected).then_some(disconnected_at + ABANDONMENT_WARNING_MS),
        forfeit_at_ms: (!connected).then_some(disconnected_at + ABANDONMENT_FORFEIT_MS),
    }
}

fn opponent(player: &str) -> PlayerId {
    if player == "white" {
        "black".into()
    } else {
        "white".into()
    }
}

impl Deref for StoredGame {
    type Target = GameState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl DerefMut for StoredGame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use brainfuck_chess_engine::types::{Board, ChessemblyProgramCache};

    fn empty_game() -> GameState {
        GameState {
            id: "clock-test".into(),
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
        }
    }

    #[test]
    fn supported_definitions_are_exact() {
        assert_eq!(
            TimeControlId::FiveZero.definition().initial_time_ms,
            Some(300_000)
        );
        assert_eq!(
            TimeControlId::TenZero.definition().initial_time_ms,
            Some(600_000)
        );
        assert_eq!(TimeControlId::FiveThree.definition().increment_ms, 3_000);
        assert_eq!(TimeControlId::TenFive.definition().increment_ms, 5_000);
        assert_eq!(TimeControlId::FifteenTen.definition().increment_ms, 10_000);
        assert_eq!(
            TimeControlId::Unlimited.definition().mode,
            TimeControlMode::Unlimited
        );
    }

    #[test]
    fn countdown_debits_only_active_player_and_adds_increment() {
        let mut clock = ClockState::new(TimeControlId::FiveThree, 1_000);
        let running = clock.snapshot(9_000, true);
        assert_eq!(running.white_remaining_ms, Some(292_000));
        assert_eq!(running.black_remaining_ms, Some(300_000));
        clock.finish_turn("white", "black", 9_000, false);
        let switched = clock.snapshot(11_000, true);
        assert_eq!(switched.white_remaining_ms, Some(295_000));
        assert_eq!(switched.black_remaining_ms, Some(298_000));
    }

    #[test]
    fn every_fischer_increment_is_applied_exactly_once() {
        for (control, expected) in [
            (TimeControlId::FiveThree, 303_000),
            (TimeControlId::TenFive, 605_000),
            (TimeControlId::FifteenTen, 910_000),
        ] {
            let mut clock = ClockState::new(control, 100);
            clock.finish_turn("white", "black", 100, false);
            assert_eq!(clock.snapshot(100, true).white_remaining_ms, Some(expected));
        }
    }

    #[test]
    fn same_player_follow_up_action_debits_time_but_defers_increment_until_turn_switch() {
        let mut clock = ClockState::new(TimeControlId::FiveThree, 0);

        clock.finish_turn("white", "white", 2_000, false);
        let pending_landing = clock.snapshot(2_000, true);
        assert_eq!(pending_landing.white_remaining_ms, Some(298_000));
        assert_eq!(pending_landing.active_color, "white");

        clock.finish_turn("white", "black", 5_000, false);
        let completed_turn = clock.snapshot(5_000, true);
        assert_eq!(completed_turn.white_remaining_ms, Some(298_000));
        assert_eq!(completed_turn.active_color, "black");
    }

    #[test]
    fn rejected_action_path_cannot_grant_increment() {
        let clock = ClockState::new(TimeControlId::FiveThree, 0);
        assert_eq!(
            clock.snapshot(2_000, true).white_remaining_ms,
            Some(298_000)
        );
    }

    #[test]
    fn unlimited_accumulates_active_elapsed_time_without_timeout() {
        let mut clock = ClockState::new(TimeControlId::Unlimited, 0);
        assert_eq!(clock.snapshot(13_000, true).white_elapsed_ms, 13_000);
        assert_eq!(clock.timeout_loser(999_999_999), None);
        clock.finish_turn("white", "black", 13_000, false);
        let snapshot = clock.snapshot(21_000, true);
        assert_eq!(snapshot.white_elapsed_ms, 13_000);
        assert_eq!(snapshot.black_elapsed_ms, 8_000);
    }

    #[test]
    fn abandonment_warns_after_twenty_seconds_and_forfeits_at_total_two_minutes() {
        let presence = PresenceState {
            last_seen_ms: HashMap::from([("white".into(), 0), ("black".into(), 119_000)]),
        };
        let before_warning = HEARTBEAT_GRACE_MS + ABANDONMENT_WARNING_MS - 1;
        let white = presence_for(&presence, "white", before_warning);
        assert!(!white.connected);
        assert!(white
            .warning_at_ms
            .is_some_and(|warning| warning > before_warning));
        assert_eq!(
            abandonment_loser(&presence, HEARTBEAT_GRACE_MS + ABANDONMENT_FORFEIT_MS - 1),
            None
        );
        assert_eq!(
            abandonment_loser(&presence, HEARTBEAT_GRACE_MS + ABANDONMENT_FORFEIT_MS),
            Some("white")
        );
    }

    #[test]
    fn heartbeat_before_deadline_clears_abandonment() {
        let mut presence = PresenceState {
            last_seen_ms: HashMap::from([("white".into(), 0), ("black".into(), 0)]),
        };
        let reconnect_at = HEARTBEAT_GRACE_MS + ABANDONMENT_FORFEIT_MS - 1_000;
        presence.last_seen_ms.insert("white".into(), reconnect_at);
        assert!(presence_for(&presence, "white", reconnect_at).connected);
        assert_eq!(abandonment_loser(&presence, reconnect_at), None);
    }

    #[test]
    fn timeout_wins_race_with_abandonment_and_end_is_idempotent() {
        let mut game = StoredGame::new(empty_game(), TimeControlId::FiveZero, true, -270_000);
        game.heartbeat("white", 0);
        game.heartbeat("black", 0);
        game.adjudicate(30_000);
        assert_eq!(game.state.phase, GamePhase::Ended);
        assert_eq!(
            game.state.result.as_ref().map(|result| &result.reason),
            Some(&GameEndReason::Timeout)
        );
        game.adjudicate(1_000_000);
        assert_eq!(
            game.state.result.as_ref().map(|result| &result.reason),
            Some(&GameEndReason::Timeout)
        );
    }
}
