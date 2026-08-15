use crate::types::*;

/// Black Tempest Pawn with independent movement and promotion configuration.
pub fn tempest_pawn_black_definition() -> PieceDefinition {
    legacy_piece_definition! {
        deployment_zone: DeploymentZone::Front,
        id: "tempest-pawn-black".into(),
        name: "Tempest Pawn".into(),
        score: 2,
        chessembly_code: "\
move(0, -1);
move(1, 0);
move(-1, 0);
take(0, -2);
take(1, -1);
take(-1, -1);".into(),
        chessembly_version: "1.0".into(),
        dialect: Some(ChessemblyDialect::BrainfuckChess),
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::FirstRank,
        }),
        promotion_pool: vec![
            "tempest-queen".into(),
            "tempest-rook".into(),
            "tempest-bishop".into(),
            "tempest-knight".into(),
        ],
    }
}
