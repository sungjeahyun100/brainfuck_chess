//! Standard ExtraSummon: bounded candidate discovery and exact atomic player intent.
use crate::types::*;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SacrificeZone {
    Hand,
    Board,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SummonPolicy {
    pub sacrifice_zones: &'static [SacrificeZone],
}

/// Eligibility and sacrifice sources live in one ruleset-aware policy.
pub fn summon_policy(ruleset: DeckRuleset, type_id: &str) -> Option<SummonPolicy> {
    if ruleset != DeckRuleset::Standard {
        return None;
    }
    let sacrifice_zones: &'static [SacrificeZone] = match type_id {
        "guhang" => &[SacrificeZone::Hand, SacrificeZone::Board],
        "bomber" => &[SacrificeZone::Board],
        _ => return None,
    };
    Some(SummonPolicy { sacrifice_zones })
}

/// Validate current zones, not historical starting membership.
pub fn validate_summon_zones(state: &GameState) -> Result<(), String> {
    crate::hand::validate_hand_zones(state)?;
    let mut reserve = HashSet::new();
    for (owner, player) in &state.players {
        if player.id != *owner || player.deck.player_id != *owner {
            return Err("플레이어 소속 오류입니다.".into());
        }
        for (ids, pocket) in [
            (&player.deck.extra_deck_pieces, false),
            (&player.deck.pocket_pieces, true),
        ] {
            for id in ids {
                let p = state.pieces.get(id).ok_or("reserve 기물이 없습니다.")?;
                if !reserve.insert(id)
                    || p.id != *id
                    || p.owner != *owner
                    || p.captured
                    || p.current_square.is_some()
                    || p.in_pocket != pocket
                    || (!pocket
                        && (p.layer != PieceLayer::Ground
                            || summon_policy(state.ruleset, &p.type_id).is_none()))
                    || state.players.values().any(|other| {
                        other.deck.hand_pieces.contains(id) || other.captured_pieces.contains(id)
                    })
                    || (!pocket
                        && state
                            .players
                            .values()
                            .any(|other| other.deck.starting_pieces.contains(id)))
                {
                    return Err("reserve zone 불변조건 위반입니다.".into());
                }
            }
        }
    }
    let mut board_ids = HashSet::new();
    for (layer, squares) in [
        (PieceLayer::Ground, &state.board.squares),
        (PieceLayer::Air, &state.board.air_squares),
    ] {
        for (square, id) in squares {
            let Some(id) = id else {
                continue;
            };
            let p = state.pieces.get(id).ok_or("Board 기물이 없습니다.")?;
            if !board_ids.insert(id)
                || reserve.contains(id)
                || !p.is_on_board()
                || p.id != *id
                || p.layer != layer
                || p.current_square != Some(square.to_square())
                || !state.board.is_in_bounds(&square.to_square())
            {
                return Err("Board zone 불변조건 위반입니다.".into());
            }
        }
    }
    for p in state.pieces.values() {
        if p.is_on_board()
            && (!board_ids.contains(&p.id)
                || !state.players.contains_key(&p.owner)
                || state
                    .players
                    .values()
                    .any(|other| other.captured_pieces.contains(&p.id)))
        {
            return Err("Board 기물 소속 오류입니다.".into());
        }
    }
    Ok(())
}

fn source(state: &GameState, owner: &PlayerId, id: &PieceId) -> Option<SacrificeZone> {
    let p = state.pieces.get(id)?;
    let def = state.piece_definitions.get(&p.type_id)?;
    if p.owner != *owner || def.is_king || p.captured {
        return None;
    }
    if p.is_on_board() {
        Some(SacrificeZone::Board)
    } else if state.players.get(owner)?.deck.hand_pieces.contains(id) {
        Some(SacrificeZone::Hand)
    } else {
        None
    }
}

fn extra_policy(state: &GameState, id: &PieceId) -> Result<SummonPolicy, String> {
    if state.ruleset != DeckRuleset::Standard
        || state.phase != GamePhase::Playing
        || state.result.is_some()
        || crate::legal_moves::pending_landing_piece_id(state).is_some()
    {
        return Err("현재 Extra 소환을 할 수 없습니다.".into());
    }
    let p = state.pieces.get(id).ok_or("Extra 기물이 없습니다.")?;
    if p.owner != state.current_player
        || !state
            .players
            .get(&state.current_player)
            .is_some_and(|player| player.deck.extra_deck_pieces.contains(id))
    {
        return Err("자신의 Extra 기물만 소환할 수 있습니다.".into());
    }
    let def = state
        .piece_definitions
        .get(&p.type_id)
        .ok_or("기물 정의가 없습니다.")?;
    if def.is_king {
        return Err("King은 소환할 수 없습니다.".into());
    }
    summon_policy(state.ruleset, &p.type_id).ok_or("Extra 소환 정책이 없습니다.".into())
}

/// IDs only; never enumerates sacrifice subsets.
pub fn sacrifice_candidates(state: &GameState, id: &PieceId) -> Result<Vec<PieceId>, String> {
    validate_summon_zones(state)?;
    let policy = extra_policy(state, id)?;
    let mut ids: Vec<_> = state
        .pieces
        .keys()
        .filter(|id| {
            source(state, &state.current_player, id)
                .is_some_and(|zone| policy.sacrifice_zones.contains(&zone))
        })
        .cloned()
        .collect();
    ids.sort();
    Ok(ids)
}

fn validate_selection(state: &GameState, extra: &PieceId, ids: &[PieceId]) -> Result<(), String> {
    if extra.as_str().trim().is_empty()
        || extra.as_str().len() > 256
        || extra.as_str().chars().any(char::is_control)
        || ids.iter().any(|id| {
            id.as_str().trim().is_empty()
                || id.as_str().len() > 256
                || id.as_str().chars().any(char::is_control)
        })
    {
        return Err("기물 ID 형식이 올바르지 않습니다.".into());
    }
    validate_summon_zones(state)?;
    let policy = extra_policy(state, extra)?;
    let mut unique = HashSet::new();
    let mut score = 0u64;
    for id in ids {
        if !unique.insert(id) {
            return Err("중복 제물은 허용하지 않습니다.".into());
        }
        let zone = source(state, &state.current_player, id)
            .ok_or("제물의 소유자 또는 zone이 유효하지 않습니다.")?;
        if !policy.sacrifice_zones.contains(&zone) {
            return Err("허용되지 않는 제물 zone입니다.".into());
        }
        score += u64::from(state.piece_definitions[&state.pieces[id].type_id].score);
    }
    let cost = state.piece_definitions[&state.pieces[extra].type_id].score;
    if score < u64::from(cost) {
        return Err("제물 점수가 소환 비용보다 부족합니다.".into());
    }
    Ok(())
}

/// Sacrifice is removal, not enemy capture: no capture list, rewards or death cascade.
pub(crate) fn remove_sacrifices(state: &mut GameState, ids: &[PieceId]) {
    for id in ids {
        let p = state.pieces.get_mut(id).expect("validated sacrifice");
        if let Some(square) = p.current_square {
            state.board.set_piece_at_layer(square, p.layer, None);
        }
        p.current_square = None;
        p.in_pocket = false;
        p.captured = true;
        state
            .players
            .get_mut(&p.owner)
            .expect("validated owner")
            .deck
            .hand_pieces
            .retain(|entry| entry != id);
    }
}

/// Selected subset only: at most board-size² actions. No RNG or subset search.
pub fn generate_extra_summon_actions(
    state: &GameState,
    extra: &PieceId,
    ids: &[PieceId],
) -> Result<Vec<ExtraSummonAction>, String> {
    validate_selection(state, extra, ids)?;
    let mut candidate = state.clone();
    remove_sacrifices(&mut candidate, ids);
    Ok(
        crate::legal_moves::generate_piece_drop_targets(&candidate, extra)
            .into_iter()
            .map(|drop| ExtraSummonAction {
                player_id: state.current_player.clone(),
                extra_piece_id: extra.clone(),
                sacrifice_piece_ids: ids.to_vec(),
                target_square: drop.to,
            })
            .collect(),
    )
}

pub fn validate_extra_summon(state: &GameState, action: &ExtraSummonAction) -> Result<(), String> {
    if action.player_id != state.current_player {
        return Err("현재 차례가 아닙니다.".into());
    }
    if !generate_extra_summon_actions(state, &action.extra_piece_id, &action.sacrifice_piece_ids)?
        .iter()
        .any(|candidate| candidate == action)
    {
        return Err("소환할 수 없는 위치입니다.".into());
    }
    Ok(())
}

pub(crate) fn apply_extra_summon(mut state: GameState, action: ExtraSummonAction) -> GameState {
    remove_sacrifices(&mut state, &action.sacrifice_piece_ids);
    state
        .players
        .get_mut(&action.player_id)
        .expect("validated player")
        .deck
        .extra_deck_pieces
        .retain(|id| id != &action.extra_piece_id);
    let captured_piece_id = state.board.get_piece_at(&action.target_square).cloned();
    crate::endgame::apply_extra_placement(
        state,
        DropAction {
            sacrifice_piece_ids: Vec::new(),
            player_id: action.player_id,
            piece_id: action.extra_piece_id,
            to: action.target_square,
            captured_piece_id,
        },
    )
}
