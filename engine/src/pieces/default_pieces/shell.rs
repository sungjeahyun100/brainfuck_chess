use crate::types::*;

pub const DETONATE_ABILITY_ID: &str = "detonate";

/// Shell: immobile explosive that drops on empty squares and immediately explodes.
pub fn shell_definition() -> PieceDefinition {
    PieceDefinition {
        id: "shell".into(),
        name: "포탄".into(),
        score: 3,
        ai_board_value: Some(0),
        ai_pocket_value: Some(3),
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: vec![MoveOptionDefinition {
            id: DETONATE_ABILITY_ID.into(),
            name: "폭발".into(),
            description: "자신과 상하좌우 한 칸의 모든 지상 기물을 제거합니다. 포켓에서 착수하면 즉시 발동합니다.".into(),
            kind: MoveOptionKind::Ability,
            layer_ids: Vec::new(),
            execution_mode: MoveOptionExecutionMode::StandaloneAction,
            contributes_to_attack_map: false,
            ammo_cost: 0,
            enabled_when: Vec::new(),
            cooldown: None,
        }],
        visual: PieceVisualDefinition {
            default_asset_key: "shell".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("shell definition must be valid")
}
