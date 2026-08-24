use std::collections::HashMap;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::endgame::{apply_and_advance_turn, apply_move_action};
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
        id: "ammo-air-test".into(),
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

fn make_airborne(state: &mut GameState, id: &str, square: Square, remaining: u32) {
    let piece = state.pieces.get_mut(id).unwrap();
    state
        .board
        .squares
        .insert(piece.current_square.unwrap().to_id(), None);
    piece.current_square = Some(square);
    piece.layer = PieceLayer::Air;
    piece.remaining_flight_turns = remaining;
    piece
        .state
        .insert("airborne".into(), PieceStateValue::Boolean(true));
    state
        .board
        .air_squares
        .insert(square.to_id(), Some(id.into()));
}

#[test]
fn surface_to_air_missile_has_mirrored_front_line_movement() {
    for (owner, type_id, origin, expected) in [
        (
            "white",
            "surface-to-air-missile-white",
            Square::new(3, 2),
            [
                Square::new(2, 3),
                Square::new(3, 3),
                Square::new(3, 4),
                Square::new(4, 3),
            ],
        ),
        (
            "black",
            "surface-to-air-missile-black",
            Square::new(3, 5),
            [
                Square::new(2, 4),
                Square::new(3, 4),
                Square::new(3, 3),
                Square::new(4, 4),
            ],
        ),
    ] {
        let mut game = state();
        game.current_player = owner.into();
        add_piece(&mut game, "sam", owner, type_id, origin);
        let moves = generate_piece_legal_move_actions(&game, &"sam".into());
        assert_eq!(moves.len(), expected.len());
        for square in expected {
            assert!(
                moves.iter().any(|action| action.to == square),
                "{type_id}: {square:?}"
            );
        }
        let definition = &game.piece_definitions[type_id];
        assert_eq!(definition.score, 2);
        assert_eq!(definition.max_ammo, 2);
        assert_eq!(definition.deployment_zone, DeploymentZone::Front);
        assert!(definition.move_options.iter().any(|option| {
            option.id == "intercept" && option.cooldown.is_none() && option.ammo_cost == 1
        }));
    }
}

#[test]
fn intercept_targets_only_enemy_air_in_the_five_by_three_area_and_spends_ammo() {
    let mut game = state();
    add_piece(
        &mut game,
        "sam",
        "white",
        "surface-to-air-missile-white",
        Square::new(3, 3),
    );
    for (id, owner, square) in [
        ("enemy-a", "black", Square::new(1, 2)),
        ("enemy-b", "black", Square::new(5, 4)),
        ("friendly", "white", Square::new(3, 3)),
        ("too-far-file", "black", Square::new(6, 3)),
        ("too-far-rank", "black", Square::new(3, 5)),
    ] {
        add_piece(&mut game, id, owner, "bomber", Square::new(0, 0));
        make_airborne(&mut game, id, square, 5);
    }
    add_piece(
        &mut game,
        "ground-enemy",
        "black",
        "rook",
        Square::new(4, 3),
    );

    let actions = generate_piece_legal_ability_actions(&game, &"sam".into(), "intercept");
    assert_eq!(actions.len(), 2);
    assert!(actions.iter().all(|action| {
        matches!(
            action.target_piece_id.as_ref().map(PieceId::as_str),
            Some("enemy-a" | "enemy-b")
        )
    }));

    let first = actions
        .into_iter()
        .find(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy-a"))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(first)).unwrap();
    assert!(game.pieces["enemy-a"].captured);
    assert!(game
        .board
        .get_piece_at_layer(&Square::new(1, 2), PieceLayer::Air)
        .is_none());
    assert_eq!(game.pieces["sam"].current_ammo, 1);
    assert!(game.pieces["sam"].move_option_cooldowns.is_empty());

    game.current_player = "white".into();
    let second = generate_piece_legal_ability_actions(&game, &"sam".into(), "intercept")
        .into_iter()
        .find(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy-b"))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(second)).unwrap();
    assert!(game.pieces["enemy-b"].captured);
    assert_eq!(game.pieces["sam"].current_ammo, 0);
    game.current_player = "white".into();
    assert!(generate_piece_legal_ability_actions(&game, &"sam".into(), "intercept").is_empty());
}

#[test]
fn mortar_spends_its_shell_outside_home_and_replenishes_on_home_entry() {
    let mut game = state();
    add_piece(&mut game, "m", "white", "mortar", Square::new(3, 2));
    let shot = generate_piece_legal_ability_actions(&game, &"m".into(), "mortar-barrage")
        .pop()
        .unwrap();
    game = submit_action(game, TurnAction::Ability(shot)).unwrap();
    assert_eq!(game.pieces["m"].current_ammo, 0);
    assert!(generate_piece_legal_ability_actions(&game, &"m".into(), "mortar-barrage").is_empty());

    game.current_player = "white".into();
    let home_move = generate_piece_legal_move_actions(&game, &"m".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 1))
        .unwrap();
    game = apply_move_action(game, home_move);
    assert_eq!(game.pieces["m"].current_ammo, 1);
    assert_eq!(
        game.pieces["m"].move_option_cooldowns["mortar-barrage"].remaining,
        2
    );

    game.pieces.get_mut("m").unwrap().current_ammo = 0;
    let within_home = generate_piece_legal_move_actions(&game, &"m".into())
        .into_iter()
        .find(|action| action.to == Square::new(2, 1))
        .unwrap();
    game = apply_move_action(game, within_home);
    assert_eq!(game.pieces["m"].current_ammo, 0);
}

#[test]
fn tank_aim_stops_at_first_piece_and_blast_removes_friendlies() {
    let mut game = state();
    add_piece(&mut game, "tank", "white", "tank", Square::new(1, 1));
    add_piece(&mut game, "screen", "white", "rook", Square::new(4, 1));
    add_piece(&mut game, "friend", "white", "bishop", Square::new(3, 2));
    let actions = generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire");
    assert!(actions
        .iter()
        .any(|action| action.to == Some(Square::new(4, 1))));
    assert!(!actions
        .iter()
        .any(|action| action.to == Some(Square::new(5, 1))));
    let shot = actions
        .into_iter()
        .find(|action| action.to == Some(Square::new(3, 1)))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(shot)).unwrap();
    assert!(game.pieces["screen"].captured);
    assert!(game.pieces["friend"].captured);
    assert_eq!(game.pieces["tank"].current_ammo, 2);
    assert_eq!(
        game.pieces["tank"].move_option_cooldowns["tank-fire"].remaining,
        1
    );
}

#[test]
fn tank_cannot_fire_a_fourth_shell() {
    let mut game = state();
    add_piece(&mut game, "tank", "white", "tank", Square::new(3, 3));
    for expected in [2, 1, 0] {
        let shot = generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire")
            .pop()
            .unwrap();
        game = apply_and_advance_turn(game, TurnAction::Ability(shot));
        assert_eq!(game.pieces["tank"].current_ammo, expected);
        game.current_player = "white".into();
        game.pieces
            .get_mut("tank")
            .unwrap()
            .move_option_cooldowns
            .clear();
    }
    assert!(generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire").is_empty());
}

#[test]
fn depleted_ground_piece_replenishes_immediately_when_already_in_its_home_zone() {
    let mut game = state();
    add_piece(&mut game, "tank", "white", "tank", Square::new(1, 1));
    game.pieces.get_mut("tank").unwrap().current_ammo = 1;
    let shot = generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire")
        .into_iter()
        .find(|action| action.to == Some(Square::new(1, 4)))
        .unwrap();

    game = submit_action(game, TurnAction::Ability(shot)).unwrap();

    assert_eq!(game.pieces["tank"].current_ammo, 3);
}

#[test]
fn tank_cooldown_one_waits_through_the_next_owner_turn_only() {
    let mut game = state();
    add_piece(&mut game, "tank", "white", "tank", Square::new(3, 3));
    add_piece(&mut game, "wk", "white", "king", Square::new(0, 0));
    add_piece(&mut game, "bk", "black", "king", Square::new(7, 7));
    let shot = generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire")
        .into_iter()
        .find(|action| action.to == Some(Square::new(3, 5)))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(shot)).unwrap();
    let black_move = generate_piece_legal_move_actions(&game, &"bk".into())
        .into_iter()
        .next()
        .unwrap();
    game = submit_action(game, TurnAction::Move(black_move)).unwrap();
    assert!(generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire").is_empty());
    let white_move = generate_piece_legal_move_actions(&game, &"wk".into())
        .into_iter()
        .next()
        .unwrap();
    game = submit_action(game, TurnAction::Move(white_move)).unwrap();
    let black_move = generate_piece_legal_move_actions(&game, &"bk".into())
        .into_iter()
        .next()
        .unwrap();
    game = submit_action(game, TurnAction::Move(black_move)).unwrap();
    assert!(!generate_piece_legal_ability_actions(&game, &"tank".into(), "tank-fire").is_empty());
}

#[test]
fn bomber_requires_five_ground_squares_and_enters_air_layer_exactly_five_away() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    for square in [Square::new(5, 1), Square::new(1, 5), Square::new(5, 5)] {
        add_piece(
            &mut game,
            &format!("block-{}-{}", square.file, square.rank),
            "white",
            "rook",
            square,
        );
    }
    let takeoffs = generate_piece_legal_ability_actions(&game, &"b".into(), "takeoff");
    assert!(takeoffs
        .iter()
        .all(|action| action.to != Some(Square::new(6, 1))));

    game.board.squares.insert(Square::new(5, 1).to_id(), None);
    game.pieces.get_mut("block-5-1").unwrap().captured = true;
    let takeoff = generate_piece_legal_ability_actions(&game, &"b".into(), "takeoff")
        .into_iter()
        .find(|action| action.to == Some(Square::new(6, 1)))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(takeoff)).unwrap();
    assert!(game.board.get_piece_at(&Square::new(1, 1)).is_none());
    assert_eq!(
        game.board
            .get_piece_at_layer(&Square::new(6, 1), PieceLayer::Air),
        Some(&"b".into())
    );
    assert_eq!(game.pieces["b"].remaining_flight_turns, 5);
    assert_eq!(game.pieces["b"].current_ammo, 3);
}

#[test]
fn airborne_movement_ignores_ground_and_preserves_same_coordinate_occupancy() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    make_airborne(&mut game, "b", Square::new(2, 3), 5);
    for file in 3..=6 {
        add_piece(
            &mut game,
            &format!("g{file}"),
            "black",
            "rook",
            Square::new(file, 3),
        );
    }
    let flight = generate_piece_legal_move_actions(&game, &"b".into())
        .into_iter()
        .find(|action| action.to == Square::new(6, 3))
        .unwrap();
    assert!(flight.captured_piece_id.is_none());
    game = submit_action(game, TurnAction::Move(flight)).unwrap();
    assert_eq!(
        game.board.get_piece_at(&Square::new(6, 3)),
        Some(&"g6".into())
    );
    assert_eq!(
        game.board
            .get_piece_at_layer(&Square::new(6, 3), PieceLayer::Air),
        Some(&"b".into())
    );
    assert!(!game.pieces["g6"].captured);
}

#[test]
fn air_piece_blocks_and_can_be_captured_by_another_air_piece() {
    let mut game = state();
    add_piece(&mut game, "white-b", "white", "bomber", Square::new(1, 1));
    add_piece(&mut game, "black-b", "black", "bomber", Square::new(6, 6));
    make_airborne(&mut game, "white-b", Square::new(2, 3), 5);
    make_airborne(&mut game, "black-b", Square::new(5, 3), 5);
    let moves = generate_piece_legal_move_actions(&game, &"white-b".into());
    let capture = moves
        .iter()
        .find(|action| action.to == Square::new(5, 3))
        .unwrap();
    assert_eq!(
        capture.captured_piece_id.as_ref().map(PieceId::as_str),
        Some("black-b")
    );
    assert!(!moves.iter().any(|action| action.to == Square::new(6, 3)));
}

#[test]
fn opponent_turn_does_not_reduce_flight_duration() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    add_piece(&mut game, "bk", "black", "king", Square::new(7, 7));
    make_airborne(&mut game, "b", Square::new(4, 4), 5);
    game.current_player = "black".into();
    let black_move = generate_piece_legal_move_actions(&game, &"bk".into())
        .into_iter()
        .next()
        .unwrap();
    game = submit_action(game, TurnAction::Move(black_move)).unwrap();
    assert_eq!(game.pieces["b"].remaining_flight_turns, 5);
}

#[test]
fn bombing_hits_ground_cross_including_friendlies_but_not_air_and_stops_at_zero_ammo() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    make_airborne(&mut game, "b", Square::new(4, 4), 5);
    add_piece(&mut game, "below", "black", "rook", Square::new(4, 4));
    add_piece(&mut game, "friend", "white", "bishop", Square::new(5, 4));
    for expected in [2, 1, 0] {
        let bomb = generate_piece_legal_ability_actions(&game, &"b".into(), "bomb")
            .pop()
            .unwrap();
        game = apply_and_advance_turn(game, TurnAction::Ability(bomb));
        game.current_player = "white".into();
        assert_eq!(game.pieces["b"].current_ammo, expected);
    }
    assert!(game.pieces["below"].captured);
    assert!(game.pieces["friend"].captured);
    assert!(!game.pieces["b"].captured);
    assert!(generate_piece_legal_ability_actions(&game, &"b".into(), "bomb").is_empty());
    assert!(!generate_piece_legal_move_actions(&game, &"b".into()).is_empty());
}

#[test]
fn fifth_owner_turn_forces_a_four_square_landing_and_home_replenishment() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    add_piece(&mut game, "wk", "white", "king", Square::new(0, 0));
    make_airborne(&mut game, "b", Square::new(6, 1), 5);
    game.pieces.get_mut("b").unwrap().current_ammo = 0;

    for remaining in (0..5).rev() {
        let king_move = generate_piece_legal_move_actions(&game, &"wk".into())
            .into_iter()
            .next()
            .unwrap();
        game = apply_and_advance_turn(game, TurnAction::Move(king_move));
        assert_eq!(game.pieces["b"].remaining_flight_turns, remaining);
        if remaining > 0 {
            game.current_player = "white".into();
        }
    }
    assert_eq!(game.current_player, "white");
    assert!(generate_piece_legal_move_actions(&game, &"wk".into()).is_empty());
    let landing = generate_piece_legal_ability_actions(&game, &"b".into(), "forced-landing")
        .into_iter()
        .find(|action| action.to == Some(Square::new(2, 1)))
        .unwrap();
    game = submit_action(game, TurnAction::Ability(landing)).unwrap();
    assert_eq!(game.pieces["b"].layer, PieceLayer::Ground);
    assert_eq!(game.pieces["b"].current_ammo, 3);
    assert_eq!(game.current_player, "black");
}

#[test]
fn bomber_crashes_in_enemy_zone_or_without_a_landing_route() {
    for enemy_zone in [true, false] {
        let mut game = state();
        add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
        add_piece(&mut game, "wk", "white", "king", Square::new(0, 0));
        let air_square = if enemy_zone {
            Square::new(4, 6)
        } else {
            Square::new(4, 4)
        };
        make_airborne(&mut game, "b", air_square, 1);
        if !enemy_zone {
            for (index, (dx, dy)) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ]
            .into_iter()
            .enumerate()
            {
                add_piece(
                    &mut game,
                    &format!("wall{index}"),
                    "black",
                    "rook",
                    Square::new(air_square.file + dx, air_square.rank + dy),
                );
            }
        }
        let action = generate_piece_legal_move_actions(&game, &"wk".into())
            .into_iter()
            .next()
            .unwrap();
        game = apply_and_advance_turn(game, TurnAction::Move(action));
        assert!(game.pieces["b"].captured);
        assert!(game
            .board
            .get_piece_at_layer(&air_square, PieceLayer::Air)
            .is_none());
    }
}

#[test]
fn serialized_state_preserves_ammo_layer_flight_and_cooldown() {
    let mut game = state();
    add_piece(&mut game, "b", "white", "bomber", Square::new(1, 1));
    make_airborne(&mut game, "b", Square::new(4, 4), 3);
    let piece = game.pieces.get_mut("b").unwrap();
    piece.current_ammo = 2;
    piece
        .move_option_cooldowns
        .insert("bomb".into(), CooldownState { remaining: 1 });
    let restored: GameState = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    let piece = &restored.pieces["b"];
    assert_eq!(piece.current_ammo, 2);
    assert_eq!(piece.layer, PieceLayer::Air);
    assert_eq!(piece.remaining_flight_turns, 3);
    assert_eq!(piece.move_option_cooldowns["bomb"].remaining, 1);
    assert_eq!(
        restored
            .board
            .get_piece_at_layer(&Square::new(4, 4), PieceLayer::Air),
        Some(&"b".into())
    );
}
