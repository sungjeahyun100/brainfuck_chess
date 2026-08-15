use crate::types::*;

/// Bouncing Bishop: follows diagonals and reflects off board edges.
pub fn bouncing_bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bouncing-bishop".into(),
        name: "Bouncing Bishop".into(),
        score: 7,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: bouncing_bishop_chessembly_code().to_string(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

fn bouncing_bishop_chessembly_code() -> &'static str {
    "\
do
take-move(1, 1)
while
edge(1, 1) {
  take-move(-1, 1) repeat(1)
} {
  take-move(1, -1) repeat(1)
};

do
    take-move(-1, 1)
while
edge(-1, 1) {
  take-move(1, 1) repeat(1)
} {
  take-move(-1, -1) repeat(1)
};

do
    take-move(1, -1)
while
edge(1, -1) {
  take-move(1, 1) repeat(1)
} {
  take-move(-1, -1) repeat(1)
};

do
    take-move(-1, -1)
while
edge(-1, -1) {
  take-move(1, -1) repeat(1)
} {
  take-move(-1, 1) repeat(1)
};"
}
