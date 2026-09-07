use crate::types::*;

pub const REPAIR_WALLS_ABILITY_ID: &str = "repair-walls";

/// Repairman: orthogonal step movement and reconstruction of its forward wall.
pub fn repairman_definition() -> PieceDefinition {
    let movement = "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);"
        .to_string();
    PieceDefinition {
        id: "repairman".into(),
        name: "수리병".into(),
        score: 4,
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
            id: "repairman_step".into(),
            chessembly_code: movement,
            enabled_when: Vec::new(),
            on_commit: Vec::new(),
        }],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "상하좌우로 한 칸 이동하거나 포획합니다.".into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["repairman_step".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: REPAIR_WALLS_ABILITY_ID.into(),
                name: "성벽 수리".into(),
                description: "전방 3칸에 성벽이 하나 이상 있으면 나머지 빈칸을 성벽으로 채웁니다."
                    .into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "repairman".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("repairman definition must be valid")
}
