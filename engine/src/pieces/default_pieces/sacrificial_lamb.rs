use crate::types::*;

/// Sacrificial Lamb: an immobile one-point setup piece.
pub fn sacrificial_lamb_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "sacrificial-lamb".into(),
        name: "희생양".into(),
        score: 1,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
