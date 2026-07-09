use crate::types::*;

use super::pawn::is_pawn_type;
use crate::movegen::action_builder::push_action_if_unique;

pub fn generate_en_passant_actions(
    state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    piece_id: &PieceId,
    player_id: &PlayerId,
    from: Square,
    ability_id: Option<&str>,
) -> Vec<MoveAction> {
    let _ = definition;
    let mut actions = Vec::new();

    let Some(dir) = super::pawn::pawn_forward_dir(&piece.type_id) else {
        return actions;
    };

    if state.en_passant_available_to.as_ref() != Some(player_id) {
        return actions;
    }

    let Some(target) = state.en_passant_target else {
        return actions;
    };

    if target.rank != from.rank + dir
        || (target.file - from.file).abs() != 1
        || !state.board.is_empty(&target)
    {
        return actions;
    }

    let adjacent = Square::new(target.file, from.rank);
    if let Some(captured_id) = state.board.get_piece_at(&adjacent) {
        if let Some(captured_piece) = state.pieces.get(captured_id) {
            if captured_piece.owner != *player_id && is_pawn_type(&captured_piece.type_id) {
                push_action_if_unique(
                    &mut actions,
                    MoveAction {
                        player_id: player_id.clone(),
                        piece_id: piece_id.clone(),
                        from,
                        to: target,
                        captured_piece_id: Some(captured_id.clone()),
                        promotion: None,
                        ability_id: ability_id.map(str::to_string),
                    },
                );
            }
        }
    }

    actions
}
