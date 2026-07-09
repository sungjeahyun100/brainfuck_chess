use brainfuck_chess_engine::types::{MoveAction, PlayerId, Square};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LabPieceOptionsRequest {
    pub board_size: i32,
    pub pieces: Vec<LabPieceRequest>,
    pub selected_piece_id: String,
    pub ability_id: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct LabPieceRequest {
    pub id: String,
    pub piece_type: String,
    pub owner: PlayerId,
    pub square: Square,
}

#[derive(Serialize)]
pub struct LabPieceOptionsResponse {
    pub moves: Vec<Square>,
    pub legal_moves: Vec<MoveAction>,
    pub attacks: Vec<Square>,
    pub abilities: Vec<LabAbilityOption>,
}

#[derive(Serialize)]
pub struct LabAbilityOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub available: bool,
    pub connected: bool,
}
