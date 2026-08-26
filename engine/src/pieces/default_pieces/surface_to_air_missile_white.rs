use crate::types::*;

pub const INTERCEPT_ABILITY_ID: &str = "intercept";

/// White Surface-to-Air Missile: advances toward increasing ranks and can
/// destroy a nearby enemy piece on the Air Layer.
pub fn surface_to_air_missile_white_definition() -> PieceDefinition {
    let movement = "\
take-move(1, 1);
take-move(0, 1) take-move(0, 1);
take-move(-1, 1);"
        .to_string();
    PieceDefinition {
        id: "surface-to-air-missile-white".into(),
        name: "지대공 미사일".into(),
        score: 2,
        max_ammo: 2,
        deployment_zone: DeploymentZone::Front,
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
            id: "missile_move".into(),
            chessembly_code: movement,
            enabled_when: Vec::new(),
            on_commit: Vec::new(),
        }],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: "전방 대각선으로 1칸 또는 전방으로 연속 2칸 이동하거나 포획합니다."
                    .into(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["missile_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: INTERCEPT_ABILITY_ID.into(),
                name: "격추".into(),
                description: "자신 중심 5파일×3랭크 범위의 적 비행 기물 하나를 제거합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                ammo_cost: 1,
                enabled_when: Vec::new(),
                cooldown: None,
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "surface-to-air-missile".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("white surface-to-air missile definition must be valid")
}
