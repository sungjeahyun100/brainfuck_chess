use crate::types::*;

pub fn bouncing_pawn_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bouncing-pawn-white".into(),
        name: "Bouncing Pawn".into(),
        score: 2,
        chessembly_code: "\
move(0, 1);
observe(0, 1) move(0, 2);
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
            "bouncing-rook".into(),
            "bouncing-bishop".into(),
            "bouncing-queen".into(),
        ],
    }
}
