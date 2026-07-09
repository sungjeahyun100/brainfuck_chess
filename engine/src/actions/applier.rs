use crate::endgame::{apply_activate_ability_action, apply_drop_action, apply_move_action};
use crate::types::*;

pub struct ActionApplier;

impl ActionApplier {
    pub fn apply_turn_action(state: GameState, action: TurnAction) -> GameState {
        match action {
            TurnAction::Move(action) => apply_move_action(state, action),
            TurnAction::Drop(action) => apply_drop_action(state, action),
            TurnAction::ActivateAbility(action) => apply_activate_ability_action(state, action),
        }
    }
}
