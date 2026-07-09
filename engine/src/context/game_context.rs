use std::sync::Arc;

use crate::catalog::PieceCatalog;
use crate::chessembly::ast::Program;
use crate::runtime::RuntimeResources;
use crate::types::GameState;
use crate::types::{Piece, PieceDefinition, PieceTypeId, TurnAction};

pub struct GameContext<'a> {
    pub state: &'a GameState,
    pub catalog: &'a PieceCatalog,
    pub runtime: &'a RuntimeResources,
}

impl<'a> GameContext<'a> {
    pub fn can_generate_move_or_drop(&self) -> bool {
        !self
            .state
            .turn_state
            .actions
            .iter()
            .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
    }

    pub fn ensure_chessembly_cache(&self) {
        self.runtime.ensure_chessembly_cache(self.catalog);
    }

    pub fn chessembly_program(&self, type_id: &PieceTypeId) -> Option<Arc<Program>> {
        if let Some(program) = self.runtime.chessembly_cache.get(type_id) {
            crate::profiling::record_cache_hit(1);
            return Some(program);
        }

        let definition = self.catalog.get(type_id)?;
        Some(
            self.runtime
                .chessembly_cache
                .get_or_parse(type_id, definition),
        )
    }

    pub fn effective_chessembly_program(
        &self,
        piece: &Piece,
        definition: &PieceDefinition,
    ) -> Option<Arc<Program>> {
        if let Some(active) = &piece.active_ability {
            if let Some(ability) = definition
                .abilities
                .iter()
                .find(|ability| ability.id == active.ability_id)
            {
                if let Some(program) = self
                    .runtime
                    .chessembly_cache
                    .get_ability(&definition.id, &ability.id)
                {
                    crate::profiling::record_cache_hit(1);
                    return Some(program);
                }
                return Some(
                    self.runtime
                        .chessembly_cache
                        .get_or_parse_ability(&definition.id, ability),
                );
            }
        }

        self.chessembly_program(&piece.type_id)
    }

    pub fn cached_chessembly_program_count(&self) -> usize {
        self.runtime.chessembly_cache.len()
    }
}
