use crate::types::*;

pub const MORTAR_BARRAGE_ABILITY_ID: &str = "mortar-barrage";

/// Mortar: takes one orthogonal step and shells around a selected point in its
/// file or either adjacent file.
pub fn mortar_definition() -> PieceDefinition {
    let movement = "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);"
        .to_string();
    PieceDefinition {
        id: "mortar".into(),
        name: "박격포병".into(),
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
            id: "mortar_step".into(),
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
                layer_ids: vec!["mortar_step".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                cooldown: None,
            },
            MoveOptionDefinition {
                id: MORTAR_BARRAGE_ABILITY_ID.into(),
                name: "박격포 사격".into(),
                description: "자신의 파일과 양옆 파일에서 한 지점을 선택해, 그 지점과 상하좌우에 인접한 모든 기물을 제거합니다. 사용 후 소유자의 다음 2개 턴 동안 사용할 수 없습니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                cooldown: Some(CooldownDefinition {
                    turns: 2,
                    clock: CooldownClock::OwnerTurns,
                }),
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "mortar".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("mortar definition must be valid")
}
