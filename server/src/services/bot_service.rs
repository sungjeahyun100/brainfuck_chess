use brainfuck_chess_engine::{
    actions::{
        apply_turn_action_with_effects, effect_builder::build_state_diff_effects, ActionEffect,
    },
    ai::{play_bot_turn_detailed, AiAction, BotDifficulty},
    rules::{can_end_turn, end_turn},
    types::{GameState, PlayerId, TurnAction},
};

use crate::dto::timeline::ActionTimelineFrame;

pub struct BotTurnExecution {
    pub state: GameState,
    pub actions: Vec<AiAction>,
    pub timeline: Vec<ActionTimelineFrame>,
    pub searched_nodes: u64,
    pub depth_reached: u8,
    pub elapsed_ms: u64,
}

pub fn run_bot_turn(
    state: GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
) -> Result<BotTurnExecution, String> {
    let result = play_bot_turn_detailed(state.clone(), bot_player_id, difficulty)?;
    let timeline = build_action_timeline(state, &result.actions)?;

    Ok(BotTurnExecution {
        state: result.state,
        actions: result.actions,
        timeline,
        searched_nodes: result.searched_nodes,
        depth_reached: result.depth_reached,
        elapsed_ms: result.elapsed_ms,
    })
}

fn build_action_timeline(
    mut state: GameState,
    actions: &[AiAction],
) -> Result<Vec<ActionTimelineFrame>, String> {
    let mut timeline = Vec::with_capacity(actions.len());

    for action in actions {
        let (next_state, effects) = match action {
            AiAction::Move(action) => {
                let applied =
                    apply_turn_action_with_effects(state, TurnAction::Move(action.clone()))
                        .map_err(|error| {
                            format!("봇 이동 timeline을 생성할 수 없습니다: {error}")
                        })?;
                (applied.state, applied.effects)
            }
            AiAction::Drop(action) => {
                let applied =
                    apply_turn_action_with_effects(state, TurnAction::Drop(action.clone()))
                        .map_err(|error| {
                            format!("봇 착수 timeline을 생성할 수 없습니다: {error}")
                        })?;
                (applied.state, applied.effects)
            }
            AiAction::EndTurn => apply_end_turn_with_effects(state)?,
        };

        timeline.push(ActionTimelineFrame {
            action: action.clone(),
            effects,
        });
        state = next_state;
    }

    Ok(timeline)
}

fn apply_end_turn_with_effects(state: GameState) -> Result<(GameState, Vec<ActionEffect>), String> {
    if !can_end_turn(&state) {
        return Err("행동 없이 봇 턴 종료 timeline을 생성할 수 없습니다.".into());
    }

    let next = end_turn(state.clone());
    let effects = build_state_diff_effects(&state, &next);
    Ok((next, effects))
}
