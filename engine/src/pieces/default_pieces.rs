//! Standard chess piece definitions expressed in Chessembly DSL.
//!
//! Pawn direction is handled by storing separate definitions for White and Black.
//! `hasMoved` tracking is done by the rule engine (Pawn 2-step rule).

use crate::types::{
    AbilityDuration, PieceAbilityDefinition, PieceDefinition, PromotionCondition, PromotionRule,
};

/// King: one step in any of 8 directions, can move and capture.
pub fn king_definition() -> PieceDefinition {
    PieceDefinition {
        id: "king".into(),
        name: "King".into(),
        score: 0,
        chessembly_code: "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);
take-move(1, 1);
take-move(1, -1);
take-move(-1, 1);
take-move(-1, -1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: true,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Queen: slides in 8 directions.
pub fn queen_definition() -> PieceDefinition {
    PieceDefinition {
        id: "queen".into(),
        name: "Queen".into(),
        score: 9,
        chessembly_code: "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Rook: slides horizontally and vertically.
pub fn rook_definition() -> PieceDefinition {
    PieceDefinition {
        id: "rook".into(),
        name: "Rook".into(),
        score: 5,
        chessembly_code: "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Cannon Rook: moves like a Rook normally, or like a Janggi cannon when its
/// `cannon_move` ability is selected for the move.
pub fn cannon_rook_definition() -> PieceDefinition {
    let rook_code = rook_definition().chessembly_code;
    PieceDefinition {
        id: "cannon-rook".into(),
        name: "Cannon Rook".into(),
        score: 7,
        chessembly_code: rook_code,
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: vec![PieceAbilityDefinition {
            id: "cannon_move".into(),
            name: "포 이동".into(),
            description: "이번 이동 동안 장기의 포처럼 정확히 하나의 기물을 뛰어넘어 이동합니다. 사용 후 3턴 동안 다시 사용할 수 없습니다.".into(),
            chessembly_code: "\
do peek(0, 1) while take-move(0, 1) repeat(1);
do peek(1, 0) while take-move(1, 0) repeat(1);
do peek(0, -1) while take-move(0, -1) repeat(1);
do peek(-1, 0) while take-move(-1, 0) repeat(1);"
                .into(),
            duration: AbilityDuration::UntilPieceMoves,
            once_per_turn: false,
            cooldown_turns: 3,
        }],
    }
}

/// Bishop: slides diagonally.
pub fn bishop_definition() -> PieceDefinition {
    let bounce_code = bouncing_bishop_definition().chessembly_code;
    PieceDefinition {
        id: "bishop".into(),
        name: "Bishop".into(),
        score: 3,
        chessembly_code: "\
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Knight: L-shaped jump.
pub fn knight_definition() -> PieceDefinition {
    PieceDefinition {
        id: "knight".into(),
        name: "Knight".into(),
        score: 3,
        chessembly_code: "\
take-move(1, 2);
take-move(2, 1);
take-move(2, -1);
take-move(1, -2);
take-move(-1, -2);
take-move(-2, -1);
take-move(-2, 1);
take-move(-1, 2);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Amazon: combines Queen sliding moves with Knight jumps.
pub fn amazon_definition() -> PieceDefinition {
    PieceDefinition {
        id: "amazon".into(),
        name: "Amazon".into(),
        score: 13,
        chessembly_code: "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);
take-move(1, 2);
take-move(2, 1);
take-move(2, -1);
take-move(1, -2);
take-move(-1, -2);
take-move(-2, -1);
take-move(-2, 1);
take-move(-1, 2);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Tempest Rook: steps diagonally, then storms horizontally and vertically away
/// from that diagonal landing square.
pub fn tempest_rook_definition() -> PieceDefinition {
    PieceDefinition {
        id: "tempest-rook".into(),
        name: "Tempest Rook".into(),
        score: 8,
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
};"
        .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Bouncing Bishop: slides diagonally and reflects off board edges.
pub fn bouncing_bishop_definition() -> PieceDefinition {
    PieceDefinition {
        id: "bouncing-bishop".into(),
        name: "Bouncing Bishop".into(),
        score: 7,
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
};"
        .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// White Pawn:
/// - Moves forward (rank+1) with `move`
/// - Attacks diagonally forward with `take`
/// - 2-step initial move only available from rank 1 (base zone second rank)
///   guarded by `observe(0, 1)` to ensure the path is clear.
pub fn pawn_white_definition() -> PieceDefinition {
    PieceDefinition {
        id: "pawn-white".into(),
        name: "Pawn".into(),
        score: 1,
        chessembly_code: "\
move(0, 1);
observe(0, 1) move(0, 2);
take(1, 1);
take(-1, 1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: Some(crate::types::ChessemblyDialect::BrainfuckChess),
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec![
            "queen".into(),
            "rook".into(),
            "bishop".into(),
            "knight".into(),
        ],
        abilities: Vec::new(),
    }
}

/// Black Pawn: mirror of White Pawn (rank direction reversed).
pub fn pawn_black_definition() -> PieceDefinition {
    PieceDefinition {
        id: "pawn-black".into(),
        name: "Pawn".into(),
        score: 1,
        chessembly_code: "\
move(0, -1);
observe(0, -1) move(0, -2);
take(1, -1);
take(-1, -1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: Some(crate::types::ChessemblyDialect::BrainfuckChess),
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::FirstRank,
        }),
        promotion_pool: vec![
            "queen".into(),
            "rook".into(),
            "bishop".into(),
            "knight".into(),
        ],
        abilities: Vec::new(),
    }
}

fn tempest_pawn_promotion_pool() -> Vec<String> {
    vec![
        "tempest-queen".into(),
        "tempest-rook".into(),
        "bouncing-bishop".into(),
        "tempest-knight".into(),
    ]
}

/// White Tempest Pawn: moves exactly like a White Pawn, but promotes only into
/// tempest-line pieces.
pub fn tempest_pawn_white_definition() -> PieceDefinition {
    let mut definition = pawn_white_definition();
    definition.id = "tempest-pawn-white".into();
    definition.name = "Tempest Pawn".into();
    definition.promotion_pool = tempest_pawn_promotion_pool();
    definition
}

/// Black Tempest Pawn: moves exactly like a Black Pawn, but promotes only into
/// tempest-line pieces.
pub fn tempest_pawn_black_definition() -> PieceDefinition {
    let mut definition = pawn_black_definition();
    definition.id = "tempest-pawn-black".into();
    definition.name = "Tempest Pawn".into();
    definition.promotion_pool = tempest_pawn_promotion_pool();
    definition
}

pub fn tempest_queen_definition() -> PieceDefinition {
    PieceDefinition {
        id: "tempest-queen".into(),
        name: "Tempest Queen".into(),
        score: 12,
        chessembly_code: "\
    {
        take-move(1, 1)
        { take-move(1, 0) repeat(1) }
        { take-move(1, 1) repeat(1) }
        { take-move(0, 1) repeat(1) }
    }
    {
        take-move(-1, 1)
        { take-move(-1, 0) repeat(1) }
        { take-move(-1, 1) repeat(1) }
        { take-move(0, 1) repeat(1) }
    }
    {
        take-move(-1, -1)
        { take-move(-1, 0) repeat(1) }
        { take-move(-1, -1) repeat(1) }
        { take-move(0, -1) repeat(1) }
    }
    {
        take-move(1, -1)
        { take-move(1, 0) repeat(1) }
        { take-move(1, -1) repeat(1) }
        { take-move(0, -1) repeat(1) }
    };"
        .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

pub fn tempest_knight_definition() -> PieceDefinition {
    PieceDefinition {
        id: "tempest-knight".into(),
        name: "Tempest Knight".into(),
        score: 7,
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
        take-move(0, -3);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
        abilities: Vec::new(),
    }
}

/// Return all standard piece definitions.
pub fn all_default_definitions() -> Vec<PieceDefinition> {
    vec![
        king_definition(),
        queen_definition(),
        rook_definition(),
        bishop_definition(),
        knight_definition(),
        amazon_definition(),
        cannon_rook_definition(),
        tempest_rook_definition(),
        bouncing_bishop_definition(),
        pawn_white_definition(),
        pawn_black_definition(),
        tempest_pawn_white_definition(),
        tempest_pawn_black_definition(),
        tempest_queen_definition(),
        tempest_knight_definition(),
    ]
}
