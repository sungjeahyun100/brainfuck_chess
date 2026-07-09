use std::collections::BTreeSet;

use crate::types::{GameState, PieceId};

use super::effect::ActionEffect;

pub fn build_state_diff_effects(before: &GameState, after: &GameState) -> Vec<ActionEffect> {
    let piece_ids: BTreeSet<PieceId> = before
        .pieces
        .keys()
        .chain(after.pieces.keys())
        .cloned()
        .collect();
    let mut effects = Vec::new();

    for piece_id in &piece_ids {
        let (Some(previous), Some(current)) =
            (before.pieces.get(piece_id), after.pieces.get(piece_id))
        else {
            continue;
        };

        if !previous.captured && current.captured {
            if let Some(at) = previous.current_square {
                effects.push(ActionEffect::CapturePiece {
                    piece_id: piece_id.clone(),
                    at,
                });
            }
        }
    }

    for piece_id in &piece_ids {
        let (Some(previous), Some(current)) =
            (before.pieces.get(piece_id), after.pieces.get(piece_id))
        else {
            continue;
        };

        if previous.in_pocket && !current.in_pocket {
            if let Some(to) = current.current_square {
                effects.push(ActionEffect::DropPiece {
                    piece_id: piece_id.clone(),
                    to,
                });
            }
        } else if !current.captured {
            if let (Some(from), Some(to)) = (previous.current_square, current.current_square) {
                if from != to {
                    effects.push(ActionEffect::MovePiece {
                        piece_id: piece_id.clone(),
                        from,
                        to,
                    });
                }
            }
        }

        if previous.type_id != current.type_id {
            effects.push(ActionEffect::PromotePiece {
                piece_id: piece_id.clone(),
                from_type: previous.type_id.clone(),
                to_type: current.type_id.clone(),
            });
        }
    }

    for piece_id in &piece_ids {
        let (Some(previous), Some(current)) =
            (before.pieces.get(piece_id), after.pieces.get(piece_id))
        else {
            continue;
        };

        let previous_ability = previous
            .active_ability
            .as_ref()
            .map(|ability| ability.ability_id.as_str());
        let current_ability = current
            .active_ability
            .as_ref()
            .map(|ability| ability.ability_id.as_str());

        if previous_ability != current_ability {
            if let Some(ability_id) = previous_ability {
                effects.push(ActionEffect::ClearPieceAbility {
                    piece_id: piece_id.clone(),
                    ability_id: ability_id.to_string(),
                });
            }
            if let Some(ability_id) = current_ability {
                effects.push(ActionEffect::SetPieceAbility {
                    piece_id: piece_id.clone(),
                    ability_id: ability_id.to_string(),
                });
            }
        }

        let mut cooldowns: Vec<_> = current.ability_cooldowns.iter().collect();
        cooldowns.sort_by(|left, right| left.0.cmp(right.0));
        for (ability_id, usable_turn) in cooldowns {
            if previous.ability_cooldowns.get(ability_id) != Some(usable_turn) {
                effects.push(ActionEffect::SetAbilityCooldown {
                    piece_id: piece_id.clone(),
                    ability_id: ability_id.clone(),
                    usable_turn: *usable_turn,
                });
            }
        }
    }

    if before.en_passant_target != after.en_passant_target
        || before.en_passant_available_to != after.en_passant_available_to
    {
        effects.push(ActionEffect::SetEnPassant {
            target: after.en_passant_target,
            available_to: after.en_passant_available_to.clone(),
        });
    }

    if before.current_player != after.current_player || before.turn_number != after.turn_number {
        effects.push(ActionEffect::AdvanceTurn {
            from_player: before.current_player.clone(),
            to_player: after.current_player.clone(),
            turn_number: after.turn_number,
        });
    }

    if before.result != after.result {
        if let Some(result) = after.result.clone() {
            effects.push(ActionEffect::EndGame { result });
        }
    }

    effects
}
