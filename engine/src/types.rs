use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::chessembly::ast::Program;
use crate::chessembly::parser::parse;
use crate::custom_pieces::CustomPieceManifestEntry;

// ─── Primitive ID types ─────────────────────────────────────────────────────

pub type PlayerId = String;
pub type PieceTypeId = String;
pub type TerrainTypeId = String;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoardVariant {
    #[default]
    Plain,
    CentralHighGround,
}

/// Stable external piece id with allocation-free clones inside the engine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(Arc<str>);

impl PieceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PieceId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for PieceId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl Borrow<str> for PieceId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PieceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl PartialEq<str> for PieceId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for PieceId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for PieceId {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

/// Compact, allocation-free square key used by engine maps and sets.
///
/// Its serde representation remains `"file_rank"` so existing board and API
/// JSON stays compatible while the engine no longer allocates square strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SquareId {
    pub file: i32,
    pub rank: i32,
}

impl SquareId {
    pub const fn new(file: i32, rank: i32) -> Self {
        Self { file, rank }
    }

    pub const fn to_square(self) -> Square {
        Square::new(self.file, self.rank)
    }
}

impl fmt::Display for SquareId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}_{}", self.file, self.rank)
    }
}

impl Serialize for SquareId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SquareId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let (file, rank) = value
            .split_once('_')
            .ok_or_else(|| de::Error::custom("square id must be `file_rank`"))?;
        Ok(Self::new(
            file.parse().map_err(de::Error::custom)?,
            rank.parse().map_err(de::Error::custom)?,
        ))
    }
}

// ─── Square ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Square {
    pub file: i32,
    pub rank: i32,
}

impl Square {
    pub const fn new(file: i32, rank: i32) -> Self {
        Self { file, rank }
    }

    pub const fn to_id(&self) -> SquareId {
        SquareId::new(self.file, self.rank)
    }
}

// ─── Board ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub size: i32,
    /// Maps SquareId → PieceId (None means empty)
    pub squares: HashMap<SquareId, Option<PieceId>>,
    /// Independent air-layer occupancy. Legacy states omit this field.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub air_squares: HashMap<SquareId, Option<PieceId>>,
    /// Static terrain attached to squares independently from piece occupancy.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub terrain: HashMap<SquareId, TerrainCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainCell {
    pub type_id: TerrainTypeId,
}

impl Board {
    pub fn is_in_bounds(&self, sq: &Square) -> bool {
        sq.file >= 0 && sq.file < self.size && sq.rank >= 0 && sq.rank < self.size
    }

    pub fn get_piece_at(&self, sq: &Square) -> Option<&PieceId> {
        self.squares.get(&sq.to_id())?.as_ref()
    }

    pub fn is_empty(&self, sq: &Square) -> bool {
        self.get_piece_at(sq).is_none()
    }

    pub fn get_piece_at_layer(&self, sq: &Square, layer: PieceLayer) -> Option<&PieceId> {
        match layer {
            PieceLayer::Ground => self.get_piece_at(sq),
            PieceLayer::Air => self.air_squares.get(&sq.to_id())?.as_ref(),
        }
    }

    pub fn is_empty_at_layer(&self, sq: &Square, layer: PieceLayer) -> bool {
        self.get_piece_at_layer(sq, layer).is_none()
    }

    pub fn set_piece_at_layer(&mut self, sq: Square, layer: PieceLayer, piece_id: Option<PieceId>) {
        match layer {
            PieceLayer::Ground => {
                self.squares.insert(sq.to_id(), piece_id);
            }
            PieceLayer::Air => {
                self.air_squares.insert(sq.to_id(), piece_id);
            }
        }
    }
}

// ─── PieceDefinition ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceDefinition {
    pub id: PieceTypeId,
    pub name: String,
    /// Point cost for deck building (King is excluded from scoring)
    pub score: u32,
    /// Material value used by the AI while this piece is on the board.
    /// Missing values fall back to `score` for legacy and custom definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_board_value: Option<u32>,
    /// Material value used by the AI while this piece is in a pocket.
    /// Missing values fall back to `score` for legacy and custom definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_pocket_value: Option<u32>,
    /// Maximum ammunition per concrete instance. Zero means no ammo resource.
    #[serde(default)]
    pub max_ammo: u32,
    /// Setup rank reserved for this piece before the game begins.
    #[serde(default)]
    pub deployment_zone: DeploymentZone,
    pub chessembly_code: String,
    pub chessembly_version: String,
    pub dialect: Option<ChessemblyDialect>,
    pub extensions: Option<Vec<String>>,
    /// If true, capturing this piece ends the game immediately
    pub is_king: bool,
    /// Whether this piece may capture an enemy by dropping onto its square.
    #[serde(default)]
    pub can_capture_on_drop: bool,
    /// Optional rule that decides when this piece may promote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<PromotionRule>,
    /// Piece types this piece may promote into when its promotion rule matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotion_pool: Vec<PieceTypeId>,
    /// Per-instance state schema. New pieces are initialized from these values.
    #[serde(default)]
    pub state_schema: Vec<PieceStateDefinition>,
    /// Independent Chessembly programs used by move options.
    #[serde(default)]
    pub move_layers: Vec<MoveLayerDefinition>,
    /// Player-selectable normal and special movement choices.
    #[serde(default)]
    pub move_options: Vec<MoveOptionDefinition>,
    /// Logical asset selection derived from per-piece state.
    #[serde(default)]
    pub visual: PieceVisualDefinition,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentZone {
    Front,
    #[default]
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PieceStateValue {
    Integer(i32),
    Boolean(bool),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceStateValueType {
    Integer,
    Boolean,
    Text,
}

impl PieceStateValue {
    pub const fn value_type(&self) -> PieceStateValueType {
        match self {
            Self::Integer(_) => PieceStateValueType::Integer,
            Self::Boolean(_) => PieceStateValueType::Boolean,
            Self::Text(_) => PieceStateValueType::Text,
        }
    }
}

impl fmt::Display for PieceStateValueType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Text => "text",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceStateDefinition {
    pub key: String,
    pub default_value: PieceStateValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceStatePredicate {
    pub key: String,
    pub condition: PieceStateCondition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceStateCondition {
    Equals(PieceStateValue),
    NotEquals(PieceStateValue),
}

impl PieceStatePredicate {
    pub fn matches(&self, state: &HashMap<String, PieceStateValue>) -> bool {
        let value = state.get(&self.key);
        match &self.condition {
            PieceStateCondition::Equals(expected) => value == Some(expected),
            PieceStateCondition::NotEquals(expected) => {
                value.is_some_and(|value| value != expected)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceStateUpdateDefinition {
    pub key: String,
    pub value: PieceStateValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveLayerDefinition {
    pub id: String,
    pub chessembly_code: String,
    #[serde(default)]
    pub enabled_when: Vec<PieceStatePredicate>,
    #[serde(default)]
    pub on_commit: Vec<PieceStateUpdateDefinition>,
}

impl MoveLayerDefinition {
    pub fn is_enabled_for(&self, piece: &Piece) -> bool {
        self.enabled_when
            .iter()
            .all(|predicate| predicate.matches(&piece.state))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveOptionKind {
    Normal,
    Ability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveOptionExecutionMode {
    MoveModifier,
    StandaloneAction,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CooldownClock {
    #[default]
    OwnerTurns,
    GlobalTurns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CooldownDefinition {
    pub turns: u32,
    #[serde(default)]
    pub clock: CooldownClock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveOptionDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub kind: MoveOptionKind,
    pub layer_ids: Vec<String>,
    pub execution_mode: MoveOptionExecutionMode,
    #[serde(default = "default_true")]
    pub contributes_to_attack_map: bool,
    /// Ammunition consumed when this option is committed.
    #[serde(default)]
    pub ammo_cost: u32,
    /// Runtime state predicates controlling option availability.
    #[serde(default)]
    pub enabled_when: Vec<PieceStatePredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<CooldownDefinition>,
}

impl MoveOptionDefinition {
    pub fn is_enabled_for(&self, piece: &Piece) -> bool {
        self.enabled_when
            .iter()
            .all(|predicate| predicate.matches(&piece.state))
            && piece.current_ammo >= self.ammo_cost
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CooldownState {
    pub remaining: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceVisualDefinition {
    pub default_asset_key: String,
    #[serde(default)]
    pub variants: Vec<PieceVisualVariantDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceVisualVariantDefinition {
    pub id: String,
    #[serde(default)]
    pub enabled_when: Vec<PieceStatePredicate>,
    pub asset_key: String,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRule {
    pub condition: PromotionCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromotionCondition {
    FirstRank,
    LastRank,
    Rank { rank: i32 },
}

/// Move generation dispatch seam. Native implementations can be enabled per
/// definition without changing callers; custom pieces remain Chessembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovegenBackend {
    Native,
    Chessembly,
}

impl PieceDefinition {
    pub fn ai_board_value(&self) -> u32 {
        self.ai_board_value.unwrap_or(self.score)
    }

    pub fn ai_pocket_value(&self) -> u32 {
        self.ai_pocket_value.unwrap_or(self.score)
    }

    pub fn movegen_backend(&self) -> MovegenBackend {
        // The first optimization pass keeps behavior identical. Native
        // backends can be introduced piece-by-piece with parity tests.
        MovegenBackend::Chessembly
    }

    pub fn promotion_options_for_rank(&self, rank: i32, board_size: i32) -> Option<&[PieceTypeId]> {
        let rule = self.promotion.as_ref()?;
        if self.promotion_pool.is_empty() || !rule.condition.matches_rank(rank, board_size) {
            return None;
        }
        Some(self.promotion_pool.as_slice())
    }

    /// Canonicalizes definitions and validates every string reference.
    pub fn normalize_and_validate(mut self) -> Result<Self, String> {
        if self.move_layers.is_empty() {
            self.move_layers.push(MoveLayerDefinition {
                id: "default".into(),
                chessembly_code: self.chessembly_code.clone(),
                enabled_when: Vec::new(),
                on_commit: Vec::new(),
            });
        }
        if self.move_options.is_empty() {
            let default_layer_ids = if self.move_layers.iter().any(|layer| layer.id == "default") {
                vec!["default".into()]
            } else {
                self.move_layers
                    .iter()
                    .map(|layer| layer.id.clone())
                    .collect()
            };
            self.move_options.push(MoveOptionDefinition {
                id: "normal".into(),
                name: "Normal move".into(),
                description: String::new(),
                kind: MoveOptionKind::Normal,
                layer_ids: default_layer_ids,
                execution_mode: MoveOptionExecutionMode::MoveModifier,
                contributes_to_attack_map: true,
                ammo_cost: 0,
                enabled_when: Vec::new(),
                cooldown: None,
            });
        }

        if self.visual.default_asset_key.is_empty() {
            self.visual.default_asset_key = self.id.clone();
        }
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        fn duplicate(mut values: impl Iterator<Item = String>) -> Option<String> {
            let mut seen = HashSet::new();
            values.find(|value| !seen.insert(value.clone()))
        }

        if let Some(key) = duplicate(self.state_schema.iter().map(|state| state.key.clone())) {
            return Err(format!("{}: duplicate state key `{key}`", self.id));
        }
        if let Some(id) = duplicate(self.move_layers.iter().map(|layer| layer.id.clone())) {
            return Err(format!("{}: duplicate move layer id `{id}`", self.id));
        }
        if let Some(id) = duplicate(self.move_options.iter().map(|option| option.id.clone())) {
            return Err(format!("{}: duplicate move option id `{id}`", self.id));
        }
        if let Some(id) = duplicate(
            self.visual
                .variants
                .iter()
                .map(|variant| variant.id.clone()),
        ) {
            return Err(format!("{}: duplicate visual variant id `{id}`", self.id));
        }
        if self.visual.default_asset_key.trim().is_empty() {
            return Err(format!("{}: visual asset key is empty", self.id));
        }

        let state_keys: HashSet<_> = self
            .state_schema
            .iter()
            .map(|state| state.key.as_str())
            .collect();
        let layer_ids: HashSet<_> = self
            .move_layers
            .iter()
            .map(|layer| layer.id.as_str())
            .collect();
        let schema = self
            .state_schema
            .iter()
            .map(|state| (state.key.as_str(), state.default_value.value_type()))
            .collect::<HashMap<_, _>>();
        let validate_predicates = |owner: &str, predicates: &[PieceStatePredicate]| {
            predicates.iter().find_map(|predicate| {
                let Some(expected) = schema.get(predicate.key.as_str()) else {
                    return Some(format!(
                        "{}: {owner} references unknown state key `{}`",
                        self.id, predicate.key
                    ));
                };
                let actual = match &predicate.condition {
                    PieceStateCondition::Equals(value) | PieceStateCondition::NotEquals(value) => {
                        value.value_type()
                    }
                };
                (*expected != actual).then(|| {
                    format!(
                        "{}: {owner} predicate for state `{}` expects {} but received {}",
                        self.id, predicate.key, expected, actual
                    )
                })
            })
        };

        for layer in &self.move_layers {
            if let Some(error) =
                validate_predicates(&format!("layer `{}`", layer.id), &layer.enabled_when)
            {
                return Err(error);
            }
            if let Some(update) = layer
                .on_commit
                .iter()
                .find(|update| !state_keys.contains(update.key.as_str()))
            {
                return Err(format!(
                    "{}: layer `{}` updates unknown state key `{}`",
                    self.id, layer.id, update.key
                ));
            }
            if let Some(update) = layer.on_commit.iter().find(|update| {
                schema
                    .get(update.key.as_str())
                    .is_some_and(|expected| *expected != update.value.value_type())
            }) {
                return Err(format!(
                    "{}: layer `{}` update for state `{}` expects {} but received {}",
                    self.id,
                    layer.id,
                    update.key,
                    schema[update.key.as_str()],
                    update.value.value_type()
                ));
            }
        }
        for option in &self.move_options {
            if let Some(error) = validate_predicates(
                &format!("move option `{}`", option.id),
                &option.enabled_when,
            ) {
                return Err(error);
            }
            if option.layer_ids.is_empty()
                && option.execution_mode == MoveOptionExecutionMode::MoveModifier
            {
                return Err(format!(
                    "{}: move option `{}` has no layers",
                    self.id, option.id
                ));
            }
            if let Some(layer_id) = option
                .layer_ids
                .iter()
                .find(|layer_id| !layer_ids.contains(layer_id.as_str()))
            {
                return Err(format!(
                    "{}: move option `{}` references unknown layer `{layer_id}`",
                    self.id, option.id
                ));
            }
        }
        for variant in &self.visual.variants {
            if variant.asset_key.trim().is_empty() {
                return Err(format!(
                    "{}: visual variant `{}` has an empty asset key",
                    self.id, variant.id
                ));
            }
            if let Some(error) = validate_predicates(
                &format!("visual variant `{}`", variant.id),
                &variant.enabled_when,
            ) {
                return Err(error);
            }
        }
        if let Some(priority) = duplicate(
            self.visual
                .variants
                .iter()
                .map(|variant| variant.priority.to_string()),
        ) {
            return Err(format!(
                "{}: duplicate visual variant priority `{priority}`",
                self.id
            ));
        }
        Ok(())
    }

    pub fn initial_state(&self) -> HashMap<String, PieceStateValue> {
        self.state_schema
            .iter()
            .map(|definition| (definition.key.clone(), definition.default_value.clone()))
            .collect()
    }

    pub fn normal_move_option(&self) -> Option<&MoveOptionDefinition> {
        self.move_options
            .iter()
            .find(|option| option.kind == MoveOptionKind::Normal)
    }

    pub fn resolve_asset_key<'a>(&'a self, piece: &'a Piece) -> &'a str {
        self.visual
            .variants
            .iter()
            .filter(|variant| {
                variant
                    .enabled_when
                    .iter()
                    .all(|predicate| predicate.matches(&piece.state))
            })
            .max_by_key(|variant| variant.priority)
            .map(|variant| variant.asset_key.as_str())
            .unwrap_or(self.visual.default_asset_key.as_str())
    }
}

impl PromotionCondition {
    pub fn matches_rank(&self, rank: i32, board_size: i32) -> bool {
        match self {
            PromotionCondition::FirstRank => rank == 0,
            PromotionCondition::LastRank => rank == board_size - 1,
            PromotionCondition::Rank { rank: target_rank } => rank == *target_rank,
        }
    }
}

// ─── Chessembly Program Cache ───────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CachedChessemblyProgram {
    // Kept beside the compiled program so a game-local definition replacement
    // invalidates only the accessed layer, without scanning the whole catalog.
    source: String,
    program: Arc<Program>,
}

#[derive(Debug, Default)]
pub struct ChessemblyProgramCache {
    programs: RwLock<HashMap<String, CachedChessemblyProgram>>,
}

impl Clone for ChessemblyProgramCache {
    fn clone(&self) -> Self {
        Self {
            programs: RwLock::new(self.read_programs().clone()),
        }
    }
}

impl ChessemblyProgramCache {
    pub fn from_definitions(definitions: &HashMap<PieceTypeId, PieceDefinition>) -> Self {
        let cache = Self::default();
        cache.rebuild(definitions);
        cache
    }

    pub fn rebuild(&self, definitions: &HashMap<PieceTypeId, PieceDefinition>) {
        crate::profiling::record_cache_rebuild(1);
        let mut programs = HashMap::new();
        for (type_id, definition) in definitions {
            for layer in &definition.move_layers {
                programs.insert(
                    Self::layer_key(type_id, &layer.id),
                    CachedChessemblyProgram {
                        source: layer.chessembly_code.clone(),
                        program: Arc::new(parse(&layer.chessembly_code)),
                    },
                );
            }
        }
        *self.write_programs() = programs;
    }

    pub fn get(&self, type_id: &PieceTypeId, source: &str) -> Option<Arc<Program>> {
        self.get_layer(type_id, "default", source)
    }

    pub fn get_layer(
        &self,
        type_id: &PieceTypeId,
        layer_id: &str,
        source: &str,
    ) -> Option<Arc<Program>> {
        crate::profiling::record_cache_check(1);
        self.read_programs()
            .get(&Self::layer_key(type_id, layer_id))
            .filter(|cached| cached.source == source)
            .map(|cached| cached.program.clone())
    }

    pub fn get_or_parse(
        &self,
        type_id: &PieceTypeId,
        definition: &PieceDefinition,
    ) -> Arc<Program> {
        if let Some(program) = self.get(type_id, &definition.chessembly_code) {
            return program;
        }

        let program = Arc::new(parse(&definition.chessembly_code));
        let mut programs = self.write_programs();
        programs
            .entry(Self::layer_key(type_id, "default"))
            .and_modify(|cached| {
                if cached.source != definition.chessembly_code {
                    *cached = CachedChessemblyProgram {
                        source: definition.chessembly_code.clone(),
                        program: program.clone(),
                    };
                }
            })
            .or_insert_with(|| CachedChessemblyProgram {
                source: definition.chessembly_code.clone(),
                program: program.clone(),
            })
            .program
            .clone()
    }

    pub fn len(&self) -> usize {
        self.read_programs().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read_programs().is_empty()
    }

    pub fn layer_key(type_id: &PieceTypeId, layer_id: &str) -> String {
        format!("{type_id}::{layer_id}")
    }

    fn read_programs(&self) -> RwLockReadGuard<'_, HashMap<String, CachedChessemblyProgram>> {
        self.programs
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_programs(&self) -> RwLockWriteGuard<'_, HashMap<String, CachedChessemblyProgram>> {
        self.programs
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChessemblyDialect {
    Classic,
    BrainfuckChess,
}

// ─── Piece ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceLayer {
    #[default]
    Ground,
    Air,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub owner: PlayerId,
    pub type_id: PieceTypeId,
    /// None when in pocket or captured
    pub current_square: Option<Square>,
    pub in_pocket: bool,
    pub captured: bool,
    /// Whether this piece has ever moved (used for Pawn 2-step rule)
    pub has_moved: bool,
    /// Current ammunition, initialized from the definition's max_ammo.
    #[serde(default)]
    pub current_ammo: u32,
    /// The single physical layer occupied by this piece.
    #[serde(default)]
    pub layer: PieceLayer,
    /// Owner airborne turns remaining. Zero while grounded.
    #[serde(default)]
    pub remaining_flight_turns: u32,
    /// State owned by this concrete piece instance.
    #[serde(default)]
    pub state: HashMap<String, PieceStateValue>,
    /// Cooldowns owned by this concrete piece and move option.
    #[serde(default)]
    pub move_option_cooldowns: HashMap<String, CooldownState>,
}

impl Piece {
    pub fn is_on_board(&self) -> bool {
        self.current_square.is_some() && !self.in_pocket && !self.captured
    }

    pub fn initialize_from_definition(&mut self, definition: &PieceDefinition) {
        self.state = definition.initial_state();
        self.move_option_cooldowns.clear();
        self.current_ammo = definition.max_ammo;
        self.layer = PieceLayer::Ground;
        self.remaining_flight_turns = 0;
    }
}

// ─── Deck ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub player_id: PlayerId,
    /// Pieces placed on the board at game start
    pub starting_pieces: Vec<PieceId>,
    /// Pieces held in pocket, deployable during drop turns
    pub pocket_pieces: Vec<PieceId>,
    pub score_limit: u32,
    pub total_score: u32,
}

// ─── Player ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub deck: Deck,
    pub captured_pieces: Vec<PieceId>,
}

// ─── Actions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TurnAction {
    Move(MoveAction),
    Drop(DropAction),
    Ability(AbilityAction),
}

/// A canonical, server-generated standalone ability action. Optional targets
/// are explicit so clients never submit an inferred board mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub ability_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_piece_id: Option<PieceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pocket_piece_id: Option<PieceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Square>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployments: Vec<AbilityDeployment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityDeployment {
    pub pocket_piece_id: PieceId,
    pub to: Square,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlobalStateUpdate {
    pub key: String,
    pub value: i32,
}

/// Backward-compatible Rust name for Chessembly's global `set-state` output.
pub type StateUpdate = GlobalStateUpdate;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PieceStateUpdate {
    pub piece_id: PieceId,
    pub key: String,
    pub value: PieceStateValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CooldownUpdate {
    pub piece_id: PieceId,
    pub move_option_id: String,
    pub remaining: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ActionEffects {
    #[serde(default)]
    pub global_state_updates: Vec<GlobalStateUpdate>,
    #[serde(default)]
    pub piece_state_updates: Vec<PieceStateUpdate>,
    #[serde(default)]
    pub cooldown_updates: Vec<CooldownUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece_type_transition: Option<PieceTypeTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PieceTypeTransition {
    pub piece_id: PieceId,
    pub target_type_id: PieceTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChessemblyActionEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_state: Option<StateUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_to: Option<PieceTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub from: Square,
    pub to: Square,
    pub captured_piece_id: Option<PieceId>,
    /// Piece type to promote to when the moving piece's definition allows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<PieceTypeId>,
    pub move_option_id: String,
    #[serde(default)]
    pub source_layer_ids: Vec<String>,
    #[serde(default)]
    pub effects: ActionEffects,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropAction {
    pub player_id: PlayerId,
    pub piece_id: PieceId,
    pub to: Square,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_piece_id: Option<PieceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub turn_number: u32,
    pub player_id: PlayerId,
    pub action: TurnAction,
}

/// Search-oriented drop candidate. Identical pocket pieces are represented by
/// their type and count instead of one action per concrete piece id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropCandidateByType {
    pub player_id: PlayerId,
    pub piece_type_id: PieceTypeId,
    pub count: u16,
    pub to: SquareId,
}

// ─── GameResult ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub winner: Option<PlayerId>,
    pub reason: GameEndReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameEndReason {
    KingCapture,
    Resignation,
    Timeout,
    Abandonment,
    Draw,
}

// ─── GamePhase ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    Setup,
    Playing,
    Ended,
}

// ─── GameState ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub id: String,
    pub board: Board,
    /// All piece instances, keyed by PieceId
    pub pieces: HashMap<PieceId, Piece>,
    /// All piece definitions, keyed by PieceTypeId
    pub piece_definitions: HashMap<PieceTypeId, PieceDefinition>,
    /// Immutable provenance for custom definitions embedded in this game.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_piece_manifest: Vec<CustomPieceManifestEntry>,
    pub players: HashMap<PlayerId, Player>,
    pub current_player: PlayerId,
    pub turn_number: u32,
    pub phase: GamePhase,
    /// En passant target square (the passed-over square after a 2-step pawn move).
    pub en_passant_target: Option<Square>,
    /// Player allowed to capture via en passant on this turn.
    pub en_passant_available_to: Option<PlayerId>,
    /// Chessembly global state variables read by `if-state` and written by `set-state`.
    #[serde(default)]
    pub global_state: HashMap<String, i32>,
    #[serde(default)]
    pub history: Vec<ActionRecord>,
    pub result: Option<GameResult>,
    #[serde(skip, default)]
    pub chessembly_program_cache: ChessemblyProgramCache,
}

impl GameState {
    pub fn rebuild_chessembly_cache(&self) {
        self.chessembly_program_cache
            .rebuild(&self.piece_definitions);
    }

    pub fn ensure_chessembly_cache(&self) {
        // Deserialized states start empty and are rebuilt eagerly. Partially
        // populated caches are safe because each lookup validates its source
        // and compiles a missing or changed layer lazily.
        if self.chessembly_program_cache.is_empty() {
            self.rebuild_chessembly_cache();
        }
    }

    pub fn chessembly_program(&self, type_id: &PieceTypeId) -> Option<Arc<Program>> {
        let definition = self.piece_definitions.get(type_id)?;
        if let Some(program) = self
            .chessembly_program_cache
            .get(type_id, &definition.chessembly_code)
        {
            crate::profiling::record_cache_hit(1);
            return Some(program);
        }

        Some(
            self.chessembly_program_cache
                .get_or_parse(type_id, definition),
        )
    }

    pub fn chessembly_layer_program(
        &self,
        type_id: &PieceTypeId,
        layer: &MoveLayerDefinition,
    ) -> Arc<Program> {
        if let Some(program) =
            self.chessembly_program_cache
                .get_layer(type_id, &layer.id, &layer.chessembly_code)
        {
            crate::profiling::record_cache_hit(1);
            return program;
        }

        let key = ChessemblyProgramCache::layer_key(type_id, &layer.id);
        let program = Arc::new(parse(&layer.chessembly_code));
        let mut programs = self.chessembly_program_cache.write_programs();
        programs
            .entry(key)
            .and_modify(|cached| {
                if cached.source != layer.chessembly_code {
                    *cached = CachedChessemblyProgram {
                        source: layer.chessembly_code.clone(),
                        program: program.clone(),
                    };
                }
            })
            .or_insert_with(|| CachedChessemblyProgram {
                source: layer.chessembly_code.clone(),
                program: program.clone(),
            })
            .program
            .clone()
    }

    pub fn cached_chessembly_program_count(&self) -> usize {
        self.chessembly_program_cache.len()
    }
}

// ─── Validation ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn fail(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }
}

// ─── AttackMap ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackMap {
    pub player_id: PlayerId,
    pub attacked_squares: HashSet<SquareId>,
    /// Which pieces attack each square
    pub source_map: HashMap<SquareId, Vec<PieceId>>,
}

// ─── ChessemblyResult ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChessemblyResult {
    /// Squares the piece can move to (empty squares only for move-only pieces)
    pub movement_squares: Vec<Square>,
    /// Squares the piece threatens/attacks
    pub attack_squares: Vec<Square>,
    /// Optional effects attached to activated squares.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub effects: HashMap<SquareId, ChessemblyActionEffect>,
}
