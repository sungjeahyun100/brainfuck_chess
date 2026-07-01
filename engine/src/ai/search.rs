use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::ai::evaluate::{evaluate, WIN_SCORE};
use crate::ai::move_ordering::order_ai_actions;
use crate::ai::types::{
    AiAction, BotDecision, BotDifficulty, BotTurnResult, SearchLimits, SearchStats,
};
use crate::endgame::{apply_drop_action, apply_move_action};
use crate::legal_moves::{generate_legal_drop_actions, generate_legal_move_actions};
use crate::rules::{can_end_turn, end_turn};
use crate::types::{GamePhase, GameState, PlayerId, TurnAction, TurnMode};

pub fn generate_ai_actions(state: &GameState) -> Vec<AiAction> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Vec::new();
    }

    match state.turn_state.mode {
        TurnMode::Undecided => generate_legal_move_actions(state)
            .into_iter()
            .map(AiAction::Move)
            .chain(
                generate_legal_drop_actions(state)
                    .into_iter()
                    .map(AiAction::Drop),
            )
            .collect(),
        TurnMode::Move => {
            let mut actions: Vec<_> = generate_legal_move_actions(state)
                .into_iter()
                .map(AiAction::Move)
                .collect();
            if can_end_turn(state) {
                actions.push(AiAction::EndTurn);
            }
            actions
        }
        TurnMode::Drop => {
            let already_dropped = state
                .turn_state
                .actions
                .iter()
                .any(|action| matches!(action, TurnAction::Drop(_)));
            if already_dropped {
                vec![AiAction::EndTurn]
            } else {
                generate_legal_drop_actions(state)
                    .into_iter()
                    .map(AiAction::Drop)
                    .collect()
            }
        }
    }
}

pub fn apply_ai_action(mut state: GameState, action: &AiAction) -> Result<GameState, String> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Err("게임이 이미 종료되었습니다.".into());
    }

    match action {
        AiAction::Move(action) => {
            let legal = generate_legal_move_actions(&state)
                .iter()
                .any(|candidate| candidate == action);
            if !legal {
                return Err("AI가 합법적이지 않은 이동을 선택했습니다.".into());
            }
            state.turn_state.mode = TurnMode::Move;
            let state = apply_move_action(state, action.clone());
            if state.phase == GamePhase::Ended || state.result.is_some() {
                Ok(state)
            } else {
                Ok(end_turn(state))
            }
        }
        AiAction::Drop(action) => {
            let legal = generate_legal_drop_actions(&state)
                .iter()
                .any(|candidate| candidate == action);
            if !legal {
                return Err("AI가 합법적이지 않은 착수를 선택했습니다.".into());
            }
            state.turn_state.mode = TurnMode::Drop;
            Ok(end_turn(apply_drop_action(state, action.clone())))
        }
        AiAction::EndTurn => {
            if !can_end_turn(&state) {
                return Err("행동 없이 턴을 종료할 수 없습니다.".into());
            }
            Ok(end_turn(state))
        }
    }
}

struct SearchContext<'a> {
    bot_player_id: &'a PlayerId,
    limits: &'a SearchLimits,
    started: Instant,
    stats: SearchStats,
}

impl SearchContext<'_> {
    fn exhausted(&self) -> bool {
        self.stats.searched_nodes >= self.limits.max_nodes
            || self.started.elapsed() >= Duration::from_millis(self.limits.hard_time_ms)
    }
}

fn search(
    state: GameState,
    depth: u8,
    ply: u8,
    mut alpha: i32,
    mut beta: i32,
    context: &mut SearchContext<'_>,
) -> i32 {
    context.stats.searched_nodes += 1;
    context.stats.depth_reached = context.stats.depth_reached.max(ply);
    if depth == 0
        || state.phase == GamePhase::Ended
        || state.result.is_some()
        || context.exhausted()
    {
        return evaluate(&state, context.bot_player_id);
    }

    let maximizing = &state.current_player == context.bot_player_id;
    let mut actions = generate_ai_actions(&state);
    if actions.is_empty() {
        return evaluate(&state, context.bot_player_id);
    }
    order_ai_actions(&state, &mut actions, context.bot_player_id);

    let mut best = if maximizing { i32::MIN } else { i32::MAX };
    for action in actions {
        if context.exhausted() {
            break;
        }
        let Ok(next_state) = apply_ai_action(state.clone(), &action) else {
            continue;
        };
        let score = search(next_state, depth - 1, ply + 1, alpha, beta, context);
        if maximizing {
            best = best.max(score);
            alpha = alpha.max(best);
        } else {
            best = best.min(score);
            beta = beta.min(best);
        }
        if beta <= alpha {
            break;
        }
    }

    if best == i32::MIN || best == i32::MAX {
        evaluate(&state, context.bot_player_id)
    } else {
        best
    }
}

fn easy_choice_index(candidate_count: usize) -> usize {
    if candidate_count <= 1 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.subsec_nanos() as usize);
    nanos % candidate_count.min(3)
}

pub fn choose_bot_action(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
) -> BotDecision {
    let limits = difficulty.limits();
    let started = Instant::now();
    let mut actions = generate_ai_actions(state);
    order_ai_actions(state, &mut actions, bot_player_id);

    if actions.is_empty() {
        return BotDecision {
            action: AiAction::EndTurn,
            score: evaluate(state, bot_player_id),
            searched_nodes: 0,
            depth_reached: 0,
        };
    }
    let fallback_action = actions[0].clone();

    let mut context = SearchContext {
        bot_player_id,
        limits: &limits,
        started,
        stats: SearchStats::default(),
    };
    let mut scored = Vec::new();
    for action in actions {
        if context.exhausted()
            || (!scored.is_empty()
                && started.elapsed() >= Duration::from_millis(limits.soft_time_ms))
        {
            break;
        }
        let Ok(next_state) = apply_ai_action(state.clone(), &action) else {
            continue;
        };
        let score = if next_state
            .result
            .as_ref()
            .and_then(|result| result.winner.as_ref())
            == Some(bot_player_id)
        {
            WIN_SCORE
        } else {
            search(
                next_state,
                limits.max_depth_actions.saturating_sub(1),
                1,
                i32::MIN + 1,
                i32::MAX,
                &mut context,
            )
        };
        scored.push((action, score));
    }

    if scored.is_empty() {
        return BotDecision {
            action: fallback_action,
            score: evaluate(state, bot_player_id),
            searched_nodes: context.stats.searched_nodes,
            depth_reached: context.stats.depth_reached,
        };
    }

    let maximizing = &state.current_player == bot_player_id;
    scored.sort_by(|left, right| {
        if maximizing {
            right.1.cmp(&left.1)
        } else {
            left.1.cmp(&right.1)
        }
    });
    let winning_capture = scored.iter().position(|(_, score)| *score >= WIN_SCORE);
    let index = if difficulty == BotDifficulty::Easy && winning_capture.is_none() {
        easy_choice_index(scored.len())
    } else {
        winning_capture.unwrap_or(0)
    };
    let (action, score) = scored.swap_remove(index.min(scored.len().saturating_sub(1)));

    BotDecision {
        action,
        score,
        searched_nodes: context.stats.searched_nodes,
        depth_reached: context.stats.depth_reached,
    }
}

pub fn play_bot_turn_detailed(
    mut state: GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
) -> Result<BotTurnResult, String> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Err("게임이 이미 종료되었습니다.".into());
    }
    if &state.current_player != bot_player_id {
        return Err("현재 턴 플레이어와 bot_player_id가 일치하지 않습니다.".into());
    }

    let started = Instant::now();
    let limits = difficulty.limits();
    let mut actions = Vec::new();
    let mut searched_nodes = 0_u64;
    let mut depth_reached = 0_u8;

    while &state.current_player == bot_player_id && state.phase != GamePhase::Ended {
        if actions.len() >= usize::from(limits.max_actions_per_turn) {
            return Err("봇 턴 행동 제한 안에 턴을 종료하지 못했습니다.".into());
        }

        let must_end = can_end_turn(&state)
            && (matches!(state.turn_state.mode, TurnMode::Drop)
                || actions.len() + 1 >= usize::from(limits.max_actions_per_turn));
        let decision = if must_end {
            BotDecision {
                action: AiAction::EndTurn,
                score: evaluate(&state, bot_player_id),
                searched_nodes: 0,
                depth_reached: 0,
            }
        } else {
            choose_bot_action(&state, bot_player_id, difficulty)
        };
        searched_nodes = searched_nodes.saturating_add(decision.searched_nodes);
        depth_reached = depth_reached.max(decision.depth_reached);

        if matches!(decision.action, AiAction::EndTurn) && !can_end_turn(&state) {
            return Err("봇이 수행할 합법 행동이 없어 턴을 종료할 수 없습니다.".into());
        }
        state = apply_ai_action(state, &decision.action)?;
        let ended_turn = matches!(decision.action, AiAction::EndTurn);
        actions.push(decision.action);
        if ended_turn || state.phase == GamePhase::Ended || state.result.is_some() {
            break;
        }
    }

    Ok(BotTurnResult {
        state,
        actions,
        searched_nodes,
        depth_reached,
        elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

pub fn play_bot_turn(
    state: GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
) -> Result<GameState, String> {
    play_bot_turn_detailed(state, bot_player_id, difficulty).map(|result| result.state)
}
