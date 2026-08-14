use crate::types::*;

/// Nightrider: repeats Knight leaps in a straight line until blocked.
pub fn nightrider_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "nightrider".into(),
        name: "Nightrider".into(),
        score: 5,
        chessembly_code: "\
take-move(2, 1) repeat(1);
take-move(2, -1) repeat(1);
take-move(-2, 1) repeat(1);
take-move(-2, -1) repeat(1);
take-move(1, 2) repeat(1);
take-move(-1, 2) repeat(1);
take-move(1, -2) repeat(1);
take-move(-1, -2) repeat(1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
