use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::custom_piece;

#[derive(Serialize)]
pub(crate) struct CustomPieceImageAssetResponse {
    pub(crate) asset_key: String,
}

#[derive(Serialize)]
pub(crate) struct CustomPieceImageAssetError {
    error: String,
    code: &'static str,
}

pub(crate) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((id, version)): Path<(String, u32)>,
) -> Result<Json<CustomPieceImageAssetResponse>, (StatusCode, Json<CustomPieceImageAssetError>)> {
    let owner = custom_piece::authenticated_owner(&headers).map_err(|message| {
        (
            StatusCode::UNAUTHORIZED,
            Json(CustomPieceImageAssetError {
                error: message,
                code: "authentication_required",
            }),
        )
    })?;
    let package = app
        .custom_pieces
        .runtime_package(&owner, &id, version)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(CustomPieceImageAssetError {
                    error: "커스텀 기물 이미지를 찾을 수 없습니다.".into(),
                    code: "custom_piece_image_not_found",
                }),
            )
        })?;
    let definition = package
        .definitions
        .iter()
        .find(|definition| definition.id == package.exposed_type_id)
        .ok_or_else(|| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(CustomPieceImageAssetError {
                    error: "대표 커스텀 기물 정의에 이미지가 없습니다.".into(),
                    code: "custom_piece_image_missing",
                }),
            )
        })?;

    Ok(Json(CustomPieceImageAssetResponse {
        asset_key: definition.visual.default_asset_key.clone(),
    }))
}
