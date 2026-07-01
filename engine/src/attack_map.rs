use std::collections::{HashMap, HashSet};

use crate::chessembly::run_effective_chessembly_for_piece;
use crate::types::*;

/// Compute the full attack map for a player: the union of attackSquares from
/// every piece the player has on the board.
pub fn generate_attack_map(
    game_state: &GameState,
    player_id: &PlayerId,
    // Pre-computed attack maps for other players (used by `danger()` expression)
    existing_attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> AttackMap {
    crate::profiling::record_attack_map(1);
    game_state.ensure_chessembly_cache();

    let mut attacked_squares: HashSet<SquareId> = HashSet::new();
    let mut source_map: HashMap<SquareId, Vec<PieceId>> = HashMap::new();

    let empty_global_state = HashMap::new();

    for (piece_id, piece) in &game_state.pieces {
        if piece.owner != *player_id || !piece.is_on_board() {
            continue;
        }
        let definition = match game_state.piece_definitions.get(&piece.type_id) {
            Some(d) => d,
            None => continue,
        };

        let chessembly_result = run_effective_chessembly_for_piece(
            game_state,
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
