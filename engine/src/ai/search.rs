use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::actions::{apply_canonical_action, submit_action};
use crate::ai::evaluate::{evaluate, WIN_SCORE};
use crate::ai::move_ordering::{order_ai_actions, order_quiescence_actions};
use crate::ai::transposition_table::{
    BoundType, PositionKey, TranspositionEntry, TranspositionTable,
};
use crate::ai::types::{
    ActionTimelineFrame, AiAction, BotDecision, BotDifficulty, BotTurnResult, SearchLimits,
    SearchOptions, SearchStats,
};
use crate::legal_moves::{
    generate_legal_ability_actions, generate_legal_drop_actions, generate_legal_move_actions,
    generate_piece_legal_ability_actions, generate_piece_legal_drop_actions,
};
use crate::types::{
    AbilityAction, DropAction, GamePhase, GameState, MoveAction, PlayerId, TurnAction,
};

const MAX_QUIESCENCE_PLIES: u8 = 8;

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
    transposition_table: Option<TranspositionTable>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TableProbe {
    cutoff_score: Option<i32>,
}

fn apply_table_bound(
    entry: &TranspositionEntry,
    depth: u8,
    alpha: &mut i32,
    beta: &mut i32,
) -> TableProbe {
    if entry.depth < depth {
        return TableProbe { cutoff_score: None };
    }
    match entry.bound {
        BoundType::Exact => {
            return TableProbe {
                cutoff_score: Some(entry.score),
            };
        }
        BoundType::LowerBound => *alpha = (*alpha).max(entry.score),
        BoundType::UpperBound => *beta = (*beta).min(entry.score),
    }
    TableProbe {
        cutoff_score: (*alpha >= *beta).then_some(entry.score),
    }
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
    if depth == 0 {
        return quiescence_search(state, alpha, beta, 0, context, false);
    }
    let original_alpha = alpha;
    let original_beta = beta;
    let position_key = context
        .transposition_table
        .is_some()
        .then(|| PositionKey::from_state(&state));
    let table_entry = context.transposition_table.as_ref().and_then(|table| {
        context.stats.tt_probes += 1;
        table
            .get(
                position_key
                    .as_ref()
                    .expect("enabled TT must have a position key"),
            )
            .cloned()
    });
    if let Some(entry) = table_entry.as_ref() {
        context.stats.tt_hits += 1;
        let probe = apply_table_bound(entry, depth, &mut alpha, &mut beta);
        if let Some(score) = probe.cutoff_score {
            context.stats.tt_cutoffs += 1;
            return SearchOutcome::Complete(score);
        }
    }
    if state.phase == GamePhase::Ended || state.result.is_some() {
        let score = evaluate(&state, context.bot_player_id);
        store_table_entry(
            context,
            position_key,
            TranspositionEntry {
                depth,
                score,
                bound: BoundType::Exact,
                best_action: None,
            },
        );
        return SearchOutcome::Complete(score);
    }

    let maximizing = &state.current_player == context.bot_player_id;
    let mut actions = generate_ai_actions(&state);
    if actions.is_empty() {
        let score = evaluate(&state, context.bot_player_id);
        store_table_entry(
            context,
            position_key,
            TranspositionEntry {
                depth,
                score,
                bound: BoundType::Exact,
                best_action: None,
            },
        );
        return SearchOutcome::Complete(score);
    }
    order_ai_actions(&state, &mut actions, context.bot_player_id);
    if let Some(best_action) = table_entry.and_then(|entry| entry.best_action) {
        if let Some(index) = actions.iter().position(|action| action == &best_action) {
            actions.swap(0, index);
        }
    }

    let mut best = if maximizing { i32::MIN } else { i32::MAX };
    let mut best_action = None;
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
            if score > best {
                best = score;
                best_action = Some(action.clone());
            }
            alpha = alpha.max(best);
        } else {
            if score < best {
                best = score;
                best_action = Some(action.clone());
            }
            beta = beta.min(best);
        }
        if beta <= alpha {
            context.stats.beta_cutoffs += 1;
            break;
        }
    }

    let score = if best == i32::MIN || best == i32::MAX {
        evaluate(&state, context.bot_player_id)
    } else {
        best
    };
    let bound = if score <= original_alpha {
        BoundType::UpperBound
    } else if score >= original_beta {
        BoundType::LowerBound
    } else {
        BoundType::Exact
    };
    store_table_entry(
        context,
        position_key,
        TranspositionEntry {
            depth,
            score,
            bound,
            best_action,
        },
    );
    SearchOutcome::Complete(score)
}

fn is_noisy_move(action: &MoveAction) -> bool {
    action.captured_piece_id.is_some() || action.promotion.is_some()
}

fn is_noisy_drop(action: &DropAction) -> bool {
    action.captured_piece_id.is_some()
}

fn is_noisy_ability(state: &GameState, action: &AbilityAction) -> bool {
    action.ability_id == "recall"
        && action
            .target_piece_id
            .as_ref()
            .and_then(|id| state.pieces.get(id))
            .is_some_and(|target| target.owner != action.player_id)
}

fn generate_quiescence_actions(state: &GameState) -> Vec<AiAction> {
    let moves = generate_legal_move_actions(state)
        .into_iter()
        .filter(is_noisy_move)
        .map(AiAction::Move);

    let mut capture_drop_piece_ids = state
        .players
        .get(&state.current_player)
        .into_iter()
        .flat_map(|player| &player.deck.pocket_pieces)
        .filter(|piece_id| {
            state
                .pieces
                .get(*piece_id)
                .and_then(|piece| state.piece_definitions.get(&piece.type_id))
                .is_some_and(|definition| definition.can_capture_on_drop)
        })
        .cloned()
        .collect::<Vec<_>>();
    capture_drop_piece_ids.sort();
    let drops = capture_drop_piece_ids.into_iter().flat_map(|piece_id| {
        generate_piece_legal_drop_actions(state, &piece_id)
            .into_iter()
            .filter(is_noisy_drop)
            .map(AiAction::Drop)
    });

    let mut recall_actor_ids = state
        .pieces
        .iter()
        .filter_map(|(piece_id, piece)| {
            (piece.owner == state.current_player
                && piece.is_on_board()
                && state
                    .piece_definitions
                    .get(&piece.type_id)
                    .is_some_and(|definition| {
                        definition
                            .move_options
                            .iter()
                            .any(|option| option.id == "recall")
                    }))
            .then_some(piece_id.clone())
        })
        .collect::<Vec<_>>();
    recall_actor_ids.sort();
    let abilities = recall_actor_ids.into_iter().flat_map(|piece_id| {
        generate_piece_legal_ability_actions(state, &piece_id, "recall")
            .into_iter()
            .filter(|action| is_noisy_ability(state, action))
            .map(AiAction::Ability)
    });

    let mut actions = moves.chain(drops).chain(abilities).collect::<Vec<_>>();
    order_quiescence_actions(state, &mut actions);
    actions
}

fn quiescence_search(
    state: GameState,
    mut alpha: i32,
    mut beta: i32,
    qply: u8,
    context: &mut SearchContext<'_>,
    count_search_node: bool,
) -> SearchOutcome {
    if count_search_node {
        if context.hard_limit_reached() {
            return SearchOutcome::Aborted;
        }
        context.stats.searched_nodes += 1;
    }
    context.stats.qnodes += 1;

    let stand_pat = evaluate(&state, context.bot_player_id);
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return SearchOutcome::Complete(stand_pat);
    }

    let maximizing = &state.current_player == context.bot_player_id;
    let mut best = stand_pat;
    if maximizing {
        if stand_pat >= beta {
            return SearchOutcome::Complete(stand_pat);
        }
        alpha = alpha.max(stand_pat);
    } else {
        if stand_pat <= alpha {
            return SearchOutcome::Complete(stand_pat);
        }
        beta = beta.min(stand_pat);
    }
    if qply >= MAX_QUIESCENCE_PLIES {
        return SearchOutcome::Complete(stand_pat);
    }

    for action in generate_quiescence_actions(&state) {
        if context.hard_limit_reached() {
            return SearchOutcome::Aborted;
        }
        let next_state = apply_generated_action(state.clone(), &action);
        let SearchOutcome::Complete(score) =
            quiescence_search(next_state, alpha, beta, qply + 1, context, true)
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
            break;
        }
    }
    SearchOutcome::Complete(best)
}

fn store_table_entry(
    context: &mut SearchContext<'_>,
    key: Option<PositionKey>,
    entry: TranspositionEntry,
) {
    if key.is_some_and(|key| {
        context
            .transposition_table
            .as_mut()
            .is_some_and(|table| table.store(key, entry))
    }) {
        context.stats.tt_stores += 1;
    }
}

struct RootSearchResult {
    best: Option<(AiAction, i32)>,
    scores: Vec<(AiAction, i32)>,
    completed: bool,
}

fn prioritize_action(actions: &mut [AiAction], priority: Option<&AiAction>) {
    let Some(priority) = priority else {
        return;
    };
    if let Some(index) = actions.iter().position(|action| action == priority) {
        actions.swap(0, index);
    }
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
    choose_bot_action_with_options(state, bot_player_id, difficulty, SearchOptions::default())
}

pub fn choose_bot_action_with_options(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
    options: SearchOptions,
) -> Option<BotDecision> {
    let limits = difficulty.limits();
    choose_bot_action_with_limits_and_options(state, bot_player_id, difficulty, limits, options)
}

pub fn choose_bot_action_with_limits_and_options(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
    limits: SearchLimits,
    options: SearchOptions,
) -> Option<BotDecision> {
    choose_bot_action_with_config(
        state,
        bot_player_id,
        difficulty,
        limits,
        options.use_transposition_table,
    )
}

#[cfg(test)]
fn choose_bot_action_with_limits(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
    limits: SearchLimits,
) -> Option<BotDecision> {
    choose_bot_action_with_limits_and_options(
        state,
        bot_player_id,
        difficulty,
        limits,
        SearchOptions::default(),
    )
}

fn choose_bot_action_with_config(
    state: &GameState,
    bot_player_id: &PlayerId,
    difficulty: BotDifficulty,
    limits: SearchLimits,
    transposition_table_enabled: bool,
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
        transposition_table: transposition_table_enabled
            .then(|| TranspositionTable::new(limits.max_nodes.min(65_536) as usize)),
    };
    let mut last_completed = None;
    let mut previous_iteration_best = None;
    for depth in 1..=limits.max_depth_actions {
        if context.hard_limit_reached() {
            break;
        }
        let mut iteration_actions = actions.clone();
        prioritize_action(&mut iteration_actions, previous_iteration_best.as_ref());
        context.stats.iterations_started += 1;
        let root_result = search_root(state, &iteration_actions, depth, &mut context);
        if !root_result.completed {
            break;
        }
        previous_iteration_best = root_result.best.as_ref().map(|(action, _)| action.clone());
        context.stats.completed_depth = depth;
        context.stats.iterations_completed += 1;
        last_completed = Some(root_result);
        if context.soft_limit_reached() {
            break;
        }
    }

    let Some(mut root_result) = last_completed else {
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
    let (best_action, best_score) = root_result
        .best
        .take()
        .expect("a completed root search with legal actions must have a best action");

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
            ("wr", "white", "rook", Square::new(7, 0)),
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

        let limits = SearchLimits {
            max_depth_actions: 3,
            max_nodes: 1,
            soft_time_ms: 1_000,
            hard_time_ms: 1_000,
        };
        let bot_player_id = "white".to_string();
        let mut context = SearchContext {
            bot_player_id: &bot_player_id,
            limits: &limits,
            started: Instant::now(),
            stats: SearchStats::default(),
            transposition_table: Some(TranspositionTable::new(100)),
        };
        assert_eq!(
            alpha_beta(state, 3, 0, i32::MIN + 1, i32::MAX, &mut context),
            SearchOutcome::Aborted
        );
        assert_eq!(context.transposition_table.as_ref().unwrap().len(), 0);
        assert_eq!(context.stats.tt_stores, 0);
    }

    fn search_with_limits_and_tt(
        state: &GameState,
        limits: SearchLimits,
        use_transposition_table: bool,
    ) -> BotDecision {
        choose_bot_action_with_config(
            state,
            &"white".into(),
            BotDifficulty::Normal,
            limits,
            use_transposition_table,
        )
        .unwrap()
    }

    #[test]
    fn iterative_deepening_completes_every_depth_with_sufficient_budget() {
        let decision = search_with_limits_and_tt(
            &searchable_state(),
            SearchLimits {
                max_depth_actions: 3,
                max_nodes: 100_000,
                soft_time_ms: 10_000,
                hard_time_ms: 20_000,
            },
            true,
        );

        assert_eq!(decision.completed_depth, 3);
        assert_eq!(decision.stats.iterations_started, 3);
        assert_eq!(decision.stats.iterations_completed, 3);
    }

    #[test]
    fn aborted_iteration_returns_the_last_completed_result() {
        let state = searchable_state();
        let depth_one = search_with_limits_and_tt(
            &state,
            SearchLimits {
                max_depth_actions: 1,
                max_nodes: 100_000,
                soft_time_ms: 10_000,
                hard_time_ms: 20_000,
            },
            false,
        );
        let interrupted = search_with_limits_and_tt(
            &state,
            SearchLimits {
                max_depth_actions: 2,
                max_nodes: depth_one.searched_nodes + 2,
                soft_time_ms: 10_000,
                hard_time_ms: 20_000,
            },
            false,
        );

        assert_eq!(interrupted.completed_depth, 1);
        assert_eq!(interrupted.stats.iterations_started, 2);
        assert_eq!(interrupted.stats.iterations_completed, 1);
        assert_eq!(interrupted.action, depth_one.action);
        assert_eq!(interrupted.score, depth_one.score);
        assert!(interrupted.depth_reached > interrupted.completed_depth);
        assert!(interrupted.stats.qnodes > depth_one.stats.qnodes);
    }

    #[test]
    fn soft_limit_stops_only_after_a_completed_iteration() {
        let decision = search_with_limits_and_tt(
            &searchable_state(),
            SearchLimits {
                max_depth_actions: 3,
                max_nodes: 100_000,
                soft_time_ms: 0,
                hard_time_ms: 20_000,
            },
            false,
        );

        assert_eq!(decision.completed_depth, 1);
        assert_eq!(decision.stats.iterations_started, 1);
        assert_eq!(decision.stats.iterations_completed, 1);
    }

    #[test]
    fn transposition_table_is_reused_across_iterations() {
        let state = searchable_state();
        let limits = |max_depth_actions| SearchLimits {
            max_depth_actions,
            max_nodes: 100_000,
            soft_time_ms: 10_000,
            hard_time_ms: 20_000,
        };
        let depth_two = search_with_limits_and_tt(&state, limits(2), true);
        let depth_three = search_with_limits_and_tt(&state, limits(3), true);

        assert_eq!(depth_three.completed_depth, 3);
        assert_eq!(depth_three.stats.iterations_completed, 3);
        assert!(depth_three.stats.tt_hits > depth_two.stats.tt_hits);
        assert!(depth_three.stats.tt_hits > depth_three.stats.tt_cutoffs);
    }

    #[test]
    fn previous_iteration_best_is_prioritized_without_duplication() {
        let state = searchable_state();
        let mut actions = generate_ai_actions(&state);
        order_ai_actions(&state, &mut actions, &"white".into());
        let original_len = actions.len();
        let priority = actions.last().unwrap().clone();

        prioritize_action(&mut actions, Some(&priority));

        assert_eq!(actions.first(), Some(&priority));
        assert_eq!(actions.len(), original_len);
        assert_eq!(
            actions.iter().filter(|action| *action == &priority).count(),
            1
        );
    }

    #[test]
    fn strong_difficulties_are_deterministic_with_identical_limits() {
        let state = searchable_state();
        let limits = SearchLimits {
            max_depth_actions: 2,
            max_nodes: 100_000,
            soft_time_ms: 10_000,
            hard_time_ms: 20_000,
        };
        for difficulty in [BotDifficulty::Normal, BotDifficulty::Hard] {
            let first =
                choose_bot_action_with_config(&state, &"white".into(), difficulty, limits, true)
                    .unwrap();
            let second =
                choose_bot_action_with_config(&state, &"white".into(), difficulty, limits, true)
                    .unwrap();
            assert_eq!(first.action, second.action);
            assert_eq!(first.score, second.score);
            assert_eq!(first.completed_depth, second.completed_depth);
        }
    }

    fn qsearch_result(state: GameState, max_nodes: u64, qply: u8) -> (SearchOutcome, SearchStats) {
        let limits = SearchLimits {
            max_depth_actions: 1,
            max_nodes,
            soft_time_ms: 10_000,
            hard_time_ms: 20_000,
        };
        let player = "white".to_string();
        let mut context = SearchContext {
            bot_player_id: &player,
            limits: &limits,
            started: Instant::now(),
            stats: SearchStats::default(),
            transposition_table: Some(TranspositionTable::new(100)),
        };
        let result = quiescence_search(state, i32::MIN + 1, i32::MAX, qply, &mut context, true);
        (result, context.stats)
    }

    #[test]
    fn noisy_policy_includes_capture_promotion_drop_capture_and_enemy_recall() {
        let mut promotion = searchable_state();
        add_test_piece(
            &mut promotion,
            "promoting-pawn",
            "white",
            "pawn-white",
            Some(Square::new(1, 6)),
        );
        let promotion_action = generate_quiescence_actions(&promotion)
            .into_iter()
            .find(|action| matches!(action, AiAction::Move(action) if action.promotion.is_some()))
            .expect("non-capture promotion must be noisy");
        let promoted = apply_generated_action(promotion, &promotion_action);
        let AiAction::Move(promotion_action) = promotion_action else {
            unreachable!();
        };
        assert_eq!(
            promoted.pieces[&promotion_action.piece_id].type_id,
            promotion_action.promotion.unwrap()
        );

        let mut drops = searchable_state();
        add_test_piece(
            &mut drops,
            "drop-target",
            "black",
            "queen",
            Some(Square::new(3, 0)),
        );
        add_test_piece(&mut drops, "para", "white", "paratrooper", None);
        add_test_piece(&mut drops, "quiet-pocket", "white", "knight", None);
        let drop_actions = generate_quiescence_actions(&drops);
        assert!(drop_actions.iter().any(|action| matches!(
            action,
            AiAction::Drop(action)
                if action.piece_id == "para"
                    && action.captured_piece_id.as_ref().map(crate::types::PieceId::as_str)
                        == Some("drop-target")
        )));
        assert!(drop_actions.iter().all(|action| !matches!(
            action,
            AiAction::Drop(action) if action.captured_piece_id.is_none()
        )));

        let mut recalls = searchable_state();
        add_test_piece(
            &mut recalls,
            "camp",
            "white",
            "green-camp",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut recalls,
            "friendly",
            "white",
            "bishop",
            Some(Square::new(4, 3)),
        );
        add_test_piece(
            &mut recalls,
            "enemy",
            "black",
            "bishop",
            Some(Square::new(2, 3)),
        );
        let recall_actions = generate_quiescence_actions(&recalls);
        assert!(recall_actions.iter().any(|action| matches!(
            action,
            AiAction::Ability(action)
                if action.target_piece_id.as_ref().map(crate::types::PieceId::as_str)
                    == Some("enemy")
        )));
        assert!(recall_actions.iter().all(|action| !matches!(
            action,
            AiAction::Ability(action)
                if action.target_piece_id.as_ref().map(crate::types::PieceId::as_str)
                    == Some("friendly")
        )));
    }

    #[test]
    fn quiet_moves_drops_airdrop_relieve_and_friendly_recall_are_excluded() {
        let mut state = searchable_state();
        add_test_piece(
            &mut state,
            "camp",
            "white",
            "green-camp",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut state,
            "friendly",
            "white",
            "bishop",
            Some(Square::new(4, 3)),
        );
        add_test_piece(
            &mut state,
            "airborne",
            "white",
            "airborne",
            Some(Square::new(1, 1)),
        );
        add_test_piece(
            &mut state,
            "soldier",
            "white",
            "alternating-soldier",
            Some(Square::new(5, 5)),
        );
        add_test_piece(&mut state, "reserve", "white", "knight", None);

        let actions = generate_quiescence_actions(&state);
        assert!(actions.iter().all(|action| match action {
            AiAction::Move(action) => is_noisy_move(action),
            AiAction::Drop(action) => is_noisy_drop(action),
            AiAction::Ability(action) => {
                is_noisy_ability(&state, action)
                    && action.ability_id != "airdrop"
                    && action.ability_id != "relieve"
            }
        }));
        assert!(actions.iter().all(|action| !matches!(
            action,
            AiAction::Ability(action)
                if action.target_piece_id.as_ref().map(crate::types::PieceId::as_str)
                    == Some("friendly")
        )));
    }

    #[test]
    fn qsearch_scores_recapture_capture_drop_and_enemy_recall_horizons() {
        let mut recapture = searchable_state();
        relocate_test_piece(&mut recapture, "wr", Square::new(1, 0));
        add_test_piece(
            &mut recapture,
            "white-queen",
            "white",
            "queen",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut recapture,
            "black-rook",
            "black",
            "rook",
            Some(Square::new(3, 7)),
        );
        recapture.current_player = "black".into();
        assert!(generate_quiescence_actions(&recapture)
            .iter()
            .any(|action| matches!(
                action,
                AiAction::Move(action)
                    if action.piece_id == "black-rook"
                        && action.captured_piece_id.as_ref().map(crate::types::PieceId::as_str)
                            == Some("white-queen")
            )));
        let recapture_stand_pat = evaluate(&recapture, &"white".into());
        let (SearchOutcome::Complete(recapture_score), _) = qsearch_result(recapture, 10_000, 0)
        else {
            panic!("recapture qsearch aborted");
        };
        assert!(
            recapture_score < recapture_stand_pat,
            "recapture score {recapture_score} should be below stand-pat {recapture_stand_pat}"
        );

        let mut capture_drop = searchable_state();
        relocate_test_piece(&mut capture_drop, "wr", Square::new(1, 0));
        add_test_piece(
            &mut capture_drop,
            "drop-victim",
            "white",
            "queen",
            Some(Square::new(3, 7)),
        );
        add_test_piece(
            &mut capture_drop,
            "black-para",
            "black",
            "paratrooper",
            None,
        );
        capture_drop.current_player = "black".into();
        let drop_actions = generate_quiescence_actions(&capture_drop);
        let capture_action = drop_actions
            .iter()
            .find(|action| matches!(action, AiAction::Drop(action) if action.captured_piece_id.as_ref().map(crate::types::PieceId::as_str) == Some("drop-victim")))
            .expect("canonical capture-on-drop must be noisy");
        assert!(apply_ai_action(capture_drop.clone(), capture_action).is_ok());
        let drop_stand_pat = evaluate(&capture_drop, &"white".into());
        let (SearchOutcome::Complete(drop_score), _) = qsearch_result(capture_drop, 10_000, 0)
        else {
            panic!("capture-drop qsearch aborted");
        };
        assert!(drop_score < drop_stand_pat);

        let mut recall = searchable_state();
        relocate_test_piece(&mut recall, "wr", Square::new(1, 0));
        add_test_piece(
            &mut recall,
            "black-camp",
            "black",
            "green-camp",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut recall,
            "recall-victim",
            "white",
            "queen",
            Some(Square::new(4, 3)),
        );
        recall
            .pieces
            .get_mut("black-camp")
            .unwrap()
            .move_option_cooldowns
            .insert(
                "normal".into(),
                crate::types::CooldownState { remaining: 1 },
            );
        recall.current_player = "black".into();
        let recall_action = generate_quiescence_actions(&recall)
            .into_iter()
            .find(|action| matches!(action, AiAction::Ability(action) if action.target_piece_id.as_ref().map(crate::types::PieceId::as_str) == Some("recall-victim")))
            .expect("enemy recall must be noisy");
        assert!(apply_ai_action(recall.clone(), &recall_action).is_ok());
        let recall_stand_pat = evaluate(&recall, &"white".into());
        let (SearchOutcome::Complete(recall_score), _) = qsearch_result(recall, 10_000, 0) else {
            panic!("recall qsearch aborted");
        };
        assert!(recall_score < recall_stand_pat);
    }

    #[test]
    fn qsearch_respects_node_abort_and_safety_limit_without_tt_store() {
        let mut tactical = searchable_state();
        add_test_piece(
            &mut tactical,
            "white-queen",
            "white",
            "queen",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut tactical,
            "black-rook",
            "black",
            "rook",
            Some(Square::new(3, 7)),
        );
        tactical.current_player = "black".into();

        let (aborted, aborted_stats) = qsearch_result(tactical.clone(), 1, 0);
        assert_eq!(aborted, SearchOutcome::Aborted);
        assert_eq!(aborted_stats.searched_nodes, 1);
        assert_eq!(aborted_stats.qnodes, 1);
        assert_eq!(aborted_stats.tt_stores, 0);

        let stand_pat = evaluate(&tactical, &"white".into());
        let (bounded, bounded_stats) = qsearch_result(tactical, 10_000, MAX_QUIESCENCE_PLIES);
        assert_eq!(bounded, SearchOutcome::Complete(stand_pat));
        assert_eq!(bounded_stats.searched_nodes, 1);
        assert_eq!(bounded_stats.qnodes, 1);
        assert_eq!(bounded_stats.tt_stores, 0);
    }

    #[test]
    fn table_bound_lookup_obeys_depth_and_window_semantics() {
        let entry = |depth, score, bound| TranspositionEntry {
            depth,
            score,
            bound,
            best_action: None,
        };

        let mut alpha = -100;
        let mut beta = 100;
        assert_eq!(
            apply_table_bound(&entry(2, 7, BoundType::Exact), 2, &mut alpha, &mut beta)
                .cutoff_score,
            Some(7)
        );

        let mut alpha = -100;
        let mut beta = 10;
        assert_eq!(
            apply_table_bound(
                &entry(2, 10, BoundType::LowerBound),
                2,
                &mut alpha,
                &mut beta
            )
            .cutoff_score,
            Some(10)
        );

        let mut alpha = -10;
        let mut beta = 100;
        assert_eq!(
            apply_table_bound(
                &entry(2, -10, BoundType::UpperBound),
                2,
                &mut alpha,
                &mut beta
            )
            .cutoff_score,
            Some(-10)
        );

        let mut alpha = -100;
        let mut beta = 100;
        assert_eq!(
            apply_table_bound(&entry(1, 99, BoundType::Exact), 2, &mut alpha, &mut beta)
                .cutoff_score,
            None
        );
        assert_eq!((alpha, beta), (-100, 100));
    }

    #[test]
    fn representative_variant_positions_match_with_table_enabled_and_disabled() {
        let mut positions = Vec::new();

        let mut stateful = searchable_state();
        add_test_piece(
            &mut stateful,
            "windmill",
            "white",
            "windmill",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut stateful,
            "cannon",
            "white",
            "cannon-rook",
            Some(Square::new(5, 2)),
        );
        stateful
            .pieces
            .get_mut("cannon")
            .unwrap()
            .move_option_cooldowns
            .insert(
                "cannon_move".into(),
                crate::types::CooldownState { remaining: 2 },
            );
        positions.push(("piece-state-cooldown", stateful));

        let mut drop_capture = searchable_state();
        add_test_piece(
            &mut drop_capture,
            "enemy",
            "black",
            "knight",
            Some(Square::new(3, 0)),
        );
        add_test_piece(&mut drop_capture, "para", "white", "paratrooper", None);
        positions.push(("drop-capture", drop_capture));

        let mut airborne = searchable_state();
        add_test_piece(
            &mut airborne,
            "airborne",
            "white",
            "airborne",
            Some(Square::new(3, 3)),
        );
        add_test_piece(&mut airborne, "air-bishop", "white", "bishop", None);
        add_test_piece(&mut airborne, "air-knight", "white", "knight", None);
        positions.push(("airborne-deployment", airborne));

        let mut alternating = searchable_state();
        add_test_piece(
            &mut alternating,
            "soldier",
            "white",
            "alternating-soldier",
            Some(Square::new(3, 3)),
        );
        add_test_piece(
            &mut alternating,
            "swap-target",
            "white",
            "bishop",
            Some(Square::new(4, 4)),
        );
        add_test_piece(
            &mut alternating,
            "enemy",
            "black",
            "bishop",
            Some(Square::new(2, 2)),
        );
        add_test_piece(&mut alternating, "swap-reserve", "white", "knight", None);
        positions.push(("alternating-soldier-pocket-swap", alternating));
        positions.push(("immediate-king-capture", searchable_state()));

        let limits = SearchLimits {
            max_depth_actions: 2,
            max_nodes: 100_000,
            soft_time_ms: 10_000,
            hard_time_ms: 20_000,
        };
        let mut hits = 0;
        for (name, state) in positions {
            let without = choose_bot_action_with_config(
                &state,
                &"white".into(),
                BotDifficulty::Normal,
                limits,
                false,
            )
            .unwrap_or_else(|| panic!("{name} produced no non-TT decision"));
            let with = choose_bot_action_with_config(
                &state,
                &"white".into(),
                BotDifficulty::Normal,
                limits,
                true,
            )
            .unwrap_or_else(|| panic!("{name} produced no TT decision"));
            assert_eq!(with.action, without.action, "{name} action differs");
            assert_eq!(with.score, without.score, "{name} score differs");
            assert_eq!(with.completed_depth, 2, "{name} TT search aborted");
            assert_eq!(without.completed_depth, 2, "{name} non-TT search aborted");
            hits += with.stats.tt_hits;
        }
        assert!(
            hits > 0,
            "representative searches should exercise TT lookup"
        );
    }

    fn add_test_piece(
        state: &mut GameState,
        id: &str,
        owner: &str,
        type_id: &str,
        square: Option<Square>,
    ) {
        let piece_id: crate::types::PieceId = id.into();
        let definition = &state.piece_definitions[type_id];
        if let Some(square) = square {
            state
                .board
                .squares
                .insert(square.to_id(), Some(piece_id.clone()));
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .starting_pieces
                .push(piece_id.clone());
        } else {
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .pocket_pieces
                .push(piece_id.clone());
        }
        state.pieces.insert(
            piece_id.clone(),
            Piece {
                id: piece_id,
                owner: owner.into(),
                type_id: type_id.into(),
                current_square: square,
                in_pocket: square.is_none(),
                captured: false,
                has_moved: true,
                state: definition.initial_state(),
                move_option_cooldowns: HashMap::new(),
            },
        );
    }

    fn relocate_test_piece(state: &mut GameState, id: &str, to: Square) {
        let piece_id: crate::types::PieceId = id.into();
        let from = state.pieces[&piece_id].current_square.unwrap();
        state.board.squares.insert(from.to_id(), None);
        state
            .board
            .squares
            .insert(to.to_id(), Some(piece_id.clone()));
        state.pieces.get_mut(&piece_id).unwrap().current_square = Some(to);
    }
}
