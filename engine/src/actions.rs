use crate::endgame::apply_and_advance_turn;
use crate::legal_moves::{
    generate_piece_legal_drop_actions, is_legal_ability_action,
    generate_piece_legal_move_actions_with_options, MoveGenerationOptions,
};
use crate::types::{GamePhase, GameState, TurnAction};

/// Validate and apply every public gameplay action through one boundary.
pub fn submit_action(state: GameState, action: TurnAction) -> Result<GameState, String> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Err("게임이 이미 종료되었습니다.".into());
    }

    let legal = match &action {
        TurnAction::Move(action) => {
            action.player_id == state.current_player
                && generate_piece_legal_move_actions_with_options(
                    &state,
                    &action.piece_id,
                    &MoveGenerationOptions {
                        move_option_id: Some(action.move_option_id.clone()),
                    },
                )
                .into_iter()
                .any(|candidate| candidate == *action)
        }
        TurnAction::Drop(action) => {
            action.player_id == state.current_player
                && generate_piece_legal_drop_actions(&state, &action.piece_id)
                    .into_iter()
                    .any(|candidate| candidate == *action)
        }
        TurnAction::Ability(action) => is_legal_ability_action(&state, action),
    };
    if !legal {
        return Err("canonical legal action이 아닙니다.".into());
    }
    Ok(apply_and_advance_turn(state, action))
}
