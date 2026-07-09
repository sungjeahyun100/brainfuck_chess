use crate::types::*;

pub struct MovegenContext<'a> {
    pub state: &'a GameState,
    pub player_id: &'a PlayerId,
}

impl<'a> MovegenContext<'a> {
    pub fn new(state: &'a GameState) -> Self {
        Self {
            state,
            player_id: &state.current_player,
        }
    }

    pub fn can_generate_move_or_drop(&self) -> bool {
        !self
            .state
            .turn_state
            .actions
            .iter()
            .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
    }
}
