use crate::types::*;

/// Black Dozer: mirrored White Dozer movement and first-rank promotion.
pub fn dozer_black_definition() -> PieceDefinition {
    legacy_piece_definition! {
        deployment_zone: DeploymentZone::Front,
        id: "dozer-black".into(),
        name: "Dozer".into(),
        score: 2,
        chessembly_code: "\
take-move(-2, -1);
take-move(-1, -1);
take-move(0, -1);
take-move(1, -1);
take-move(2, -1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::FirstRank,
        }),
        promotion_pool: vec!["knight".into(), "bishop".into()],
    }
}
