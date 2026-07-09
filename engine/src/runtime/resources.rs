use crate::catalog::PieceCatalog;
use crate::types::ChessemblyProgramCache;

#[derive(Debug, Clone)]
pub struct RuntimeResources {
    pub chessembly_cache: ChessemblyProgramCache,
}

impl RuntimeResources {
    pub fn from_catalog(catalog: &PieceCatalog) -> Self {
        Self {
            chessembly_cache: ChessemblyProgramCache::from_definitions(catalog.definitions()),
        }
    }

    pub fn ensure_chessembly_cache(&self, catalog: &PieceCatalog) {
        if !self.chessembly_cache.is_complete_for(catalog.definitions()) {
            self.chessembly_cache.rebuild(catalog.definitions());
        }
    }
}
