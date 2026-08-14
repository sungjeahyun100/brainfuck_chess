use crate::types::*;

/// Bishop: slides diagonally.
pub fn bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bishop".into(),
        name: "Bishop".into(),
        score: 3,
        chessembly_code: "\
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
