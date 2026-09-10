use crate::types::*;

/// Cannon Rook: moves like a Rook normally, or like a Janggi cannon when its
/// `cannon_move` ability is selected for the move.
pub fn cannon_rook_definition() -> PieceDefinition {
    let rook_code = "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);"
        .to_string();
    let cannon_code = "\
do peek(0, 1) while take-move(0, 1) repeat(1);
do peek(1, 0) while take-move(1, 0) repeat(1);
do peek(0, -1) while take-move(0, -1) repeat(1);
do peek(-1, 0) while take-move(-1, 0) repeat(1);"
        .to_string();

    PieceDefinition {
        id: "cannon-rook".into(),
        name: "Cannon Rook".into(),
        score: 7,
        ai_board_value: None,
        ai_pocket_value: None,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: rook_code.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: vec![
            MoveLayerDefinition {
                id: "rook_move".into(),
                chessembly_code: rook_code,
                enabled_when: Vec::new(),
                on_commit: Vec::new(),
            },
            MoveLayerDefinition {
                id: "cannon_move".into(),
                chessembly_code: cannon_code,
                enabled_when: Vec::new(),
                on_commit: Vec::new(),
            },
        ],
        move_options: vec![
            MoveOptionDefinition {
                id: "normal".into(),
                name: "일반 이동".into(),
                description: String::new(),
                kind: MoveOptionKind::Normal,
                layer_ids: vec!["rook_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            },
            MoveOptionDefinition {
                id: "cannon_move".into(),
                name: "포 이동".into(),
                description: "장기의 포처럼 아군 또는 상대 기물 정확히 하나를 뛰어넘습니다. 사용 후 소유자의 다음 3개 턴 동안 사용할 수 없습니다.".into(),
                kind: MoveOptionKind::Ability,
                layer_ids: vec!["cannon_move".into()],
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: Some(CooldownDefinition {
                    turns: 3,
                    clock: CooldownClock::OwnerTurns,
                }),
            },
        ],
        visual: PieceVisualDefinition {
            default_asset_key: "cannon-rook".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("cannon rook definition must be valid")
}
