use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::actions::{apply_canonical_action, submit_action};
use crate::ai::evaluate::{evaluate, WIN_SCORE};
use crate::ai::move_ordering::order_ai_actions;
use crate::ai::types::{
    ActionTimelineFrame, AiAction, BotDecision, BotDifficulty, BotTurnResult, SearchLimits,
    SearchStats,
};
use crate::legal_moves::{
    generate_legal_ability_actions, generate_legal_drop_actions, generate_legal_move_actions,
};
use crate::types::{GamePhase, GameState, PlayerId, TurnAction};

pub fn generate_ai_actions(state: &GameState) -> Vec<AiAction> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Vec::new();
    }

    generate_legal_move_actions(state)
        .into_iter()
        .map(AiAction::Move)
        .chain(
            generate_legal_drop_actions(state)
                .into_iter()
                .map(AiAction::Drop),
        )
        .chain(
            generate_legal_ability_actions(state)
                .into_iter()
                .map(AiAction::Ability),
        )
        .collect()
}

pub fn apply_ai_action(state: GameState, action: &AiAction) -> Result<GameState, String> {
    submit_action(state, to_turn_action(action))
}

fn to_turn_action(action: &AiAction) -> TurnAction {
    match action {
        AiAction::Move(action) => TurnAction::Move(action.clone()),
        AiAction::Drop(action) => TurnAction::Drop(action.clone()),
        AiAction::Ability(action) => TurnAction::Ability(action.clone()),
    }
}

fn apply_generated_action(state: GameState, action: &AiAction) -> GameState {
    apply_canonical_action(state, to_turn_action(action))
}

struct SearchContext<'a> {
    bot_player_id: &'a PlayerId,
    limits: &'a SearchLimits,
    started: Instant,
    stats: SearchStats,
}

impl SearchContext<'_> {
    fn hard_limit_reached(&self) -> bool {
        self.stats.searched_nodes >= self.limits.max_nodes
            || self.started.elapsed() >= Duration::from_millis(self.limits.hard_time_ms)
    }

    fn soft_limit_reached(&self) -> bool {
        self.started.elapsed() >= Duration::from_millis(self.limits.soft_time_ms)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchOutcome {
    Complete(i32),
    Aborted,
}

fn alpha_beta(
    state: GameState,
    depth: u8,
    ply: u8,
    mut alpha: i32,
    mut beta: i32,
    context: &mut SearchContext<'_>,
) -> SearchOutcome {
    if context.hard_limit_reached() {
        return SearchOutcome::Aborted;
    }
    context.stats.searched_nodes += 1;
    context.stats.depth_reached = context.stats.depth_reached.max(ply);
    if depth == 0 || state.phase == GamePhase::Ended || state.result.is_some() {
        return SearchOutcome::Complete(evaluate(&state, context.bot_player_id));
    }

    let maximizing = &state.current_player == context.bot_player_id;
    let mut actions = generate_ai_actions(&state);
    if actions.is_empty() {
        return SearchOutcome::Complete(evaluate(&state, context.bot_player_id));
    }
    order_ai_actions(&state, &mut actions, context.bot_player_id);

    let mut best = if maximizing { i32::MIN } else { i32::MAX };
    for action in actions {
        if context.hard_limit_reached() {
            return SearchOutcome::Aborted;
        }
        let next_state = apply_generated_action(state.clone(), &action);
        let SearchOutcome::Complete(score) =
            alpha_beta(next_state, depth - 1, ply + 1, alpha, beta, context)
        else {
            return SearchOutcome::Aborted;
        };
        if maximizing {
            best = best.max(score);
            alpha = alpha.max(best);
        } else {
            best = best.min(score);
            beta = beta.min(best);
        }
        if beta <= alpha {
            context.stats.beta_cutoffs += 1;
            break;
        }
    }

    if best == i32::MIN || best == i32::MAX {
        SearchOutcome::Complete(evaluate(&state, context.bot_player_id))
    } else {
        SearchOutcome::Complete(best)
    }
}

struct RootSearchResult {
    best: Option<(AiAction, i32)>,
    scores: Vec<(AiAction, i32)>,
    completed: bool,
}

fn search_root(
    state: &GameState,
    actions: &[AiAction],
    depth: u8,
    context: &mut SearchContext<'_>,
) -> RootSearchResult {
    let maximizing = &state.current_player == context.bot_player_id;
    let mut best: Option<(AiAction, i32)> = None;
    let mut scores = Vec::with_capacity(actions.len());
    let mut alpha = i32::MIN + 1;
    let mut beta = i32::MAX;

    for action in actions {
        if context.hard_limit_reached() {
            return RootSearchResult {
                best: None,
                scores: Vec::new(),
                completed: false,
            };
        }
        let next_state = apply_generated_action(state.clone(), action);
        let score = if next_state
            .result
            .as_ref()
            .and_then(|result| result.winner.as_ref())
            == Some(context.bot_player_id)
        {
            WIN_SCORE
        } else {
            let SearchOutcome::Complete(score) =
                alpha_beta(next_state, depth.saturating_sub(1), 1, alpha, beta, context)
            else {
                return RootSearchResult {
                    best: None,
                    scores: Vec::new(),
                    completed: false,
                };
            };
            score
        };
        let improves = best.as_ref().is_none_or(|(_, current)| {
            if maximizing {
                score > *current
            } else {
                score < *current
            }
        });
        if improves {
            best = Some((action.clone(), score));
        }
        scores.push((action.clone(), score));
        if maximizing {
            alpha = alpha.max(score);
        } else {
            beta = beta.min(score);
        }
    }
    RootSearchResult {
        best,
        scores,
        completed: true,
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
) -> Option<BotDecision> {
    let limits = difficulty.limits();
    choose_bot_action_with_limits(state, bot_player_id, difficulty, limits)
}

fn choose_bot_action_with_limits(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
    limits: SearchLimits,
) -> Option<BotDecision> {
    let started = Instant::now();
    let mut actions = generate_ai_actions(state);
    order_ai_actions(state, &mut actions, bot_player_id);

    if actions.is_empty() {
        return None;
    }
    let fallback_action = actions[0].clone();

    let mut context = SearchContext {
        bot_player_id,
        limits: &limits,
        started,
        stats: SearchStats::default(),
    };
    let root_result = if context.soft_limit_reached() {
        RootSearchResult {
            best: None,
            scores: Vec::new(),
            completed: false,
        }
    } else {
        search_root(state, &actions, limits.max_depth_actions, &mut context)
    };
    if root_result.completed {
        context.stats.completed_depth = limits.max_depth_actions;
    }

    let Some((best_action, best_score)) = root_result.best else {
        let score = evaluate(state, bot_player_id);
        return Some(BotDecision {
            action: fallback_action,
            score,
            searched_nodes: context.stats.searched_nodes,
            depth_reached: context.stats.depth_reached,
            completed_depth: context.stats.completed_depth,
            stats: context.stats,
        });
    };

    // Easy retains a small amount of variety without using incomplete scores.
    let (action, score) = if difficulty == BotDifficulty::Easy && best_score < WIN_SCORE {
        let mut scores = root_result.scores;
        let maximizing = &state.current_player == bot_player_id;
        scores.sort_by(|left, right| {
            if maximizing {
                right.1.cmp(&left.1)
            } else {
                left.1.cmp(&right.1)
            }
        });
        let index = easy_choice_index(scores.len());
        scores.swap_remove(index)
    } else {
        (best_action, best_score)
    };

    Some(BotDecision {
        action,
        score,
        searched_nodes: context.stats.searched_nodes,
        depth_reached: context.stats.depth_reached,
        completed_depth: context.stats.completed_depth,
        stats: context.stats,
    })
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
    let decision = choose_bot_action(&state, bot_player_id, difficulty)
        .ok_or_else(|| "봇이 수행할 합법 행동이 없습니다.".to_string())?;
    let searched_nodes = decision.searched_nodes;
    let depth_reached = decision.depth_reached;
    let completed_depth = decision.completed_depth;
    let stats = decision.stats;
    let action = decision.action;
    state = apply_ai_action(state, &action)?;
    let actions = vec![action.clone()];
    let timeline = vec![ActionTimelineFrame {
        action,
        state: state.clone(),
    }];

    Ok(BotTurnResult {
        state,
        actions,
        timeline,
        searched_nodes,
        depth_reached,
        completed_depth,
        stats,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::pieces::default_pieces::all_default_definitions;
    use crate::rules::create_board;
    use crate::types::{ChessemblyProgramCache, Deck, Piece, Square};

    fn searchable_state() -> GameState {
        let definitions: HashMap<_, _> = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect();
        let players = ["white", "black"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    crate::types::Player {
                        id: id.into(),
                        deck: Deck {
                            player_id: id.into(),
                            starting_pieces: Vec::new(),
                            pocket_pieces: Vec::new(),
                            score_limit: 39,
                            total_score: 0,
                        },
                        captured_pieces: Vec::new(),
                    },
                )
            })
            .collect();
        let mut state = GameState {
            id: "abort-propagation".into(),
            board: create_board(8),
            pieces: HashMap::new(),
            chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
            piece_definitions: definitions,
            custom_piece_manifest: Vec::new(),
            players,
            current_player: "white".into(),
            turn_number: 1,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: Vec::new(),
            result: None,
        };
        for (id, owner, type_id, square) in [
            ("wk", "white", "king", Square::new(0, 0)),
            ("wr", "white", "rook", Square::new(3, 0)),
            ("bk", "black", "king", Square::new(7, 7)),
        ] {
            let piece_id: crate::types::PieceId = id.into();
            state
                .board
                .squares
                .insert(square.to_id(), Some(piece_id.clone()));
            state.pieces.insert(
                piece_id.clone(),
                Piece {
                    id: piece_id.clone(),
                    owner: owner.into(),
                    type_id: type_id.into(),
                    current_square: Some(square),
                    in_pocket: false,
                    captured: false,
                    has_moved: false,
                    state: HashMap::new(),
                    move_option_cooldowns: HashMap::new(),
                },
            );
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .starting_pieces
                .push(piece_id);
        }
        state
    }

    #[test]
    fn node_limit_abort_is_not_recorded_as_completed_depth() {
        let state = searchable_state();
        let decision = choose_bot_action_with_limits(
            &state,
            &"white".into(),
            BotDifficulty::Normal,
            SearchLimits {
                max_depth_actions: 3,
                max_nodes: 1,
                soft_time_ms: 1_000,
                hard_time_ms: 1_000,
            },
        )
        .unwrap();
        assert_eq!(decision.searched_nodes, 1);
        assert_eq!(decision.completed_depth, 0);
        assert!(generate_ai_actions(&state).contains(&decision.action));
    }
}
