//! Game-local custom piece packages and reproducible catalog snapshots.
//!
//! A package source is JSON containing `definitions: PieceDefinition[]`. Each
//! definition's `chessembly_code` remains the existing Chessembly dialect; this
//! envelope deliberately does not invent a new piece-declaration grammar.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::chessembly::parser::parse;
use crate::pieces::default_pieces::all_default_definitions;
use crate::types::{GameState, PieceDefinition, PieceTypeId};

pub const CUSTOM_PIECE_SCRIPT_FORMAT: &str = "brainfuck-chess-piece-set-v1";
pub const MAX_CUSTOM_SOURCE_BYTES: usize = 64 * 1024;
pub const MAX_CUSTOM_DEFINITIONS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomPieceManifestEntry {
    pub package_id: String,
    pub version: u32,
    pub content_hash: String,
    pub definition_snapshot_hash: String,
    pub exposed_type_id: PieceTypeId,
    pub runtime_type_ids: Vec<PieceTypeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPiecePackage {
    pub package_id: String,
    pub version: u32,
    pub content_hash: String,
    pub raw_script: String,
    pub exposed_piece_key: String,
    pub exposed_type_id: PieceTypeId,
    pub definitions: Vec<PieceDefinition>,
    pub internal_type_ids: Vec<PieceTypeId>,
    pub score: u32,
}

#[derive(Debug, Clone)]
pub struct CustomPiecePackageInput {
    pub package_id: String,
    pub version: u32,
    pub expected_content_hash: Option<String>,
    pub raw_script: String,
    pub exposed_piece_key: String,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PieceSetDocument {
    format: String,
    definitions: Vec<PieceDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomPieceError {
    ParseFailure(String),
    ChessemblySyntax {
        piece: String,
        layer: String,
        message: String,
        line: usize,
        column: usize,
    },
    SemanticValidation(String),
    MissingExposedPiece(String),
    MissingInternalReference {
        piece: String,
        target: String,
    },
    IdentifierCollision(String),
    ExecutionLimitExceeded(&'static str),
    UnsupportedFeature(String),
    CorruptSnapshot(String),
    DefinitionVersionMismatch {
        expected: String,
        actual: String,
    },
}

impl fmt::Display for CustomPieceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseFailure(message) => write!(f, "custom piece parse failure: {message}"),
            Self::ChessemblySyntax {
                piece,
                layer,
                message,
                line,
                column,
            } => write!(
                f,
                "piece `{piece}` layer `{layer}`: {message} ({line}행 {column}열)"
            ),
            Self::SemanticValidation(message) => {
                write!(f, "custom piece semantic validation failed: {message}")
            }
            Self::MissingExposedPiece(key) => write!(f, "exposed piece `{key}` does not exist"),
            Self::MissingInternalReference { piece, target } => {
                write!(
                    f,
                    "piece `{piece}` references missing internal piece `{target}`"
                )
            }
            Self::IdentifierCollision(id) => write!(f, "piece identifier collision: `{id}`"),
            Self::ExecutionLimitExceeded(limit) => write!(f, "execution limit exceeded: {limit}"),
            Self::UnsupportedFeature(feature) => write!(f, "unsupported feature: {feature}"),
            Self::CorruptSnapshot(message) => write!(f, "corrupt game snapshot: {message}"),
            Self::DefinitionVersionMismatch { expected, actual } => write!(
                f,
                "definition version mismatch: expected `{expected}`, got `{actual}`"
            ),
        }
    }
}

impl std::error::Error for CustomPieceError {}

pub fn custom_runtime_type_id(package_id: &str, version: u32, local_key: &str) -> String {
    format!("custom:{package_id}:v{version}:{local_key}")
}

/// Builds a validated package while preserving `raw_script` byte-for-byte.
pub fn validate_and_build_custom_piece_package(
    input: CustomPiecePackageInput,
) -> Result<CustomPiecePackage, CustomPieceError> {
    if input.raw_script.len() > MAX_CUSTOM_SOURCE_BYTES {
        return Err(CustomPieceError::ExecutionLimitExceeded("source_bytes"));
    }
    validate_component("package_id", &input.package_id)?;
    validate_component("exposed_piece_key", &input.exposed_piece_key)?;
    let document: PieceSetDocument = serde_json::from_str(&input.raw_script)
        .map_err(|error| CustomPieceError::ParseFailure(error.to_string()))?;
    if document.format != CUSTOM_PIECE_SCRIPT_FORMAT {
        return Err(CustomPieceError::UnsupportedFeature(document.format));
    }
    if document.definitions.is_empty() {
        return Err(CustomPieceError::SemanticValidation(
            "at least one definition is required".into(),
        ));
    }
    if document.definitions.len() > MAX_CUSTOM_DEFINITIONS {
        return Err(CustomPieceError::ExecutionLimitExceeded(
            "internal_definitions",
        ));
    }

    let mut local_keys = HashSet::new();
    for definition in &document.definitions {
        validate_component("piece id", &definition.id)?;
        if !local_keys.insert(definition.id.clone()) {
            return Err(CustomPieceError::IdentifierCollision(definition.id.clone()));
        }
        if definition.is_king || definition.can_capture_on_drop {
            return Err(CustomPieceError::UnsupportedFeature(format!(
                "privileged capability on `{}`",
                definition.id
            )));
        }
    }
    if !local_keys.contains(&input.exposed_piece_key) {
        return Err(CustomPieceError::MissingExposedPiece(
            input.exposed_piece_key,
        ));
    }

    for definition in &document.definitions {
        validate_transition_references(definition, &local_keys)?;
        validate_programs_are_nonempty(definition)?;
        for target in &definition.promotion_pool {
            if !local_keys.contains(target) {
                return Err(CustomPieceError::MissingInternalReference {
                    piece: definition.id.clone(),
                    target: target.clone(),
                });
            }
        }
    }

    let content_hash = stable_content_hash(&input.raw_script);
    if let Some(expected) = input.expected_content_hash.as_ref() {
        if expected != &content_hash {
            return Err(CustomPieceError::DefinitionVersionMismatch {
                expected: expected.clone(),
                actual: content_hash,
            });
        }
    }

    let id_map = local_keys
        .iter()
        .map(|key| {
            (
                key.clone(),
                custom_runtime_type_id(&input.package_id, input.version, key),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut definitions = Vec::with_capacity(document.definitions.len());
    for mut definition in document.definitions {
        let local_id = definition.id.clone();
        definition.id = id_map[&local_id].clone();
        definition.score = if local_id == input.exposed_piece_key {
            input.score
        } else {
            definition.score
        };
        definition.is_king = false;
        definition.can_capture_on_drop = false;
        definition.promotion_pool = definition
            .promotion_pool
            .iter()
            .map(|target| id_map[target].clone())
            .collect();
        definition.chessembly_code = rewrite_transitions(&definition.chessembly_code, &id_map);
        for layer in &mut definition.move_layers {
            layer.chessembly_code = rewrite_transitions(&layer.chessembly_code, &id_map);
        }
        definitions.push(
            definition
                .normalize_and_validate()
                .map_err(CustomPieceError::SemanticValidation)?,
        );
    }

    let exposed_type_id = id_map[&input.exposed_piece_key].clone();
    let internal_type_ids = id_map
        .iter()
        .filter(|(key, _)| *key != &input.exposed_piece_key)
        .map(|(_, id)| id.clone())
        .collect();
    Ok(CustomPiecePackage {
        package_id: input.package_id,
        version: input.version,
        content_hash,
        raw_script: input.raw_script,
        exposed_piece_key: input.exposed_piece_key,
        exposed_type_id,
        definitions,
        internal_type_ids,
        score: input.score,
    })
}

fn validate_programs_are_nonempty(definition: &PieceDefinition) -> Result<(), CustomPieceError> {
    for (layer, code) in std::iter::once(("default", definition.chessembly_code.as_str())).chain(
        definition
            .move_layers
            .iter()
            .map(|layer| (layer.id.as_str(), layer.chessembly_code.as_str())),
    ) {
        validate_supported_tokens(&definition.id, layer, code)?;
        if !code.trim().is_empty() && parse(code).is_empty() {
            return Err(CustomPieceError::ParseFailure(format!(
                "piece `{}` layer `{layer}` contains no supported Chessembly expressions",
                definition.id
            )));
        }
    }
    Ok(())
}

fn validate_supported_tokens(
    piece_id: &str,
    layer: &str,
    source: &str,
) -> Result<(), CustomPieceError> {
    const IDENTIFIERS: &[&str] = &[
        "do",
        "while",
        "not",
        "true",
        "false",
        "check",
        "end",
        "move",
        "take",
        "take-move",
        "catch",
        "jump",
        "shift",
        "anchor",
        "absolute-x",
        "absolute-y",
        "observe",
        "peek",
        "enemy",
        "friendly",
        "bound",
        "edge",
        "corner",
        "danger",
        "repeat",
        "jmp",
        "jne",
        "label",
        "read",
        "read-and",
        "read-or",
        "read-xor",
        "write",
        "piece",
        "if-state",
        "transition",
        "set-state",
        "piece-on",
    ];
    let mut parentheses = 0i32;
    let mut braces = 0i32;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let character = bytes[index] as char;
        match character {
            '(' => parentheses += 1,
            ')' => {
                parentheses -= 1;
                if parentheses < 0 {
                    return syntax_error(piece_id, layer, source, index, "unmatched `)`");
                }
            }
            '{' => braces += 1,
            '}' => {
                braces -= 1;
                if braces < 0 {
                    return syntax_error(piece_id, layer, source, index, "unmatched `}`");
                }
            }
            character if character.is_ascii_alphabetic() || character == '_' => {
                let start = index;
                index += 1;
                while index < bytes.len() {
                    let next = bytes[index] as char;
                    if next.is_ascii_alphanumeric() || matches!(next, '-' | '_' | ':') {
                        index += 1;
                    } else {
                        break;
                    }
                }
                let identifier = &source[start..index];
                let is_argument = parentheses > 0
                    && !source[index..].trim_start().starts_with('(')
                    && source[..start].trim_end().ends_with(['(', ',']);
                if !is_argument && !IDENTIFIERS.contains(&identifier) {
                    return syntax_error(
                        piece_id,
                        layer,
                        source,
                        start,
                        &format!(
                            "현재 덱체스 인터프리터에서 지원하지 않는 체섬블리 명령입니다. \
                             지원하지 않는 명령: {identifier}"
                        ),
                    );
                }
                continue;
            }
            character
                if character.is_ascii_whitespace()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | ',' | ';') => {}
            _ => {
                return syntax_error(
                    piece_id,
                    layer,
                    source,
                    index,
                    &format!(
                        "현재 덱체스 인터프리터에서 지원하지 않는 문자입니다. \
                         지원하지 않는 문자: {character}"
                    ),
                );
            }
        }
        index += 1;
    }
    if parentheses != 0 || braces != 0 {
        return syntax_error(
            piece_id,
            layer,
            source,
            source.len().saturating_sub(1),
            "unclosed delimiter",
        );
    }
    Ok(())
}

fn syntax_error<T>(
    piece_id: &str,
    layer: &str,
    source: &str,
    byte_index: usize,
    message: &str,
) -> Result<T, CustomPieceError> {
    let prefix = &source[..byte_index.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit('\n')
        .next()
        .map(str::chars)
        .map(Iterator::count)
        .unwrap_or(0)
        + 1;
    Err(CustomPieceError::ChessemblySyntax {
        piece: piece_id.to_owned(),
        layer: layer.to_owned(),
        message: message.to_owned(),
        line,
        column,
    })
}

fn runtime_definitions_with_source_visuals(
    package: &CustomPiecePackage,
) -> Result<Vec<PieceDefinition>, CustomPieceError> {
    let document: PieceSetDocument = serde_json::from_str(&package.raw_script)
        .map_err(|error| CustomPieceError::ParseFailure(error.to_string()))?;
    if document.format != CUSTOM_PIECE_SCRIPT_FORMAT {
        return Err(CustomPieceError::UnsupportedFeature(document.format));
    }

    let source_variants = document
        .definitions
        .into_iter()
        .map(|definition| {
            (
                custom_runtime_type_id(&package.package_id, package.version, &definition.id),
                definition.visual.variants,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut definitions = package.definitions.clone();
    for definition in &mut definitions {
        let variants = source_variants.get(&definition.id).ok_or_else(|| {
            CustomPieceError::CorruptSnapshot(format!(
                "package `{}` source is missing definition `{}`",
                package.package_id, definition.id
            ))
        })?;
        definition.visual.variants = variants.clone();
    }
    Ok(definitions)
}

pub fn install_runtime_catalog(
    state: &mut GameState,
    packages: &[CustomPiecePackage],
) -> Result<(), CustomPieceError> {
    let reserved = all_default_definitions()
        .into_iter()
        .map(|definition| definition.id)
        .collect::<HashSet<_>>();
    let mut definitions = reserved.clone();
    definitions.extend(state.piece_definitions.keys().cloned());
    let mut package_versions = HashMap::new();
    let mut runtime_definitions = Vec::with_capacity(packages.len());

    for package in packages {
        let actual_hash = stable_content_hash(&package.raw_script);
        if actual_hash != package.content_hash {
            return Err(CustomPieceError::DefinitionVersionMismatch {
                expected: package.content_hash.clone(),
                actual: actual_hash,
            });
        }
        if let Some(hash) = package_versions.insert(
            (package.package_id.as_str(), package.version),
            &package.content_hash,
        ) {
            if hash != &package.content_hash {
                return Err(CustomPieceError::DefinitionVersionMismatch {
                    expected: hash.clone(),
                    actual: package.content_hash.clone(),
                });
            }
        }
        let prepared = runtime_definitions_with_source_visuals(package)?;
        for definition in &prepared {
            if reserved.contains(&definition.id) || !definitions.insert(definition.id.clone()) {
                return Err(CustomPieceError::IdentifierCollision(definition.id.clone()));
            }
        }
        runtime_definitions.push(prepared);
    }
    for (package, prepared) in packages.iter().zip(runtime_definitions) {
        for definition in &prepared {
            state
                .piece_definitions
                .insert(definition.id.clone(), definition.clone());
        }
        state.custom_piece_manifest.push(CustomPieceManifestEntry {
            package_id: package.package_id.clone(),
            version: package.version,
            content_hash: package.content_hash.clone(),
            definition_snapshot_hash: definitions_hash(&prepared)?,
            exposed_type_id: package.exposed_type_id.clone(),
            runtime_type_ids: prepared
                .iter()
                .map(|definition| definition.id.clone())
                .collect(),
        });
    }
    state.rebuild_chessembly_cache();
    Ok(())
}

pub fn serialize_game_snapshot(state: &GameState) -> Result<String, CustomPieceError> {
    validate_snapshot(state)?;
    serde_json::to_string(state)
        .map_err(|error| CustomPieceError::CorruptSnapshot(error.to_string()))
}

pub fn restore_game_snapshot(snapshot: &str) -> Result<GameState, CustomPieceError> {
    let state: GameState = serde_json::from_str(snapshot)
        .map_err(|error| CustomPieceError::CorruptSnapshot(error.to_string()))?;
    validate_snapshot(&state)?;
    state.rebuild_chessembly_cache();
    Ok(state)
}

pub fn validate_snapshot(state: &GameState) -> Result<(), CustomPieceError> {
    for piece in state.pieces.values() {
        if !state.piece_definitions.contains_key(&piece.type_id) {
            return Err(CustomPieceError::CorruptSnapshot(format!(
                "piece `{}` references missing definition `{}`",
                piece.id, piece.type_id
            )));
        }
    }
    for entry in &state.custom_piece_manifest {
        if entry.runtime_type_ids.is_empty()
            || !entry
                .runtime_type_ids
                .iter()
                .all(|id| state.piece_definitions.contains_key(id))
            || !entry.runtime_type_ids.contains(&entry.exposed_type_id)
        {
            return Err(CustomPieceError::CorruptSnapshot(format!(
                "package `{}` has missing definitions",
                entry.package_id
            )));
        }
        let definitions = entry
            .runtime_type_ids
            .iter()
            .filter_map(|id| state.piece_definitions.get(id))
            .cloned()
            .collect::<Vec<_>>();
        let actual = definitions_hash(&definitions)?;
        if actual != entry.definition_snapshot_hash {
            return Err(CustomPieceError::DefinitionVersionMismatch {
                expected: entry.definition_snapshot_hash.clone(),
                actual,
            });
        }
    }
    Ok(())
}

pub fn deck_selectable_custom_type_ids(state: &GameState) -> Vec<&str> {
    state
        .custom_piece_manifest
        .iter()
        .map(|entry| entry.exposed_type_id.as_str())
        .collect()
}

fn validate_component(label: &str, value: &str) -> Result<(), CustomPieceError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(CustomPieceError::SemanticValidation(format!(
            "{label} contains unsupported characters"
        )));
    }
    Ok(())
}

fn validate_transition_references(
    definition: &PieceDefinition,
    local_keys: &HashSet<String>,
) -> Result<(), CustomPieceError> {
    for code in std::iter::once(&definition.chessembly_code).chain(
        definition
            .move_layers
            .iter()
            .map(|layer| &layer.chessembly_code),
    ) {
        let program = parse(code);
        for target in transition_targets(&program) {
            if !local_keys.contains(target) {
                return Err(CustomPieceError::MissingInternalReference {
                    piece: definition.id.clone(),
                    target: target.clone(),
                });
            }
        }
    }
    Ok(())
}

fn transition_targets(program: &[Vec<crate::chessembly::ast::Expr>]) -> Vec<&String> {
    fn collect<'a>(expressions: &'a [crate::chessembly::ast::Expr], out: &mut Vec<&'a String>) {
        for expression in expressions {
            match expression {
                crate::chessembly::ast::Expr::Transition(target) => out.push(target),
                crate::chessembly::ast::Expr::Block(inner) => collect(inner, out),
                _ => {}
            }
        }
    }
    let mut targets = Vec::new();
    for chain in program {
        collect(chain, &mut targets);
    }
    targets
}

fn rewrite_transitions(source: &str, id_map: &HashMap<String, String>) -> String {
    let mut rewritten = source.to_owned();
    for (local, runtime) in id_map {
        rewritten = rewritten.replace(
            &format!("transition({local})"),
            &format!("transition({runtime})"),
        );
    }
    rewritten
}

fn stable_content_hash(source: &str) -> String {
    // Stable, dependency-free FNV-1a digest. This is an integrity identifier,
    // not an authentication primitive.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn definitions_hash(definitions: &[PieceDefinition]) -> Result<String, CustomPieceError> {
    let mut ordered = definitions.to_vec();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let encoded = serde_json::to_string(&ordered)
        .map_err(|error| CustomPieceError::CorruptSnapshot(error.to_string()))?;
    Ok(stable_content_hash(&encoded))
}
