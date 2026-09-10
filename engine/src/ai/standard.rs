//! Small deterministic Standard heuristics; none of these constants are rules.
use crate::ai::types::AiAction;
use crate::summon::{generate_extra_summon_actions, sacrifice_candidates};
use crate::types::{DeckRuleset, GameState, PieceId};

pub const EXTRA_SUBSET_LIMIT: usize = 4;
const SUBSETS_PER_SCORE: usize = 4;
pub const SUBSET_FRONTIER_LIMIT: usize = 128;
pub const POCKET_DISCOUNT_PERCENT: i64 = 40;
pub const HIDDEN_HAND_VALUE: i64 = 200;
pub const HIDDEN_RESERVE_THREAT_CAP: i64 = 1600;
pub const UNKNOWN_POCKET_THREAT: i64 = 100;

/// Centipoints, retaining each definition's board/reserve value contract.
pub fn sacrifice_utility(state: &GameState, id: &PieceId) -> i64 {
    let Some(piece) = state.pieces.get(id) else {
        return 0;
    };
    let Some(def) = state.piece_definitions.get(&piece.type_id) else {
        return 0;
    };
    if let Some(square) = piece.current_square.filter(|_| piece.is_on_board()) {
        let center = state.board.size - 1;
        let positional = (state.board.size * 2
            - (square.file * 2 - center).abs()
            - (square.rank * 2 - center).abs())
        .max(0);
        i64::from(def.ai_board_value()) * 100 + i64::from(positional)
    } else {
        i64::from(def.ai_pocket_value()) * 100
    }
}

#[derive(Clone, Debug)]
pub struct SacrificeSubset {
    pub piece_ids: Vec<PieceId>,
    pub score: u64,
    pub utility_loss: i64,
}

fn rank(subset: &SacrificeSubset, cost: u64) -> (i64, usize, &Vec<PieceId>) {
    (
        subset
            .utility_loss
            .saturating_add(subset.score.saturating_sub(cost).min(i64::MAX as u64) as i64 * 5),
        subset.piece_ids.len(),
        &subset.piece_ids,
    )
}

/// Bounded beam knapsack. Keeps several exact-ID alternatives per paid score;
/// never allocates a cost-sized array or enumerates 2^N subsets.
pub fn select_sacrifice_subsets(state: &GameState, extra: &PieceId) -> Vec<SacrificeSubset> {
    let Ok(candidates) = sacrifice_candidates(state, extra) else {
        return Vec::new();
    };
    let Some(def) = state
        .pieces
        .get(extra)
        .and_then(|p| state.piece_definitions.get(&p.type_id))
    else {
        return Vec::new();
    };
    let cost = u64::from(def.score);
    let mut frontier = vec![SacrificeSubset {
        piece_ids: Vec::new(),
        score: 0,
        utility_loss: 0,
    }];
    let mut complete = Vec::new();
    for id in candidates {
        let Some(def) = state
            .pieces
            .get(&id)
            .and_then(|p| state.piece_definitions.get(&p.type_id))
        else {
            continue;
        };
        if def.score == 0 {
            continue;
        }
        let loss = sacrifice_utility(state, &id);
        let mut next = frontier.clone();
        for subset in &frontier {
            let mut extended = subset.clone();
            extended.piece_ids.push(id.clone());
            extended.score += u64::from(def.score);
            extended.utility_loss += loss;
            if extended.score >= cost {
                complete.push(extended);
            } else {
                next.push(extended);
            }
        }
        complete.sort_by(|a, b| rank(a, cost).cmp(&rank(b, cost)));
        complete.truncate(EXTRA_SUBSET_LIMIT);
        next.sort_by(|a, b| rank(a, cost).cmp(&rank(b, cost)));
        let mut per_score = std::collections::HashMap::new();
        next.retain(|subset| {
            let count = per_score.entry(subset.score).or_insert(0);
            *count += 1;
            *count <= SUBSETS_PER_SCORE
        });
        // Preserve progress toward high costs as well as cheap alternatives.
        if next.len() > SUBSET_FRONTIER_LIMIT {
            next.sort_by(|a, b| {
                let density = |s: &SacrificeSubset| s.utility_loss / s.score.max(1) as i64;
                density(a)
                    .cmp(&density(b))
                    .then_with(|| b.score.cmp(&a.score))
                    .then_with(|| rank(a, cost).cmp(&rank(b, cost)))
            });
            next.truncate(SUBSET_FRONTIER_LIMIT - 1);
            if !next.iter().any(|s| s.piece_ids.is_empty()) {
                next.push(SacrificeSubset {
                    piece_ids: Vec::new(),
                    score: 0,
                    utility_loss: 0,
                });
            }
        }
        frontier = next;
    }
    complete
}

pub(crate) fn extra_actions(state: &GameState) -> Vec<AiAction> {
    if state.ruleset != DeckRuleset::Standard {
        return Vec::new();
    }
    let mut actions = Vec::new();
    if let Some(player) = state.players.get(&state.current_player) {
        for extra in &player.deck.extra_deck_pieces {
            for subset in select_sacrifice_subsets(state, extra) {
                if let Ok(targets) = generate_extra_summon_actions(state, extra, &subset.piece_ids)
                {
                    actions.extend(targets.into_iter().map(AiAction::ExtraSummon));
                }
            }
        }
    }
    actions
}
