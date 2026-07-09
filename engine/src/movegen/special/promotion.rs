use crate::types::*;

use crate::movegen::action_builder::push_action_if_unique;

pub(crate) fn promotion_options_for_rank(
    definition: &PieceDefinition,
    rank: i32,
    board_size: i32,
) -> Option<&[PieceTypeId]> {
    definition.promotion_options_for_rank(rank, board_size)
}

pub(crate) fn push_move_or_promotions(
    actions: &mut Vec<MoveAction>,
    definition: &PieceDefinition,
    board_size: i32,
    player_id: &PlayerId,
    piece_id: &PieceId,
    from: Square,
    to: Square,
    captured_piece_id: Option<PieceId>,
    ability_id: Option<&str>,
) {
    if let Some(promotion_options) = promotion_options_for_rank(definition, to.rank, board_size) {
        for promo in promotion_options {
            push_action_if_unique(
                actions,
                MoveAction {
                    player_id: player_id.clone(),
                    piece_id: piece_id.clone(),
                    from,
                    to,
                    captured_piece_id: captured_piece_id.clone(),
                    promotion: Some(promo.clone()),
                    ability_id: ability_id.map(str::to_string),
                },
            );
        }
    } else {
        push_action_if_unique(
            actions,
            MoveAction {
                player_id: player_id.clone(),
                piece_id: piece_id.clone(),
                from,
                to,
                captured_piece_id,
                promotion: None,
                ability_id: ability_id.map(str::to_string),
            },
        );
    }
}
