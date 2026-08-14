use crate::types::*;

/// Airborne: moves one square in any direction and deploys eligible pocket
/// pieces into its forward area.
pub fn airborne_definition() -> PieceDefinition {
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
        id: "airborne".into(),
        name: "공수부대".into(),
        score: 6,
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
                cooldown: None,
            },
            MoveOptionDefinition {
                id: "airdrop".into(),
                name: "공중 소환".into(),
                description: "전방 2×3 구역에 점수 4 이하인 포켓 기물을 소환합니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: Vec::new(),
                execution_mode: MoveOptionExecutionMode::StandaloneAction,
                contributes_to_attack_map: false,
                cooldown: None,
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "airborne".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("airborne definition must be valid")
}
