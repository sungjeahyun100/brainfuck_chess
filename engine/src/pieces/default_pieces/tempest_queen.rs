use crate::types::*;

pub fn tempest_queen_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-queen".into(),
        name: "Tempest Queen".into(),
        score: 10,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
        # 템페스트 룩 부분
    {
        take-move(1, 1)
        { take-move(1, 0) repeat(1) }
        { take-move(0, 1) repeat(1) }
    }
    {
        take-move(-1, 1)
        { take-move(-1, 0) repeat(1) }
        { take-move(0, 1) repeat(1) }
    }
    {
        take-move(-1, -1)
        { take-move(-1, 0) repeat(1) }
        { take-move(0, -1) repeat(1) }
    }
    {
        take-move(1, -1)
        { take-move(1, 0) repeat(1) }
        { take-move(0, -1) repeat(1) }
    }
    # 템페스트 비숍 부분
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
