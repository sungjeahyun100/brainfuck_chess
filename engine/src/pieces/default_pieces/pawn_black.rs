use crate::types::*;

/// Black Pawn: mirror of White Pawn (rank direction reversed).
pub fn pawn_black_definition() -> PieceDefinition {
    legacy_piece_definition! {
        deployment_zone: DeploymentZone::Front,
        id: "pawn-black".into(),
        name: "Pawn".into(),
        score: 1,
        chessembly_code: "\
move(0, -1);
observe(0, -1) move(0, -2);
take(1, -1);
take(-1, -1);".into(),
        chessembly_version: "1.0".into(),
        dialect: Some(ChessemblyDialect::BrainfuckChess),
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::FirstRank,
        }),
        promotion_pool: vec!["queen".into(), "rook".into(), "bishop".into(), "knight".into()],
    }
}
