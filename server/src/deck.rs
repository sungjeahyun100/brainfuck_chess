//! Account persistence validates safe editable content independently of game readiness.
use crate::{
    app_state::AppState, database::DataSchema, DeckPieceRef, PlayerDeckSpec, StartingPieceSpec,
};
use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use brainfuck_chess_engine::types::DeckRuleset;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

// The maximum-format regression fixture is ~104 KiB; retain a bounded 128 KiB request.
pub(crate) const MAX_DECK_BYTES: usize = 131_072;
const MAX_DECKS: i64 = 200;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeckData {
    // Preserve legacy JSON/fingerprints; Standard is stored explicitly in JSONB.
    #[serde(default, skip_serializing_if = "DeckRuleset::is_legacy")]
    ruleset: DeckRuleset,
    map_id: String,
    board_size: i32,
    starting: Vec<Placement>,
    pocket: BTreeMap<String, u32>,
    custom_pieces: Vec<CustomRef>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    piece_type: String,
    square: Position,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Position {
    file: i32,
    rank: i32,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CustomRef {
    id: String,
    version: u32,
    content_hash: String,
    exposed_piece_key: String,
}
impl CustomRef {
    fn key(&self) -> String {
        format!(
            "custom:{}:v{}:{}",
            self.id, self.version, self.exposed_piece_key
        )
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeckInput {
    name: String,
    deck_data: DeckData,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateInput {
    request_id: String,
    name: String,
    deck_data: DeckData,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateInput {
    expected_version: i64,
    name: String,
    deck_data: DeckData,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeleteInput {
    expected_version: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedDeck {
    id: String,
    name: String,
    #[serde(flatten)]
    data: DeckData,
    created_at: i64,
    updated_at: i64,
    version: i64,
}
#[derive(Debug)]
pub(crate) enum DeckError {
    NotFound,
    Conflict,
    Limit,
    Unavailable,
    Invalid(String),
    Unauthorized,
    IdentityChanged,
    Origin,
}
impl IntoResponse for DeckError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "deck_not_found", "덱을 찾을 수 없습니다.".into()),
            Self::Conflict => (StatusCode::CONFLICT, "deck_conflict", "다른 곳에서 덱이 변경되었습니다. 덱 코드를 복사해 편집 내용을 보관한 뒤 목록에서 다시 열어 주세요.".into()),
            Self::Limit => (StatusCode::CONFLICT, "deck_limit", "계정 덱은 최대 200개까지 저장할 수 있습니다.".into()),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "deck_store_unavailable", "계정 덱 저장소를 사용할 수 없습니다. 잠시 후 다시 시도해 주세요.".into()),
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, "invalid_deck", message),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "authentication_required", "로그인이 필요합니다.".into()),
            Self::IdentityChanged => (StatusCode::CONFLICT, "deck_identity_changed", "로그인 계정이 변경되었습니다. 새로고침해 주세요.".into()),
            Self::Origin => (StatusCode::FORBIDDEN, "origin_rejected", "요청 origin이 허용되지 않습니다.".into()),
        };
        (
            status,
            Json(serde_json::json!({"code":code,"error":message})),
        )
            .into_response()
    }
}
type Result<T> = std::result::Result<T, DeckError>;
pub(crate) type DeckStore = Arc<dyn DeckRepository>;
#[async_trait]
pub(crate) trait DeckRepository: Send + Sync {
    async fn list(&self, owner: &str) -> Result<Vec<SavedDeck>>;
    async fn get(&self, owner: &str, id: &str) -> Result<SavedDeck>;
    async fn create(&self, owner: &str, key: &str, input: DeckInput) -> Result<SavedDeck>;
    async fn update(
        &self,
        owner: &str,
        id: &str,
        version: i64,
        input: DeckInput,
    ) -> Result<SavedDeck>;
    async fn delete(&self, owner: &str, id: &str, version: i64) -> Result<()>;
}

// No database in local mode is an explicit unavailable state, never a successful volatile save.
pub(crate) struct PostgresDeckRepository {
    pool: Option<PgPool>,
    schema: DataSchema,
}
impl PostgresDeckRepository {
    pub(crate) fn new(pool: Option<PgPool>, schema: DataSchema) -> Self {
        Self { pool, schema }
    }
    fn pool(&self) -> Result<&PgPool> {
        self.pool.as_ref().ok_or(DeckError::Unavailable)
    }
    fn table(&self) -> String {
        self.schema.table("decks")
    }
}
fn from_row(row: sqlx::postgres::PgRow) -> Result<SavedDeck> {
    if row
        .try_get::<i32, _>("format_version")
        .map_err(|_| DeckError::Unavailable)?
        != 1
    {
        return Err(DeckError::Unavailable);
    }
    let data: serde_json::Value = row
        .try_get("deck_data")
        .map_err(|_| DeckError::Unavailable)?;
    Ok(SavedDeck {
        id: row.try_get("id").map_err(|_| DeckError::Unavailable)?,
        name: row.try_get("name").map_err(|_| DeckError::Unavailable)?,
        data: serde_json::from_value(data).map_err(|_| DeckError::Unavailable)?,
        created_at: row
            .try_get("created_at_ms")
            .map_err(|_| DeckError::Unavailable)?,
        updated_at: row
            .try_get("updated_at_ms")
            .map_err(|_| DeckError::Unavailable)?,
        version: row.try_get("version").map_err(|_| DeckError::Unavailable)?,
    })
}
fn fingerprint(input: &DeckInput) -> String {
    // Sorting is only for identity; the stored representation remains lossless.
    let mut normalized = input.clone();
    normalized
        .deck_data
        .starting
        .sort_by_key(|p| (p.square.rank, p.square.file, p.piece_type.clone()));
    normalized.deck_data.pocket.retain(|_, count| *count > 0);
    normalized
        .deck_data
        .custom_pieces
        .sort_by_key(CustomRef::key);
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&normalized).expect("typed deck is JSON"))
    )
}
fn imported_name(name: &str, existing: &[String]) -> String {
    if !existing.iter().any(|entry| entry == name) {
        return name.into();
    }
    for number in 2.. {
        let suffix = format!(" (가져옴 {number})");
        let prefix: String = name.chars().take(100 - suffix.chars().count()).collect();
        let candidate = format!("{prefix}{suffix}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}
#[async_trait]
impl DeckRepository for PostgresDeckRepository {
    async fn list(&self, owner: &str) -> Result<Vec<SavedDeck>> {
        sqlx::query(&format!(
            "SELECT * FROM {} WHERE owner_id=$1 ORDER BY updated_at_ms DESC, id",
            self.table()
        ))
        .bind(owner)
        .fetch_all(self.pool()?)
        .await
        .map_err(|_| DeckError::Unavailable)?
        .into_iter()
        .map(from_row)
        .collect()
    }
    async fn get(&self, owner: &str, id: &str) -> Result<SavedDeck> {
        from_row(
            sqlx::query(&format!(
                "SELECT * FROM {} WHERE owner_id=$1 AND id=$2",
                self.table()
            ))
            .bind(owner)
            .bind(id)
            .fetch_optional(self.pool()?)
            .await
            .map_err(|_| DeckError::Unavailable)?
            .ok_or(DeckError::NotFound)?,
        )
    }
    async fn create(&self, owner: &str, key: &str, mut input: DeckInput) -> Result<SavedDeck> {
        let mut tx = self
            .pool()?
            .begin()
            .await
            .map_err(|_| DeckError::Unavailable)?;
        // Serialize per-owner creates (including retries) so limits and import identity are atomic.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{}:decks:{owner}", self.schema.name()))
            .execute(&mut *tx)
            .await
            .map_err(|_| DeckError::Unavailable)?;
        let hash = fingerprint(&input);
        if let Some(row) = sqlx::query(&format!(
            "SELECT * FROM {} WHERE owner_id=$1 AND request_key=$2",
            self.table()
        ))
        .bind(owner)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| DeckError::Unavailable)?
        {
            let original: String = row
                .try_get("create_hash")
                .map_err(|_| DeckError::Unavailable)?;
            if original != hash {
                return Err(DeckError::Conflict);
            }
            return from_row(row);
        }
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {} WHERE owner_id=$1",
            self.table()
        ))
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| DeckError::Unavailable)?;
        if count >= MAX_DECKS {
            return Err(DeckError::Limit);
        }
        if key.starts_with("import:") {
            let names: Vec<String> = sqlx::query_scalar(&format!(
                "SELECT name FROM {} WHERE owner_id=$1",
                self.table()
            ))
            .bind(owner)
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| DeckError::Unavailable)?;
            input.name = imported_name(&input.name, &names);
        }
        let now = crate::now_ms() as i64;
        let row = sqlx::query(&format!("INSERT INTO {} (id,owner_id,name,deck_data,created_at_ms,updated_at_ms,version,request_key,create_hash) VALUES ($1,$2,$3,$4,$5,$5,1,$6,$7) RETURNING *", self.table()))
            .bind(Uuid::new_v4().to_string()).bind(owner).bind(&input.name).bind(sqlx::types::Json(&input.deck_data))
            .bind(now).bind(key).bind(hash).fetch_one(&mut *tx).await.map_err(|_| DeckError::Unavailable)?;
        let deck = from_row(row)?;
        tx.commit().await.map_err(|_| DeckError::Unavailable)?;
        Ok(deck)
    }
    async fn update(
        &self,
        owner: &str,
        id: &str,
        version: i64,
        input: DeckInput,
    ) -> Result<SavedDeck> {
        let row = sqlx::query(&format!("UPDATE {} SET name=$4,deck_data=$5,version=version+1,updated_at_ms=GREATEST(updated_at_ms+1,$6) WHERE owner_id=$1 AND id=$2 AND version=$3 RETURNING *", self.table()))
            .bind(owner).bind(id).bind(version).bind(input.name).bind(sqlx::types::Json(input.deck_data)).bind(crate::now_ms() as i64)
            .fetch_optional(self.pool()?).await.map_err(|_| DeckError::Unavailable)?;
        match row {
            Some(row) => from_row(row),
            None => {
                self.get(owner, id).await?;
                Err(DeckError::Conflict)
            }
        }
    }
    async fn delete(&self, owner: &str, id: &str, version: i64) -> Result<()> {
        let removed = sqlx::query(&format!(
            "DELETE FROM {} WHERE owner_id=$1 AND id=$2 AND version=$3",
            self.table()
        ))
        .bind(owner)
        .bind(id)
        .bind(version)
        .execute(self.pool()?)
        .await
        .map_err(|_| DeckError::Unavailable)?
        .rows_affected();
        if removed == 0 {
            self.get(owner, id).await?;
            return Err(DeckError::Conflict);
        }
        Ok(())
    }
}

async fn owner(app: &AppState, headers: &HeaderMap, write: bool) -> Result<String> {
    if write {
        app.auth
            .validate_origin(headers)
            .map_err(|_| DeckError::Origin)?;
    }
    let owner = app
        .auth
        .authenticate(headers)
        .map_err(|_| DeckError::Unauthorized)?;
    if app
        .accounts
        .authenticated_user(&owner)
        .await
        .map_err(|_| DeckError::Unavailable)?
        .is_none()
    {
        return Err(DeckError::Unauthorized);
    }
    // A request begun in another tab/account must never write through a newly changed cookie.
    if headers.get("x-deck-account").and_then(|v| v.to_str().ok())
        != Some(app.auth.account_context(&owner).as_str())
    {
        return Err(DeckError::IdentityChanged);
    }
    Ok(owner)
}
fn valid_id(id: &str) -> Result<()> {
    Uuid::parse_str(id).map_err(|_| DeckError::Invalid("덱 식별자가 올바르지 않습니다.".into()))?;
    Ok(())
}
fn invalid(message: &str) -> DeckError {
    DeckError::Invalid(message.into())
}
fn safe_piece_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
impl DeckInput {
    fn spec(&mut self) -> Result<PlayerDeckSpec> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty()
            || self.name.chars().count() > 100
            || self.name.chars().any(char::is_control)
        {
            return Err(invalid(
                "덱 이름은 1~100자여야 하며 제어 문자를 포함할 수 없습니다.",
            ));
        }
        let data = &self.deck_data;
        if !(8..=12).contains(&data.board_size)
            || data.starting.len() > 144
            || data.pocket.len() > 256
            || data.custom_pieces.len() > 256
            || serde_json::to_vec(self)
                .map_err(|_| DeckError::Unavailable)?
                .len()
                > MAX_DECK_BYTES
        {
            return Err(invalid("덱 크기가 허용 범위를 벗어났습니다."));
        }
        let mut custom = BTreeMap::new();
        for piece in &data.custom_pieces {
            if !safe_piece_id(&piece.id)
                || !safe_piece_id(&piece.exposed_piece_key)
                || piece.version == 0
                || piece.content_hash.is_empty()
                || piece.content_hash.len() > 256
                || !piece
                    .content_hash
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b':')
                || custom.insert(piece.key(), piece).is_some()
            {
                return Err(invalid("커스텀 기물 참조가 올바르지 않습니다."));
            }
        }
        let resolve = |id: &str| -> Result<DeckPieceRef> {
            if !safe_piece_id(id) {
                return Err(invalid("기물 식별자가 올바르지 않습니다."));
            }
            if let Some(piece) = custom.get(id) {
                return Ok(DeckPieceRef::Custom {
                    custom_piece_id: piece.id.clone(),
                    version: piece.version,
                    content_hash: piece.content_hash.clone(),
                    exposed_piece_key: piece.exposed_piece_key.clone(),
                });
            }
            if crate::resolve_piece_type("white", id).is_none() {
                return Err(invalid("알 수 없는 기물입니다."));
            }
            Ok(DeckPieceRef::BuiltIn {
                piece_type: id.into(),
            })
        };
        let mut used = HashSet::new();
        let mut occupied = HashSet::new();
        let mut starting = Vec::new();
        for piece in &data.starting {
            // Validate before neutral-to-black mirroring, including i32 extrema.
            if !(0..data.board_size).contains(&piece.square.file)
                || !(0..data.board_size).contains(&piece.square.rank)
            {
                return Err(invalid("시작 기물 좌표가 보드 범위를 벗어났습니다."));
            }
            if !occupied.insert((piece.square.file, piece.square.rank)) {
                return Err(invalid("같은 칸에 여러 시작 기물을 배치할 수 없습니다."));
            }
            used.insert(piece.piece_type.as_str());
            starting.push(StartingPieceSpec {
                piece: resolve(&piece.piece_type)?,
                square: crate::Square {
                    file: piece.square.file,
                    rank: piece.square.rank,
                },
            });
        }
        let mut pocket = Vec::new();
        for (id, count) in &data.pocket {
            if *count == 0 {
                if !safe_piece_id(id) {
                    return Err(invalid("기물 식별자가 올바르지 않습니다."));
                }
                continue;
            }
            let piece = resolve(id)?;
            if *count > 1024 || pocket.len() + *count as usize > 4096 {
                return Err(invalid("포켓 수량이 허용 범위를 벗어났습니다."));
            }
            if *count > 0 {
                used.insert(id.as_str());
            }
            pocket.extend(std::iter::repeat(piece).take(*count as usize));
        }
        if custom.keys().any(|key| !used.contains(key.as_str())) {
            return Err(invalid(
                "사용하지 않는 커스텀 기물 참조가 포함되어 있습니다.",
            ));
        }
        Ok(PlayerDeckSpec {
            ruleset: data.ruleset,
            name: Some(self.name.clone()),
            starting,
            pocket,
        })
    }
    async fn validate(mut self, app: &AppState, owner: &str) -> Result<Self> {
        self.spec()?;
        crate::resolve_board_map(
            Some(&self.deck_data.map_id),
            self.deck_data.board_size,
            crate::BoardVariant::Plain,
        )
        .map_err(DeckError::Invalid)?;
        // Pinned owned versions remain editable after deactivation. Game start still
        // resolves active runtime packages and enforces all gameplay rules.
        for reference in &self.deck_data.custom_pieces {
            let stored = app
                .custom_pieces
                .version(owner, &reference.id, reference.version)
                .await
                .map_err(|_| DeckError::Unavailable)?
                .ok_or_else(|| invalid("소유한 커스텀 기물 버전을 찾을 수 없습니다."))?;
            if stored.package.content_hash != reference.content_hash
                || stored.package.exposed_piece_key != reference.exposed_piece_key
            {
                return Err(invalid(
                    "커스텀 기물 참조가 저장된 버전과 일치하지 않습니다.",
                ));
            }
        }
        Ok(self)
    }
}

pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let owner = owner(&app, &headers, false).await?;
    Ok(Json(
        serde_json::json!({"items": app.decks.list(&owner).await?}),
    ))
}
pub(crate) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SavedDeck>> {
    let owner = owner(&app, &headers, false).await?;
    valid_id(&id)?;
    Ok(Json(app.decks.get(&owner, &id).await?))
}
pub(crate) async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateInput>,
) -> Result<(StatusCode, Json<SavedDeck>)> {
    let owner = owner(&app, &headers, true).await?;
    valid_id(&input.request_id)?;
    let deck = DeckInput {
        name: input.name,
        deck_data: input.deck_data,
    }
    .validate(&app, &owner)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(
            app.decks
                .create(&owner, &format!("create:{}", input.request_id), deck)
                .await?,
        ),
    ))
}
pub(crate) async fn import(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DeckInput>,
) -> Result<Json<SavedDeck>> {
    let owner = owner(&app, &headers, true).await?;
    let deck = input.validate(&app, &owner).await?;
    let key = format!("import:{}", fingerprint(&deck));
    Ok(Json(app.decks.create(&owner, &key, deck).await?))
}
pub(crate) async fn update(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateInput>,
) -> Result<Json<SavedDeck>> {
    let owner = owner(&app, &headers, true).await?;
    valid_id(&id)?;
    let existing = app.decks.get(&owner, &id).await?;
    if existing.version != input.expected_version {
        return Err(DeckError::Conflict);
    }
    let deck = DeckInput {
        name: input.name,
        deck_data: input.deck_data,
    }
    .validate(&app, &owner)
    .await?;
    Ok(Json(
        app.decks
            .update(&owner, &id, input.expected_version, deck)
            .await?,
    ))
}
pub(crate) async fn delete(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<DeleteInput>,
) -> Result<StatusCode> {
    let owner = owner(&app, &headers, true).await?;
    valid_id(&id)?;
    app.decks
        .delete(&owner, &id, input.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
pub(crate) mod tests;
