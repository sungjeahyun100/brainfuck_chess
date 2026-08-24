use crate::types::*;

/// Green Camp: moves one square in any direction and returns a nearby non-king
/// piece to its owner's pocket.
pub fn green_camp_definition() -> PieceDefinition {
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
        id: "green-camp".into(),
        name: "그린캠프".into(),
        score: 5,
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
            id: "king_step".into(),
            chessembly_code: movement,
            enabled_when: Vec::new(),
            on_commit: Vec::new(),
        }],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "왕처럼 한 칸 이동합니다.".into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["king_step".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: "recall".into(),
                name: "포켓 복귀".into(),
                description: "주변 8칸의 기물 하나를 그 기물 소유자의 포켓으로 돌려보냅니다."
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
            default_asset_key: "green-camp".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("green camp definition must be valid")
}
