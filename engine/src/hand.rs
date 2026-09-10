//! Runtime Hand membership and deterministic zone transfer; no Draw policy or RNG.
use std::collections::HashSet;

use crate::types::*;

/// Validate authoritative Hand membership, including cross-player and board indexes.
pub fn validate_hand_zones(state: &GameState) -> Result<(), String> {
    let mut seen = HashSet::new();
    for (owner, player) in &state.players {
        for id in &player.deck.hand_pieces {
            let piece = state.pieces.get(id).ok_or("Hand 기물이 없습니다.")?;
            if state.ruleset != DeckRuleset::Standard
                || player.id != *owner
                || player.deck.player_id != *owner
                || piece.id != *id
                || piece.owner != *owner
                || !seen.insert(id)
                || piece.current_square.is_some()
                || piece.in_pocket
                || piece.captured
                || state.players.values().any(|other| {
                    other.deck.starting_pieces.contains(id)
                        || other.deck.pocket_pieces.contains(id)
                        || other.deck.extra_deck_pieces.contains(id)
                        || other.captured_pieces.contains(id)
                })
                || state
                    .board
                    .squares
                    .values()
                    .chain(state.board.air_squares.values())
                    .any(|occupant| occupant.as_ref() == Some(id))
            {
                return Err("Hand zone 불변조건 위반입니다.".into());
            }
        }
    }
    Ok(())
}

/// Transfer an already selected owned Pocket ID into Hand atomically.
/// The caller chooses the ID; this does not implement automatic Draw or randomness.
pub fn move_pocket_piece_to_hand(
    state: &mut GameState,
    owner: &PlayerId,
    id: &PieceId,
) -> Result<(), String> {
    validate_hand_zones(state)?;
    let player = state.players.get(owner).ok_or("플레이어가 없습니다.")?;
    let piece = state.pieces.get(id).ok_or("기물이 없습니다.")?;
    if state.ruleset != DeckRuleset::Standard
        || piece.id != *id
        || piece.owner != *owner
        || !piece.in_pocket
        || piece.captured
        || piece.current_square.is_some()
        || player
            .deck
            .pocket_pieces
            .iter()
            .filter(|entry| *entry == id)
            .count()
            != 1
        || state.players.iter().any(|(other_owner, other)| {
            other.deck.extra_deck_pieces.contains(id)
                || other.captured_pieces.contains(id)
                || (other_owner != owner
                    && (other.deck.pocket_pieces.contains(id)
                        || other.deck.starting_pieces.contains(id)))
        })
        || state
            .board
            .squares
            .values()
            .chain(state.board.air_squares.values())
            .any(|occupant| occupant.as_ref() == Some(id))
    {
        return Err("소유한 Standard Pocket 기물만 Hand로 이동할 수 있습니다.".into());
    }
    let mut next = state.clone();
    let deck = &mut next.players.get_mut(owner).unwrap().deck;
    deck.pocket_pieces.retain(|entry| entry != id);
    // A returned starting piece may still have a historical setup membership.
    // Frozen GameRecord deck snapshots retain the original setup separately.
    deck.starting_pieces.retain(|entry| entry != id);
    deck.hand_pieces.push(id.clone());
    next.pieces.get_mut(id).unwrap().in_pocket = false;
    validate_hand_zones(&next)?;
    *state = next;
    Ok(())
}

pub fn ordinary_drop_pieces<'a>(state: &GameState, player: &'a Player) -> &'a [PieceId] {
    match state.ruleset {
        DeckRuleset::Legacy => &player.deck.pocket_pieces,
        DeckRuleset::Standard => &player.deck.hand_pieces,
    }
}

pub(crate) fn is_ordinary_drop_source(state: &GameState, owner: &PlayerId, id: &PieceId) -> bool {
    let Some(player) = state.players.get(owner) else {
        return false;
    };
    let Some(piece) = state.pieces.get(id) else {
        return false;
    };
    if piece.owner != *owner || piece.captured || !ordinary_drop_pieces(state, player).contains(id)
    {
        return false;
    }
    match state.ruleset {
        DeckRuleset::Legacy => piece.in_pocket,
        DeckRuleset::Standard => validate_hand_zones(state).is_ok(),
    }
}
