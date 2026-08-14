use crate::types::*;

/// White Tempest Pawn with independent movement and promotion configuration.
pub fn tempest_pawn_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-pawn-white".into(),
        name: "Tempest Pawn".into(),
        score: 1,
        chessembly_code: "\
move(0, 1);
move(1, 0);
move(-1, 0);
take(0, 2);
take(1, 1);
take(-1, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: Some(ChessemblyDialect::BrainfuckChess),
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec![
            "tempest-queen".into(),
            "tempest-rook".into(),
            "tempest-bishop".into(),
            "tempest-knight".into(),
        ],
    }
}
