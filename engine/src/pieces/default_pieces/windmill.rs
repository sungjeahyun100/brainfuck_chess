use crate::types::*;

/// Windmill: alternates Bishop and Rook movement after each successful move.
pub fn windmill_definition() -> PieceDefinition {
    let bishop_code = "\
take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);"
        .to_string();
    let rook_code = "\
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);"
        .to_string();

    PieceDefinition {
        id: "windmill".into(),
        name: "Windmill".into(),
        score: 4, // TODO: balance value
        chessembly_code: bishop_code.clone(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: vec![PieceStateDefinition {
            key: "mode".into(),
            default_value: PieceStateValue::Text("bishop".into()),
        }],
        move_layers: vec![
            MoveLayerDefinition {
                id: "bishop_mode".into(),
                chessembly_code: bishop_code,
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("bishop".into())),
                }],
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "mode".into(),
                    value: PieceStateValue::Text("rook".into()),
                }],
            },
            MoveLayerDefinition {
                id: "rook_mode".into(),
                chessembly_code: rook_code,
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("rook".into())),
                }],
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "mode".into(),
                    value: PieceStateValue::Text("bishop".into()),
                }],
            },
        ],
        move_options: vec![MoveOptionDefinition {
            id: "normal".into(),
            name: "일반 이동".into(),
            description: "현재 모드의 이동 후 비숍/룩 모드를 전환합니다.".into(),
            kind: MoveOptionKind::Normal,
            layer_ids: vec!["bishop_mode".into(), "rook_mode".into()],
            execution_mode: MoveOptionExecutionMode::MoveModifier,
            contributes_to_attack_map: true,
            cooldown: None,
        }],
        visual: PieceVisualDefinition {
            default_asset_key: "windmill-bishop".into(),
            variants: vec![PieceVisualVariantDefinition {
                id: "rook_mode".into(),
                enabled_when: vec![PieceStatePredicate {
                    key: "mode".into(),
                    condition: PieceStateCondition::Equals(PieceStateValue::Text("rook".into())),
                }],
                asset_key: "windmill-rook".into(),
                priority: 10,
            }],
        },
    }
    .normalize_and_validate()
    .expect("windmill definition must be valid")
}
