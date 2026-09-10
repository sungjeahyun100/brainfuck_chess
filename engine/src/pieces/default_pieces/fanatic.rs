use crate::types::*;

/// Fanatic: advances one or two squares and destroys its capturer when captured.
pub fn fanatic_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "fanatic".into(),
        name: "광신도".into(),
        score: 2,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Front,
        chessembly_code: "take-move(0, 1) take-move(0, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
