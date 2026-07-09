use std::collections::{HashMap, HashSet};

use crate::catalog::PieceCatalog;
use crate::chessembly::run_effective_chessembly_for_context;
use crate::context::GameContext;
use crate::runtime::RuntimeResources;
use crate::types::*;

/// Compute the full attack map for a player: the union of attackSquares from
/// every piece the player has on the board.
pub fn generate_attack_map(
    game_state: &GameState,
    player_id: &PlayerId,
    // Pre-computed attack maps for other players (used by `danger()` expression)
    existing_attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> AttackMap {
    let catalog = PieceCatalog::default_catalog();
    let runtime = RuntimeResources::from_catalog(&catalog);
    let context = GameContext {
        state: game_state,
        catalog: &catalog,
        runtime: &runtime,
    };
    generate_attack_map_for_context(&context, player_id, existing_attack_maps)
}

pub fn generate_attack_map_for_context(
    context: &GameContext<'_>,
    player_id: &PlayerId,
    existing_attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> AttackMap {
    crate::profiling::record_attack_map(1);
    context.ensure_chessembly_cache();
    let game_state = context.state;

    let mut attacked_squares: HashSet<SquareId> = HashSet::new();
    let mut source_map: HashMap<SquareId, Vec<PieceId>> = HashMap::new();

    let empty_global_state = HashMap::new();

    for (piece_id, piece) in &game_state.pieces {
        if piece.owner != *player_id || !piece.is_on_board() {
            continue;
        }
        let definition = match context.catalog.get(&piece.type_id) {
            Some(d) => d,
            None => continue,
        };

        let chessembly_result = run_effective_chessembly_for_context(
            context,
            piece,
            definition,
            player_id.clone(),
            &empty_global_state,
            existing_attack_maps,
        );

        for sq in &chessembly_result.attack_squares {
            let sq_id = sq.to_id();
            attacked_squares.insert(sq_id);
            source_map.entry(sq_id).or_default().push(piece_id.clone());
        }
    }

    AttackMap {
        player_id: player_id.clone(),
        attacked_squares,
        source_map,
    }
}
