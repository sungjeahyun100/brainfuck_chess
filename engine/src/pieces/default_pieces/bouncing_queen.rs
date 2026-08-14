use crate::types::*;

/// Bouncing Queen owns the complete Bishop and Rook bounce programs.
pub fn bouncing_queen_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bouncing-queen".into(),
        name: "Bouncing Queen".into(),
        score: 13,
        chessembly_code: "\
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
};
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
};".to_string(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
