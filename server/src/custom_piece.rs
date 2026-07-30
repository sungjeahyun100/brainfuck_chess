use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::custom_pieces::{
    install_runtime_catalog, validate_and_build_custom_piece_package, CustomPieceError,
    CustomPiecePackage, CustomPiecePackageInput, MAX_CUSTOM_SOURCE_BYTES,
};
use brainfuck_chess_engine::legal_moves::{
    generate_piece_attack_squares, generate_piece_legal_drop_actions,
    generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
};
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;

const USER_HEADER: &str = "x-user-id";
const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 2_000;
const MIN_SCORE: u32 = 1;
const MAX_SCORE: u32 = 30;
const MAX_PIECES_PER_USER: usize = 100;
const MAX_IMAGE_BYTES: usize = 512 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 2048;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
    code: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
                code: self.code,
            }),
        )
            .into_response()
    }
}

fn error(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiError {
    ApiError {
        status,
        code,
        message: message.into(),
    }
}

pub(crate) fn authenticated_owner(headers: &HeaderMap) -> Result<String, String> {
    let value = headers
        .get(USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| {
            (1..=128).contains(&value.len())
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
        .ok_or_else(|| "인증된 사용자 ID가 필요합니다.".to_string())?;
    Ok(value.to_owned())
}

fn owner(headers: &HeaderMap) -> ApiResult<String> {
    authenticated_owner(headers)
        .map_err(|message| error(StatusCode::UNAUTHORIZED, "authentication_required", message))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ImageRef {
    BuiltIn { asset_key: String },
    Uploaded { asset_id: String },
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CustomPieceRecord {
    id: String,
    owner_id: String,
    name: String,
    description: String,
    score: u32,
    image: ImageRef,
    raw_script: String,
    exposed_piece_key: String,
    internal_piece_keys: Vec<String>,
    validation_status: &'static str,
    version: u32,
    content_hash: String,
    created_at: u64,
    updated_at: u64,
    active: bool,
}

#[derive(Clone)]
pub(crate) struct StoredVersion {
    pub(crate) record: CustomPieceRecord,
    pub(crate) package: CustomPiecePackage,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ImageAsset {
    asset_id: String,
    media_type: String,
    width: u32,
    height: u32,
    content_hash: String,
}

#[derive(Clone)]
pub(crate) struct StoredImage {
    owner_id: String,
    metadata: ImageAsset,
    #[allow(dead_code)]
    bytes: Vec<u8>,
}

trait CustomPieceRepository: Send + Sync {
    fn list(&self, owner: &str) -> Vec<CustomPieceRecord>;
    fn latest(&self, owner: &str, id: &str) -> Option<StoredVersion>;
    fn version(&self, owner: &str, id: &str, version: u32) -> Option<StoredVersion>;
    fn create(&self, version: StoredVersion) -> Result<(), &'static str>;
    fn replace(&self, expected_version: u32, version: StoredVersion) -> Result<(), &'static str>;
    fn deactivate(&self, owner: &str, id: &str, expected_version: u32) -> Result<(), &'static str>;
    fn count(&self, owner: &str) -> usize;
    fn put_image(&self, image: StoredImage);
    fn owns_image(&self, owner: &str, asset_id: &str) -> bool;
}

#[derive(Default)]
pub(crate) struct InMemoryCustomPieceRepository {
    pieces: RwLock<HashMap<String, Vec<StoredVersion>>>,
    images: DashMap<String, StoredImage>,
}

impl CustomPieceRepository for InMemoryCustomPieceRepository {
    fn list(&self, owner: &str) -> Vec<CustomPieceRecord> {
        self.pieces
            .read()
            .expect("custom piece repository lock poisoned")
            .values()
            .filter_map(|versions| versions.last())
            .filter(|stored| stored.record.owner_id == owner && stored.record.active)
            .map(|stored| stored.record.clone())
            .collect()
    }

    fn latest(&self, owner: &str, id: &str) -> Option<StoredVersion> {
        self.pieces
            .read()
            .ok()?
            .get(id)?
            .last()
            .filter(|stored| stored.record.owner_id == owner && stored.record.active)
            .cloned()
    }

    fn version(&self, owner: &str, id: &str, version: u32) -> Option<StoredVersion> {
        self.pieces
            .read()
            .ok()?
            .get(id)?
            .iter()
            .find(|stored| stored.record.owner_id == owner && stored.record.version == version)
            .cloned()
    }

    fn create(&self, version: StoredVersion) -> Result<(), &'static str> {
        let mut pieces = self.pieces.write().map_err(|_| "unavailable")?;
        if pieces.contains_key(&version.record.id) {
            return Err("conflict");
        }
        pieces.insert(version.record.id.clone(), vec![version]);
        Ok(())
    }

    fn replace(&self, expected_version: u32, version: StoredVersion) -> Result<(), &'static str> {
        let mut pieces = self.pieces.write().map_err(|_| "unavailable")?;
        let versions = pieces.get_mut(&version.record.id).ok_or("not_found")?;
        let latest = versions.last().ok_or("not_found")?;
        if latest.record.owner_id != version.record.owner_id || !latest.record.active {
            return Err("not_found");
        }
        if latest.record.version != expected_version {
            return Err("conflict");
        }
        versions.push(version);
        Ok(())
    }

    fn deactivate(&self, owner: &str, id: &str, expected_version: u32) -> Result<(), &'static str> {
        let mut pieces = self.pieces.write().map_err(|_| "unavailable")?;
        let versions = pieces.get_mut(id).ok_or("not_found")?;
        let latest = versions.last_mut().ok_or("not_found")?;
        if latest.record.owner_id != owner || !latest.record.active {
            return Err("not_found");
        }
        if latest.record.version != expected_version {
            return Err("conflict");
        }
        latest.record.active = false;
        latest.record.updated_at = now();
        Ok(())
    }

    fn count(&self, owner: &str) -> usize {
        self.list(owner).len()
    }

    fn put_image(&self, image: StoredImage) {
        self.images.insert(image.metadata.asset_id.clone(), image);
    }

    fn owns_image(&self, owner: &str, asset_id: &str) -> bool {
        self.images
            .get(asset_id)
            .is_some_and(|image| image.owner_id == owner)
    }
}

impl InMemoryCustomPieceRepository {
    /// Resolves an immutable version for a new deck/game. A soft-deleted
    /// package remains available for existing game snapshots, but cannot be
    /// selected for a new game.
    pub(crate) fn resolve_active_version(
        &self,
        owner: &str,
        id: &str,
        version: u32,
    ) -> Option<StoredVersion> {
        let pieces = self.pieces.read().ok()?;
        let versions = pieces.get(id)?;
        let latest = versions.last()?;
        if latest.record.owner_id != owner || !latest.record.active {
            return None;
        }
        versions
            .iter()
            .find(|stored| stored.record.version == version)
            .cloned()
    }

    pub(crate) fn runtime_package(
        &self,
        owner: &str,
        id: &str,
        version: u32,
    ) -> Option<CustomPiecePackage> {
        let stored = self.resolve_active_version(owner, id, version)?;
        let asset_key = match &stored.record.image {
            ImageRef::BuiltIn { asset_key } => asset_key.clone(),
            ImageRef::Uploaded { asset_id } => {
                let image = self.images.get(asset_id)?;
                if image.owner_id != owner {
                    return None;
                }
                format!(
                    "data:{};base64,{}",
                    image.metadata.media_type,
                    base64_encode(&image.bytes)
                )
            }
        };
        let mut package = stored.package;
        for definition in &mut package.definitions {
            definition.visual.default_asset_key = asset_key.clone();
            definition.visual.variants.clear();
            if definition.id == package.exposed_type_id {
                definition.name = stored.record.name.clone();
            }
        }
        Some(package)
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(TABLE[((bits >> 18) & 63) as usize] as char);
        encoded.push(TABLE[((bits >> 12) & 63) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[((bits >> 6) & 63) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(bits & 63) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[derive(Deserialize, Clone)]
pub(crate) struct PieceInput {
    name: String,
    #[serde(default)]
    description: String,
    score: u32,
    image: ImageRef,
    raw_script: String,
    exposed_piece_key: String,
}

#[derive(Deserialize)]
pub(crate) struct UpdateInput {
    #[serde(flatten)]
    piece: PieceInput,
    expected_version: u32,
}

#[derive(Deserialize)]
pub(crate) struct DeleteInput {
    expected_version: u32,
}

#[derive(Serialize)]
pub(crate) struct ListResponse {
    items: Vec<CustomPieceRecord>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ValidationResponse {
    valid: bool,
    diagnostics: Vec<Diagnostic>,
    exposed_piece_key: Option<String>,
    internal_piece_keys: Vec<String>,
    preview_definitions: Vec<PieceDefinition>,
}

#[derive(Debug, Serialize)]
struct Diagnostic {
    severity: &'static str,
    code: &'static str,
    message: String,
    limit_exceeded: bool,
}

pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ListResponse>> {
    let owner = owner(&headers)?;
    Ok(Json(ListResponse {
        items: app.custom_pieces.list(&owner),
    }))
}

pub(crate) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<CustomPieceRecord>> {
    let owner = owner(&headers)?;
    app.custom_pieces
        .latest(&owner, &id)
        .map(|stored| Json(stored.record))
        .ok_or_else(not_found)
}

pub(crate) async fn get_version(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((id, version)): Path<(String, u32)>,
) -> ApiResult<Json<CustomPieceRecord>> {
    let owner = owner(&headers)?;
    app.custom_pieces
        .version(&owner, &id, version)
        .map(|stored| Json(stored.record))
        .ok_or_else(not_found)
}

pub(crate) async fn validate(
    headers: HeaderMap,
    Json(input): Json<PieceInput>,
) -> ApiResult<Json<ValidationResponse>> {
    owner(&headers)?;
    validate_metadata(&input)?;
    Ok(Json(validation_response(build_package(
        "validation",
        1,
        &input,
    ))))
}

pub(crate) async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(mut input): Json<PieceInput>,
) -> ApiResult<(StatusCode, Json<CustomPieceRecord>)> {
    let owner = owner(&headers)?;
    if app.custom_pieces.count(&owner) >= MAX_PIECES_PER_USER {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "piece_limit_exceeded",
            "사용자별 커스텀 기물 수 제한을 초과했습니다.",
        ));
    }
    normalize_and_validate(&app, &owner, &mut input)?;
    let id = Uuid::new_v4().to_string();
    let stored = make_version(id, owner, input, 1, None)?;
    let response = stored.record.clone();
    app.custom_pieces.create(stored).map_err(repository_error)?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub(crate) async fn update(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut input): Json<UpdateInput>,
) -> ApiResult<Json<CustomPieceRecord>> {
    let owner = owner(&headers)?;
    let previous = app
        .custom_pieces
        .latest(&owner, &id)
        .ok_or_else(not_found)?;
    normalize_and_validate(&app, &owner, &mut input.piece)?;
    let stored = make_version(
        id,
        owner,
        input.piece,
        previous.record.version + 1,
        Some(previous.record.created_at),
    )?;
    let response = stored.record.clone();
    app.custom_pieces
        .replace(input.expected_version, stored)
        .map_err(repository_error)?;
    Ok(Json(response))
}

pub(crate) async fn deactivate(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<DeleteInput>,
) -> ApiResult<StatusCode> {
    let owner = owner(&headers)?;
    app.custom_pieces
        .deactivate(&owner, &id, input.expected_version)
        .map_err(repository_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn normalize_and_validate(app: &AppState, owner: &str, input: &mut PieceInput) -> ApiResult<()> {
    input.name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    validate_metadata(input)?;
    validate_image_reference(app, owner, &input.image)?;
    build_package("validation", 1, input).map_err(validation_error)?;
    Ok(())
}

fn validate_metadata(input: &PieceInput) -> ApiResult<()> {
    let normalized_name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized_name.is_empty() || normalized_name.chars().count() > MAX_NAME_CHARS {
        return Err(validation("name_invalid", "이름은 1~80자여야 합니다."));
    }
    if input.description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(validation(
            "description_too_long",
            "설명은 2000자를 넘을 수 없습니다.",
        ));
    }
    if !(MIN_SCORE..=MAX_SCORE).contains(&input.score) {
        return Err(validation(
            "score_out_of_range",
            "점수는 1부터 30 사이여야 합니다.",
        ));
    }
    if input.raw_script.len() > MAX_CUSTOM_SOURCE_BYTES {
        return Err(validation(
            "source_too_long",
            "코드 길이 제한을 초과했습니다.",
        ));
    }
    if !valid_component(&input.exposed_piece_key) {
        return Err(validation(
            "exposed_piece_key_invalid",
            "대표 기물 식별자 형식이 올바르지 않습니다.",
        ));
    }
    Ok(())
}

fn validate_image_reference(app: &AppState, owner: &str, image: &ImageRef) -> ApiResult<()> {
    match image {
        ImageRef::BuiltIn { asset_key }
            if matches!(
                asset_key.as_str(),
                "pawn" | "rook" | "bishop" | "knight" | "queen" | "king"
            ) =>
        {
            Ok(())
        }
        ImageRef::Uploaded { asset_id } if app.custom_pieces.owns_image(owner, asset_id) => Ok(()),
        _ => Err(validation(
            "image_reference_invalid",
            "사용할 수 없는 이미지 참조입니다.",
        )),
    }
}

fn make_version(
    id: String,
    owner_id: String,
    input: PieceInput,
    version: u32,
    created_at: Option<u64>,
) -> ApiResult<StoredVersion> {
    let package = build_package(&id, version, &input).map_err(validation_error)?;
    let timestamp = now();
    let internal_piece_keys = package
        .definitions
        .iter()
        .filter_map(|definition| {
            definition
                .id
                .rsplit(':')
                .next()
                .filter(|key| *key != input.exposed_piece_key)
                .map(str::to_owned)
        })
        .collect();
    let record = CustomPieceRecord {
        id,
        owner_id,
        name: input.name,
        description: input.description,
        score: input.score,
        image: input.image,
        raw_script: input.raw_script,
        exposed_piece_key: input.exposed_piece_key,
        internal_piece_keys,
        validation_status: "valid",
        version,
        content_hash: package.content_hash.clone(),
        created_at: created_at.unwrap_or(timestamp),
        updated_at: timestamp,
        active: true,
    };
    Ok(StoredVersion { record, package })
}

fn build_package(
    id: &str,
    version: u32,
    input: &PieceInput,
) -> Result<CustomPiecePackage, CustomPieceError> {
    validate_and_build_custom_piece_package(CustomPiecePackageInput {
        package_id: id.to_owned(),
        version,
        expected_content_hash: None,
        raw_script: input.raw_script.clone(),
        exposed_piece_key: input.exposed_piece_key.clone(),
        score: input.score,
    })
}

fn validation_response(result: Result<CustomPiecePackage, CustomPieceError>) -> ValidationResponse {
    match result {
        Ok(package) => ValidationResponse {
            valid: true,
            diagnostics: Vec::new(),
            exposed_piece_key: Some(package.exposed_piece_key.clone()),
            internal_piece_keys: package
                .definitions
                .iter()
                .filter_map(|definition| definition.id.rsplit(':').next())
                .filter(|key| *key != package.exposed_piece_key)
                .map(str::to_owned)
                .collect(),
            preview_definitions: package.definitions,
        },
        Err(error) => ValidationResponse {
            valid: false,
            diagnostics: vec![diagnostic(&error)],
            exposed_piece_key: None,
            internal_piece_keys: Vec::new(),
            preview_definitions: Vec::new(),
        },
    }
}

fn diagnostic(error: &CustomPieceError) -> Diagnostic {
    let (code, limit_exceeded) = match error {
        CustomPieceError::ParseFailure(_) => ("chessembly_parse_error", false),
        CustomPieceError::MissingExposedPiece(_) => ("exposed_piece_missing", false),
        CustomPieceError::MissingInternalReference { .. } => ("internal_reference_missing", false),
        CustomPieceError::IdentifierCollision(_) => ("identifier_collision", false),
        CustomPieceError::ExecutionLimitExceeded(_) => ("execution_limit_exceeded", true),
        CustomPieceError::UnsupportedFeature(_) => ("unsupported_feature", false),
        _ => ("chessembly_validation_error", false),
    };
    Diagnostic {
        severity: "error",
        code,
        message: error.to_string(),
        limit_exceeded,
    }
}

fn validation_error(error_value: CustomPieceError) -> ApiError {
    let diagnostic = diagnostic(&error_value);
    error(
        StatusCode::UNPROCESSABLE_ENTITY,
        diagnostic.code,
        diagnostic.message,
    )
}

fn validation(code: &'static str, message: &'static str) -> ApiError {
    error(StatusCode::UNPROCESSABLE_ENTITY, code, message)
}

fn not_found() -> ApiError {
    error(
        StatusCode::NOT_FOUND,
        "custom_piece_not_found",
        "커스텀 기물을 찾을 수 없습니다.",
    )
}

fn repository_error(value: &'static str) -> ApiError {
    match value {
        "not_found" => not_found(),
        "conflict" => error(
            StatusCode::CONFLICT,
            "version_conflict",
            "다른 변경이 먼저 저장되었습니다.",
        ),
        _ => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "repository_unavailable",
            "저장소를 사용할 수 없습니다.",
        ),
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Deserialize)]
pub(crate) struct ImageUpload {
    filename: String,
    media_type: String,
    bytes: Vec<u8>,
}

pub(crate) async fn upload_image(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImageUpload>,
) -> ApiResult<(StatusCode, Json<ImageAsset>)> {
    let owner = owner(&headers)?;
    let (media_type, width, height) = inspect_image(&input)?;
    let metadata = ImageAsset {
        asset_id: Uuid::new_v4().to_string(),
        media_type,
        width,
        height,
        content_hash: stable_hash(&input.bytes),
    };
    app.custom_pieces.put_image(StoredImage {
        owner_id: owner,
        metadata: metadata.clone(),
        bytes: input.bytes,
    });
    Ok((StatusCode::CREATED, Json(metadata)))
}

fn inspect_image(input: &ImageUpload) -> ApiResult<(String, u32, u32)> {
    if input.bytes.is_empty() || input.bytes.len() > MAX_IMAGE_BYTES {
        return Err(validation(
            "image_size_invalid",
            "이미지는 1바이트 이상 512KiB 이하여야 합니다.",
        ));
    }
    let extension = input
        .filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (actual, width, height) = if input.bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        parse_png(&input.bytes)?
    } else if input.bytes.starts_with(&[0xff, 0xd8]) {
        parse_jpeg(&input.bytes)?
    } else if input.bytes.starts_with(b"<")
        || input
            .bytes
            .get(..5)
            .is_some_and(|bytes| bytes.eq_ignore_ascii_case(b"<?xml"))
    {
        parse_svg(&input.bytes)?
    } else {
        return Err(validation(
            "image_type_invalid",
            "지원하지 않거나 손상된 이미지입니다.",
        ));
    };
    let expected_extension = match actual {
        "image/png" => extension == "png",
        "image/jpeg" => extension == "jpg" || extension == "jpeg",
        "image/svg+xml" => extension == "svg",
        _ => false,
    };
    if input.media_type != actual || !expected_extension {
        return Err(validation(
            "image_mime_mismatch",
            "파일 내용, MIME 타입과 확장자가 일치하지 않습니다.",
        ));
    }
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(validation(
            "image_dimensions_invalid",
            "이미지 크기는 각 변 1~2048 픽셀이어야 합니다.",
        ));
    }
    Ok((actual.to_owned(), width, height))
}

fn parse_png(bytes: &[u8]) -> ApiResult<(&'static str, u32, u32)> {
    if bytes.len() < 33
        || &bytes[12..16] != b"IHDR"
        || !bytes
            .windows(12)
            .any(|window| window == b"\0\0\0\0IEND\xaeB`\x82")
    {
        return Err(validation("image_corrupt", "손상된 PNG 이미지입니다."));
    }
    Ok((
        "image/png",
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    ))
}

fn parse_jpeg(bytes: &[u8]) -> ApiResult<(&'static str, u32, u32)> {
    if !bytes.ends_with(&[0xff, 0xd9]) {
        return Err(validation("image_corrupt", "손상된 JPEG 이미지입니다."));
    }
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        if marker == 0xd8 || marker == 0xd9 {
            index += 2;
            continue;
        }
        let length = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if length < 2 || index + 2 + length > bytes.len() {
            break;
        }
        if matches!(marker, 0xc0..=0xc2) && length >= 7 {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]) as u32;
            return Ok(("image/jpeg", width, height));
        }
        index += 2 + length;
    }
    Err(validation("image_corrupt", "손상된 JPEG 이미지입니다."))
}

fn parse_svg(bytes: &[u8]) -> ApiResult<(&'static str, u32, u32)> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| validation("image_corrupt", "SVG는 올바른 UTF-8이어야 합니다."))?;
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<svg")
        || [
            "<script",
            "foreignobject",
            "<iframe",
            "<object",
            "<embed",
            "<style",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        || lower.contains("javascript:")
        || lower.contains("http:")
        || lower.contains("https:")
        || lower.contains("url(")
        || lower.contains("xlink:href")
        || contains_event_handler(&lower)
    {
        return Err(validation(
            "unsafe_svg",
            "SVG에 허용되지 않는 요소 또는 외부 참조가 있습니다.",
        ));
    }
    let view_box = attribute(&lower, "viewbox")
        .ok_or_else(|| validation("svg_viewbox_missing", "SVG viewBox가 필요합니다."))?;
    let values = view_box
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| validation("svg_viewbox_invalid", "SVG viewBox가 올바르지 않습니다."))?;
    if values.len() != 4 || values[2] <= 0.0 || values[3] <= 0.0 {
        return Err(validation(
            "svg_viewbox_invalid",
            "SVG viewBox가 올바르지 않습니다.",
        ));
    }
    Ok((
        "image/svg+xml",
        values[2].ceil() as u32,
        values[3].ceil() as u32,
    ))
}

fn contains_event_handler(text: &str) -> bool {
    text.split(|c: char| c == '<' || c == '>' || c.is_ascii_whitespace())
        .any(|token| token.starts_with("on") && token.contains('='))
}

fn attribute<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = text.find(name)? + name.len();
    let rest = text.get(start..)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    rest.get(1..)?.split(quote).next()
}

fn stable_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[derive(Clone, Deserialize)]
pub(crate) struct TestPiece {
    id: String,
    piece_key: String,
    owner: PlayerId,
    square: Square,
    #[serde(default)]
    state: HashMap<String, PieceStateValue>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct TestBoard {
    board_size: i32,
    pieces: Vec<TestPiece>,
    current_player: PlayerId,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum TestDefinition {
    Draft(PieceInput),
    Stored {
        custom_piece_id: String,
        version: u32,
    },
}

#[derive(Deserialize)]
pub(crate) struct TestOptionsRequest {
    definition: TestDefinition,
    board: TestBoard,
    selected_piece_id: String,
    #[serde(default)]
    move_option_id: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TestOptionsResponse {
    state: GameState,
    legal_moves: Vec<MoveAction>,
    legal_drops: Vec<DropAction>,
    attacks: Vec<Square>,
}

pub(crate) async fn test_options(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestOptionsRequest>,
) -> ApiResult<Json<TestOptionsResponse>> {
    let owner = owner(&headers)?;
    let package = resolve_test_package(&app, &owner, input.definition)?;
    let state = build_test_state(input.board, &package)?;
    let piece_id = PieceId::from(input.selected_piece_id);
    let piece = state
        .pieces
        .get(&piece_id)
        .ok_or_else(|| validation("piece_missing", "선택한 기물을 찾을 수 없습니다."))?;
    let (legal_moves, legal_drops, attacks) = if piece.in_pocket {
        (
            Vec::new(),
            generate_piece_legal_drop_actions(&state, &piece_id),
            Vec::new(),
        )
    } else {
        (
            generate_piece_legal_move_actions_with_options(
                &state,
                &piece_id,
                &MoveGenerationOptions {
                    move_option_id: input.move_option_id,
                },
            ),
            Vec::new(),
            generate_piece_attack_squares(&state, &piece_id),
        )
    };
    Ok(Json(TestOptionsResponse {
        state,
        legal_moves,
        legal_drops,
        attacks,
    }))
}

#[derive(Deserialize)]
pub(crate) struct TestActionRequest {
    definition: TestDefinition,
    board: TestBoard,
    action: TurnAction,
}

pub(crate) async fn test_action(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestActionRequest>,
) -> ApiResult<Json<TestOptionsResponse>> {
    let owner = owner(&headers)?;
    let package = resolve_test_package(&app, &owner, input.definition)?;
    let state = build_test_state(input.board, &package)?;
    let selected = match &input.action {
        TurnAction::Move(action) => action.piece_id.clone(),
        TurnAction::Drop(action) => action.piece_id.clone(),
    };
    let state = submit_action(state, input.action).map_err(|_| {
        validation(
            "illegal_test_action",
            "현재 테스트 상태에서 허용되지 않는 행동입니다.",
        )
    })?;
    let legal_moves = generate_piece_legal_move_actions_with_options(
        &state,
        &selected,
        &MoveGenerationOptions::default(),
    );
    let attacks = generate_piece_attack_squares(&state, &selected);
    Ok(Json(TestOptionsResponse {
        state,
        legal_moves,
        legal_drops: Vec::new(),
        attacks,
    }))
}

fn resolve_test_package(
    app: &AppState,
    owner: &str,
    definition: TestDefinition,
) -> ApiResult<CustomPiecePackage> {
    match definition {
        TestDefinition::Draft(input) => {
            validate_metadata(&input)?;
            build_package("test", 1, &input).map_err(validation_error)
        }
        TestDefinition::Stored {
            custom_piece_id,
            version,
        } => app
            .custom_pieces
            .version(owner, &custom_piece_id, version)
            .map(|stored| stored.package)
            .ok_or_else(not_found),
    }
}

fn build_test_state(board_spec: TestBoard, package: &CustomPiecePackage) -> ApiResult<GameState> {
    if !(8..=12).contains(&board_spec.board_size)
        || !matches!(board_spec.current_player.as_str(), "white" | "black")
        || board_spec.pieces.len() > 144
    {
        return Err(validation(
            "test_board_invalid",
            "테스트 보드 구조가 올바르지 않습니다.",
        ));
    }
    let mut board = create_board(board_spec.board_size);
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    let mut state = GameState {
        id: "custom-piece-test".into(),
        board: board.clone(),
        pieces: HashMap::new(),
        piece_definitions: definitions,
        custom_piece_manifest: Vec::new(),
        players: HashMap::from([
            (
                "white".into(),
                Player {
                    id: "white".into(),
                    deck: Deck {
                        player_id: "white".into(),
                        starting_pieces: Vec::new(),
                        pocket_pieces: Vec::new(),
                        score_limit: 0,
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            ),
            (
                "black".into(),
                Player {
                    id: "black".into(),
                    deck: Deck {
                        player_id: "black".into(),
                        starting_pieces: Vec::new(),
                        pocket_pieces: Vec::new(),
                        score_limit: 0,
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            ),
        ]),
        current_player: board_spec.current_player,
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        global_state: HashMap::new(),
        history: Vec::new(),
        result: None,
        chessembly_program_cache: Default::default(),
    };
    install_runtime_catalog(&mut state, std::slice::from_ref(package)).map_err(validation_error)?;
    let custom_keys = package
        .definitions
        .iter()
        .filter_map(|definition| definition.id.rsplit(':').next())
        .collect::<HashSet<_>>();
    let mut ids = HashSet::new();
    for input in board_spec.pieces {
        if !ids.insert(input.id.clone())
            || !matches!(input.owner.as_str(), "white" | "black")
            || !state.board.is_in_bounds(&input.square)
            || !state.board.is_empty(&input.square)
            || (!custom_keys.contains(input.piece_key.as_str())
                && !state.piece_definitions.contains_key(&input.piece_key))
        {
            return Err(validation(
                "test_piece_invalid",
                "테스트 기물 참조, 좌표 또는 ID가 올바르지 않습니다.",
            ));
        }
        let type_id = package
            .definitions
            .iter()
            .find(|definition| definition.id.ends_with(&format!(":{}", input.piece_key)))
            .map(|definition| definition.id.clone())
            .or_else(|| {
                state
                    .piece_definitions
                    .contains_key(&input.piece_key)
                    .then(|| input.piece_key.clone())
            })
            .ok_or_else(|| validation("test_piece_invalid", "기물 정의가 없습니다."))?;
        let definition = &state.piece_definitions[&type_id];
        let mut piece_state = definition.initial_state();
        for (key, value) in input.state {
            let schema = definition
                .state_schema
                .iter()
                .find(|schema| schema.key == key)
                .ok_or_else(|| validation("test_state_invalid", "상태 키가 올바르지 않습니다."))?;
            if std::mem::discriminant(&schema.default_value) != std::mem::discriminant(&value) {
                return Err(validation(
                    "test_state_invalid",
                    "상태 값 타입이 올바르지 않습니다.",
                ));
            }
            piece_state.insert(key, value);
        }
        let piece_id = PieceId::from(input.id.clone());
        let piece = Piece {
            id: piece_id.clone(),
            type_id,
            owner: input.owner.clone(),
            current_square: Some(input.square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            state: piece_state,
            move_option_cooldowns: HashMap::new(),
        };
        board
            .squares
            .insert(input.square.to_id(), Some(piece_id.clone()));
        state.pieces.insert(piece_id.clone(), piece);
        state
            .players
            .get_mut(&input.owner)
            .unwrap()
            .deck
            .starting_pieces
            .push(piece_id);
    }
    state.board = board;
    state.rebuild_chessembly_cache();
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use brainfuck_chess_engine::custom_pieces::CUSTOM_PIECE_SCRIPT_FORMAT;
    use serde_json::json;

    fn headers(user: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_HEADER, HeaderValue::from_str(user).unwrap());
        headers
    }

    fn source(code: &str) -> String {
        let mut definition = all_default_definitions()
            .into_iter()
            .find(|definition| definition.id == "knight")
            .unwrap();
        definition.id = "hero".into();
        definition.name = "Hero".into();
        definition.is_king = false;
        definition.chessembly_code = code.into();
        definition.move_layers.clear();
        definition.move_options.clear();
        serde_json::to_string(&json!({
            "format": CUSTOM_PIECE_SCRIPT_FORMAT,
            "definitions": [definition],
        }))
        .unwrap()
    }

    fn input(code: &str) -> PieceInput {
        PieceInput {
            name: "  Test   Hero ".into(),
            description: "description".into(),
            score: 7,
            image: ImageRef::BuiltIn {
                asset_key: "knight".into(),
            },
            raw_script: source(code),
            exposed_piece_key: "hero".into(),
        }
    }

    #[tokio::test]
    async fn authenticated_owner_can_create_list_read_update_and_deactivate() {
        let app = AppState::in_memory();
        let (_, Json(created)) = create(
            State(app.clone()),
            headers("alice"),
            Json(input("move(1, 0);")),
        )
        .await
        .unwrap();
        assert_eq!(created.name, "Test Hero");
        assert_eq!(created.version, 1);

        let Json(listed) = list(State(app.clone()), headers("alice")).await.unwrap();
        assert_eq!(listed.items.len(), 1);
        let Json(found) = get(
            State(app.clone()),
            headers("alice"),
            Path(created.id.clone()),
        )
        .await
        .unwrap();
        assert_eq!(found.raw_script, created.raw_script);

        let Json(updated) = update(
            State(app.clone()),
            headers("alice"),
            Path(created.id.clone()),
            Json(UpdateInput {
                piece: input("move(0, 1);"),
                expected_version: 1,
            }),
        )
        .await
        .unwrap();
        assert_eq!(updated.version, 2);
        assert_ne!(updated.content_hash, created.content_hash);
        assert!(get_version(
            State(app.clone()),
            headers("alice"),
            Path((created.id.clone(), 1)),
        )
        .await
        .is_ok());

        deactivate(
            State(app.clone()),
            headers("alice"),
            Path(created.id.clone()),
            Json(DeleteInput {
                expected_version: 2,
            }),
        )
        .await
        .unwrap();
        assert!(get(State(app), headers("alice"), Path(created.id))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn pinned_custom_deck_builds_game_catalog_and_uses_server_score() {
        let app = AppState::in_memory();
        let (_, Json(created)) = create(
            State(app.clone()),
            headers("alice"),
            Json(input("move(1, 0);")),
        )
        .await
        .unwrap();
        let custom_ref = crate::DeckPieceRef::Custom {
            custom_piece_id: created.id.clone(),
            version: created.version,
            content_hash: created.content_hash.clone(),
            exposed_piece_key: created.exposed_piece_key.clone(),
        };
        let white = crate::PlayerDeckSpec {
            starting: vec![
                crate::StartingPieceSpec {
                    piece: crate::DeckPieceRef::BuiltIn {
                        piece_type: "king".into(),
                    },
                    square: Square::new(4, 0),
                },
                crate::StartingPieceSpec {
                    piece: custom_ref.clone(),
                    square: Square::new(2, 1),
                },
            ],
            pocket: vec![custom_ref],
        };
        let black = crate::PlayerDeckSpec {
            starting: vec![crate::StartingPieceSpec {
                piece: crate::DeckPieceRef::BuiltIn {
                    piece_type: "king".into(),
                },
                square: Square::new(4, 7),
            }],
            pocket: vec![],
        };
        let packages =
            crate::resolve_custom_packages(&app, &[("alice", &white), ("alice", &black)]).unwrap();
        let state =
            crate::build_game_state("custom-game".into(), 8, &white, &black, packages).unwrap();

        assert_eq!(state.custom_piece_manifest.len(), 1);
        let manifest = &state.custom_piece_manifest[0];
        assert_eq!(manifest.package_id, created.id);
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.content_hash, created.content_hash);
        assert!(state
            .piece_definitions
            .contains_key(&manifest.exposed_type_id));
        let custom_board_piece = state
            .pieces
            .values()
            .find(|piece| piece.current_square == Some(Square::new(2, 1)))
            .unwrap();
        assert!(!generate_piece_legal_move_actions_with_options(
            &state,
            &custom_board_piece.id,
            &MoveGenerationOptions::default(),
        )
        .is_empty());
        assert_eq!(state.players["white"].deck.total_score, 14);
        assert!(crate::resolve_custom_packages(&app, &[("mallory", &white)]).is_err());

        let mut changed = input("move(0, 1);");
        changed.score = 11;
        let _ = update(
            State(app.clone()),
            headers("alice"),
            Path(created.id.clone()),
            Json(UpdateInput {
                piece: changed,
                expected_version: 1,
            }),
        )
        .await
        .unwrap();
        let pinned = crate::resolve_custom_packages(&app, &[("alice", &white)]).unwrap();
        assert_eq!(pinned[0].version, 1);
        assert_eq!(state.players["white"].deck.total_score, 14);

        let mut room = crate::MultiplayerRoom {
            id: "ROOM01".into(),
            board_size: 8,
            host_side: "white".into(),
            guest_side: "black".into(),
            host_client_id: "host-client".into(),
            guest_client_id: Some("guest-client".into()),
            host_owner_id: "alice".into(),
            guest_owner_id: Some("alice".into()),
            host_deck: Some(white),
            guest_deck: Some(crate::PlayerDeckSpec {
                starting: vec![crate::StartingPieceSpec {
                    piece: crate::DeckPieceRef::BuiltIn {
                        piece_type: "king".into(),
                    },
                    square: Square::new(4, 0),
                }],
                pocket: vec![],
            }),
            host_ready: true,
            guest_ready: true,
            game_id: None,
        };
        let room_game = crate::start_room_game(&mut room, &app).unwrap().unwrap();
        assert_eq!(room_game.state.custom_piece_manifest.len(), 1);
        assert_eq!(room_game.state.custom_piece_manifest[0].version, 1);

        deactivate(
            State(app.clone()),
            headers("alice"),
            Path(created.id.clone()),
            Json(DeleteInput {
                expected_version: 2,
            }),
        )
        .await
        .unwrap();
        assert!(crate::resolve_custom_packages(
            &app,
            &[("alice", room.host_deck.as_ref().unwrap())]
        )
        .is_err());
        assert_eq!(room_game.state.custom_piece_manifest[0].version, 1);
    }

    #[tokio::test]
    async fn ownership_is_enforced_for_all_record_and_version_reads_and_writes() {
        let app = AppState::in_memory();
        let (_, Json(created)) = create(
            State(app.clone()),
            headers("alice"),
            Json(input("move(1, 0);")),
        )
        .await
        .unwrap();
        assert!(get(
            State(app.clone()),
            headers("mallory"),
            Path(created.id.clone())
        )
        .await
        .is_err());
        assert!(get_version(
            State(app.clone()),
            headers("mallory"),
            Path((created.id.clone(), 1))
        )
        .await
        .is_err());
        assert!(update(
            State(app.clone()),
            headers("mallory"),
            Path(created.id.clone()),
            Json(UpdateInput {
                piece: input("move(0, 1);"),
                expected_version: 1,
            })
        )
        .await
        .is_err());
        assert!(deactivate(
            State(app),
            headers("mallory"),
            Path(created.id),
            Json(DeleteInput {
                expected_version: 1
            })
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn validation_reports_parse_missing_exposed_and_source_limits() {
        let mut invalid = input("move(1, 0);");
        invalid.raw_script = "{".into();
        let Json(response) = validate(headers("alice"), Json(invalid)).await.unwrap();
        assert!(!response.valid);
        assert_eq!(response.diagnostics[0].code, "chessembly_parse_error");

        let mut missing = input("move(1, 0);");
        missing.exposed_piece_key = "missing".into();
        let Json(response) = validate(headers("alice"), Json(missing)).await.unwrap();
        assert!(!response.valid);
        assert_eq!(response.diagnostics[0].code, "exposed_piece_missing");

        let mut long = input("move(1, 0);");
        long.raw_script = "x".repeat(MAX_CUSTOM_SOURCE_BYTES + 1);
        let error = validate(headers("alice"), Json(long)).await.unwrap_err();
        assert_eq!(error.code, "source_too_long");
    }

    #[tokio::test]
    async fn create_revalidates_and_version_conflicts_are_rejected() {
        let app = AppState::in_memory();
        let mut invalid = input("move(1, 0);");
        invalid.raw_script = "{}".into();
        assert!(create(State(app.clone()), headers("alice"), Json(invalid))
            .await
            .is_err());
        let (_, Json(created)) = create(
            State(app.clone()),
            headers("alice"),
            Json(input("move(1,0);")),
        )
        .await
        .unwrap();
        let error = update(
            State(app),
            headers("alice"),
            Path(created.id),
            Json(UpdateInput {
                piece: input("move(0,1);"),
                expected_version: 9,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
    }

    #[test]
    fn image_validation_accepts_safe_svg_and_rejects_mime_script_and_paths() {
        let safe = ImageUpload {
            filename: "../../piece.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: br#"<svg viewBox="0 0 64 64"><path d="M0 0"/></svg>"#.to_vec(),
        };
        assert_eq!(inspect_image(&safe).unwrap().1, 64);
        let mut wrong_mime = safe;
        wrong_mime.media_type = "image/png".into();
        assert_eq!(
            inspect_image(&wrong_mime).unwrap_err().code,
            "image_mime_mismatch"
        );
        let unsafe_svg = ImageUpload {
            filename: "piece.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: br#"<svg viewBox="0 0 64 64" onload="x"><script>x</script></svg>"#.to_vec(),
        };
        assert_eq!(inspect_image(&unsafe_svg).unwrap_err().code, "unsafe_svg");
        let external_svg = ImageUpload {
            filename: "piece.svg".into(),
            media_type: "image/svg+xml".into(),
            bytes: br#"<svg viewBox="0 0 64 64"><image href="https://example.test/x"/></svg>"#
                .to_vec(),
        };
        assert_eq!(inspect_image(&external_svg).unwrap_err().code, "unsafe_svg");
    }

    #[test]
    fn image_validation_accepts_structural_png_and_jpeg_and_rejects_corruption() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&64u32.to_be_bytes());
        png.extend_from_slice(&32u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        png.extend_from_slice(b"\0\0\0\0IEND\xaeB`\x82");
        assert_eq!(
            inspect_image(&ImageUpload {
                filename: "piece.png".into(),
                media_type: "image/png".into(),
                bytes: png.clone(),
            })
            .unwrap(),
            ("image/png".into(), 64, 32)
        );
        png.truncate(30);
        assert_eq!(
            inspect_image(&ImageUpload {
                filename: "piece.png".into(),
                media_type: "image/png".into(),
                bytes: png,
            })
            .unwrap_err()
            .code,
            "image_corrupt"
        );

        let jpeg = vec![0xff, 0xd8, 0xff, 0xc0, 0, 7, 8, 0, 32, 0, 64, 0xff, 0xd9];
        assert_eq!(
            inspect_image(&ImageUpload {
                filename: "piece.jpeg".into(),
                media_type: "image/jpeg".into(),
                bytes: jpeg,
            })
            .unwrap(),
            ("image/jpeg".into(), 64, 32)
        );
    }

    #[tokio::test]
    async fn test_options_are_preview_only_and_legal_action_is_applied() {
        let app = AppState::in_memory();
        let board = TestBoard {
            board_size: 8,
            current_player: "white".into(),
            pieces: vec![
                TestPiece {
                    id: "hero-1".into(),
                    piece_key: "hero".into(),
                    owner: "white".into(),
                    square: Square::new(3, 3),
                    state: HashMap::new(),
                },
                TestPiece {
                    id: "official-rook".into(),
                    piece_key: "rook".into(),
                    owner: "black".into(),
                    square: Square::new(7, 7),
                    state: HashMap::new(),
                },
            ],
        };
        let Json(preview) = test_options(
            State(app.clone()),
            headers("alice"),
            Json(TestOptionsRequest {
                definition: TestDefinition::Draft(input("move(1, 0);")),
                board: board.clone(),
                selected_piece_id: "hero-1".into(),
                move_option_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            preview.state.pieces[&PieceId::from("hero-1")].current_square,
            Some(Square::new(3, 3))
        );
        assert_eq!(
            preview.state.pieces[&PieceId::from("official-rook")].type_id,
            "rook"
        );
        let action = preview.legal_moves[0].clone();
        let Json(applied) = test_action(
            State(app),
            headers("alice"),
            Json(TestActionRequest {
                definition: TestDefinition::Draft(input("move(1, 0);")),
                board,
                action: TurnAction::Move(action.clone()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            applied.state.pieces[&PieceId::from("hero-1")].current_square,
            Some(action.to)
        );

        let malformed = TestBoard {
            board_size: 8,
            current_player: "white".into(),
            pieces: vec![TestPiece {
                id: "bad".into(),
                piece_key: "missing".into(),
                owner: "white".into(),
                square: Square::new(99, 99),
                state: HashMap::new(),
            }],
        };
        assert!(test_options(
            State(AppState::in_memory()),
            headers("alice"),
            Json(TestOptionsRequest {
                definition: TestDefinition::Draft(input("move(1, 0);")),
                board: malformed,
                selected_piece_id: "bad".into(),
                move_option_id: None,
            }),
        )
        .await
        .is_err());
    }
}
