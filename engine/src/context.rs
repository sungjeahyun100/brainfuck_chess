use std::collections::HashMap;

use crate::pieces::default_pieces::all_default_definitions;
use crate::types::{ChessemblyProgramCache, GameState, PieceDefinition, PieceTypeId};

/// Definitions used by one game. Game-local definitions always override the
/// built-in definition with the same id.
#[derive(Debug, Clone)]
pub struct PieceCatalog {
    definitions: HashMap<PieceTypeId, PieceDefinition>,
}

impl PieceCatalog {
    pub fn for_state(state: &GameState) -> Self {
        let mut definitions = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect::<HashMap<_, _>>();
        definitions.extend(state.piece_definitions.clone());
        Self { definitions }
    }

    pub fn definitions(&self) -> &HashMap<PieceTypeId, PieceDefinition> {
        &self.definitions
    }

    pub fn get(&self, type_id: &str) -> Option<&PieceDefinition> {
        self.definitions.get(type_id)
    }
}

/// Non-serializable resources derived from a game's effective catalog.
#[derive(Debug)]
pub struct RuntimeResources {
    pub chessembly_programs: ChessemblyProgramCache,
}

impl RuntimeResources {
    pub fn for_catalog(catalog: &PieceCatalog) -> Self {
        Self {
            chessembly_programs: ChessemblyProgramCache::from_definitions(catalog.definitions()),
        }
    }
}

/// Request-scoped view that keeps state, definitions, and compiled resources
/// together throughout generation, validation, application, and AI work.
#[derive(Debug)]
pub struct GameContext<'a> {
    pub state: &'a GameState,
    pub catalog: PieceCatalog,
    pub runtime: RuntimeResources,
}

impl<'a> GameContext<'a> {
    pub fn new(state: &'a GameState) -> Self {
        let catalog = PieceCatalog::for_state(state);
        let runtime = RuntimeResources::for_catalog(&catalog);
        Self {
            state,
            catalog,
            runtime,
        }
    }
}
