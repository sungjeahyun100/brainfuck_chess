use std::collections::{HashMap, HashSet};

use crate::chessembly::run_chessembly_layer_for_piece;
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

    for (piece_id, piece) in &game_state.pieces {
        if piece.owner != *player_id || !piece.is_on_board() {
            continue;
        }
        let definition = match game_state
            .piece_definitions
            .get(&piece.type_id)
            .and_then(|definition| definition.clone().normalize_and_validate().ok())
        {
            Some(definition) => definition,
            None => continue,
        };
        let option = piece
            .active_ability
            .as_ref()
            .and_then(|active| {
                definition
                    .move_options
                    .iter()
                    .find(|option| option.id == active.ability_id)
            })
            .or_else(|| definition.normal_move_option());
        let Some(option) = option else {
            continue;
        };
        for layer_id in &option.layer_ids {
            let Some(layer) = definition
                .move_layers
                .iter()
                .find(|layer| &layer.id == layer_id)
            else {
                continue;
            };
            if !layer.is_enabled_for(piece) {
                continue;
            }
            let chessembly_result = run_chessembly_layer_for_piece(
                game_state,
                piece,
                &definition,
                layer,
                player_id.clone(),
                &game_state.global_state,
                existing_attack_maps,
            );

            for sq in &chessembly_result.attack_squares {
                let sq_id = sq.to_id();
                attacked_squares.insert(sq_id);
                let sources = source_map.entry(sq_id).or_default();
                if !sources.contains(piece_id) {
                    sources.push(piece_id.clone());
                }
            }
        }
    }

    AttackMap {
        player_id: player_id.clone(),
        attacked_squares,
        source_map,
    }
}
