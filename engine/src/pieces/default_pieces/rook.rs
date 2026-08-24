use crate::types::*;

/// Rook: slides horizontally and vertically.
pub fn rook_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "rook".into(),
        name: "Rook".into(),
        score: 5,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
