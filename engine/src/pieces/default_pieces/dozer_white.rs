use crate::types::*;

/// White Dozer: advances one rank across a five-file-wide front.
pub fn dozer_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "dozer-white".into(),
        name: "Dozer".into(),
        score: 2,
        chessembly_code: "\
take-move(-2, 1);
take-move(-1, 1);
take-move(0, 1);
take-move(1, 1);
take-move(2, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec!["knight".into(), "bishop".into()],
    }
}
