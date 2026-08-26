use crate::types::*;

pub fn tempest_bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-bishop".into(),
        name: "Tempest Bishop".into(),
        score: 5,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
        take-move(0, 1) 
        { take-move(-1, 1) repeat(1) }
        { take-move(1, 1) repeat(1) };
        take-move(0, -1) 
        { take-move(-1, -1) repeat(1) }
        { take-move(1, -1) repeat(1) };
        take-move(1, 0) 
        { take-move(1, 1) repeat(1) }
        { take-move(1, -1) repeat(1) };
        take-move(-1, 0) 
        { take-move(-1, 1) repeat(1) }
        { take-move(-1, -1) repeat(1) };".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
