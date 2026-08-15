use crate::types::*;

pub const MACHINE_GUN_BARRAGE_ABILITY_ID: &str = "machine-gun-barrage";

/// Machine Gunner: takes one orthogonal step and can clear its three forward files.
pub fn machine_gunner_definition() -> PieceDefinition {
    let movement = "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);"
        .to_string();
    PieceDefinition {
        id: "machine-gunner".into(),
        name: "기관총 사수".into(),
        score: 8,
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
            id: "machine_gunner_step".into(),
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
                layer_ids: vec!["machine_gunner_step".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                cooldown: None,
            },
            MoveOptionDefinition {
                id: MACHINE_GUN_BARRAGE_ABILITY_ID.into(),
                name: "기관총 사격".into(),
                description: "자신과 양옆 파일에서 전방에 있는 모든 기물을 제거합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                cooldown: None,
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "paratrooper".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("machine gunner definition must be valid")
}
