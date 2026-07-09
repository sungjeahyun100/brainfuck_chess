use serde::{Deserialize, Serialize};

use crate::types::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionEffect {
    MovePiece {
        piece_id: PieceId,
        from: Square,
        to: Square,
    },
    CapturePiece {
        piece_id: PieceId,
        at: Square,
    },
    DropPiece {
        piece_id: PieceId,
        to: Square,
    },
    PromotePiece {
        piece_id: PieceId,
        from_type: PieceTypeId,
        to_type: PieceTypeId,
    },
    SwapPieces {
        first_piece_id: PieceId,
        second_piece_id: PieceId,
        first_to: Square,
        second_to: Square,
    },
    SetPieceAbility {
        piece_id: PieceId,
        ability_id: String,
    },
    ClearPieceAbility {
        piece_id: PieceId,
        ability_id: String,
    },
    SetAbilityCooldown {
        piece_id: PieceId,
        ability_id: String,
        usable_turn: u32,
    },
    SetEnPassant {
        target: Option<Square>,
        available_to: Option<PlayerId>,
    },
    AdvanceTurn {
        from_player: PlayerId,
        to_player: PlayerId,
        turn_number: u32,
    },
    EndGame {
        result: GameResult,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedAction {
    pub action: TurnAction,
    pub effects: Vec<ActionEffect>,
    pub state: GameState,
}
