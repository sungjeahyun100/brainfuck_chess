use crate::types::*;

pub const TRANSFER_CIRCLE: &str = "transfer-circle";
pub const ALEKHINES_GUN: &str = "alekhines-gun";
pub const WIZARD_KNIGHT_CATCH: &str = "wizard-knight-catch";
pub const WIZARD_BISHOP_JUMP_CATCH: &str = "wizard-bishop-jump-catch";
pub const ENCOURAGE: &str = "encourage";
pub const LINKED_TELEPORT: &str = "linked-teleport";
pub const EXTRA_MOVE: &str = "extra_move_remaining";
// Stable promotion order for every implemented Wizard back-rank piece.
pub const WIZARD_PROMOTION_POOL: &[&str] = &[
    "wizard-queen",
    "wizard-knight",
    "wizard-bishop",
    "wizard-rook",
];

fn ability(id: &str, name: &str, description: &str) -> MoveOptionDefinition {
    MoveOptionDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        kind: MoveOptionKind::Ability,
        layer_ids: Vec::new(),
        execution_mode: MoveOptionExecutionMode::StandaloneAction,
        contributes_to_attack_map: false,
        ammo_cost: 0,
        enabled_when: Vec::new(),
        cooldown: None,
    }
}

pub fn wizard_king_definition() -> PieceDefinition {
    let mut definition = super::king_definition();
    definition.id = "wizard-king".into();
    definition.name = "마법사 킹".into();
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.visual.default_asset_key = "wizard-king".into();
    definition.move_options.push(ability(ENCOURAGE, "격려",
        "상하좌우에 아군 마법사 생도가 있으면 아군 마법사에게 이번 턴의 추가 이동 1회를 부여합니다. 턴을 사용하지 않습니다."));
    definition
}

pub fn wizard_cadet_definition() -> PieceDefinition {
    let mut definition = legacy_piece_definition! {
        id: "wizard-cadet".into(), name: "마법사 생도".into(), score: 1,
        max_ammo: 0, deployment_zone: DeploymentZone::Front,
        chessembly_code: "take-move(0, 1);".into(), chessembly_version: "1.0".into(),
        dialect: None, extensions: None, is_king: false,
        promotion: Some(PromotionRule { condition: PromotionCondition::LastRank }),
        promotion_pool: WIZARD_PROMOTION_POOL.iter().map(|id| (*id).into()).collect(),
    };
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.visual.default_asset_key = "wizard-cadet".into();
    definition.move_options.push(ability(
        LINKED_TELEPORT,
        "연동되는 축지법",
        "자신을 제외한 아군 마법사 하나와 위치를 교환하고 턴을 마칩니다.",
    ));
    definition
}

/// Like existing pawn definitions, black uses the mirrored Chessembly direction.
pub fn wizard_cadet_black_definition() -> PieceDefinition {
    let mut definition = wizard_cadet_definition();
    definition.id = "wizard-cadet-black".into();
    definition.chessembly_code = "take-move(0, -1);".into();
    for layer in &mut definition.move_layers {
        layer.chessembly_code = definition.chessembly_code.clone();
    }
    definition.promotion = Some(PromotionRule {
        condition: PromotionCondition::FirstRank,
    });
    definition
}

pub fn wizard_queen_definition() -> PieceDefinition {
    let mut definition = super::queen_definition();
    definition.id = "wizard-queen".into();
    definition.name = "마법사 퀸".into();
    definition.visual.default_asset_key = "wizard-queen".into();
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.move_options.push(ability(ALEKHINES_GUN, "알레킨의 총",
        "같은 직선의 아군 마법사 룩 두 개로 마법진을 구성합니다. 끝 룩 앞의 모든 아군·적군 기물을 제거하고 턴을 마칩니다."));
    definition
}

pub fn wizard_rook_definition() -> PieceDefinition {
    let mut definition = super::rook_definition();
    definition.id = "wizard-rook".into();
    definition.name = "마법사 룩".into();
    definition.visual.default_asset_key = "wizard-rook".into();
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.move_options.push(ability(TRANSFER_CIRCLE, "전이 마법진",
        "바로 좌우에 아군 마법사 생도가 있으면 바로 뒤의 아군 기물을 보드 전체의 빈칸으로 순간이동시키고 턴을 마칩니다."));
    definition
}

pub fn wizard_knight_definition() -> PieceDefinition {
    let mut definition = super::knight_definition();
    definition.id = "wizard-knight".into();
    definition.name = "마법사 나이트".into();
    definition.visual.default_asset_key = "wizard-knight".into();
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.move_options.push(ability(
        WIZARD_KNIGHT_CATCH,
        "퀸형 포획",
        "보드 내부의 모든 나이트 목적지가 마법사로 채워지면 8방향에서 처음 만나는 적을 포획합니다.",
    ));
    definition
}

pub fn wizard_bishop_definition() -> PieceDefinition {
    let mut definition = super::bishop_definition();
    definition.id = "wizard-bishop".into();
    definition.name = "마법사 비숍".into();
    definition.visual.default_asset_key = "wizard-bishop".into();
    definition.state_schema.push(PieceStateDefinition {
        key: EXTRA_MOVE.into(),
        default_value: PieceStateValue::Integer(0),
    });
    definition.move_options.push(ability(
        WIZARD_BISHOP_JUMP_CATCH,
        "도약 포획",
        "상하좌우가 마법사 생도로 채워지면 대각선의 첫 기물을 넘어 바로 뒤의 적을 포획합니다.",
    ));
    definition
}
