use crate::types::*;

/// Guhang: enters each orthogonal direction and repeatedly fans out along the
/// perpendicular axis.
pub fn guhang_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "guhang".into(),
        name: "구행".into(),
        score: 25,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
do
take-move(1, 0)
{ take-move(0, 1) repeat(1) }
{ take-move(0, -1) repeat(1) }
while;

do
take-move(-1, 0)
{ take-move(0, 1) repeat(1) }
{ take-move(0, -1) repeat(1) }
while;

do
take-move(0, 1)
{ take-move(1, 0) repeat(1) }
{ take-move(-1, 0) repeat(1) }
while;

do
take-move(0, -1)
{ take-move(1, 0) repeat(1) }
{ take-move(-1, 0) repeat(1) }
while;".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
