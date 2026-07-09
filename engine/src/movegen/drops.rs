use std::collections::HashMap;

use crate::placement::get_placement_squares;
use crate::types::*;

use super::context::MovegenContext;

pub fn generate_piece_legal_drop_actions(
    game_state: &GameState,
    piece_id: &PieceId,
) -> Vec<DropAction> {
    let context = MovegenContext::new(game_state);
    let player_id = context.player_id;

    if context.state.turn_state.mode == TurnMode::Move || !context.can_generate_move_or_drop() {
        return Vec::new();
    }

    let Some(player) = game_state.players.get(player_id) else {
        return Vec::new();
    };
    if !player.deck.pocket_pieces.contains(piece_id) {
        return Vec::new();
    }

    let Some(piece) = game_state.pieces.get(piece_id) else {
        return Vec::new();
    };
    if piece.owner != *player_id || !piece.in_pocket || piece.captured {
        return Vec::new();
    }

    let Some(def) = game_state.piece_definitions.get(&piece.type_id) else {
        return Vec::new();
    };
    if def.is_king {
        return Vec::new();
    }

    get_placement_squares(game_state, player_id)
        .into_iter()
        .map(|sq| DropAction {
            player_id: player_id.clone(),
            piece_id: piece_id.clone(),
            to: sq,
        })
        .collect()
}

pub fn generate_legal_drop_actions(game_state: &GameState) -> Vec<DropAction> {
    let context = MovegenContext::new(game_state);
    let player_id = context.player_id;

    if context.state.turn_state.mode == TurnMode::Move || !context.can_generate_move_or_drop() {
        return Vec::new();
    }

    let player = match game_state.players.get(player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut actions = Vec::new();
    for piece_id in &player.deck.pocket_pieces {
        actions.extend(generate_piece_legal_drop_actions(game_state, piece_id));
    }

    crate::profiling::record_drops(actions.len());
    actions
}

pub fn generate_drop_candidates_by_type(
    game_state: &GameState,
    player_id: &PlayerId,
) -> Vec<DropCandidateByType> {
    let context = MovegenContext::new(game_state);

    if &game_state.current_player != player_id
        || context.state.turn_state.mode == TurnMode::Move
        || !context.can_generate_move_or_drop()
    {
        return Vec::new();
    }

    let Some(player) = game_state.players.get(player_id) else {
        return Vec::new();
    };

    let mut counts: HashMap<PieceTypeId, u16> = HashMap::new();
    for piece_id in &player.deck.pocket_pieces {
        let Some(piece) = game_state.pieces.get(piece_id) else {
            continue;
        };
        if piece.owner != *player_id || !piece.in_pocket || piece.captured {
            continue;
        }
        let Some(definition) = game_state.piece_definitions.get(&piece.type_id) else {
            continue;
        };
        if definition.is_king {
            continue;
        }
        let count = counts.entry(piece.type_id.clone()).or_default();
        *count = count.saturating_add(1);
    }

    let mut type_counts: Vec<_> = counts.into_iter().collect();
    type_counts.sort_by(|left, right| left.0.cmp(&right.0));
    let mut squares = get_placement_squares(game_state, player_id);
    squares.sort_by_key(|square| (square.rank, square.file));

    type_counts
        .into_iter()
        .flat_map(|(piece_type_id, count)| {
            squares.iter().map(move |square| DropCandidateByType {
                player_id: player_id.clone(),
                piece_type_id: piece_type_id.clone(),
                count,
                to: square.to_id(),
            })
        })
        .collect()
}
