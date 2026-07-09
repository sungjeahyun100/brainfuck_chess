use crate::rules::end_turn;
use crate::types::*;

use super::applier::ActionApplier;
use super::effect::AppliedAction;
use super::effect_builder::build_state_diff_effects;
use super::error::ActionError;
use super::validator::ActionValidator;

pub fn submit_turn_action(state: GameState, action: TurnAction) -> Result<GameState, ActionError> {
    apply_turn_action_with_effects(state, action).map(|applied| applied.state)
}

pub fn apply_turn_action_with_effects(
    mut state: GameState,
    action: TurnAction,
) -> Result<AppliedAction, ActionError> {
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

    let mut applied = ActionApplier::apply_turn_action_with_effects(state, action);

    if applied.state.phase == GamePhase::Ended || applied.state.result.is_some() {
        return Ok(applied);
    }

    if applied
        .state
        .turn_state
        .actions
        .iter()
        .any(|action| matches!(action, TurnAction::Move(_) | TurnAction::Drop(_)))
    {
        let before_end_turn = applied.state;
        let after_end_turn = end_turn(before_end_turn.clone());
        applied
            .effects
            .extend(build_state_diff_effects(&before_end_turn, &after_end_turn));
        applied.state = after_end_turn;
    }

    Ok(applied)
}
