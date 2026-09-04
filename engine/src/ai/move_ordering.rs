use std::cmp::Ordering;

use crate::ai::beam::tactical_impact;
use crate::ai::types::AiAction;
use crate::types::{
    ActionEffects, CooldownUpdate, GameState, GlobalStateUpdate, PieceStateUpdate, PieceStateValue,
    PieceTypeTransition, PlayerId,
};

fn action_priority(state: &GameState, action: &AiAction) -> (u8, u32, i32) {
    match action {
        AiAction::Move(action) => {
            let Some(captured) = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id))
            else {
                return (2, 0, 0);
            };
            if captured.is_king {
                (7, u32::MAX, 0)
            } else {
                (6, captured.score, 0)
            }
        }
        AiAction::Drop(action) => {
            if action.captured_piece_id.is_some() {
                let impact = tactical_impact(state, &AiAction::Drop(action.clone()));
                return (
                    if impact.captures_king { 7 } else { 6 },
                    impact.removed_enemy_value,
                    0,
                );
            }
            (3, 0, drop_positional_score(state, action))
        }
        AiAction::Ability(_) => {
            let impact = tactical_impact(state, action);
            if impact.captures_king {
                (7, u32::MAX, 0)
            } else if impact.removed_enemy_pieces > 0 {
                (
                    6,
                    impact.removed_enemy_value,
                    impact.removed_enemy_pieces as i32,
                )
            } else {
                (3, 0, 0)
            }
        }
    }
}

fn drop_positional_score(state: &GameState, action: &crate::types::DropAction) -> i32 {
    let center_twice = state.board.size - 1;
    let center_distance =
        (action.to.file * 2 - center_twice).abs() + (action.to.rank * 2 - center_twice).abs();
    let center_bonus = state.board.size * 2 - center_distance;
    let king_bonus = |owner_matches: bool, weight: i32| {
        state
            .pieces
            .values()
            .find(|piece| {
                (piece.owner == action.player_id) == owner_matches
                    && piece.is_on_board()
                    && state
                        .piece_definitions
                        .get(&piece.type_id)
                        .is_some_and(|definition| definition.is_king)
            })
            .and_then(|king| king.current_square)
            .map_or(0, |square| {
                let distance = (square.file - action.to.file)
                    .abs()
                    .max((square.rank - action.to.rank).abs());
                (state.board.size - distance).max(0) * weight
            })
    };
    center_bonus + king_bonus(false, 3) + king_bonus(true, 1)
}

pub fn order_ai_actions(state: &GameState, actions: &mut [AiAction], _bot_player_id: &PlayerId) {
    let mut scored = actions
        .iter()
        .cloned()
        .map(|action| {
            let priority = action_priority(state, &action);
            (action, priority)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(left, left_priority), (right, right_priority)| {
        right_priority
            .cmp(left_priority)
            .then_with(|| canonical_action_cmp(left, right))
    });
    for (target, (action, _)) in actions.iter_mut().zip(scored) {
        *target = action;
    }
}

pub(crate) fn order_quiescence_actions(state: &GameState, actions: &mut [AiAction]) {
    actions.sort_by(|left, right| {
        quiescence_priority(state, right)
            .cmp(&quiescence_priority(state, left))
            .then_with(|| canonical_action_cmp(left, right))
    });
}

fn quiescence_priority(state: &GameState, action: &AiAction) -> (u8, u32) {
    let captured_value = |captured_id: Option<&crate::types::PieceId>| {
        captured_id
            .and_then(|id| state.pieces.get(id))
            .and_then(|piece| state.piece_definitions.get(&piece.type_id))
            .map_or(0, |definition| definition.score)
    };
    match action {
        AiAction::Move(action) => {
            let captured = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id));
            if captured.is_some_and(|definition| definition.is_king) {
                (7, u32::MAX)
            } else if action.captured_piece_id.is_some() && action.promotion.is_some() {
                (6, captured.map_or(0, |definition| definition.score))
            } else if action.captured_piece_id.is_some() {
                (5, captured.map_or(0, |definition| definition.score))
            } else if action.promotion.is_some() {
                (4, 0)
            } else {
                (0, 0)
            }
        }
        AiAction::Drop(action) => {
            let captured = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id));
            if captured.is_some_and(|definition| definition.is_king) {
                (7, u32::MAX)
            } else {
                (3, captured.map_or(0, |definition| definition.score))
            }
        }
        AiAction::Ability(action) => (2, captured_value(action.target_piece_id.as_ref())),
    }
}

fn canonical_action_cmp(left: &AiAction, right: &AiAction) -> Ordering {
    match (left, right) {
        (AiAction::Move(left), AiAction::Move(right)) => left
            .piece_id
            .cmp(&right.piece_id)
            .then_with(|| square_cmp(left.from, right.from))
            .then_with(|| square_cmp(left.to, right.to))
            .then_with(|| left.captured_piece_id.cmp(&right.captured_piece_id))
            .then_with(|| left.promotion.cmp(&right.promotion))
            .then_with(|| left.move_option_id.cmp(&right.move_option_id))
            .then_with(|| left.source_layer_ids.cmp(&right.source_layer_ids))
            .then_with(|| action_effects_cmp(&left.effects, &right.effects)),
        (AiAction::Drop(left), AiAction::Drop(right)) => left
            .piece_id
            .cmp(&right.piece_id)
            .then_with(|| square_cmp(left.to, right.to))
            .then_with(|| left.captured_piece_id.cmp(&right.captured_piece_id)),
        (AiAction::Ability(left), AiAction::Ability(right)) => left
            .piece_id
            .cmp(&right.piece_id)
            .then_with(|| left.ability_id.cmp(&right.ability_id))
            .then_with(|| left.target_piece_id.cmp(&right.target_piece_id))
            .then_with(|| left.pocket_piece_id.cmp(&right.pocket_piece_id))
            .then_with(|| optional_square_cmp(left.to, right.to))
            .then_with(|| {
                left.deployments
                    .iter()
                    .zip(&right.deployments)
                    .find_map(|(left, right)| {
                        let ordering = left
                            .pocket_piece_id
                            .cmp(&right.pocket_piece_id)
                            .then_with(|| square_cmp(left.to, right.to));
                        (ordering != Ordering::Equal).then_some(ordering)
                    })
                    .unwrap_or_else(|| left.deployments.len().cmp(&right.deployments.len()))
            }),
        (AiAction::Move(_), _) => Ordering::Less,
        (AiAction::Drop(_), AiAction::Move(_)) => Ordering::Greater,
        (AiAction::Drop(_), AiAction::Ability(_)) => Ordering::Less,
        (AiAction::Ability(_), _) => Ordering::Greater,
    }
}

fn action_effects_cmp(left: &ActionEffects, right: &ActionEffects) -> Ordering {
    slice_cmp_by(
        &left.global_state_updates,
        &right.global_state_updates,
        global_state_update_cmp,
    )
    .then_with(|| {
        slice_cmp_by(
            &left.piece_state_updates,
            &right.piece_state_updates,
            piece_state_update_cmp,
        )
    })
    .then_with(|| {
        slice_cmp_by(
            &left.cooldown_updates,
            &right.cooldown_updates,
            cooldown_update_cmp,
        )
    })
    .then_with(|| {
        option_cmp_by(
            left.piece_type_transition.as_ref(),
            right.piece_type_transition.as_ref(),
            piece_type_transition_cmp,
        )
    })
}

fn global_state_update_cmp(left: &GlobalStateUpdate, right: &GlobalStateUpdate) -> Ordering {
    left.key
        .cmp(&right.key)
        .then_with(|| left.value.cmp(&right.value))
}

fn piece_state_update_cmp(left: &PieceStateUpdate, right: &PieceStateUpdate) -> Ordering {
    left.piece_id
        .cmp(&right.piece_id)
        .then_with(|| left.key.cmp(&right.key))
        .then_with(|| piece_state_value_cmp(&left.value, &right.value))
}

fn piece_state_value_cmp(left: &PieceStateValue, right: &PieceStateValue) -> Ordering {
    match (left, right) {
        (PieceStateValue::Integer(left), PieceStateValue::Integer(right)) => left.cmp(right),
        (PieceStateValue::Boolean(left), PieceStateValue::Boolean(right)) => left.cmp(right),
        (PieceStateValue::Text(left), PieceStateValue::Text(right)) => left.cmp(right),
        (PieceStateValue::Integer(_), _) => Ordering::Less,
        (PieceStateValue::Boolean(_), PieceStateValue::Integer(_)) => Ordering::Greater,
        (PieceStateValue::Boolean(_), PieceStateValue::Text(_)) => Ordering::Less,
        (PieceStateValue::Text(_), _) => Ordering::Greater,
    }
}

fn cooldown_update_cmp(left: &CooldownUpdate, right: &CooldownUpdate) -> Ordering {
    left.piece_id
        .cmp(&right.piece_id)
        .then_with(|| left.move_option_id.cmp(&right.move_option_id))
        .then_with(|| left.remaining.cmp(&right.remaining))
}

fn piece_type_transition_cmp(left: &PieceTypeTransition, right: &PieceTypeTransition) -> Ordering {
    left.piece_id
        .cmp(&right.piece_id)
        .then_with(|| left.target_type_id.cmp(&right.target_type_id))
}

fn slice_cmp_by<T>(left: &[T], right: &[T], cmp: fn(&T, &T) -> Ordering) -> Ordering {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            let ordering = cmp(left, right);
            (ordering != Ordering::Equal).then_some(ordering)
        })
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn option_cmp_by<T>(left: Option<&T>, right: Option<&T>, cmp: fn(&T, &T) -> Ordering) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => cmp(left, right),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    }
}

fn square_cmp(left: crate::types::Square, right: crate::types::Square) -> Ordering {
    (left.file, left.rank).cmp(&(right.file, right.rank))
}

fn optional_square_cmp(
    left: Option<crate::types::Square>,
    right: Option<crate::types::Square>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => square_cmp(left, right),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    }
}
