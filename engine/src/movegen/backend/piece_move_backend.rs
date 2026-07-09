use serde::{Deserialize, Serialize};

use crate::types::*;

pub struct PieceMoveContext<'a> {
    pub state: &'a GameState,
    pub piece: &'a Piece,
    pub definition: &'a PieceDefinition,
    pub player_id: &'a PlayerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveIntent {
    MoveTo { to: Square },
    CaptureOnDestination { to: Square },
    RemoteCapture { target: Square },
    Swap { target: Square },
    JumpCapture { captured: Square, landing: Square },
    Transition { to: Square, piece_type: PieceTypeId },
    SetState { to: Square, key: String, value: i32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceMovePattern {
    pub movement_squares: Vec<Square>,
    pub attack_squares: Vec<Square>,
    pub intents: Vec<MoveIntent>,
}

pub trait PieceMoveBackend {
    fn generate(&self, ctx: PieceMoveContext<'_>) -> PieceMovePattern;
}
