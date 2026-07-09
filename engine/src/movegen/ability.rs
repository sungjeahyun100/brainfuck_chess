use std::collections::{HashMap, HashSet};

use crate::chessembly::run_effective_chessembly_for_context;
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

pub(crate) fn run_selected_ability_for_piece(
    context: &GameContext<'_>,
    piece: &Piece,
    definition: &PieceDefinition,
    ability: &PieceAbilityDefinition,
    player_id: &PlayerId,
    empty_global_state: &HashMap<String, i32>,
    empty_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    let mut ability_piece = piece.clone();
    ability_piece.active_ability = Some(ActiveAbilityState {
        ability_id: ability.id.clone(),
        activated_turn_number: context.state.turn_number,
        activated_player: player_id.clone(),
        duration: ability.duration.clone(),
    });
    run_effective_chessembly_for_context(
        context,
        &ability_piece,
        definition,
        player_id.clone(),
        empty_global_state,
        empty_maps,
    )
}
