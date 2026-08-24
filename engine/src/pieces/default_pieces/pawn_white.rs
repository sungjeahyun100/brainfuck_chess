use crate::types::*;

/// White Pawn: moves forward, attacks diagonally, and promotes on the last rank.
pub fn pawn_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
        deployment_zone: DeploymentZone::Front,
        id: "pawn-white".into(),
        name: "Pawn".into(),
        score: 1,
        max_ammo: 0,
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
        promotion_pool: vec!["queen".into(), "rook".into(), "bishop".into(), "knight".into()],
    }
}
