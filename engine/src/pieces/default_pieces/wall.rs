use crate::types::*;

/// Wall: immobile and immune to ordinary movement/drop captures.
pub fn wall_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "wall".into(),
        name: "성벽".into(),
        score: 1,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Front,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
