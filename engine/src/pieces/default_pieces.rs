//! Standard chess piece definitions expressed in Chessembly DSL.
//!
//! Pawn direction is handled by storing separate definitions for White and Black.
//! `hasMoved` tracking is done by the rule engine (Pawn 2-step rule).

use crate::types::*;

/// Keeps the individual definitions compact while ensuring legacy single-code
/// pieces are exposed as a validated default layer/normal option model.
macro_rules! legacy_piece_definition {
    ($($field:tt)*) => {{
        PieceDefinition {
            $($field)*
            state_schema: Vec::new(),
            move_layers: Vec::new(),
            move_options: Vec::new(),
            visual: PieceVisualDefinition::default(),
            can_capture_on_drop: false,
        }
        .normalize_and_validate()
        .expect("built-in piece definition must be valid")
    }};
}

/// King: one step in any of 8 directions, can move and capture.
pub fn king_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Queen: slides in 8 directions.
pub fn queen_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Rook: slides horizontally and vertically.
pub fn rook_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Cannon Rook: moves like a Rook normally, or like a Janggi cannon when its
/// `cannon_move` ability is selected for the move.
pub fn cannon_rook_definition() -> PieceDefinition {
    let rook_code = rook_definition().chessembly_code;
    let cannon_code = "\
do peek(0, 1) while take-move(0, 1) repeat(1);
do peek(1, 0) while take-move(1, 0) repeat(1);
do peek(0, -1) while take-move(0, -1) repeat(1);
do peek(-1, 0) while take-move(-1, 0) repeat(1);"
        .to_string();
    PieceDefinition {
        id: "cannon-rook".into(),
        name: "Cannon Rook".into(),
        score: 7,
        chessembly_code: rook_code.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: vec![
            MoveLayerDefinition {
                id: "rook_move".into(),
                chessembly_code: rook_code,
                enabled_when: Vec::new(),
                on_commit: Vec::new(),
            },
            MoveLayerDefinition {
                id: "cannon_move".into(),
                chessembly_code: cannon_code,
                enabled_when: Vec::new(),
                on_commit: Vec::new(),
            },
        ],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: String::new(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["rook_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                cooldown: None,
            },
            MoveOptionDefinition {
                id: "cannon_move".into(),
                name: "포 이동".into(),
                description: "장기의 포처럼 정확히 하나의 기물을 뛰어넘습니다. 사용 후 소유자의 다음 3개 턴 동안 사용할 수 없습니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: vec!["cannon_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                cooldown: Some(CooldownDefinition {
                    turns: 3,
                    clock: CooldownClock::OwnerTurns,
                }),
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "cannon-rook".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("cannon rook definition must be valid")
}

/// Bishop: slides diagonally.
pub fn bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Windmill: alternates Bishop and Rook movement after each successful move.
pub fn windmill_definition() -> PieceDefinition {
    let bishop_code = bishop_definition().chessembly_code;
    let rook_code = rook_definition().chessembly_code;
    PieceDefinition {
        id: "windmill".into(),
        name: "Windmill".into(),
        score: 4, // TODO: balance value
        chessembly_code: bishop_code.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: vec![PieceStateDefinition {
            key: "mode".into(),
            default_value: PieceStateValue::Text("bishop".into()),
        }],
        move_layers: vec![
            MoveLayerDefinition {
                id: "bishop_mode".into(),
                chessembly_code: bishop_code,
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("bishop".into())),
                }],
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "mode".into(),
                    value: PieceStateValue::Text("rook".into()),
                }],
            },
            MoveLayerDefinition {
                id: "rook_mode".into(),
                chessembly_code: rook_code,
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("rook".into())),
                }],
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "mode".into(),
                    value: PieceStateValue::Text("bishop".into()),
                }],
            },
        ],
        move_options: vec![MoveOptionDefinition {
            id: "normal".into(),
            name: "일반 이동".into(),
            description: "현재 모드의 이동 후 비숍/룩 모드를 전환합니다.".into(),
            kind: MoveOptionKind::Normal,
            layer_ids: vec!["bishop_mode".into(), "rook_mode".into()],
            execution_mode: MoveOptionExecutionMode::MoveModifier,
            contributes_to_attack_map: true,
            cooldown: None,
        }],
        visual: PieceVisualDefinition {
            default_asset_key: "windmill-bishop".into(),
            variants: vec![PieceVisualVariantDefinition {
                id: "rook_mode".into(),
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("rook".into())),
                }],
                asset_key: "windmill-rook".into(),
                priority: 10,
            }],
        },
    }
    .normalize_and_validate()
    .expect("windmill definition must be valid")
}

/// Knight: L-shaped jump.
pub fn knight_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Nightrider: repeats Knight leaps in a straight line until blocked.
pub fn nightrider_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "nightrider".into(),
        name: "Nightrider".into(),
        score: 5,
        chessembly_code: "\
take-move(2, 1) repeat(1);
take-move(2, -1) repeat(1);
take-move(-2, 1) repeat(1);
take-move(-2, -1) repeat(1);
take-move(1, 2) repeat(1);
take-move(-1, 2) repeat(1);
take-move(1, -2) repeat(1);
take-move(-1, -2) repeat(1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

/// Paratrooper: cannot move on the board and may capture only while dropping.
pub fn paratrooper_definition() -> PieceDefinition {
    PieceDefinition {
        id: "paratrooper".into(),
        name: "공수부대 대원".into(),
        score: 3,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: true,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition {
            default_asset_key: "paratrooper".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("paratrooper definition must be valid")
}

/// Amazon: combines Queen sliding moves with Knight jumps.
pub fn amazon_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Guhang: enters each orthogonal direction and repeatedly fans out along the
/// perpendicular axis.
pub fn guhang_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "guhang".into(),
        name: "구행".into(),
        score: 25,
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
while;"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

/// Tempest Rook: steps diagonally, then storms horizontally and vertically away
/// from that diagonal landing square.
pub fn tempest_rook_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Bouncing Bishop: follows diagonals and reflects off board edges as its
/// normal movement.
pub fn bouncing_bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "bouncing-bishop".into(),
        name: "Bouncing Bishop".into(),
        score: 7,
        chessembly_code: bouncing_bishop_chessembly_code().into(),
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

/// White Pawn:
/// - Moves forward (rank+1) with `move`
/// - Attacks diagonally forward with `take`
/// - 2-step initial move only available from rank 1 (base zone second rank)
///   guarded by `observe(0, 1)` to ensure the path is clear.
pub fn pawn_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// Black Pawn: mirror of White Pawn (rank direction reversed).
pub fn pawn_black_definition() -> PieceDefinition {
    legacy_piece_definition! {
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
    }
}

/// White Dozer: advances one rank across a five-file-wide front and promotes
/// to a Bishop or Knight on the last rank.
pub fn dozer_white_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "dozer-white".into(),
        name: "Dozer".into(),
        score: 2,
        chessembly_code: "\
take-move(-2, 1);
take-move(-1, 1);
take-move(0, 1);
take-move(1, 1);
take-move(2, 1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec!["knight".into(), "bishop".into()],
    }
}

/// Black Dozer: mirrored White Dozer movement and first-rank promotion.
pub fn dozer_black_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "dozer-black".into(),
        name: "Dozer".into(),
        score: 2,
        chessembly_code: "\
take-move(-2, -1);
take-move(-1, -1);
take-move(0, -1);
take-move(1, -1);
take-move(2, -1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::FirstRank,
        }),
        promotion_pool: vec!["knight".into(), "bishop".into()],
    }
}

fn tempest_pawn_promotion_pool() -> Vec<String> {
    vec![
        "tempest-queen".into(),
        "tempest-rook".into(),
        "tempest-bishop".into(),
        "tempest-knight".into(),
    ]
}

/// White Tempest Pawn: can advance or sidestep one square, captures one square
/// diagonally forward or two squares straight ahead, and promotes only into
/// tempest-line pieces.
pub fn tempest_pawn_white_definition() -> PieceDefinition {
    let mut definition = pawn_white_definition();
    definition.id = "tempest-pawn-white".into();
    definition.name = "Tempest Pawn".into();
    let chessembly_code = "\
move(0, 1);
move(1, 0);
move(-1, 0);
take(0, 2);
take(1, 1);
take(-1, 1);"
        .into();
    definition.chessembly_code = chessembly_code;
    definition.move_layers[0].chessembly_code = definition.chessembly_code.clone();
    definition.promotion_pool = tempest_pawn_promotion_pool();
    definition.visual.default_asset_key = "tempest-pawn-white".into();
    definition
}

/// Black Tempest Pawn: mirrored White Tempest Pawn movement and first-rank
/// promotion into tempest-line pieces.
pub fn tempest_pawn_black_definition() -> PieceDefinition {
    let mut definition = pawn_black_definition();
    definition.id = "tempest-pawn-black".into();
    definition.name = "Tempest Pawn".into();
    let chessembly_code = "\
move(0, -1);
move(1, 0);
move(-1, 0);
take(0, -2);
take(1, -1);
take(-1, -1);"
        .into();
    definition.chessembly_code = chessembly_code;
    definition.move_layers[0].chessembly_code = definition.chessembly_code.clone();
    definition.promotion_pool = tempest_pawn_promotion_pool();
    definition.visual.default_asset_key = "tempest-pawn-black".into();
    definition
}

pub fn tempest_queen_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-queen".into(),
        name: "Tempest Queen".into(),
        score: 10,
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
    { take-move(-1, -1) repeat(1) };"
        .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

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
        take-move(0, -3);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

pub fn tempest_bishop_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "tempest-bishop".into(),
        name: "Tempest Bishop".into(),
        score: 5,
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
        { take-move(-1, -1) repeat(1) };"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

/// Return all standard piece definitions.
pub fn all_default_definitions() -> Vec<PieceDefinition> {
    vec![
        king_definition(),
        queen_definition(),
        rook_definition(),
        bishop_definition(),
        windmill_definition(),
        knight_definition(),
        nightrider_definition(),
        paratrooper_definition(),
        amazon_definition(),
        guhang_definition(),
        cannon_rook_definition(),
        tempest_rook_definition(),
        bouncing_bishop_definition(),
        pawn_white_definition(),
        pawn_black_definition(),
        dozer_white_definition(),
        dozer_black_definition(),
        tempest_pawn_white_definition(),
        tempest_pawn_black_definition(),
        tempest_queen_definition(),
        tempest_knight_definition(),
        tempest_bishop_definition(),
    ]
}
