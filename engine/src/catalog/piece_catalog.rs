use std::collections::HashMap;

use crate::pieces::default_pieces::all_default_definitions;
use crate::types::{PieceDefinition, PieceTypeId};

#[derive(Debug, Clone)]
pub struct PieceCatalog {
    definitions: HashMap<PieceTypeId, PieceDefinition>,
}

impl PieceCatalog {
    pub fn default_catalog() -> Self {
        Self {
            definitions: all_default_definitions()
                .into_iter()
                .map(|definition| (definition.id.clone(), definition))
                .collect(),
        }
    }

    pub fn from_definitions(definitions: HashMap<PieceTypeId, PieceDefinition>) -> Self {
        Self { definitions }
    }

    pub fn definitions(&self) -> &HashMap<PieceTypeId, PieceDefinition> {
        &self.definitions
    }

    pub fn get(&self, type_id: &PieceTypeId) -> Option<&PieceDefinition> {
        self.definitions.get(type_id)
    }

    pub fn contains(&self, type_id: &PieceTypeId) -> bool {
        self.definitions.contains_key(type_id)
    }
}
