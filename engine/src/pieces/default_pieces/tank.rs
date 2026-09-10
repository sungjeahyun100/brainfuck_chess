use crate::types::*;

pub const TANK_FIRE_ABILITY_ID: &str = "tank-fire";
pub const TANK_FIRE_RANGE: i32 = 5;

pub fn tank_definition() -> PieceDefinition {
    let movement = "\
take-move(0, 1) take-move(0, 1);
take-move(0, -1) take-move(0, -1);
take-move(1, 0) take-move(1, 0);
take-move(-1, 0) take-move(-1, 0);"
        .to_string();
    PieceDefinition {
        id: "tank".into(),
        name: "탱크".into(),
        score: 12,
        ai_board_value: None,
        ai_pocket_value: None,
        max_ammo: 3,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: movement.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: vec![MoveLayerDefinition {
            id: "tank_move".into(),
            chessembly_code: movement,
            enabled_when: Vec::new(),
            on_commit: Vec::new(),
        }],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "상하좌우로 연속해 두 칸 이동합니다.".into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["tank_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: TANK_FIRE_ABILITY_ID.into(),
                name: "주포 발사".into(),
                description:
                    "8방향으로 최대 5칸까지 조준해 착탄지와 상하좌우의 지상 기물을 제거합니다."
                        .into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 1,
                enabled_when: Vec::new(),
                cooldown: Some(CooldownDefinition {
                    turns: 1,
                    clock: CooldownClock::OwnerTurns,
                }),
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "tank".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("tank definition must be valid")
}
