use crate::types::*;

/// King: one step in any of 8 directions, can move and capture.
pub fn king_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "king".into(),
        name: "King".into(),
        score: 0,
        chessembly_code: "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);
take-move(1, 1);
take-move(1, -1);
take-move(-1, 1);
take-move(-1, -1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: true,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
