use std::collections::HashMap;

use crate::chessembly::run_effective_chessembly_for_piece;

use super::{PieceMoveBackend, PieceMoveContext, PieceMovePattern};

pub struct ChessemblyBackend;

impl PieceMoveBackend for ChessemblyBackend {
    fn generate(&self, ctx: PieceMoveContext<'_>) -> PieceMovePattern {
        let empty_global_state = HashMap::new();
        let empty_maps = HashMap::new();

        let result = run_effective_chessembly_for_piece(
            ctx.state,
            ctx.piece,
            ctx.definition,
            ctx.player_id.clone(),
            &empty_global_state,
            &empty_maps,
        );

        PieceMovePattern {
            movement_squares: result.movement_squares,
            attack_squares: result.attack_squares,
            intents: Vec::new(),
        }
    }
}
