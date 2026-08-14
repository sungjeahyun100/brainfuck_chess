use crate::types::*;

/// Paratrooper: cannot move on the board and may capture only while dropping.
pub fn paratrooper_definition() -> PieceDefinition {
    PieceDefinition {
        id: "paratrooper".into(),
        name: "공수부대 대원".into(),
        score: 3,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: true,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition {
            default_asset_key: "paratrooper".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .expect("paratrooper definition must be valid")
}
