use crate::rules::end_turn;
use crate::types::*;

use super::applier::ActionApplier;
use super::error::ActionError;
use super::validator::ActionValidator;

pub fn submit_turn_action(
    mut state: GameState,
    action: TurnAction,
) -> Result<GameState, ActionError> {
    ActionValidator::validate_turn_action(&state, &action)?;

    match &action {
        TurnAction::Move(_) => {
            state.turn_state.mode = TurnMode::Move;
        }
        TurnAction::Drop(_) => {
            state.turn_state.mode = TurnMode::Drop;
        }
        TurnAction::ActivateAbility(_) => {
            state.turn_state.mode = TurnMode::Move;
        }
    }

    let next = ActionApplier::apply_turn_action(state, action);

    if next.phase == GamePhase::Ended || next.result.is_some() {
        return Ok(next);
    }

    if next
        .turn_state
        .actions
        .iter()
        .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
    {
        Ok(end_turn(next))
    } else {
        Ok(next)
    }
}
