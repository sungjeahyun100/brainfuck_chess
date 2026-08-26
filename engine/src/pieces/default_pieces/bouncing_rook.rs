use crate::types::*;

/// Bouncing Rook: follows ranks and files, then turns at board edges.
pub fn bouncing_rook_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bouncing-rook".into(),
        name: "Bouncing Rook".into(),
        score: 6,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: bouncing_rook_chessembly_code(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

pub fn bouncing_rook_chessembly_code() -> String {
    "\
do
take-move(0, 1)
while
edge(0, 1) {
  take-move(1, 0) repeat(1)
} {
  take-move(-1, 0) repeat(1)
};

do
take-move(0, -1)
while
edge(0, -1) {
  take-move(1, 0) repeat(1)
} {
  take-move(-1, 0) repeat(1)
};

do
take-move(1, 0)
while
edge(1, 0) {
  take-move(0, 1) repeat(1)
} {
  take-move(0, -1) repeat(1)
};

do
take-move(-1, 0)
while
edge(-1, 0) {
  take-move(0, 1) repeat(1)
} {
  take-move(0, -1) repeat(1)
};"
    .to_string()
}
