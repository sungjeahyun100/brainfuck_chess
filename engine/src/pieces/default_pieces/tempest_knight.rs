use crate::types::*;

pub fn tempest_knight_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-knight".into(),
        name: "Tempest Knight".into(),
        score: 5,
        chessembly_code: "\
        {
            take-move(1, 1)
            { take-move(2, 1) }
            { take-move(1, 2) }
        }
        {
            take-move(1, -1)
            { take-move(2, -1) }
            { take-move(1, -2) }
        }
        {
            take-move(-1, 1)
            { take-move(-2, 1) }
            { take-move(-1, 2) }
        }
        {
            take-move(-1, -1)
            { take-move(-2, -1) }
            { take-move(-1, -2) }
        };
        take-move(3, 0);
        take-move(0, 3);
        take-move(-3, 0);
        take-move(0, -3);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
