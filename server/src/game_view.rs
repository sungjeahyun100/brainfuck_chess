//! Live client visibility. Engine/search/record states remain authoritative.
use std::collections::{HashMap, HashSet};

use brainfuck_chess_engine::types::*;

#[derive(Clone, Copy)]
pub(crate) enum Audience<'a> {
    Public,
    Player(&'a str),
    SharedLocal,
}

impl Audience<'_> {
    pub(crate) fn controls(self, player: &str) -> bool {
        matches!(self, Self::SharedLocal) || matches!(self, Self::Player(id) if id == player)
    }
}

/// Server-owned capabilities, set only when creating a game. Never serialized.
#[derive(Default, Clone)]
pub(crate) enum GameAccess {
    #[default]
    Unassigned,
    Local {
        client: String,
        human: Option<PlayerId>,
    },
    Multiplayer {
        clients: HashMap<String, PlayerId>,
    },
}

impl GameAccess {
    pub(crate) fn audience<'a>(
        &'a self,
        ruleset: DeckRuleset,
        client: Option<&str>,
    ) -> Audience<'a> {
        if ruleset == DeckRuleset::Legacy {
            return Audience::SharedLocal;
        }
        let Some(client) = client.filter(|id| !id.is_empty()) else {
            return Audience::Public;
        };
        match self {
            Self::Local {
                client: expected,
                human,
            } if expected == client => human
                .as_deref()
                .map(Audience::Player)
                .unwrap_or(Audience::SharedLocal),
            Self::Multiplayer { clients } => clients
                .get(client)
                .map(|side| Audience::Player(side))
                .unwrap_or(Audience::Public),
            _ => Audience::Public,
        }
    }
}

pub(crate) fn hand_counts(state: &GameState) -> HashMap<PlayerId, usize> {
    if state.ruleset == DeckRuleset::Legacy {
        return HashMap::new();
    }
    state
        .players
        .iter()
        .map(|(id, player)| (id.clone(), player.deck.hand_pieces.len()))
        .collect()
}

/// Hide the entire opponent Pocket/Hand reserve together: exposing Pocket IDs
/// would reveal a future Draw by subtraction, even if Hand IDs were removed.
pub(crate) fn project_state(state: &GameState, audience: Audience<'_>) -> GameState {
    let mut view = state.clone();
    if state.ruleset == DeckRuleset::Legacy {
        return view;
    }
    // Standard Extra is public from game start; only Pocket/Hand belong to this
    // hidden set. Committed Hand sacrifices leave Hand and become public removed pieces.
    let hidden: HashSet<_> = state
        .players
        .iter()
        .filter(|(owner, _)| !audience.controls(owner))
        .flat_map(|(_, player)| {
            player
                .deck
                .hand_pieces
                .iter()
                .chain(&player.deck.pocket_pieces)
        })
        .cloned()
        .collect();
    view.pieces.retain(|id, _| !hidden.contains(id));
    for (owner, player) in &mut view.players {
        if !audience.controls(owner) {
            player.deck.hand_pieces.clear();
            player.deck.pocket_pieces.clear();
        }
        player
            .deck
            .starting_pieces
            .retain(|id| !hidden.contains(id));
    }
    // Built-in definitions are a public catalog. Private custom definitions and
    // manifests are revealed only when a visible instance uses them.
    let visible_types: HashSet<_> = view.pieces.values().map(|p| p.type_id.clone()).collect();
    let private_types: HashSet<_> = state
        .pieces
        .iter()
        .filter(|(id, _)| hidden.contains(*id))
        .map(|(_, p)| p.type_id.clone())
        .filter(|id| !visible_types.contains(id))
        .collect();
    let private_packages: HashSet<_> = view
        .custom_piece_manifest
        .iter()
        .filter(|entry| {
            entry
                .runtime_type_ids
                .iter()
                .any(|id| private_types.contains(id))
                && !entry
                    .runtime_type_ids
                    .iter()
                    .any(|id| visible_types.contains(id))
        })
        .flat_map(|entry| entry.runtime_type_ids.iter().cloned())
        .collect();
    view.custom_piece_manifest
        .retain(|entry| !private_packages.contains(&entry.exposed_type_id));
    view.piece_definitions
        .retain(|id, _| !private_packages.contains(id));
    view
}

/// Projection-aware revision: catalog omission must never reuse another
/// audience's private catalog. Keep the value within JavaScript's safe integers.
pub(crate) fn catalog_revision(state: &GameState, base: u64, audience: Audience<'_>) -> u64 {
    if state.ruleset == DeckRuleset::Legacy {
        return base;
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    base.hash(&mut hasher);
    audience.controls("white").hash(&mut hasher);
    audience.controls("black").hash(&mut hasher);
    let mut types: Vec<_> = state.piece_definitions.keys().collect();
    types.sort();
    types.hash(&mut hasher);
    hasher.finish() & ((1_u64 << 53) - 1)
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct TimelineFrameView {
    pub(crate) action: brainfuck_chess_engine::ai::AiAction,
    pub(crate) state: TimelineStateView,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct TimelineStateView {
    #[serde(flatten)]
    pub(crate) state: GameState,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub(crate) hand_counts: HashMap<PlayerId, usize>,
}

/// Personalized live responses must never be reused by shared/browser caches.
pub(crate) async fn prevent_live_caching(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = request.uri().path();
    let path = path.strip_prefix("/api").unwrap_or(path);
    let live = path == "/games"
        || path.starts_with("/games/")
        || path == "/rooms"
        || path.starts_with("/rooms/");
    let mut response = next.run(request).await;
    if live {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response.headers_mut().append(
            axum::http::header::VARY,
            axum::http::HeaderValue::from_static("x-game-client-id"),
        );
    }
    response
}

#[cfg(test)]
mod tests;
