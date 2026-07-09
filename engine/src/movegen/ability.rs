use crate::context::GameContext;
use crate::types::*;

#[derive(Debug, Clone, Default)]
pub struct MoveGenerationOptions {
    pub ability_id: Option<String>,
}

pub(crate) fn can_use_selected_ability(
    context: &GameContext<'_>,
    piece: &Piece,
    definition: &PieceDefinition,
    ability_id: &str,
) -> Option<PieceAbilityDefinition> {
    let state = context.state;

    if piece
        .ability_cooldowns
        .get(ability_id)
        .is_some_and(|usable_turn| *usable_turn > state.turn_number)
    {
        return None;
    }

    let ability = definition
        .abilities
        .iter()
        .find(|ability| ability.id == ability_id)?
        .clone();

    if ability.once_per_turn
        && state.turn_state.actions.iter().any(|existing| {
            matches!(
                existing,
                TurnAction::ActivateAbility(previous)
                    if previous.piece_id == piece.id && previous.ability_id == ability_id
            )
        })
    {
        return None;
    }

    Some(ability)
}
