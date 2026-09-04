use std::collections::HashSet;

use crate::actions::apply_canonical_action;
use crate::ai::types::AiAction;
use crate::types::{GameState, PieceId, PieceLayer, PieceStateValue, Square, TurnAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    Root,
    Interior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BeamPolicy {
    pub root_width: usize,
    pub moderate_width: usize,
    pub high_width: usize,
    pub dense_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BeamCategoryQuota {
    pub board: usize,
    pub quiet_drop: usize,
}

impl Default for BeamPolicy {
    fn default() -> Self {
        Self {
            root_width: 24,
            moderate_width: 24,
            high_width: 16,
            dense_width: 12,
        }
    }
}

impl BeamPolicy {
    pub(crate) fn width_for(self, kind: NodeKind, unique_count: usize) -> usize {
        if unique_count <= 24 {
            return unique_count;
        }
        if kind == NodeKind::Root {
            return self.root_width;
        }
        match unique_count {
            25..=64 => self.moderate_width,
            65..=128 => self.high_width,
            _ => self.dense_width,
        }
    }

    pub(crate) fn category_quota(self, optional_width: usize) -> BeamCategoryQuota {
        match optional_width {
            24 => BeamCategoryQuota {
                board: 16,
                quiet_drop: 8,
            },
            16 => BeamCategoryQuota {
                board: 12,
                quiet_drop: 4,
            },
            12 => BeamCategoryQuota {
                board: 8,
                quiet_drop: 4,
            },
            width => {
                let quiet_drop = width / 3;
                BeamCategoryQuota {
                    board: width - quiet_drop,
                    quiet_drop,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BeamSelectionCounts {
    pub selected: usize,
    pub mandatory: usize,
    pub drops_selected: usize,
    pub board_optional_generated: usize,
    pub board_optional_selected: usize,
    pub quiet_drop_generated: usize,
    pub quiet_drop_selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionalCategory {
    Board,
    QuietDrop,
}

fn optional_category(state: &GameState, action: &AiAction) -> OptionalCategory {
    match action {
        AiAction::Move(_) => OptionalCategory::Board,
        AiAction::Drop(_) => OptionalCategory::QuietDrop,
        AiAction::Ability(ability) => {
            let source_is_in_pocket = state
                .pieces
                .get(&ability.piece_id)
                .is_some_and(|piece| piece.in_pocket);
            if source_is_in_pocket
                || ability.pocket_piece_id.is_some()
                || !ability.deployments.is_empty()
            {
                OptionalCategory::QuietDrop
            } else {
                OptionalCategory::Board
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PocketPieceKey {
    owner: String,
    type_id: String,
    has_moved: bool,
    current_ammo: u32,
    layer: PieceLayer,
    remaining_flight_turns: u32,
    state: Vec<(String, PieceStateValue)>,
    cooldowns: Vec<(String, u32)>,
}

impl PocketPieceKey {
    pub(crate) fn from_id(state: &GameState, piece_id: &PieceId) -> Option<Self> {
        let piece = state.pieces.get(piece_id)?;
        let mut piece_state = piece
            .state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        piece_state.sort_by(|left, right| left.0.cmp(&right.0));
        let mut cooldowns = piece
            .move_option_cooldowns
            .iter()
            .map(|(key, value)| (key.clone(), value.remaining))
            .collect::<Vec<_>>();
        cooldowns.sort_by(|left, right| left.0.cmp(&right.0));
        Some(Self {
            owner: piece.owner.clone(),
            type_id: piece.type_id.clone(),
            has_moved: piece.has_moved,
            current_ammo: piece.current_ammo,
            layer: piece.layer,
            remaining_flight_turns: piece.remaining_flight_turns,
            state: piece_state,
            cooldowns,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PocketReferenceKey {
    Semantic(PocketPieceKey),
    Missing(PieceId),
}

fn pocket_reference_key(state: &GameState, piece_id: &PieceId) -> PocketReferenceKey {
    PocketPieceKey::from_id(state, piece_id)
        .map(PocketReferenceKey::Semantic)
        .unwrap_or_else(|| PocketReferenceKey::Missing(piece_id.clone()))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CanonicalActionKey {
    Drop {
        player_id: String,
        piece: PocketReferenceKey,
        to: Square,
        captured_piece_id: Option<PieceId>,
    },
    Ability {
        player_id: String,
        piece_id: PieceId,
        ability_id: String,
        target_piece_id: Option<PieceId>,
        pocket_piece: Option<PocketReferenceKey>,
        to: Option<Square>,
        deployments: Vec<(PocketReferenceKey, Square)>,
    },
}

/// Collapse only actions whose interchangeable pocket pieces have identical
/// rule-relevant state. Board-piece moves retain their concrete identity.
pub(crate) fn canonicalize_actions(state: &GameState, actions: Vec<AiAction>) -> Vec<AiAction> {
    let mut seen = HashSet::new();
    let mut unique = Vec::with_capacity(actions.len());
    for action in actions {
        let key = match &action {
            AiAction::Move(_) => {
                unique.push(action);
                continue;
            }
            AiAction::Drop(drop) => CanonicalActionKey::Drop {
                player_id: drop.player_id.clone(),
                piece: pocket_reference_key(state, &drop.piece_id),
                to: drop.to,
                captured_piece_id: drop.captured_piece_id.clone(),
            },
            AiAction::Ability(ability) => {
                let mut deployments = ability
                    .deployments
                    .iter()
                    .map(|deployment| {
                        (
                            pocket_reference_key(state, &deployment.pocket_piece_id),
                            deployment.to,
                        )
                    })
                    .collect::<Vec<_>>();
                deployments.sort_by(|left, right| {
                    format!("{:?}", left.0)
                        .cmp(&format!("{:?}", right.0))
                        .then_with(|| (left.1.file, left.1.rank).cmp(&(right.1.file, right.1.rank)))
                });
                CanonicalActionKey::Ability {
                    player_id: ability.player_id.clone(),
                    piece_id: ability.piece_id.clone(),
                    ability_id: ability.ability_id.clone(),
                    target_piece_id: ability.target_piece_id.clone(),
                    pocket_piece: ability
                        .pocket_piece_id
                        .as_ref()
                        .map(|id| pocket_reference_key(state, id)),
                    to: ability.to,
                    deployments,
                }
            }
        };
        if seen.insert(key) {
            unique.push(action);
        }
    }
    unique
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TacticalImpact {
    pub captures_king: bool,
    pub removed_enemy_pieces: u32,
    pub removed_enemy_value: u32,
    pub promotion: bool,
}

impl TacticalImpact {
    pub(crate) fn is_mandatory(self) -> bool {
        self.captures_king || self.removed_enemy_pieces > 0 || self.promotion
    }
}

pub(crate) fn tactical_impact(state: &GameState, action: &AiAction) -> TacticalImpact {
    match action {
        AiAction::Move(action) => {
            let captured = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id));
            TacticalImpact {
                captures_king: captured.is_some_and(|definition| definition.is_king),
                removed_enemy_pieces: u32::from(captured.is_some()),
                removed_enemy_value: captured.map_or(0, |definition| definition.score),
                promotion: action.promotion.is_some(),
            }
        }
        AiAction::Drop(action) => {
            let captured = action
                .captured_piece_id
                .as_ref()
                .and_then(|id| state.pieces.get(id))
                .and_then(|piece| state.piece_definitions.get(&piece.type_id));
            TacticalImpact {
                captures_king: captured.is_some_and(|definition| definition.is_king),
                removed_enemy_pieces: u32::from(captured.is_some()),
                removed_enemy_value: captured.map_or(0, |definition| definition.score),
                promotion: false,
            }
        }
        AiAction::Ability(ability) => {
            let next = apply_canonical_action(state.clone(), TurnAction::Ability(ability.clone()));
            let mut impact = TacticalImpact {
                captures_king: next
                    .result
                    .as_ref()
                    .and_then(|result| result.winner.as_ref())
                    == Some(&ability.player_id),
                ..TacticalImpact::default()
            };
            for (piece_id, before) in &state.pieces {
                if before.owner == ability.player_id || !before.is_on_board() {
                    continue;
                }
                let removed = next
                    .pieces
                    .get(piece_id)
                    .is_none_or(|after| !after.is_on_board());
                if removed {
                    impact.removed_enemy_pieces += 1;
                    impact.removed_enemy_value += state
                        .piece_definitions
                        .get(&before.type_id)
                        .map_or(0, |definition| definition.score);
                }
            }
            impact
        }
    }
}

fn has_immediate_king_capture(state: &GameState, threatened_player: &str) -> bool {
    let Some(opponent) = state
        .players
        .keys()
        .find(|player_id| player_id.as_str() != threatened_player)
        .cloned()
    else {
        return false;
    };
    let mut view = state.clone();
    view.current_player = opponent;
    crate::legal_moves::generate_legal_move_actions(&view)
        .into_iter()
        .map(AiAction::Move)
        .chain(
            crate::legal_moves::generate_legal_drop_actions(&view)
                .into_iter()
                .map(AiAction::Drop),
        )
        .chain(
            crate::legal_moves::generate_legal_ability_actions(&view)
                .into_iter()
                .map(AiAction::Ability),
        )
        .any(|action| tactical_impact(&view, &action).captures_king)
}

pub(crate) fn select_beam_actions(
    state: &GameState,
    actions: Vec<AiAction>,
    kind: NodeKind,
    enabled: bool,
    priority: Option<&AiAction>,
    policy: BeamPolicy,
) -> (Vec<AiAction>, BeamSelectionCounts) {
    if !enabled {
        let mut counts = BeamSelectionCounts {
            selected: actions.len(),
            drops_selected: actions
                .iter()
                .filter(|action| matches!(action, AiAction::Drop(_)))
                .count(),
            ..BeamSelectionCounts::default()
        };
        for action in &actions {
            if tactical_impact(state, action).is_mandatory() {
                counts.mandatory += 1;
            } else if priority.is_none_or(|priority| priority != action) {
                match optional_category(state, action) {
                    OptionalCategory::Board => {
                        counts.board_optional_generated += 1;
                        counts.board_optional_selected += 1;
                    }
                    OptionalCategory::QuietDrop => {
                        counts.quiet_drop_generated += 1;
                        counts.quiet_drop_selected += 1;
                    }
                }
            }
        }
        return (actions, counts);
    }

    let forced_landing = crate::legal_moves::pending_landing_piece_id(state).is_some();
    let threatened_player = state.current_player.clone();
    let must_answer_king_threat = has_immediate_king_capture(state, &threatened_player);
    let mut counts = BeamSelectionCounts::default();
    let mut priority_actions = Vec::new();
    let mut mandatory_actions = Vec::new();
    let mut board_actions = Vec::new();
    let mut quiet_drop_actions = Vec::new();
    for action in actions {
        let forced_defense = must_answer_king_threat && {
            let turn_action = match &action {
                AiAction::Move(action) => TurnAction::Move(action.clone()),
                AiAction::Drop(action) => TurnAction::Drop(action.clone()),
                AiAction::Ability(action) => TurnAction::Ability(action.clone()),
            };
            let next = apply_canonical_action(state.clone(), turn_action);
            !has_immediate_king_capture(&next, &threatened_player)
        };
        let tactical_or_forced =
            forced_landing || forced_defense || tactical_impact(state, &action).is_mandatory();
        if priority.is_some_and(|priority| priority == &action) {
            if tactical_or_forced {
                counts.mandatory += 1;
            }
            priority_actions.push(action);
        } else if tactical_or_forced {
            counts.mandatory += 1;
            mandatory_actions.push(action);
        } else {
            match optional_category(state, &action) {
                OptionalCategory::Board => board_actions.push(action),
                OptionalCategory::QuietDrop => quiet_drop_actions.push(action),
            }
        }
    }

    counts.board_optional_generated = board_actions.len();
    counts.quiet_drop_generated = quiet_drop_actions.len();

    let optional_count = board_actions.len() + quiet_drop_actions.len();
    let optional_width = policy.width_for(kind, optional_count);
    let quota = policy.category_quota(optional_width);
    let mut board_selected = quota.board.min(board_actions.len());
    let mut quiet_drop_selected = quota.quiet_drop.min(quiet_drop_actions.len());
    let mut remaining = optional_width.saturating_sub(board_selected + quiet_drop_selected);
    let board_available = board_actions.len().saturating_sub(board_selected);
    let board_fill = remaining.min(board_available);
    board_selected += board_fill;
    remaining -= board_fill;
    quiet_drop_selected += remaining.min(quiet_drop_actions.len() - quiet_drop_selected);

    counts.board_optional_selected = board_selected;
    counts.quiet_drop_selected = quiet_drop_selected;

    let mut selected = Vec::with_capacity(
        priority_actions.len() + mandatory_actions.len() + board_selected + quiet_drop_selected,
    );
    selected.extend(priority_actions);
    selected.extend(mandatory_actions);
    selected.extend(board_actions.into_iter().take(board_selected));
    selected.extend(quiet_drop_actions.into_iter().take(quiet_drop_selected));
    counts.drops_selected = selected
        .iter()
        .filter(|action| matches!(action, AiAction::Drop(_)))
        .count();
    counts.selected = selected.len();
    (selected, counts)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::ai::move_ordering::order_ai_actions;
    use crate::ai::search::generate_ai_actions;
    use crate::legal_moves::generate_legal_ability_actions;
    use crate::pieces::default_pieces::all_default_definitions;
    use crate::rules::create_board;
    use crate::types::{
        AbilityAction, ActionEffects, ChessemblyProgramCache, Deck, DropAction, GamePhase,
        MoveAction, Piece, Player,
    };

    fn state() -> GameState {
        let definitions: HashMap<_, _> = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect();
        let players = ["white", "black"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    Player {
                        id: id.into(),
                        deck: Deck {
                            player_id: id.into(),
                            starting_pieces: Vec::new(),
                            pocket_pieces: Vec::new(),
                            score_limit: 1_000,
                            total_score: 0,
                        },
                        captured_pieces: Vec::new(),
                    },
                )
            })
            .collect();
        GameState {
            id: "beam-test".into(),
            board: create_board(12),
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
        }
    }

    fn add_piece(
        state: &mut GameState,
        id: &str,
        owner: &str,
        type_id: &str,
        square: Option<Square>,
    ) {
        let piece_id: PieceId = id.into();
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
                current_ammo: definition.max_ammo,
                layer: PieceLayer::Ground,
                remaining_flight_turns: 0,
                state: definition.initial_state(),
                move_option_cooldowns: HashMap::new(),
            },
        );
    }

    fn quiet_actions(board_count: usize, drop_count: usize) -> Vec<AiAction> {
        let board = (0..board_count).map(|index| {
            AiAction::Move(MoveAction {
                player_id: "white".into(),
                piece_id: format!("board-{index}").into(),
                from: Square::new(0, 0),
                to: Square::new((index % 12) as i32, ((index / 12) % 12) as i32),
                captured_piece_id: None,
                promotion: None,
                move_option_id: "normal".into(),
                source_layer_ids: Vec::new(),
                effects: ActionEffects::default(),
            })
        });
        let drops = (0..drop_count).map(|index| {
            AiAction::Drop(DropAction {
                player_id: "white".into(),
                piece_id: format!("drop-{index}").into(),
                to: Square::new((index % 12) as i32, ((index / 12) % 12) as i32),
                captured_piece_id: None,
            })
        });
        board.chain(drops).collect()
    }

    fn select_quiet_counts(
        board_count: usize,
        drop_count: usize,
        kind: NodeKind,
    ) -> BeamSelectionCounts {
        let state = state();
        select_beam_actions(
            &state,
            quiet_actions(board_count, drop_count),
            kind,
            true,
            None,
            BeamPolicy::default(),
        )
        .1
    }

    #[test]
    fn category_beam_applies_root_quota_and_redistributes_unused_slots() {
        let balanced = select_quiet_counts(50, 500, NodeKind::Root);
        assert_eq!(balanced.board_optional_generated, 50);
        assert_eq!(balanced.quiet_drop_generated, 500);
        assert_eq!(balanced.board_optional_selected, 16);
        assert_eq!(balanced.quiet_drop_selected, 8);

        let scarce_board = select_quiet_counts(5, 500, NodeKind::Root);
        assert_eq!(scarce_board.board_optional_selected, 5);
        assert_eq!(scarce_board.quiet_drop_selected, 19);

        let scarce_drop = select_quiet_counts(50, 3, NodeKind::Root);
        assert_eq!(scarce_drop.board_optional_selected, 21);
        assert_eq!(scarce_drop.quiet_drop_selected, 3);

        let below_width = select_quiet_counts(7, 4, NodeKind::Root);
        assert_eq!(below_width.board_optional_selected, 7);
        assert_eq!(below_width.quiet_drop_selected, 4);
        assert_eq!(below_width.selected, 11);
    }

    #[test]
    fn category_beam_uses_adaptive_internal_quotas() {
        let width_16 = select_quiet_counts(40, 40, NodeKind::Interior);
        assert_eq!(width_16.board_optional_selected, 12);
        assert_eq!(width_16.quiet_drop_selected, 4);

        let width_12 = select_quiet_counts(80, 80, NodeKind::Interior);
        assert_eq!(width_12.board_optional_selected, 8);
        assert_eq!(width_12.quiet_drop_selected, 4);
    }

    #[test]
    fn adaptive_width_is_based_on_optional_candidates_not_mandatory_actions() {
        let mut state = state();
        let mut actions = quiet_actions(30, 30);
        for index in 0..10 {
            let target_id = format!("target-{index}");
            add_piece(
                &mut state,
                &target_id,
                "black",
                "knight",
                Some(Square::new(index, 10)),
            );
            actions.push(AiAction::Move(MoveAction {
                player_id: "white".into(),
                piece_id: format!("attacker-{index}").into(),
                from: Square::new(index, 0),
                to: Square::new(index, 10),
                captured_piece_id: Some(target_id.into()),
                promotion: None,
                move_option_id: "normal".into(),
                source_layer_ids: Vec::new(),
                effects: ActionEffects::default(),
            }));
        }

        let (_, counts) = select_beam_actions(
            &state,
            actions,
            NodeKind::Interior,
            true,
            None,
            BeamPolicy::default(),
        );
        assert_eq!(counts.mandatory, 10);
        assert_eq!(counts.board_optional_selected, 16);
        assert_eq!(counts.quiet_drop_selected, 8);
    }

    #[test]
    fn tactical_and_priority_drops_stay_ahead_of_category_optional_actions() {
        let mut state = state();
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(11, 11)));
        let mut tactical_actions = quiet_actions(50, 500);
        let tactical_drop = AiAction::Drop(DropAction {
            player_id: "white".into(),
            piece_id: "tactical-drop".into(),
            to: Square::new(11, 11),
            captured_piece_id: Some("bk".into()),
        });
        tactical_actions.push(tactical_drop.clone());
        let (selected, counts) = select_beam_actions(
            &state,
            tactical_actions,
            NodeKind::Root,
            true,
            None,
            BeamPolicy::default(),
        );
        assert!(selected.contains(&tactical_drop));
        assert_eq!(counts.mandatory, 1);
        assert_eq!(counts.board_optional_selected, 16);
        assert_eq!(counts.quiet_drop_selected, 8);

        let priority_actions = quiet_actions(50, 500);
        let priority = priority_actions.last().unwrap().clone();
        let (selected, counts) = select_beam_actions(
            &state,
            priority_actions,
            NodeKind::Root,
            true,
            Some(&priority),
            BeamPolicy::default(),
        );
        assert_eq!(selected.first(), Some(&priority));
        assert_eq!(counts.board_optional_selected, 16);
        assert_eq!(counts.quiet_drop_selected, 8);
    }

    #[test]
    fn quiet_ability_category_follows_board_or_pocket_semantics() {
        let mut state = state();
        add_piece(
            &mut state,
            "board-source",
            "white",
            "knight",
            Some(Square::new(1, 1)),
        );
        add_piece(&mut state, "pocket-source", "white", "knight", None);
        let ability = |piece_id: &str, pocket_piece_id: Option<&str>| {
            AiAction::Ability(AbilityAction {
                player_id: "white".into(),
                piece_id: piece_id.into(),
                ability_id: "quiet".into(),
                target_piece_id: None,
                pocket_piece_id: pocket_piece_id.map(Into::into),
                to: None,
                deployments: Vec::new(),
            })
        };
        assert_eq!(
            optional_category(&state, &ability("board-source", None)),
            OptionalCategory::Board
        );
        assert_eq!(
            optional_category(&state, &ability("pocket-source", None)),
            OptionalCategory::QuietDrop
        );
        assert_eq!(
            optional_category(&state, &ability("board-source", Some("pocket-source"))),
            OptionalCategory::QuietDrop
        );
    }

    #[test]
    fn identical_pocket_instances_share_one_canonical_drop_per_target() {
        let mut state = state();
        add_piece(&mut state, "wk", "white", "king", Some(Square::new(0, 0)));
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(11, 11)));
        for index in 0..25 {
            add_piece(
                &mut state,
                &format!("para-{index:02}"),
                "white",
                "paratrooper",
                None,
            );
        }
        let generated = generate_ai_actions(&state);
        let generated_drops = generated
            .iter()
            .filter(|action| matches!(action, AiAction::Drop(_)))
            .count();
        let unique = canonicalize_actions(&state, generated);
        let unique_drops = unique
            .iter()
            .filter(|action| matches!(action, AiAction::Drop(_)))
            .count();
        assert_eq!(generated_drops, unique_drops * 25);

        state
            .pieces
            .get_mut("para-24")
            .unwrap()
            .state
            .insert("variant".into(), PieceStateValue::Integer(1));
        let distinct = canonicalize_actions(&state, generate_ai_actions(&state));
        assert_eq!(
            distinct
                .iter()
                .filter(|action| matches!(action, AiAction::Drop(_)))
                .count(),
            unique_drops * 2
        );
    }

    #[test]
    fn beam_preserves_king_capture_and_explicit_priority_outside_optional_width() {
        let mut state = state();
        add_piece(&mut state, "wk", "white", "king", Some(Square::new(0, 0)));
        add_piece(&mut state, "wr", "white", "rook", Some(Square::new(5, 0)));
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(5, 11)));
        add_piece(&mut state, "wq", "white", "queen", Some(Square::new(6, 6)));
        add_piece(
            &mut state,
            "target-n",
            "black",
            "knight",
            Some(Square::new(6, 9)),
        );
        add_piece(
            &mut state,
            "target-e",
            "black",
            "bishop",
            Some(Square::new(9, 6)),
        );
        add_piece(
            &mut state,
            "target-sw",
            "black",
            "rook",
            Some(Square::new(3, 3)),
        );
        for index in 0..4 {
            add_piece(
                &mut state,
                &format!("pocket-{index}"),
                "white",
                "knight",
                None,
            );
        }
        let mut actions = canonicalize_actions(&state, generate_ai_actions(&state));
        order_ai_actions(&state, &mut actions, &"white".into());
        let priority = actions
            .iter()
            .rev()
            .find(|action| !tactical_impact(&state, action).is_mandatory())
            .unwrap()
            .clone();
        let policy = BeamPolicy {
            root_width: 0,
            moderate_width: 0,
            high_width: 0,
            dense_width: 0,
        };
        let (selected, counts) = select_beam_actions(
            &state,
            actions,
            NodeKind::Root,
            true,
            Some(&priority),
            policy,
        );
        assert!(selected.contains(&priority));
        assert!(selected.iter().any(|action| {
            matches!(action, AiAction::Move(action) if action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("bk"))
        }));
        assert!(counts.mandatory >= 4);
        assert!(selected.len() > policy.root_width);
    }

    #[test]
    fn enemy_removing_ability_is_tactical_but_quiet_ability_is_not() {
        let mut state = state();
        add_piece(&mut state, "wk", "white", "king", Some(Square::new(0, 0)));
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(11, 11)));
        add_piece(
            &mut state,
            "camp",
            "white",
            "green-camp",
            Some(Square::new(4, 4)),
        );
        add_piece(
            &mut state,
            "enemy",
            "black",
            "rook",
            Some(Square::new(5, 4)),
        );
        add_piece(
            &mut state,
            "friendly",
            "white",
            "bishop",
            Some(Square::new(3, 4)),
        );
        let actions = generate_legal_ability_actions(&state);
        let enemy_recall = actions
            .iter()
            .find(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy"));
        let friendly_recall = actions.iter().find(|action| {
            action.target_piece_id.as_ref().map(PieceId::as_str) == Some("friendly")
        });
        assert!(
            tactical_impact(&state, &AiAction::Ability(enemy_recall.unwrap().clone()))
                .is_mandatory()
        );
        assert!(
            !tactical_impact(&state, &AiAction::Ability(friendly_recall.unwrap().clone()))
                .is_mandatory()
        );
    }

    #[test]
    fn identical_airdrop_pieces_generate_semantic_combinations_not_id_permutations() {
        let mut state = state();
        add_piece(&mut state, "wk", "white", "king", Some(Square::new(0, 0)));
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(11, 11)));
        add_piece(
            &mut state,
            "airborne",
            "white",
            "airborne",
            Some(Square::new(5, 5)),
        );
        for index in 0..25 {
            add_piece(
                &mut state,
                &format!("airdrop-{index:02}"),
                "white",
                "paratrooper",
                None,
            );
        }
        let actions = generate_legal_ability_actions(&state)
            .into_iter()
            .filter(|action| action.ability_id == "airdrop")
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 63);
        assert_eq!(
            actions.iter().map(|action| action.deployments.len()).max(),
            Some(6)
        );
    }

    #[test]
    fn quiet_move_that_blocks_immediate_king_capture_is_preserved() {
        let mut state = state();
        add_piece(&mut state, "wk", "white", "king", Some(Square::new(5, 0)));
        add_piece(&mut state, "wr", "white", "rook", Some(Square::new(4, 1)));
        add_piece(&mut state, "bk", "black", "king", Some(Square::new(11, 11)));
        add_piece(&mut state, "br", "black", "rook", Some(Square::new(5, 11)));
        for index in 0..4 {
            add_piece(
                &mut state,
                &format!("reserve-{index}"),
                "white",
                "knight",
                None,
            );
        }
        let mut actions = canonicalize_actions(&state, generate_ai_actions(&state));
        order_ai_actions(&state, &mut actions, &"white".into());
        let policy = BeamPolicy {
            root_width: 0,
            moderate_width: 0,
            high_width: 0,
            dense_width: 0,
        };
        let (selected, _) =
            select_beam_actions(&state, actions, NodeKind::Root, true, None, policy);
        assert!(selected.iter().any(|action| matches!(
            action,
            AiAction::Move(action)
                if action.piece_id.as_str() == "wr" && action.to == Square::new(5, 1)
        )));
    }
}
