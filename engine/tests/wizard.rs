use std::collections::HashMap;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::endgame::apply_ability_action;
use brainfuck_chess_engine::legal_moves::{
    generate_piece_legal_ability_actions, generate_piece_legal_move_actions,
};
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::{calculate_score_limit, create_board};
use brainfuck_chess_engine::types::*;

fn state() -> GameState {
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
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
                        hand_pieces: Vec::new(),
                        extra_deck_pieces: Vec::new(),
                        pocket_pieces: Vec::new(),
                        score_limit: calculate_score_limit(8),
                        total_score: 0,
                    },
                    captured_pieces: Vec::new(),
                },
            )
        })
        .collect();
    GameState {
        ruleset: Default::default(),
        id: "wizard-test".into(),
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
    }
}

fn add_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, square: Square) {
    let definition = &state.piece_definitions[type_id];
    let piece_id: PieceId = id.into();
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
            current_ammo: definition.max_ammo,
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: definition.initial_state(),
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

const EXTRA: &str = "extra_move_remaining";

fn formation() -> GameState {
    let mut s = state();
    add_piece(&mut s, "k", "white", "wizard-king", Square::new(3, 3));
    for (id, x, y) in [("n", 3, 4), ("s", 3, 2), ("e", 4, 3), ("w", 2, 3)] {
        add_piece(&mut s, id, "white", "wizard-cadet", Square::new(x, y));
    }
    add_piece(&mut s, "r", "white", "rook", Square::new(0, 0));
    add_piece(&mut s, "enemy", "black", "wizard-cadet", Square::new(7, 6));
    add_piece(&mut s, "bk", "black", "king", Square::new(7, 7));
    s
}
fn ability(s: &GameState, actor: &str, id: &str, target: &str) -> AbilityAction {
    generate_piece_legal_ability_actions(s, &actor.into(), id)
        .into_iter()
        .find(|a| {
            a.target_piece_id
                .as_ref()
                .is_some_and(|p| p.as_str() == target)
        })
        .unwrap()
}
fn encourage(s: GameState, target: &str) -> GameState {
    let a = ability(&s, "k", "encourage", target);
    submit_action(s, TurnAction::Ability(a)).unwrap()
}
fn movement(s: &GameState, id: &str, x: i32, y: i32) -> MoveAction {
    generate_piece_legal_move_actions(s, &id.into())
        .into_iter()
        .find(|a| a.to == Square::new(x, y))
        .unwrap()
}
fn play_move(s: GameState, id: &str, x: i32, y: i32) -> GameState {
    let a = movement(&s, id, x, y);
    submit_action(s, TurnAction::Move(a)).unwrap()
}
fn accelerated(s: &GameState, id: &str) -> bool {
    s.pieces[id].state.get(EXTRA) == Some(&PieceStateValue::Integer(1))
}

#[test]
fn royal_deck_exclusivity_in_both_formats() {
    use brainfuck_chess_engine::rules::{
        get_back_zone_squares_with_ruleset, get_front_zone_squares_with_ruleset,
        validate_deck_with_ruleset,
    };
    for ruleset in [DeckRuleset::Legacy, DeckRuleset::Standard] {
        for kings in [
            vec!["king"],
            vec!["wizard-king"],
            vec!["king", "wizard-king"],
        ] {
            let mut s = state();
            for (i, kind) in kings.iter().enumerate() {
                add_piece(
                    &mut s,
                    &format!("k{i}"),
                    "white",
                    kind,
                    get_back_zone_squares_with_ruleset(&"white".into(), 8, ruleset)[i],
                );
            }
            for (i, square) in get_front_zone_squares_with_ruleset(&"white".into(), 8, ruleset)
                .into_iter()
                .enumerate()
            {
                add_piece(&mut s, &format!("c{i}"), "white", "wizard-cadet", square);
            }
            let result = validate_deck_with_ruleset(
                &s.players["white"].deck,
                8,
                &s.pieces,
                &s.piece_definitions,
                ruleset,
            );
            assert_eq!(result.valid, kings.len() == 1, "{:?}", result.errors);
        }
    }
}

#[test]
fn encouragement_requires_all_four_friendly_cadets() {
    let s = formation();
    assert!(!generate_piece_legal_ability_actions(&s, &"k".into(), "encourage").is_empty());
    for variant in 0..4 {
        let mut bad = s.clone();
        match variant {
            0 => {
                bad.board
                    .set_piece_at_layer(Square::new(3, 4), PieceLayer::Ground, None);
            }
            1 => bad.pieces.get_mut("n").unwrap().type_id = "wizard-king".into(),
            2 => bad.pieces.get_mut("n").unwrap().owner = "black".into(),
            _ => {
                bad.board
                    .set_piece_at_layer(Square::new(3, 3), PieceLayer::Ground, None);
                bad.pieces.get_mut("k").unwrap().current_square = Some(Square::new(0, 0));
                bad.board.set_piece_at_layer(
                    Square::new(0, 0),
                    PieceLayer::Ground,
                    Some("k".into()),
                );
            }
        }
        assert!(generate_piece_legal_ability_actions(&bad, &"k".into(), "encourage").is_empty());
        assert!(
            submit_action(bad, TurnAction::Ability(ability(&s, "k", "encourage", "k"))).is_err()
        );
    }
}

#[test]
fn target_and_caster_validation_rejects_forged_or_stale_actions() {
    let s = formation();
    for (actor, name) in [("k", "encourage"), ("n", "linked-teleport")] {
        let actions = generate_piece_legal_ability_actions(&s, &actor.into(), name);
        assert!(actions
            .iter()
            .any(|a| a.target_piece_id == Some("s".into())));
        for target in ["r", "enemy", "missing"] {
            let mut forged = actions[0].clone();
            forged.target_piece_id = Some(target.into());
            forged.to = s.pieces.get(target).and_then(|p| p.current_square);
            assert!(submit_action(s.clone(), TurnAction::Ability(forged)).is_err());
        }
        let mut wrong_player = actions[0].clone();
        wrong_player.player_id = "black".into();
        assert!(submit_action(s.clone(), TurnAction::Ability(wrong_player)).is_err());
        let mut wrong_caster = actions[0].clone();
        wrong_caster.piece_id = "r".into();
        assert!(submit_action(s.clone(), TurnAction::Ability(wrong_caster)).is_err());
        let mut captured = s.clone();
        captured.pieces.get_mut("s").unwrap().captured = true;
        assert!(
            submit_action(captured, TurnAction::Ability(ability(&s, actor, name, "s"))).is_err()
        );
    }
    assert!(
        generate_piece_legal_ability_actions(&s, &"k".into(), "encourage")
            .iter()
            .any(|a| a.target_piece_id == Some("k".into()))
    );
    let mut self_swap = ability(&s, "n", "linked-teleport", "s");
    self_swap.target_piece_id = Some("n".into());
    self_swap.to = s.pieces["n"].current_square;
    assert!(submit_action(s, TurnAction::Ability(self_swap)).is_err());
}

#[test]
fn acceleration_first_move_is_free_then_same_or_other_piece_ends_turn() {
    for other in [false, true] {
        let s = encourage(formation(), "n");
        assert_eq!(
            (&*s.current_player, s.turn_number, s.history.len()),
            ("white", 1, 1)
        );
        assert!(accelerated(&s, "n"));
        let s = play_move(s, "n", 3, 5);
        assert_eq!((&*s.current_player, s.turn_number), ("white", 1));
        assert!(!accelerated(&s, "n"));
        let s = if other {
            play_move(s, "r", 0, 1)
        } else {
            play_move(s, "n", 3, 6)
        };
        assert_eq!((&*s.current_player, s.turn_number), ("black", 2));
    }
}

#[test]
fn unused_acceleration_expires_and_does_not_return_next_turn() {
    let s = encourage(formation(), "n");
    let s = play_move(s, "r", 0, 1);
    assert!(!accelerated(&s, "n"));
    let s = play_move(s, "bk", 6, 7);
    assert_eq!(s.current_player, "white");
    assert!(!accelerated(&s, "n"));
}

#[test]
fn swap_is_atomic_preserves_piece_state_then_turn_expiration_clears_acceleration() {
    let s = encourage(formation(), "n");
    let a = ability(&s, "s", "linked-teleport", "n");
    let applied = apply_ability_action(s.clone(), a.clone());
    assert!(accelerated(&applied, "n"));
    assert_eq!(
        applied.pieces["n"].current_square,
        s.pieces["s"].current_square
    );
    assert_eq!(
        applied.pieces["s"].current_square,
        s.pieces["n"].current_square
    );
    assert_eq!(
        applied.board.get_piece_at(&Square::new(3, 2)),
        Some(&"n".into())
    );
    assert_eq!(applied.history.len(), s.history.len());
    let committed = submit_action(s, TurnAction::Ability(a)).unwrap();
    assert_eq!(
        (&*committed.current_player, committed.turn_number),
        ("black", 2)
    );
    assert!(!accelerated(&committed, "n"));
    assert_eq!(committed.history.len(), 2);
    assert!(matches!(
        committed.history[1].action,
        TurnAction::Ability(_)
    ));
}

#[test]
fn serialization_and_canonical_replay_reproduce_every_intermediate_state() {
    let initial = formation();
    let s1 = encourage(initial.clone(), "n");
    let s2 = play_move(s1.clone(), "n", 3, 5);
    let action = ability(&s2, "s", "linked-teleport", "n");
    let s3 = submit_action(s2.clone(), TurnAction::Ability(action)).unwrap();
    let mut replay = initial;
    for expected in [s1, s2, s3] {
        let restored: GameState =
            serde_json::from_str(&serde_json::to_string(&expected).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let action: TurnAction = serde_json::from_str(
            &serde_json::to_string(&expected.history.last().unwrap().action).unwrap(),
        )
        .unwrap();
        replay = submit_action(replay, action).unwrap();
        assert_eq!(
            serde_json::to_value(&replay).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}

#[test]
fn cadet_has_exact_movement_and_only_implemented_wizard_promotions() {
    let mut s = state();
    add_piece(&mut s, "c", "white", "wizard-cadet", Square::new(3, 6));
    let actions = generate_piece_legal_move_actions(&s, &"c".into());
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].to, Square::new(3, 7));
    assert_eq!(
        actions
            .iter()
            .map(|a| a.promotion.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["wizard-queen", "wizard-rook"]
    );
    assert_eq!(s.piece_definitions["wizard-cadet"].score, 1);
    assert_eq!(s.piece_definitions["wizard-cadet"].promotion_pool.len(), 2);
    add_piece(&mut s, "victim", "black", "rook", Square::new(3, 7));
    assert_eq!(
        movement(&s, "c", 3, 7).captured_piece_id,
        Some("victim".into())
    );
}

#[test]
fn wizard_king_reuses_king_movement_including_castling() {
    let mut original = state();
    add_piece(&mut original, "k", "white", "king", Square::new(4, 0));
    add_piece(&mut original, "r", "white", "rook", Square::new(7, 0));
    let mut wizard = original.clone();
    wizard.pieces.get_mut("k").unwrap().type_id = "wizard-king".into();
    assert_eq!(
        generate_piece_legal_move_actions(&original, &"k".into()),
        generate_piece_legal_move_actions(&wizard, &"k".into())
    );
    let wizard = play_move(wizard, "k", 6, 0);
    assert_eq!(wizard.pieces["r"].current_square, Some(Square::new(5, 0)));
}

#[test]
fn wizard_membership_is_independent_of_display_name() {
    let mut s = formation();
    s.piece_definitions.get_mut("wizard-cadet").unwrap().name = "renamed".into();
    s.piece_definitions.get_mut("rook").unwrap().name = "마법사 wizard".into();
    let actions = generate_piece_legal_ability_actions(&s, &"k".into(), "encourage");
    assert!(actions
        .iter()
        .any(|a| a.target_piece_id == Some("n".into())));
    assert!(!actions
        .iter()
        .any(|a| a.target_piece_id == Some("r".into())));
}

#[test]
fn black_cadet_uses_existing_mirrored_pawn_convention() {
    let mut s = state();
    s.current_player = "black".into();
    add_piece(
        &mut s,
        "c",
        "black",
        "wizard-cadet-black",
        Square::new(3, 1),
    );
    add_piece(&mut s, "k", "black", "wizard-king", Square::new(5, 5));
    let actions = generate_piece_legal_move_actions(&s, &"c".into());
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].to, Square::new(3, 0));
    assert_eq!(
        actions
            .iter()
            .map(|a| a.promotion.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["wizard-queen", "wizard-rook"]
    );
    assert_eq!(
        generate_piece_legal_ability_actions(&s, &"c".into(), "linked-teleport").len(),
        1
    );
}

#[test]
fn bot_finishes_accelerated_moves_in_one_recorded_turn() {
    use brainfuck_chess_engine::ai::{play_bot_turn_detailed, BotDifficulty};
    let mut s = state();
    // A single cadet has no swap target; all its available first moves are free.
    add_piece(&mut s, "c", "white", "wizard-cadet", Square::new(3, 2));
    s.pieces
        .get_mut("c")
        .unwrap()
        .state
        .insert(EXTRA.into(), PieceStateValue::Integer(1));
    let result = play_bot_turn_detailed(s, &"white".into(), BotDifficulty::Easy).unwrap();
    assert_eq!(result.state.current_player, "black");
    assert_eq!(result.actions.len(), 2);
    assert_eq!(result.timeline[0].state.current_player, "white");
    assert_eq!(result.timeline[1].state.current_player, "black");
}

#[test]
fn unrelated_custom_state_key_never_grants_a_free_move_or_gets_erased() {
    let mut s = formation();
    s.pieces
        .get_mut("r")
        .unwrap()
        .state
        .insert(EXTRA.into(), PieceStateValue::Integer(1));
    let s = play_move(s, "r", 0, 1);
    assert_eq!(s.current_player, "black");
    assert_eq!(
        s.pieces["r"].state.get(EXTRA),
        Some(&PieceStateValue::Integer(1))
    );
}

const GUN: &str = "alekhines-gun";
fn gun_formation() -> GameState {
    let mut s = state();
    add_piece(&mut s, "q", "white", "wizard-queen", Square::new(0, 0));
    add_piece(&mut s, "r1", "white", "wizard-rook", Square::new(2, 0));
    add_piece(&mut s, "r2", "white", "wizard-rook", Square::new(4, 0));
    s
}

#[test]
fn gun_all_four_directions_with_gaps_and_nearest_two_rooks() {
    let mut s = state();
    add_piece(&mut s, "q", "white", "wizard-queen", Square::new(3, 3));
    for (i, (dx, dy)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
        for distance in [1, 3] {
            add_piece(
                &mut s,
                &format!("r{i}-{distance}"),
                "white",
                "wizard-rook",
                Square::new(3 + dx * distance, 3 + dy * distance),
            );
        }
    }
    let actions = generate_piece_legal_ability_actions(&s, &"q".into(), GUN);
    assert_eq!(actions.len(), 4);
    assert!(actions
        .iter()
        .all(|a| a.target_piece_id.as_ref().unwrap().as_str().ends_with("-3")));
    let mut s = gun_formation();
    add_piece(&mut s, "r3", "white", "wizard-rook", Square::new(6, 0));
    assert_eq!(
        generate_piece_legal_ability_actions(&s, &"q".into(), GUN).len(),
        1
    );
    let a = ability(&s, "q", GUN, "r2");
    let after = submit_action(s, TurnAction::Ability(a)).unwrap();
    assert!(after.pieces["r3"].captured);
}

#[test]
fn gun_rejects_obstacles_wrong_owners_types_layers_and_stale_payloads() {
    let initial = gun_formation();
    let a = ability(&initial, "q", GUN, "r2");
    for variant in 0..10 {
        let mut s = initial.clone();
        match variant {
            0 | 1 => add_piece(
                &mut s,
                "block",
                "white",
                "pawn-white",
                Square::new(1 + variant * 2, 0),
            ),
            2 => s.pieces.get_mut("r1").unwrap().owner = "black".into(),
            3 => s.pieces.get_mut("r2").unwrap().owner = "black".into(),
            4 => s.pieces.get_mut("r1").unwrap().type_id = "rook".into(),
            5 => s.pieces.get_mut("q").unwrap().type_id = "queen".into(),
            6 => s.current_player = "black".into(),
            7 => s.pieces.get_mut("r2").unwrap().captured = true,
            8 => {
                add_piece(&mut s, "block", "black", "bomber", Square::new(1, 0));
                s.board
                    .set_piece_at_layer(Square::new(1, 0), PieceLayer::Ground, None);
                s.board.set_piece_at_layer(
                    Square::new(1, 0),
                    PieceLayer::Air,
                    Some("block".into()),
                );
                s.pieces.get_mut("block").unwrap().layer = PieceLayer::Air;
            }
            _ => {
                s.pieces.get_mut("r2").unwrap().current_square = Some(Square::new(4, 1));
            }
        }
        assert!(
            submit_action(s, TurnAction::Ability(a.clone())).is_err(),
            "variant {variant}"
        );
    }
    for target in ["r1", "q", "missing"] {
        let mut forged = a.clone();
        forged.target_piece_id = Some(target.into());
        assert!(submit_action(initial.clone(), TurnAction::Ability(forged)).is_err());
    }
    let mut s = initial;
    s.piece_definitions.get_mut("wizard-rook").unwrap().name = "renamed".into();
    assert_eq!(
        generate_piece_legal_ability_actions(&s, &"q".into(), GUN).len(),
        1
    );
}

#[test]
fn gun_sweeps_both_layers_atomically_and_replays_after_serialization() {
    let mut s = gun_formation();
    add_piece(&mut s, "ally", "white", "rook", Square::new(5, 0));
    add_piece(&mut s, "enemy", "black", "rook", Square::new(7, 0));
    add_piece(&mut s, "air", "black", "bomber", Square::new(6, 0));
    s.board
        .set_piece_at_layer(Square::new(6, 0), PieceLayer::Ground, None);
    s.board
        .set_piece_at_layer(Square::new(6, 0), PieceLayer::Air, Some("air".into()));
    s.pieces.get_mut("air").unwrap().layer = PieceLayer::Air;
    for id in ["q", "r1", "r2"] {
        s.pieces
            .get_mut(id)
            .unwrap()
            .state
            .insert(EXTRA.into(), PieceStateValue::Integer(1));
    }
    let a = ability(&s, "q", GUN, "r2");
    let applied = apply_ability_action(s.clone(), a.clone());
    assert!(accelerated(&applied, "q"));
    let after = submit_action(s.clone(), TurnAction::Ability(a)).unwrap();
    assert_eq!(after.current_player, "black");
    assert_eq!(after.history.len(), 1);
    for id in ["q", "r1", "r2"] {
        assert!(!after.pieces[id].captured);
        assert!(!accelerated(&after, id));
    }
    for id in ["ally", "enemy", "air"] {
        assert!(after.pieces[id].captured);
        assert!(after.pieces[id].current_square.is_none());
    }
    let restored: GameState =
        serde_json::from_value(serde_json::to_value(&after).unwrap()).unwrap();
    let replay = submit_action(s, restored.history[0].action.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}

#[test]
fn gun_royal_removal_uses_existing_winner_rules() {
    for owner in ["white", "black"] {
        let mut s = gun_formation();
        add_piece(&mut s, "royal", owner, "wizard-king", Square::new(6, 0));
        let a = ability(&s, "q", GUN, "r2");
        let after = submit_action(s, TurnAction::Ability(a)).unwrap();
        assert_eq!(after.phase, GamePhase::Ended);
        assert_eq!(
            after.result.unwrap().winner.unwrap(),
            if owner == "white" { "black" } else { "white" }
        );
    }
}

#[test]
fn wizard_queen_and_rook_reuse_base_movement() {
    for (base, wizard) in [("queen", "wizard-queen"), ("rook", "wizard-rook")] {
        let mut s = state();
        add_piece(&mut s, "p", "white", base, Square::new(3, 3));
        let before = generate_piece_legal_move_actions(&s, &"p".into());
        s.pieces.get_mut("p").unwrap().type_id = wizard.into();
        assert_eq!(before, generate_piece_legal_move_actions(&s, &"p".into()));
    }
}

#[test]
fn gun_rejects_diagonals_split_rooks_and_cooldown_but_allows_empty_ray() {
    for positions in [[(4, 4), (5, 5)], [(2, 3), (4, 3)], [(4, 3), (3, 4)]] {
        let mut s = state();
        add_piece(&mut s, "q", "white", "wizard-queen", Square::new(3, 3));
        for (i, (x, y)) in positions.into_iter().enumerate() {
            add_piece(
                &mut s,
                &format!("r{i}"),
                "white",
                "wizard-rook",
                Square::new(x, y),
            );
        }
        assert!(generate_piece_legal_ability_actions(&s, &"q".into(), GUN).is_empty());
    }
    let mut s = gun_formation();
    let action = ability(&s, "q", GUN, "r2");
    s.pieces
        .get_mut("q")
        .unwrap()
        .move_option_cooldowns
        .insert(GUN.into(), CooldownState { remaining: 1 });
    assert!(submit_action(s.clone(), TurnAction::Ability(action.clone())).is_err());
    s.pieces.get_mut("q").unwrap().move_option_cooldowns.clear();
    let after = submit_action(s, TurnAction::Ability(action)).unwrap();
    assert_eq!(after.current_player, "black");
    assert!(after.pieces.values().all(|p| !p.captured));
}

const TRANSFER: &str = "transfer-circle";
fn transfer_formation(owner: &str, kind: &str) -> GameState {
    let mut s = state();
    s.current_player = owner.into();
    add_piece(&mut s, "r", owner, "wizard-rook", Square::new(3, 3));
    add_piece(&mut s, "left", owner, "wizard-cadet", Square::new(2, 3));
    add_piece(&mut s, "right", owner, "wizard-cadet", Square::new(4, 3));
    add_piece(
        &mut s,
        "passenger",
        owner,
        kind,
        Square::new(3, if owner == "white" { 2 } else { 4 }),
    );
    s
}

#[test]
fn transfer_circle_matches_picture_for_both_players_and_all_board_sizes() {
    for owner in ["white", "black"] {
        for size in 8..=12 {
            for ruleset in [DeckRuleset::Legacy, DeckRuleset::Standard] {
                let mut s = transfer_formation(owner, "knight");
                // Preserve fixture occupancy while extending the board.
                let mut board = create_board(size);
                for p in s.pieces.values() {
                    board.set_piece_at_layer(
                        p.current_square.unwrap(),
                        p.layer,
                        Some(p.id.clone()),
                    );
                }
                s.board = board;
                s.ruleset = ruleset;
                let actions = generate_piece_legal_ability_actions(&s, &"r".into(), TRANSFER);
                assert_eq!(actions.len(), (size * size - 4) as usize);
                assert!(actions
                    .iter()
                    .all(|a| a.target_piece_id == Some("passenger".into())));
                for destination in [Square::new(0, 0), Square::new(size - 1, size - 1)] {
                    let a = actions
                        .iter()
                        .find(|a| a.to == Some(destination))
                        .unwrap()
                        .clone();
                    let after = submit_action(s.clone(), TurnAction::Ability(a)).unwrap();
                    assert_eq!(after.pieces["passenger"].current_square, Some(destination));
                    assert_eq!(
                        after.board.get_piece_at(&destination),
                        Some(&"passenger".into())
                    );
                    assert_ne!(after.current_player, owner);
                    assert_eq!(after.history.len(), 1);
                    for id in ["r", "left", "right"] {
                        let mut expected = s.pieces[id].clone();
                        expected.state.remove(EXTRA);
                        assert_eq!(
                            serde_json::to_value(&after.pieces[id]).unwrap(),
                            serde_json::to_value(expected).unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn transfer_circle_revalidates_formation_and_destination_on_submission() {
    let initial = transfer_formation("white", "knight");
    let a = generate_piece_legal_ability_actions(&initial, &"r".into(), TRANSFER).remove(0);
    for variant in 0..9 {
        let mut s = initial.clone();
        match variant {
            0 => s.pieces.get_mut("left").unwrap().owner = "black".into(),
            1 => s.pieces.get_mut("right").unwrap().type_id = "pawn-white".into(),
            2 => s.pieces.get_mut("passenger").unwrap().owner = "black".into(),
            3 => s.pieces.get_mut("passenger").unwrap().captured = true,
            4 => {
                s.board
                    .set_piece_at_layer(Square::new(2, 3), PieceLayer::Ground, None);
            }
            5 => s.pieces.get_mut("r").unwrap().type_id = "rook".into(),
            6 => s.current_player = "black".into(),
            7 => {
                s.pieces
                    .get_mut("r")
                    .unwrap()
                    .move_option_cooldowns
                    .insert(TRANSFER.into(), CooldownState { remaining: 1 });
            }
            _ => add_piece(&mut s, "block", "black", "rook", a.to.unwrap()),
        }
        assert!(
            submit_action(s, TurnAction::Ability(a.clone())).is_err(),
            "{variant}"
        );
    }
    for to in [
        Square::new(-1, 0),
        Square::new(8, 0),
        Square::new(3, 3),
        Square::new(3, 2),
    ] {
        let mut forged = a.clone();
        forged.to = Some(to);
        assert!(submit_action(initial.clone(), TurnAction::Ability(forged)).is_err());
    }
    for id in ["r", "left", "missing"] {
        let mut forged = a.clone();
        forged.target_piece_id = Some(id.into());
        assert!(submit_action(initial.clone(), TurnAction::Ability(forged)).is_err());
    }
    let mut renamed = initial;
    renamed
        .piece_definitions
        .get_mut("wizard-cadet")
        .unwrap()
        .name = "renamed".into();
    assert!(!generate_piece_legal_ability_actions(&renamed, &"r".into(), TRANSFER).is_empty());
}

#[test]
fn transfer_circle_preserves_royal_and_piece_state_and_expires_acceleration() {
    for kind in ["king", "pawn-white", "wizard-queen", "tank"] {
        let mut s = transfer_formation("white", kind);
        s.pieces
            .get_mut("r")
            .unwrap()
            .state
            .insert(EXTRA.into(), PieceStateValue::Integer(1));
        let a = generate_piece_legal_ability_actions(&s, &"r".into(), TRANSFER).remove(0);
        let applied = apply_ability_action(s.clone(), a.clone());
        assert!(accelerated(&applied, "r"));
        let after = submit_action(s.clone(), TurnAction::Ability(a)).unwrap();
        assert!(!accelerated(&after, "r"));
        assert_eq!(after.current_player, "black");
        assert!(after.result.is_none());
        assert!(!after.pieces["passenger"].captured);
        assert!(after.pieces["passenger"].has_moved);
        assert_eq!(
            after.pieces["passenger"].type_id,
            s.pieces["passenger"].type_id
        );
        assert_eq!(
            after.pieces["passenger"].current_ammo,
            s.pieces["passenger"].current_ammo
        );
        let restored: GameState =
            serde_json::from_value(serde_json::to_value(&after).unwrap()).unwrap();
        let replay = submit_action(s, restored.history[0].action.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(replay).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
}

#[test]
fn transfer_circle_air_passenger_keeps_layer_and_air_occupied_destinations_are_blocked() {
    let mut s = transfer_formation("white", "bomber");
    let from = s.pieces["passenger"].current_square.unwrap();
    s.board.set_piece_at_layer(from, PieceLayer::Ground, None);
    s.board
        .set_piece_at_layer(from, PieceLayer::Air, Some("passenger".into()));
    s.pieces.get_mut("passenger").unwrap().layer = PieceLayer::Air;
    s.pieces
        .get_mut("passenger")
        .unwrap()
        .remaining_flight_turns = 5;
    s.pieces
        .get_mut("passenger")
        .unwrap()
        .state
        .insert("airborne".into(), PieceStateValue::Boolean(true));
    let actions = generate_piece_legal_ability_actions(&s, &"r".into(), TRANSFER);
    assert!(actions.iter().all(|a| a.to != Some(from)));
    let a = actions[0].clone();
    let after = submit_action(s, TurnAction::Ability(a.clone())).unwrap();
    assert_eq!(
        after
            .board
            .get_piece_at_layer(&a.to.unwrap(), PieceLayer::Air),
        Some(&"passenger".into())
    );
    assert_eq!(after.pieces["passenger"].remaining_flight_turns, 4);
    assert_eq!(after.pieces["passenger"].layer, PieceLayer::Air);
}
