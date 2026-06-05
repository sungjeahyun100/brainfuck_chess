use std::collections::{HashMap, HashSet};

use crate::chessembly::interpreter::{run, ExecutionContext};
use crate::chessembly::parser::parse;
use crate::types::*;

/// Compute the full attack map for a player: the union of attackSquares from
/// every piece the player has on the board.
pub fn generate_attack_map(
    game_state: &GameState,
    player_id: &PlayerId,
    // Pre-computed attack maps for other players (used by `danger()` expression)
    existing_attack_maps: &HashMap<PlayerId, HashSet<String>>,
) -> AttackMap {
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

        let program = parse(&definition.chessembly_code);
        let ctx = ExecutionContext {
            board: &game_state.board,
            piece,
            piece_definition: definition,
            all_definitions: &game_state.piece_definitions,
            all_pieces: &game_state.pieces,
            player: player_id.clone(),
            global_state: &empty_global_state,
            attack_maps: existing_attack_maps,
        };

        let chessembly_result = run(&program, &ctx);

        for sq in &chessembly_result.attack_squares {
            let sq_id = sq.to_id();
            attacked_squares.insert(sq_id.clone());
            source_map
                .entry(sq_id)
                .or_default()
                .push(piece_id.clone());
        }
    }

    AttackMap {
        player_id: player_id.clone(),
        attacked_squares,
        source_map,
    }
}
