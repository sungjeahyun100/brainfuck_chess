use crate::endgame::{apply_activate_ability_action, apply_drop_action, apply_move_action};
use crate::legal_moves::{
    generate_piece_legal_drop_actions, generate_piece_legal_move_actions_with_options,
    MoveGenerationOptions,
};
use crate::types::{GamePhase, GameState, TurnAction, TurnMode};

/// Validate and apply every public gameplay action through one boundary.
/// Turn advancement remains an explicit caller policy because ability
/// activation deliberately does not end a turn.
pub fn submit_action(mut state: GameState, action: TurnAction) -> Result<GameState, String> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Err("게임이 이미 종료되었습니다.".into());
    }

    let player_id = match &action {
        TurnAction::Move(action) => &action.player_id,
        TurnAction::Drop(action) => &action.player_id,
        TurnAction::ActivateAbility(action) => &action.player_id,
    };
    if player_id != &state.current_player {
        return Err("현재 턴 플레이어만 행동할 수 있습니다.".into());
    }

    match action {
        TurnAction::Move(action) => {
            if state.turn_state.mode == TurnMode::Drop {
                return Err("착수 턴에는 이동할 수 없습니다.".into());
            }
            let legal = generate_piece_legal_move_actions_with_options(
                &state,
                &action.piece_id,
                &MoveGenerationOptions {
                    ability_id: action.ability_id.clone(),
                },
            )
            .into_iter()
            .any(|candidate| candidate == action);
            if !legal {
                return Err("합법적이지 않은 이동입니다.".into());
            }
            state.turn_state.mode = TurnMode::Move;
            Ok(apply_move_action(state, action))
        }
        TurnAction::Drop(action) => {
            if state.turn_state.mode == TurnMode::Move {
                return Err("이동 턴에는 착수할 수 없습니다.".into());
            }
            let legal = generate_piece_legal_drop_actions(&state, &action.piece_id)
                .into_iter()
                .any(|candidate| candidate == action);
            if !legal {
                return Err("착수 가능한 칸이 아닙니다.".into());
            }
            state.turn_state.mode = TurnMode::Drop;
            Ok(apply_drop_action(state, action))
        }
        TurnAction::ActivateAbility(action) => {
            if state.turn_state.mode == TurnMode::Drop {
                return Err("착수 턴에는 능력을 발동할 수 없습니다.".into());
            }
            let piece = state
                .pieces
                .get(&action.piece_id)
                .ok_or_else(|| "기물을 찾을 수 없습니다.".to_string())?;
            if piece.owner != state.current_player || !piece.is_on_board() {
                return Err("보드 위의 자신의 기물 능력만 발동할 수 있습니다.".into());
            }
            if piece.active_ability.is_some() {
                return Err("이미 활성화된 능력이 있습니다.".into());
            }
            let ability = state
                .piece_definitions
                .get(&piece.type_id)
                .and_then(|definition| {
                    definition
                        .abilities
                        .iter()
                        .find(|ability| ability.id == action.ability_id)
                })
                .ok_or_else(|| "해당 기물에 없는 능력입니다.".to_string())?;
            if piece
                .ability_cooldowns
                .get(&action.ability_id)
                .is_some_and(|turn| *turn > state.turn_number)
            {
                return Err("아직 재사용 대기 중인 능력입니다.".into());
            }
            if ability.once_per_turn
                && state.turn_state.actions.iter().any(|existing| {
                    matches!(existing, TurnAction::ActivateAbility(previous)
                        if previous.piece_id == action.piece_id
                            && previous.ability_id == action.ability_id)
                })
            {
                return Err("이 능력은 같은 턴에 한 번만 발동할 수 있습니다.".into());
            }
            state.turn_state.mode = TurnMode::Move;
            Ok(apply_activate_ability_action(state, action))
        }
    }
}
