use crate::types::*;

pub const SACRIFICE_ABILITY_ID: &str = "sacrifice";

/// Sacrificial Shrine: king-step movement plus a bounded multi-target sacrifice.
pub fn sacrificial_shrine_definition() -> PieceDefinition {
    let movement = "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);
take-move(1, 1);
take-move(1, -1);
take-move(-1, 1);
take-move(-1, -1);"
        .to_string();
    PieceDefinition {
        id: "sacrificial-shrine".into(),
        name: "희생의 성소".into(),
        score: 8,
        ai_board_value: None,
        ai_pocket_value: None,
        max_ammo: 0,
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
            id: "shrine_step".into(),
            chessembly_code: movement,
            enabled_when: Vec::new(),
            on_commit: Vec::new(),
        }],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "왕처럼 한 칸 이동하거나 포획합니다.".into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["shrine_step".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: SACRIFICE_ABILITY_ID.into(),
                name: "희생".into(),
                description: "주변 아군 기물(킹 제외)을 모두 희생하고, 합계 8점 한도 안에서 그 점수 이하의 적 지상 기물을 선택해 제거합니다. 주변 8칸이 모두 희생양이면 적 킹도 선택할 수 있습니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: Some(CooldownDefinition {
                    turns: 2,
                    clock: CooldownClock::OwnerTurns,
                }),
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "sacrificial-shrine".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("sacrificial shrine definition must be valid")
}
