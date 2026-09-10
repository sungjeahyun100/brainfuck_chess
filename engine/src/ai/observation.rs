//! Decision-only projection. Never commit this incomplete state to a game.
use std::collections::HashSet;

use crate::types::{DeckRuleset, GameState, PlayerId};

pub struct BotObservation {
    pub(crate) state: GameState,
    pub(crate) opponent_hand_count: usize,
}

impl BotObservation {
    pub fn new(authoritative: &GameState, viewer: &PlayerId) -> Self {
        let mut state = authoritative.clone();
        let mut opponent_hand_count = 0;
        if state.ruleset == DeckRuleset::Standard {
            let mut hidden = HashSet::new();
            for (id, player) in &mut state.players {
                if id == viewer {
                    continue;
                }
                opponent_hand_count += player.deck.hand_pieces.len();
                hidden.extend(player.deck.hand_pieces.drain(..));
                hidden.extend(player.deck.pocket_pieces.drain(..));
                // Original deck totals must not reconstruct the hidden reserve.
                player.deck.total_score = 0;
            }
            state.pieces.retain(|id, _| !hidden.contains(id));
            let visible_types: HashSet<_> =
                state.pieces.values().map(|p| p.type_id.clone()).collect();
            let private_types: HashSet<_> = state
                .custom_piece_manifest
                .iter()
                .filter(|entry| {
                    !entry
                        .runtime_type_ids
                        .iter()
                        .any(|id| visible_types.contains(id))
                })
                .flat_map(|entry| entry.runtime_type_ids.iter().cloned())
                .collect();
            state
                .custom_piece_manifest
                .retain(|entry| !private_types.contains(&entry.exposed_type_id));
            state
                .piece_definitions
                .retain(|id, _| !private_types.contains(id));
            state.chessembly_program_cache =
                crate::types::ChessemblyProgramCache::from_definitions(&state.piece_definitions);

            for player in state.players.values_mut() {
                player
                    .deck
                    .starting_pieces
                    .retain(|id| !hidden.contains(id));
            }
        }
        Self {
            state,
            opponent_hand_count,
        }
    }
}
