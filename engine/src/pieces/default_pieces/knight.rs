use crate::types::*;

/// Knight: L-shaped jump.
pub fn knight_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "knight".into(),
        name: "Knight".into(),
        score: 3,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
take-move(1, 2);
take-move(2, 1);
take-move(2, -1);
take-move(1, -2);
take-move(-1, -2);
take-move(-2, -1);
take-move(-2, 1);
take-move(-1, 2);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
