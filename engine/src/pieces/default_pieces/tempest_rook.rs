use crate::types::*;

/// Tempest Rook: steps diagonally, then storms horizontally and vertically away.
pub fn tempest_rook_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-rook".into(),
        name: "Tempest Rook".into(),
        score: 8,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
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
};".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
