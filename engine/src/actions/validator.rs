use crate::catalog::PieceCatalog;
use crate::legal_moves::{
    generate_legal_drop_actions, generate_legal_move_actions,
    generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
};
use crate::types::*;

use super::error::ActionError;

pub struct ActionValidator;

impl ActionValidator {
    pub fn validate_turn_action(state: &GameState, action: &TurnAction) -> Result<(), ActionError> {
        if state.phase == GamePhase::Ended || state.result.is_some() {
            return Err(ActionError::GameAlreadyEnded);
        }

        match action {
            TurnAction::Move(action) => Self::validate_move(state, action),
            TurnAction::Drop(action) => Self::validate_drop(state, action),
            TurnAction::ActivateAbility(action) => Self::validate_ability(state, action),
        }
    }

    pub fn validate_move(state: &GameState, action: &MoveAction) -> Result<(), ActionError> {
        if action.player_id != state.current_player {
            return Err(ActionError::WrongPlayer);
        }

        let legal = if let Some(ability_id) = action.ability_id.as_ref() {
            generate_piece_legal_move_actions_with_options(
                state,
                &action.piece_id,
                &MoveGenerationOptions {
                    ability_id: Some(ability_id.clone()),
                },
            )
        } else {
            generate_legal_move_actions(state)
        };

        if legal.iter().any(|candidate| candidate == action) {
            Ok(())
        } else {
            Err(ActionError::IllegalMove)
        }
    }

    pub fn validate_drop(state: &GameState, action: &DropAction) -> Result<(), ActionError> {
        if action.player_id != state.current_player {
            return Err(ActionError::WrongPlayer);
        }

        if generate_legal_drop_actions(state)
            .iter()
            .any(|candidate| candidate == action)
        {
            Ok(())
        } else {
            Err(ActionError::IllegalDrop)
        }
    }

    pub fn validate_ability(
        state: &GameState,
        action: &ActivateAbilityAction,
    ) -> Result<(), ActionError> {
        if action.player_id != state.current_player {
            return Err(ActionError::WrongPlayer);
        }

        let piece = state
            .pieces
            .get(&action.piece_id)
            .ok_or(ActionError::IllegalAbility)?;
        if piece.owner != state.current_player || !piece.is_on_board() {
            return Err(ActionError::IllegalAbility);
        }

        let catalog = PieceCatalog::default_catalog();
        let definition = catalog
            .get(&piece.type_id)
            .ok_or(ActionError::IllegalAbility)?;

        let ability = definition
            .abilities
            .iter()
            .find(|ability| ability.id == action.ability_id)
            .ok_or(ActionError::IllegalAbility)?;

        if state.turn_state.mode == TurnMode::Drop {
            return Err(ActionError::IllegalAbility);
        }

        if state
            .turn_state
            .actions
            .iter()
            .any(|existing| matches!(existing, TurnAction::Move(_) | TurnAction::Drop(_)))
        {
            return Err(ActionError::IllegalAbility);
        }

        if ability.once_per_turn
            && state.turn_state.actions.iter().any(|existing| {
                matches!(
                    existing,
                    TurnAction::ActivateAbility(previous)
                        if previous.piece_id == action.piece_id
                        && previous.ability_id == action.ability_id
                )
            })
        {
            return Err(ActionError::IllegalAbility);
        }

        if piece
            .ability_cooldowns
            .get(&action.ability_id)
            .is_some_and(|usable_turn| *usable_turn > state.turn_number)
        {
            return Err(ActionError::IllegalAbility);
        }

        Ok(())
    }
}
