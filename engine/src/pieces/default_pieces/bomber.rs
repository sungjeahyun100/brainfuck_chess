use crate::types::*;

pub const BOMBER_TAKEOFF_ABILITY_ID: &str = "takeoff";
pub const BOMBER_BOMB_ABILITY_ID: &str = "bomb";
pub const BOMBER_LAND_ABILITY_ID: &str = "forced-landing";
pub const BOMBER_TAKEOFF_DISTANCE: i32 = 5;
pub const BOMBER_LANDING_DISTANCE: i32 = 4;

fn airborne_is(value: bool) -> Vec<PieceStatePredicate> {
    vec![PieceStatePredicate {
        key: "airborne".into(),
        condition: PieceStateCondition::Equals(PieceStateValue::Boolean(value)),
    }]
}

pub fn bomber_definition() -> PieceDefinition {
    let ground = "\
take-move(0, 1);
take-move(0, -1);
take-move(1, 0);
take-move(-1, 0);"
        .to_string();
    let air = "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);"
        .to_string();
    PieceDefinition {
        id: "bomber".into(),
        name: "폭격기".into(),
        score: 13,
        max_ammo: 3,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: ground.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: vec![PieceStateDefinition {
            key: "airborne".into(),
            default_value: PieceStateValue::Boolean(false),
        }],
        move_layers: vec![
            MoveLayerDefinition {
                id: "ground_move".into(),
                chessembly_code: ground,
                enabled_when: airborne_is(false),
                on_commit: Vec::new(),
            },
            MoveLayerDefinition {
                id: "air_move".into(),
                chessembly_code: air,
                enabled_when: airborne_is(true),
                on_commit: Vec::new(),
            },
        ],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "지상에서는 직교 1칸, 공중에서는 퀸처럼 이동합니다.".into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["ground_move".into(), "air_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: false,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: BOMBER_TAKEOFF_ABILITY_ID.into(),
                name: "비행".into(),
                description: "지상의 빈 활주로를 따라 5칸 이륙합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 0,
                enabled_when: airborne_is(false),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: BOMBER_BOMB_ABILITY_ID.into(),
                name: "폭격".into(),
                description: "바로 아래와 상하좌우의 지상 기물을 제거합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 1,
                enabled_when: airborne_is(true),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: BOMBER_LAND_ABILITY_ID.into(),
                name: "강제 착륙".into(),
                description: "비행 지속시간이 끝난 후 4칸 직선 활주로에 착륙합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 0,
                enabled_when: airborne_is(true),
                cooldown: None,
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "bomber".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("bomber definition must be valid")
}
