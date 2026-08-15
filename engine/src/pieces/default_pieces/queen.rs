use crate::types::*;

/// Queen: slides in 8 directions.
pub fn queen_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "queen".into(),
        name: "Queen".into(),
        score: 9,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);
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
