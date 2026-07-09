use brainfuck_chess_engine::{
    ai::AiAction,
    types::{DropAction, GameState, MoveAction, PlayerId, Square, TurnAction},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateGameRequest {
    pub board_size: i32,
    pub white_deck: PlayerDeckSpec,
    pub black_deck: PlayerDeckSpec,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PlayerDeckSpec {
    pub starting: Vec<StartingPieceSpec>,
    pub pocket: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StartingPieceSpec {
    pub piece_type: String,
    pub square: Square,
}

#[derive(Serialize)]
pub struct GameResponse {
    pub id: String,
    pub state: GameState,
}

#[derive(Deserialize)]
pub struct SubmitActionRequest {
    pub action: TurnAction,
}

#[derive(Deserialize)]
pub struct BotTurnRequest {
    pub bot_player_id: PlayerId,
    #[serde(default)]
    pub difficulty: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BotTurnStats {
    pub searched_nodes: u64,
    pub depth_reached: u8,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct BotTurnResponse {
    pub ok: bool,
    pub game_state: GameState,
    pub actions: Vec<AiAction>,
    pub stats: BotTurnStats,
}

#[derive(Deserialize)]
pub struct ResignGameRequest {
    pub player_id: PlayerId,
}

#[derive(Default, Deserialize)]
pub struct PieceOptionsQuery {
    pub ability_id: Option<String>,
}

#[derive(Serialize)]
pub struct PieceOptionsResponse {
    pub moves: Vec<MoveAction>,
    pub attacks: Vec<Square>,
}

#[derive(Serialize)]
pub struct LegalMovesResponse {
    pub moves: Vec<MoveAction>,
}

#[derive(Serialize)]
pub struct LegalDropsResponse {
    pub drops: Vec<DropAction>,
}

#[derive(Serialize)]
pub struct PieceAttacksResponse {
    pub squares: Vec<Square>,
}
