use std::collections::HashMap;

use crate::ai::types::AiAction;
use crate::types::{
    GameEndReason, GamePhase, GameState, PieceId, PieceLayer, PieceStateValue, Square,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PositionKey {
    board_size: i32,
    board: Vec<(i32, i32, Option<PieceId>)>,
    air_board: Vec<(i32, i32, Option<PieceId>)>,
    terrain: Vec<(i32, i32, String)>,
    current_player: String,
    phase: u8,
    result: Option<(Option<String>, u8)>,
    pieces: Vec<PieceKey>,
    players: Vec<PlayerKey>,
    en_passant_target: Option<Square>,
    en_passant_available_to: Option<String>,
    global_state: Vec<(String, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PieceKey {
    id: PieceId,
    owner: String,
    type_id: String,
    current_square: Option<Square>,
    in_pocket: bool,
    captured: bool,
    has_moved: bool,
    current_ammo: u32,
    layer: PieceLayer,
    remaining_flight_turns: u32,
    state: Vec<(String, PieceStateValue)>,
    cooldowns: Vec<(String, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PlayerKey {
    id: String,
    deck_player_id: String,
    starting_pieces: Vec<PieceId>,
    pocket_pieces: Vec<PieceId>,
    score_limit: u32,
    total_score: u32,
    captured_pieces: Vec<PieceId>,
}

impl PositionKey {
    pub(crate) fn from_state(state: &GameState) -> Self {
        crate::profiling::record_position_key_generation(1);
        let mut board = state
            .board
            .squares
            .iter()
            .filter_map(|(square, piece_id)| {
                piece_id
                    .as_ref()
                    .map(|piece_id| (square.file, square.rank, Some(piece_id.clone())))
            })
            .collect::<Vec<_>>();
        board.sort();

        let mut air_board = state
            .board
            .air_squares
            .iter()
            .filter_map(|(square, piece_id)| {
                piece_id
                    .as_ref()
                    .map(|piece_id| (square.file, square.rank, Some(piece_id.clone())))
            })
            .collect::<Vec<_>>();
        air_board.sort();

        let mut terrain = state
            .board
            .terrain
            .iter()
            .map(|(square, cell)| (square.file, square.rank, cell.type_id.clone()))
            .collect::<Vec<_>>();
        terrain.sort();

        let mut pieces = state
            .pieces
            .values()
            .map(|piece| {
                let mut piece_state = piece
                    .state
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<Vec<_>>();
                piece_state.sort_by(|left, right| left.0.cmp(&right.0));
                let mut cooldowns = piece
                    .move_option_cooldowns
                    .iter()
                    .map(|(option_id, cooldown)| (option_id.clone(), cooldown.remaining))
                    .collect::<Vec<_>>();
                cooldowns.sort_by(|left, right| left.0.cmp(&right.0));
                PieceKey {
                    id: piece.id.clone(),
                    owner: piece.owner.clone(),
                    type_id: piece.type_id.clone(),
                    current_square: piece.current_square,
                    in_pocket: piece.in_pocket,
                    captured: piece.captured,
                    has_moved: piece.has_moved,
                    current_ammo: piece.current_ammo,
                    layer: piece.layer,
                    remaining_flight_turns: piece.remaining_flight_turns,
                    state: piece_state,
                    cooldowns,
                }
            })
            .collect::<Vec<_>>();
        pieces.sort_by(|left, right| left.id.cmp(&right.id));

        let mut players = state
            .players
            .values()
            .map(|player| {
                let mut starting_pieces = player.deck.starting_pieces.clone();
                starting_pieces.sort();
                let mut pocket_pieces = player.deck.pocket_pieces.clone();
                pocket_pieces.sort();
                let mut captured_pieces = player.captured_pieces.clone();
                captured_pieces.sort();
                PlayerKey {
                    id: player.id.clone(),
                    deck_player_id: player.deck.player_id.clone(),
                    starting_pieces,
                    pocket_pieces,
                    score_limit: player.deck.score_limit,
                    total_score: player.deck.total_score,
                    captured_pieces,
                }
            })
            .collect::<Vec<_>>();
        players.sort_by(|left, right| left.id.cmp(&right.id));

        let mut global_state = state
            .global_state
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect::<Vec<_>>();
        global_state.sort_by(|left, right| left.0.cmp(&right.0));

        Self {
            board_size: state.board.size,
            board,
            air_board,
            terrain,
            current_player: state.current_player.clone(),
            phase: phase_tag(&state.phase),
            result: state
                .result
                .as_ref()
                .map(|result| (result.winner.clone(), result_tag(&result.reason))),
            pieces,
            players,
            en_passant_target: state.en_passant_target,
            en_passant_available_to: state.en_passant_available_to.clone(),
            global_state,
        }
    }
}

const fn phase_tag(phase: &GamePhase) -> u8 {
    match phase {
        GamePhase::Setup => 0,
        GamePhase::Playing => 1,
        GamePhase::Ended => 2,
    }
}

const fn result_tag(reason: &GameEndReason) -> u8 {
    match reason {
        GameEndReason::KingCapture => 0,
        GameEndReason::Resignation => 1,
        GameEndReason::Timeout => 2,
        GameEndReason::Draw => 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranspositionEntry {
    pub(crate) depth: u8,
    pub(crate) score: i32,
    pub(crate) bound: BoundType,
    pub(crate) best_action: Option<AiAction>,
}

pub(crate) struct TranspositionTable {
    entries: HashMap<PositionKey, TranspositionEntry>,
    max_entries: usize,
}

impl TranspositionTable {
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    pub(crate) fn get(&self, key: &PositionKey) -> Option<&TranspositionEntry> {
        self.entries.get(key)
    }

    pub(crate) fn store(&mut self, key: PositionKey, entry: TranspositionEntry) -> bool {
        if let Some(existing) = self.entries.get_mut(&key) {
            if entry.depth >= existing.depth {
                *existing = entry;
                return true;
            }
            return false;
        }
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.insert(key, entry);
        true
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::rules::create_board;
    use crate::types::{
        ActionRecord, ChessemblyProgramCache, CooldownState, Deck, DropAction, GameResult,
        GameState, Piece, Player, Square, TurnAction,
    };

    fn state() -> GameState {
        let mut players = HashMap::new();
        for id in ["white", "black"] {
            players.insert(
                id.into(),
                Player {
                    id: id.into(),
                    deck: Deck {
                        player_id: id.into(),
                        starting_pieces: Vec::new(),
                        pocket_pieces: Vec::new(),
                        score_limit: 39,
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            );
        }
        let mut state = GameState {
            id: "position-key-test".into(),
            board: create_board(8),
            pieces: HashMap::new(),
            piece_definitions: HashMap::new(),
            custom_piece_manifest: Vec::new(),
            players,
            current_player: "white".into(),
            turn_number: 7,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: Vec::new(),
            result: None,
            chessembly_program_cache: ChessemblyProgramCache::default(),
        };
        add_piece(
            &mut state,
            "windmill",
            "white",
            "windmill",
            Some(Square::new(3, 3)),
        );
        add_piece(&mut state, "reserve", "white", "knight", None);
        add_piece(&mut state, "reserve-2", "white", "bishop", None);
        state
    }

    fn add_piece(
        state: &mut GameState,
        id: &str,
        owner: &str,
        type_id: &str,
        square: Option<Square>,
    ) {
        let piece_id: PieceId = id.into();
        let in_pocket = square.is_none();
        if let Some(square) = square {
            state
                .board
                .squares
                .insert(square.to_id(), Some(piece_id.clone()));
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .starting_pieces
                .push(piece_id.clone());
        } else {
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .pocket_pieces
                .push(piece_id.clone());
        }
        state.pieces.insert(
            piece_id.clone(),
            Piece {
                id: piece_id,
                owner: owner.into(),
                type_id: type_id.into(),
                current_square: square,
                in_pocket,
                captured: false,
                has_moved: false,
                current_ammo: 0,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
            },
        );
    }

    #[test]
    fn semantic_position_key_is_independent_of_map_and_membership_order() {
        let first = state();
        let mut second = first.clone();
        let mut pieces = second.pieces.into_iter().collect::<Vec<_>>();
        pieces.reverse();
        second.pieces = pieces.into_iter().collect();
        let mut players = second.players.into_iter().collect::<Vec<_>>();
        players.reverse();
        second.players = players.into_iter().collect();
        let mut squares = second.board.squares.into_iter().collect::<Vec<_>>();
        squares.reverse();
        second.board.squares = squares.into_iter().collect();
        second
            .board
            .squares
            .retain(|_, piece_id| piece_id.is_some());
        second.global_state.insert("z".into(), 2);
        second.global_state.insert("a".into(), 1);
        let mut first = first;
        first.global_state.insert("a".into(), 1);
        first.global_state.insert("z".into(), 2);
        first
            .players
            .get_mut("white")
            .unwrap()
            .deck
            .pocket_pieces
            .reverse();
        second
            .players
            .get_mut("white")
            .unwrap()
            .deck
            .pocket_pieces
            .sort();
        assert_eq!(
            PositionKey::from_state(&first),
            PositionKey::from_state(&second)
        );
    }

    #[test]
    fn rule_relevant_state_changes_position_key() {
        let base = state();
        let base_key = PositionKey::from_state(&base);
        let assert_changed = |changed: GameState| {
            assert_ne!(base_key, PositionKey::from_state(&changed));
        };

        let mut changed = base.clone();
        changed.current_player = "black".into();
        assert_changed(changed);

        let mut changed = base.clone();
        changed.phase = GamePhase::Ended;
        changed.result = Some(GameResult {
            winner: None,
            reason: GameEndReason::Draw,
        });
        assert_changed(changed);

        let mut changed = base.clone();
        changed.pieces.get_mut("reserve").unwrap().type_id = "bishop".into();
        assert_changed(changed);

        let mut changed = base.clone();
        changed.pieces.get_mut("windmill").unwrap().has_moved = true;
        assert_changed(changed);

        let mut changed = base.clone();
        changed.pieces.get_mut("reserve").unwrap().captured = true;
        assert_changed(changed);

        let mut changed = base.clone();
        changed
            .pieces
            .get_mut("windmill")
            .unwrap()
            .state
            .insert("mode".into(), PieceStateValue::Text("rook".into()));
        assert_changed(changed);

        let mut changed = base.clone();
        changed
            .pieces
            .get_mut("windmill")
            .unwrap()
            .move_option_cooldowns
            .insert("special".into(), CooldownState { remaining: 2 });
        assert_changed(changed);

        let mut changed = base.clone();
        changed.en_passant_target = Some(Square::new(4, 5));
        changed.en_passant_available_to = Some("white".into());
        assert_changed(changed);

        let mut changed = base.clone();
        changed.global_state.insert("weather".into(), 1);
        assert_changed(changed);

        let mut changed = base.clone();
        changed.board.terrain.insert(
            Square::new(3, 3).to_id(),
            crate::types::TerrainCell {
                type_id: crate::rules::HIGH_GROUND_TERRAIN_ID.into(),
            },
        );
        assert_changed(changed);

        let mut changed = base.clone();
        changed.pieces.get_mut("windmill").unwrap().current_square = Some(Square::new(4, 3));
        changed
            .board
            .squares
            .insert(Square::new(3, 3).to_id(), None);
        changed
            .board
            .squares
            .insert(Square::new(4, 3).to_id(), Some(PieceId::from("windmill")));
        assert_changed(changed);
    }

    #[test]
    fn pocket_membership_changes_position_key_even_with_the_same_board() {
        let first = state();
        let mut second = first.clone();
        second
            .players
            .get_mut("white")
            .unwrap()
            .deck
            .pocket_pieces
            .clear();
        assert_ne!(
            PositionKey::from_state(&first),
            PositionKey::from_state(&second)
        );
    }

    #[test]
    fn currently_unobservable_history_and_turn_number_are_excluded() {
        let first = state();
        let mut second = first.clone();
        second.turn_number += 20;
        second.history.push(ActionRecord {
            turn_number: 1,
            player_id: "white".into(),
            action: TurnAction::Drop(DropAction {
                player_id: "white".into(),
                piece_id: "reserve".into(),
                to: Square::new(0, 0),
                captured_piece_id: None,
            }),
        });
        assert_eq!(
            PositionKey::from_state(&first),
            PositionKey::from_state(&second)
        );
    }

    #[test]
    fn table_is_bounded_and_prefers_deeper_replacements() {
        let key = PositionKey::from_state(&state());
        let mut table = TranspositionTable::new(1);
        assert!(table.store(
            key.clone(),
            TranspositionEntry {
                depth: 2,
                score: 10,
                bound: BoundType::Exact,
                best_action: None,
            }
        ));
        assert!(!table.store(
            key.clone(),
            TranspositionEntry {
                depth: 1,
                score: 20,
                bound: BoundType::Exact,
                best_action: None,
            }
        ));
        assert_eq!(table.get(&key).unwrap().score, 10);
        let mut other = state();
        other.current_player = "black".into();
        assert!(!table.store(
            PositionKey::from_state(&other),
            TranspositionEntry {
                depth: 3,
                score: 30,
                bound: BoundType::Exact,
                best_action: None,
            }
        ));
        assert_eq!(table.len(), 1);
    }
}
