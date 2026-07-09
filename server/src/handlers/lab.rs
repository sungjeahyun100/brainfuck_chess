use std::collections::HashSet;

use axum::{http::StatusCode, Json};
use brainfuck_chess_engine::{
    catalog::PieceCatalog,
    legal_moves::{
        generate_piece_attack_squares, generate_piece_legal_move_actions_with_options,
        MoveGenerationOptions,
    },
    types::PieceId,
};

use crate::dto::error::ErrorBody as ErrorResponse;
use crate::dto::lab::{LabAbilityOption, LabPieceOptionsRequest, LabPieceOptionsResponse};
use crate::services::lab_builder::build_lab_game_state;

pub async fn piece_lab_options(
    Json(req): Json<LabPieceOptionsRequest>,
) -> Result<Json<LabPieceOptionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let state = build_lab_game_state(&req)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;
    let piece_id = PieceId::from(req.selected_piece_id.clone());
    let legal_moves = generate_piece_legal_move_actions_with_options(
        &state,
        &piece_id,
        &MoveGenerationOptions {
            ability_id: req.ability_id.clone(),
        },
    );
    let mut seen_moves = HashSet::new();
    let moves = legal_moves
        .iter()
        .map(|action| action.to)
        .filter(|square| seen_moves.insert(square.to_id()))
        .collect();
    let mut seen_attacks = HashSet::new();
    let attacks = generate_piece_attack_squares(&state, &piece_id)
        .into_iter()
        .filter(|square| seen_attacks.insert(square.to_id()))
        .collect();
    let catalog = PieceCatalog::default_catalog();
    let abilities = state
        .pieces
        .get(&piece_id)
        .and_then(|piece| catalog.get(&piece.type_id))
        .map(|definition| {
            definition
                .abilities
                .iter()
                .map(|ability| LabAbilityOption {
                    id: ability.id.clone(),
                    name: ability.name.clone(),
                    description: ability.description.clone(),
                    available: true,
                    connected: ability.id == "cannon_move",
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(LabPieceOptionsResponse {
        moves,
        legal_moves,
        attacks,
        abilities,
    }))
}
