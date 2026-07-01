use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::chessembly::ast::Program;
use crate::chessembly::parser::parse;

// ─── Primitive ID types ─────────────────────────────────────────────────────

pub type PlayerId = String;
pub type PieceTypeId = String;

/// Stable external piece id with allocation-free clones inside the engine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(Arc<str>);

impl PieceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PieceId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for PieceId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl Borrow<str> for PieceId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PieceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl PartialEq<str> for PieceId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for PieceId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for PieceId {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

/// Compact, allocation-free square key used by engine maps and sets.
///
/// Its serde representation remains `"file_rank"` so existing board and API
/// JSON stays compatible while the engine no longer allocates square strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SquareId {
    pub file: i32,
    pub rank: i32,
}

impl SquareId {
    pub const fn new(file: i32, rank: i32) -> Self {
        Self { file, rank }
    }

    pub const fn to_square(self) -> Square {
        Square::new(self.file, self.rank)
    }
}

impl fmt::Display for SquareId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}_{}", self.file, self.rank)
    }
}

impl Serialize for SquareId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SquareId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let (file, rank) = value
            .split_once('_')
            .ok_or_else(|| de::Error::custom("square id must be `file_rank`"))?;
        Ok(Self::new(
            file.parse().map_err(de::Error::custom)?,
            rank.parse().map_err(de::Error::custom)?,
        ))
    }
}

// ─── Square ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Square {
    pub file: i32,
    pub rank: i32,
}

impl Square {
    pub const fn new(file: i32, rank: i32) -> Self {
        Self { file, rank }
    }

    pub const fn to_id(&self) -> SquareId {
        SquareId::new(self.file, self.rank)
    }
}

// ─── Board ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub size: i32,
    /// Maps SquareId → PieceId (None means empty)
    pub squares: HashMap<SquareId, Option<PieceId>>,
}

impl Board {
    pub fn is_in_bounds(&self, sq: &Square) -> bool {
        sq.file >= 0 && sq.file < self.size && sq.rank >= 0 && sq.rank < self.size
    }

    pub fn get_piece_at(&self, sq: &Square) -> Option<&PieceId> {
        self.squares.get(&sq.to_id())?.as_ref()
    }

    pub fn is_empty(&self, sq: &Square) -> bool {
        self.get_piece_at(sq).is_none()
    }
}

// ─── PieceDefinition ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceDefinition {
    pub id: PieceTypeId,
    pub name: String,
    /// Point cost for deck building (King is excluded from scoring)
    pub score: u32,
    pub chessembly_code: String,
    pub chessembly_version: String,
    pub dialect: Option<ChessemblyDialect>,
    pub extensions: Option<Vec<String>>,
    /// If true, capturing this piece ends the game immediately
    pub is_king: bool,
    /// Optional rule that decides when this piece may promote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<PromotionRule>,
    /// Piece types this piece may promote into when its promotion rule matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotion_pool: Vec<PieceTypeId>,
    /// Optional activated Chessembly move/attack programs for this piece type.
    #[serde(default)]
    pub abilities: Vec<PieceAbilityDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRule {
    pub condition: PromotionCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromotionCondition {
    FirstRank,
    LastRank,
    Rank { rank: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceAbilityDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub chessembly_code: String,
    pub duration: AbilityDuration,
    #[serde(default)]
    pub once_per_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityDuration {
    UntilTurnEnd,
    Turns(u32),
    UntilPieceMoves,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveAbilityState {
    pub ability_id: String,
    pub activated_turn_number: u32,
    pub activated_player: PlayerId,
    pub duration: AbilityDuration,
}

/// Move generation dispatch seam. Native implementations can be enabled per
/// definition without changing callers; custom pieces remain Chessembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovegenBackend {
    Native,
    Chessembly,
}

impl PieceDefinition {
    pub fn movegen_backend(&self) -> MovegenBackend {
        // The first optimization pass keeps behavior identical. Native
        // backends can be introduced piece-by-piece with parity tests.
        MovegenBackend::Chessembly
    }

    pub fn promotion_options_for_rank(&self, rank: i32, board_size: i32) -> Option<&[PieceTypeId]> {
        let rule = self.promotion.as_ref()?;
        if self.promotion_pool.is_empty() || !rule.condition.matches_rank(rank, board_size) {
            return None;
        }
        Some(self.promotion_pool.as_slice())
    }
}

impl PromotionCondition {
    pub fn matches_rank(&self, rank: i32, board_size: i32) -> bool {
        match self {
            PromotionCondition::FirstRank => rank == 0,
            PromotionCondition::LastRank => rank == board_size - 1,
            PromotionCondition::Rank { rank: target_rank } => rank == *target_rank,
        }
    }
}

// ─── Chessembly Program Cache ───────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ChessemblyProgramCache {
    pub programs: RwLock<HashMap<String, Arc<Program>>>,
}

impl Clone for ChessemblyProgramCache {
    fn clone(&self) -> Self {
        Self {
            programs: RwLock::new(self.read_programs().clone()),
        }
    }
}

impl ChessemblyProgramCache {
    pub fn from_definitions(definitions: &HashMap<PieceTypeId, PieceDefinition>) -> Self {
        let cache = Self::default();
        cache.rebuild(definitions);
        cache
    }

    pub fn rebuild(&self, definitions: &HashMap<PieceTypeId, PieceDefinition>) {
        crate::profiling::record_cache_rebuild(1);
        let mut programs = HashMap::new();
        for (type_id, definition) in definitions {
            programs.insert(
                type_id.clone(),
                Arc::new(parse(&definition.chessembly_code)),
            );
            for ability in &definition.abilities {
                programs.insert(
                    Self::ability_key(type_id, &ability.id),
                    Arc::new(parse(&ability.chessembly_code)),
                );
            }
        }
        *self.write_programs() = programs;
    }

    pub fn is_complete_for(&self, definitions: &HashMap<PieceTypeId, PieceDefinition>) -> bool {
        let programs = self.read_programs();
        let expected_len: usize = definitions
            .values()
            .map(|definition| 1 + definition.abilities.len())
            .sum();
        programs.len() == expected_len
            && definitions.iter().all(|(type_id, definition)| {
                programs.contains_key(type_id)
                    && definition.abilities.iter().all(|ability| {
                        programs.contains_key(&Self::ability_key(type_id, &ability.id))
                    })
            })
    }

    pub fn get(&self, type_id: &PieceTypeId) -> Option<Arc<Program>> {
        self.read_programs().get(type_id).cloned()
    }

    pub fn get_ability(&self, type_id: &PieceTypeId, ability_id: &str) -> Option<Arc<Program>> {
        self.read_programs()
            .get(&Self::ability_key(type_id, ability_id))
            .cloned()
    }

    pub fn get_or_parse(
        &self,
        type_id: &PieceTypeId,
        definition: &PieceDefinition,
    ) -> Arc<Program> {
        if let Some(program) = self.get(type_id) {
            return program;
        }

        let program = Arc::new(parse(&definition.chessembly_code));
        let mut programs = self.write_programs();
        programs
            .entry(type_id.clone())
            .or_insert_with(|| program.clone())
            .clone()
    }

    pub fn get_or_parse_ability(
        &self,
        type_id: &PieceTypeId,
        ability: &PieceAbilityDefinition,
    ) -> Arc<Program> {
        if let Some(program) = self.get_ability(type_id, &ability.id) {
            return program;
        }

        let key = Self::ability_key(type_id, &ability.id);
        let program = Arc::new(parse(&ability.chessembly_code));
        let mut programs = self.write_programs();
        programs
            .entry(key)
            .or_insert_with(|| program.clone())
            .clone()
    }

    pub fn len(&self) -> usize {
        self.read_programs().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read_programs().is_empty()
    }

    pub fn ability_key(type_id: &PieceTypeId, ability_id: &str) -> String {
        format!("{type_id}::{ability_id}")
    }

    fn read_programs(&self) -> RwLockReadGuard<'_, HashMap<String, Arc<Program>>> {
        self.programs
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_programs(&self) -> RwLockWriteGuard<'_, HashMap<String, Arc<Program>>> {
        self.programs
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChessemblyDialect {
    Classic,
    BrainfuckChess,
}

// ─── Piece ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub owner: PlayerId,
    pub type_id: PieceTypeId,
    /// None when in pocket or captured
    pub current_square: Option<Square>,
    pub in_pocket: bool,
    pub captured: bool,
    /// Whether this piece has ever moved (used for Pawn 2-step rule)
    pub has_moved: bool,
    /// Currently active ability program, if any.
    #[serde(default)]
    pub active_ability: Option<ActiveAbilityState>,
}

impl Piece {
    pub fn is_on_board(&self) -> bool {
        self.current_square.is_some() && !self.in_pocket && !self.captured
    }
}

// ─── Deck ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub player_id: PlayerId,
    /// Pieces placed on the board at game start
    pub starting_pieces: Vec<PieceId>,
    /// Pieces held in pocket, deployable during drop turns
    pub pocket_pieces: Vec<PieceId>,
    pub score_limit: u32,
    pub total_score: u32,
}

// ─── Player ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub deck: Deck,
    pub captured_pieces: Vec<PieceId>,
}

// ─── Turn ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnMode {
    /// Player hasn't chosen a mode yet this turn
    Undecided,
    /// Moving pieces
    Move,
    /// Dropping a pocket piece onto the board
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnState {
    pub mode: TurnMode,
    pub actions: Vec<TurnAction>,
}

impl TurnState {
    pub fn new() -> Self {
        Self {
            mode: TurnMode::Undecided,
            actions: Vec::new(),
        }
    }
}

impl Default for TurnState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Actions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TurnAction {
    Move(MoveAction),
    Drop(DropAction),
    ActivateAbility(ActivateAbilityAction),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub from: Square,
    pub to: Square,
    pub captured_piece_id: Option<PieceId>,
    /// Piece type to promote to when the moving piece's definition allows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<PieceTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub to: Square,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivateAbilityAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub ability_id: String,
}

/// Search-oriented drop candidate. Identical pocket pieces are represented by
/// their type and count instead of one action per concrete piece id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropCandidateByType {
    pub player_id: PlayerId,
    pub piece_type_id: PieceTypeId,
    pub count: u16,
    pub to: SquareId,
}

// ─── GameResult ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub winner: Option<PlayerId>,
    pub reason: GameEndReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameEndReason {
    KingCapture,
    Resignation,
    Timeout,
    Draw,
}

// ─── GamePhase ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    Setup,
    Playing,
    Ended,
}

// ─── GameState ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub id: String,
    pub board: Board,
    /// All piece instances, keyed by PieceId
    pub pieces: HashMap<PieceId, Piece>,
    /// All piece definitions, keyed by PieceTypeId
    pub piece_definitions: HashMap<PieceTypeId, PieceDefinition>,
    pub players: HashMap<PlayerId, Player>,
    pub current_player: PlayerId,
    pub turn_number: u32,
    pub phase: GamePhase,
    /// En passant target square (the passed-over square after a 2-step pawn move).
    pub en_passant_target: Option<Square>,
    /// Player allowed to capture via en passant on this turn.
    pub en_passant_available_to: Option<PlayerId>,
    pub turn_state: TurnState,
    pub result: Option<GameResult>,
    #[serde(skip, default)]
    pub chessembly_program_cache: ChessemblyProgramCache,
}

impl GameState {
    pub fn rebuild_chessembly_cache(&self) {
        self.chessembly_program_cache
            .rebuild(&self.piece_definitions);
    }

    pub fn ensure_chessembly_cache(&self) {
        if !self
            .chessembly_program_cache
            .is_complete_for(&self.piece_definitions)
        {
            self.rebuild_chessembly_cache();
        }
    }

    pub fn chessembly_program(&self, type_id: &PieceTypeId) -> Option<Arc<Program>> {
        if let Some(program) = self.chessembly_program_cache.get(type_id) {
            crate::profiling::record_cache_hit(1);
            return Some(program);
        }

        let definition = self.piece_definitions.get(type_id)?;
        Some(
            self.chessembly_program_cache
                .get_or_parse(type_id, definition),
        )
    }

    pub fn effective_chessembly_program(
        &self,
        piece: &Piece,
        definition: &PieceDefinition,
    ) -> Option<Arc<Program>> {
        if let Some(active) = &piece.active_ability {
            if let Some(ability) = definition
                .abilities
                .iter()
                .find(|ability| ability.id == active.ability_id)
            {
                if let Some(program) = self
                    .chessembly_program_cache
                    .get_ability(&definition.id, &ability.id)
                {
                    crate::profiling::record_cache_hit(1);
                    return Some(program);
                }
                return Some(
                    self.chessembly_program_cache
                        .get_or_parse_ability(&definition.id, ability),
                );
            }
        }

        self.chessembly_program(&piece.type_id)
    }

    pub fn cached_chessembly_program_count(&self) -> usize {
        self.chessembly_program_cache.len()
    }
}

// ─── Validation ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn fail(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }
}

// ─── AttackMap ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackMap {
    pub player_id: PlayerId,
    pub attacked_squares: HashSet<SquareId>,
    /// Which pieces attack each square
    pub source_map: HashMap<SquareId, Vec<PieceId>>,
}

// ─── ChessemblyResult ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChessemblyResult {
    /// Squares the piece can move to (empty squares only for move-only pieces)
    pub movement_squares: Vec<Square>,
    /// Squares the piece threatens/attacks
    pub attack_squares: Vec<Square>,
}
