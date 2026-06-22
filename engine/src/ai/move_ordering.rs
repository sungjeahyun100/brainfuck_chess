use std::cmp::Reverse;

use crate::ai::types::AiAction;
use crate::types::{GameState, PlayerId};

fn action_priority(state: &GameState, action: &AiAction) -> (u8, u32) {
    match action {
        AiAction::Move(action) => {
            let Some(captured) = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id))
            else {
                return (2, 0);
            };
            if captured.is_king {
                (5, u32::MAX)
            } else {
                (4, captured.score)
            }
        }
        AiAction::Drop(_) => (3, 0),
        AiAction::EndTurn => (1, 0),
    }
}

pub fn order_ai_actions(state: &GameState, actions: &mut [AiAction], _bot_player_id: &PlayerId) {
    actions.sort_by_key(|action| Reverse(action_priority(state, action)));
}
