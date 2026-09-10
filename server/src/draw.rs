//! Server-only automatic Draw policy. Search and player actions never own RNG.
use brainfuck_chess_engine::{hand, types::*};
use serde::{Deserialize, Serialize};

const INITIAL_COUNT: usize = 3;
const TURN_COUNT: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DrawTiming {
    Initial,
    TurnStart,
}

/// Resolved instances, not seeds. Kept beside recorded actions, never in live views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DrawResolution {
    pub(crate) player_id: PlayerId,
    pub(crate) timing: DrawTiming,
    pub(crate) piece_ids: Vec<PieceId>,
}

/// OS entropy with rejection sampling (no modulo bias). Fail closed on OS failure.
pub(crate) fn random_index(upper: usize) -> Result<usize, String> {
    if upper == 0 {
        return Err("Draw 후보가 없습니다.".into());
    }
    let bound = upper as u64;
    let limit = u64::MAX - u64::MAX % bound;
    for _ in 0..128 {
        let value = getrandom::u64().map_err(|_| "Draw 난수원을 사용할 수 없습니다.")?;
        if value < limit {
            return Ok((value % bound) as usize);
        }
    }
    Err("Draw 난수 선택 한도를 초과했습니다.".into())
}

fn resolve(
    state: &mut GameState,
    owner: &str,
    timing: DrawTiming,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<DrawResolution, String> {
    let mut candidates = state
        .players
        .get(owner)
        .ok_or("Draw 플레이어가 없습니다.")?
        .deck
        .pocket_pieces
        .clone();
    let count = match timing {
        DrawTiming::Initial => INITIAL_COUNT,
        DrawTiming::TurnStart => TURN_COUNT,
    };
    let mut piece_ids = Vec::new();
    for _ in 0..count.min(candidates.len()) {
        let index = choose(candidates.len())?;
        if index >= candidates.len() {
            return Err("Draw 난수 인덱스가 범위를 벗어났습니다.".into());
        }
        piece_ids.push(candidates.swap_remove(index));
    }
    let resolution = DrawResolution {
        player_id: owner.into(),
        timing,
        piece_ids,
    };
    apply_resolution(state, &resolution)?;
    Ok(resolution)
}

fn apply_resolution(state: &mut GameState, resolution: &DrawResolution) -> Result<(), String> {
    let count = match resolution.timing {
        DrawTiming::Initial => INITIAL_COUNT,
        DrawTiming::TurnStart => TURN_COUNT,
    };
    let player = state
        .players
        .get(&resolution.player_id)
        .ok_or("Draw 플레이어가 없습니다.")?;
    if state.ruleset != DeckRuleset::Standard
        || resolution.piece_ids.len() != count.min(player.deck.pocket_pieces.len())
    {
        return Err("Draw resolution 수량 또는 룰셋이 올바르지 않습니다.".into());
    }
    let mut next = state.clone();
    for id in &resolution.piece_ids {
        hand::move_pocket_piece_to_hand(&mut next, &resolution.player_id, id)?;
    }
    hand::validate_hand_zones(&next)?;
    *state = next;
    Ok(())
}

pub(crate) fn initialize(
    state: &mut GameState,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<Vec<DrawResolution>, String> {
    if state.ruleset == DeckRuleset::Legacy {
        return Ok(Vec::new());
    }
    if state.current_player != "white"
        || state.turn_number != 1
        || state.phase != GamePhase::Playing
        || state.result.is_some()
        || !state.history.is_empty()
        || state
            .players
            .values()
            .any(|p| !p.deck.hand_pieces.is_empty())
    {
        return Err("Draw 초기화는 게임 시작 전 한 번만 가능합니다.".into());
    }
    let mut next = state.clone();
    let mut draws = Vec::new();
    for (owner, timing) in [
        ("white", DrawTiming::Initial),
        ("black", DrawTiming::Initial),
        ("white", DrawTiming::TurnStart),
    ] {
        draws.push(resolve(&mut next, owner, timing, choose)?);
    }
    hand::validate_hand_zones(&next)?;
    *state = next;
    Ok(draws)
}

pub(crate) fn starts_turn(before: &GameState, after: &GameState) -> bool {
    after.ruleset == DeckRuleset::Standard
        && before.phase != GamePhase::Ended
        && before.result.is_none()
        && after.phase != GamePhase::Ended
        && after.result.is_none()
        && before.current_player != after.current_player
}

pub(crate) fn turn_start(
    before: &GameState,
    after: &mut GameState,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<Vec<DrawResolution>, String> {
    if !starts_turn(before, after) {
        return Ok(Vec::new());
    }
    let owner = after.current_player.clone();
    Ok(vec![resolve(after, &owner, DrawTiming::TurnStart, choose)?])
}

/// Reapply an exact recorded automatic transition; never samples randomness.
pub(crate) fn replay_turn(
    before: &GameState,
    after: &mut GameState,
    draws: &[DrawResolution],
) -> Result<(), String> {
    if !starts_turn(before, after) {
        return if draws.is_empty() {
            Ok(())
        } else {
            Err("예상하지 않은 Draw resolution입니다.".into())
        };
    }
    let [draw] = draws else {
        return Err("턴 시작 Draw resolution이 필요합니다.".into());
    };
    if draw.timing != DrawTiming::TurnStart || draw.player_id != after.current_player {
        return Err("Draw resolution의 턴이 올바르지 않습니다.".into());
    }
    apply_resolution(after, draw)
}

/// Verify playable initial state against ordered authoritative metadata without RNG.
pub(crate) fn validate_initial(state: &GameState, draws: &[DrawResolution]) -> Result<(), String> {
    if state.ruleset == DeckRuleset::Legacy {
        return if draws.is_empty() {
            Ok(())
        } else {
            Err("Legacy Draw metadata".into())
        };
    }
    hand::validate_hand_zones(state)?;
    let mut before = state.clone();
    for player in before.players.values_mut() {
        for id in std::mem::take(&mut player.deck.hand_pieces) {
            before
                .pieces
                .get_mut(&id)
                .ok_or("Missing initial piece")?
                .in_pocket = true;
            player.deck.pocket_pieces.push(id);
        }
    }
    if before.current_player != "white"
        || before.turn_number != 1
        || before.phase != GamePhase::Playing
        || before.result.is_some()
        || draws.len() != 3
    {
        return Err("Invalid initial Draw state".into());
    }
    for (draw, (owner, timing)) in draws.iter().zip([
        ("white", DrawTiming::Initial),
        ("black", DrawTiming::Initial),
        ("white", DrawTiming::TurnStart),
    ]) {
        if draw.player_id != owner || draw.timing != timing {
            return Err("Invalid initial Draw timing".into());
        }
        apply_resolution(&mut before, draw)?;
    }
    if crate::analysis::state_hash(&before) != crate::analysis::state_hash(state) {
        return Err("Initial Draw state mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod g3c_tests;

#[cfg(test)]
mod g5_tests;

#[cfg(test)]
mod g8a_tests;
