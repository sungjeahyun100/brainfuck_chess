use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

// ─── Primitive ID types ─────────────────────────────────────────────────────

pub type PlayerId = String;
pub type SquareId = String;
pub type PieceId = String;
pub type PieceTypeId = String;

// ─── Square ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Square {
    pub file: i32,
    pub rank: i32,
}

impl Square {
    pub fn new(file: i32, rank: i32) -> Self {
        Self { file, rank }
    }

    pub fn to_id(&self) -> SquareId {
        format!("{}_{}", self.file, self.rank)
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
    /// Remaining move stack for this turn (reset to 1 at turn start)
    pub move_stack: u32,
    /// Whether this piece has ever moved (used for Pawn 2-step rule)
    pub has_moved: bool,
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
    /// Piece IDs that have already moved this turn
    pub moved_piece_ids: HashSet<PieceId>,
}

impl TurnState {
    pub fn new() -> Self {
        Self {
            mode: TurnMode::Undecided,
            actions: Vec::new(),
            moved_piece_ids: HashSet::new(),
        }
    }
}

// ─── Actions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TurnAction {
    Move(MoveAction),
    Drop(DropAction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub from: Square,
    pub to: Square,
    pub captured_piece_id: Option<PieceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub to: Square,
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
}

// ─── Validation ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self { valid: true, errors: Vec::new() }
    }

    pub fn fail(errors: Vec<String>) -> Self {
        Self { valid: false, errors }
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
