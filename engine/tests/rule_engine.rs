//! Brainfuck Chess rule engine integration tests.

use std::collections::HashMap;
use std::sync::Arc;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::attack_map::generate_attack_map;
use brainfuck_chess_engine::context::PieceCatalog;
use brainfuck_chess_engine::endgame::{apply_and_advance_turn, apply_move_action, has_living_king};
use brainfuck_chess_engine::legal_moves::{
    generate_drop_candidates_by_type, generate_legal_drop_actions, generate_legal_move_actions,
    generate_piece_legal_ability_actions, generate_piece_legal_drop_actions,
    generate_piece_legal_move_actions, generate_piece_legal_move_actions_with_options,
    MoveGenerationOptions,
};
use brainfuck_chess_engine::pieces::default_pieces::*;
use brainfuck_chess_engine::rules::*;
use brainfuck_chess_engine::types::*;

fn make_game_state(board_size: i32) -> GameState {
    let board = create_board(board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);

    let white_deck = Deck {
        player_id: "white".into(),
        starting_pieces: Vec::new(),
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };
    let black_deck = Deck {
        player_id: "black".into(),
        starting_pieces: Vec::new(),
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };

    let mut players = HashMap::new();
    players.insert(
        "white".into(),
        Player {
            id: "white".into(),
            deck: white_deck,
            captured_pieces: Vec::new(),
        },
    );
    players.insert(
        "black".into(),
        Player {
            id: "black".into(),
            deck: black_deck,
            captured_pieces: Vec::new(),
        },
    );

    GameState {
        id: "test".into(),
        board,
        pieces: HashMap::new(),
        piece_definitions: defs,
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
        chessembly_program_cache,
    }
}

fn add_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str, file: i32, rank: i32) {
    let sq = Square::new(file, rank);
    let piece = Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: Some(sq),
        in_pocket: false,
        captured: false,
        has_moved: false,
        current_ammo: state
            .piece_definitions
            .get(type_id)
            .map_or(0, |definition| definition.max_ammo),
        layer: brainfuck_chess_engine::types::PieceLayer::Ground,
        remaining_flight_turns: 0,
        state: state
            .piece_definitions
            .get(type_id)
            .map(PieceDefinition::initial_state)
            .unwrap_or_default(),
        move_option_cooldowns: HashMap::new(),
    };
    state.board.squares.insert(sq.to_id(), Some(id.into()));
    state.pieces.insert(id.into(), piece.clone());
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .starting_pieces
        .push(id.into());
}

fn add_pocket_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str) {
    let piece = Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: None,
        in_pocket: true,
        captured: false,
        has_moved: false,
        current_ammo: state
            .piece_definitions
            .get(type_id)
            .map_or(0, |definition| definition.max_ammo),
        layer: brainfuck_chess_engine::types::PieceLayer::Ground,
        remaining_flight_turns: 0,
        state: state
            .piece_definitions
            .get(type_id)
            .map(PieceDefinition::initial_state)
            .unwrap_or_default(),
        move_option_cooldowns: HashMap::new(),
    };
    state.pieces.insert(id.into(), piece);
    state
        .players
        .get_mut(owner)
        .unwrap()
        .deck
        .pocket_pieces
        .push(id.into());
}

fn add_front_pawn_line(state: &mut GameState, owner: &str, board_size: i32) {
    let rank = get_frontmost_base_rank(&owner.to_string(), board_size).unwrap();
    for file in 0..board_size {
        add_piece(
            state,
            &format!("{owner}-front-{file}"),
            owner,
            if owner == "white" {
                "pawn-white"
            } else {
                "pawn-black"
            },
            file,
            rank,
        );
    }
}

#[test]
fn mortar_uses_orthogonal_take_moves() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "mortar", 3, 3);
    add_piece(&mut state, "enemy", "black", "bishop", 4, 3);
    add_piece(&mut state, "friend", "white", "bishop", 2, 3);

    let moves = generate_piece_legal_move_actions(&state, &"actor".into());
    let destinations = moves.iter().map(|action| action.to).collect::<Vec<_>>();
    assert!(destinations.contains(&Square::new(3, 4)));
    assert!(destinations.contains(&Square::new(3, 2)));
    assert!(moves.iter().any(|action| {
        action.to == Square::new(4, 3)
            && action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("enemy")
    }));
    assert!(!destinations.contains(&Square::new(2, 3)));
    assert!(!destinations.contains(&Square::new(4, 4)));
    assert!(!destinations.contains(&Square::new(3, 5)));
}

#[test]
fn machine_gunner_uses_orthogonal_take_moves() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "machine-gunner", 3, 3);
    add_piece(&mut state, "enemy", "black", "bishop", 4, 3);
    add_piece(&mut state, "friend", "white", "bishop", 2, 3);

    let moves = generate_piece_legal_move_actions(&state, &"actor".into());
    let destinations = moves.iter().map(|action| action.to).collect::<Vec<_>>();
    assert!(destinations.contains(&Square::new(3, 4)));
    assert!(destinations.contains(&Square::new(3, 2)));
    assert!(moves.iter().any(|action| {
        action.to == Square::new(4, 3)
            && action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("enemy")
    }));
    assert!(!destinations.contains(&Square::new(2, 3)));
    assert!(!destinations.contains(&Square::new(4, 4)));
    assert!(!destinations.contains(&Square::new(3, 5)));
}

#[test]
fn mortar_barrage_targets_only_its_own_file_and_removes_orthogonal_neighbors() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "mortar", 3, 3);
    add_piece(&mut state, "center", "black", "bishop", 3, 5);
    add_piece(&mut state, "left", "white", "knight", 2, 5);
    add_piece(&mut state, "right", "black", "rook", 4, 5);
    add_piece(&mut state, "above", "black", "bishop", 3, 6);
    add_piece(&mut state, "below", "white", "bishop", 3, 4);
    add_piece(&mut state, "diagonal", "black", "bishop", 4, 6);
    add_piece(&mut state, "farther", "black", "bishop", 3, 7);
    add_piece(&mut state, "far-friend", "white", "knight", 6, 3);

    let actions = generate_piece_legal_ability_actions(&state, &"actor".into(), "mortar-barrage");
    assert_eq!(actions.len(), 6);
    assert!(actions
        .iter()
        .all(|action| action.to.is_some_and(|target| target.file == 3)));
    let action = actions
        .into_iter()
        .find(|action| action.to == Some(Square::new(3, 5)))
        .unwrap();
    let state = submit_action(state, TurnAction::Ability(action)).unwrap();

    for id in ["center", "left", "right", "above", "below"] {
        assert!(state.pieces[id].captured, "{id} should be removed");
        assert_eq!(state.pieces[id].current_square, None);
    }
    for id in ["diagonal", "farther"] {
        assert!(!state.pieces[id].captured, "{id} should remain");
    }
    assert_eq!(state.players["white"].captured_pieces.len(), 5);
    assert_eq!(state.history.len(), 1);
    assert_eq!(state.current_player.as_str(), "black");
}

#[test]
fn mortar_cannot_target_the_opponents_base_zone() {
    for (board_size, owner, allowed_ranks, blocked_ranks) in [
        (8, "white", 0..=5, vec![6, 7]),
        (8, "black", 2..=7, vec![0, 1]),
        (9, "white", 0..=6, vec![7, 8]),
        (9, "black", 2..=8, vec![0, 1]),
        (10, "white", 0..=6, vec![7, 8, 9]),
        (10, "black", 3..=9, vec![0, 1, 2]),
    ] {
        let mut state = make_game_state(board_size);
        state.current_player = owner.into();
        add_piece(&mut state, "actor", owner, "mortar", 3, 3);

        let actions =
            generate_piece_legal_ability_actions(&state, &"actor".into(), "mortar-barrage");
        assert_eq!(
            actions.len(),
            (board_size - blocked_ranks.len() as i32) as usize
        );
        assert!(actions.iter().all(|action| {
            action
                .to
                .is_some_and(|target| allowed_ranks.contains(&target.rank))
        }));
        assert!(blocked_ranks.iter().all(|blocked_rank| {
            actions
                .iter()
                .all(|action| action.to.is_none_or(|target| target.rank != *blocked_rank))
        }));
    }
}

#[test]
fn black_machine_gun_barrage_removes_only_the_immediate_two_by_three_area() {
    let mut state = make_game_state(8);
    state.current_player = "black".into();
    add_piece(&mut state, "actor", "black", "machine-gunner", 3, 4);
    add_piece(&mut state, "near-left", "white", "bishop", 2, 3);
    add_piece(&mut state, "near-center", "black", "knight", 3, 3);
    add_piece(&mut state, "near-right", "white", "rook", 4, 3);
    add_piece(&mut state, "far-left", "white", "bishop", 2, 2);
    add_piece(&mut state, "far-center", "white", "bishop", 3, 2);
    add_piece(&mut state, "far-right", "black", "bishop", 4, 2);
    add_piece(&mut state, "third-rank", "white", "rook", 3, 1);
    add_piece(&mut state, "outside-width", "white", "bishop", 1, 3);
    add_piece(&mut state, "behind", "white", "rook", 3, 5);

    let action =
        generate_piece_legal_ability_actions(&state, &"actor".into(), "machine-gun-barrage")
            .pop()
            .unwrap();
    assert_eq!(action.to, Some(Square::new(3, 4)));
    let state = submit_action(state, TurnAction::Ability(action)).unwrap();

    for id in [
        "near-left",
        "near-center",
        "near-right",
        "far-left",
        "far-center",
        "far-right",
    ] {
        assert!(state.pieces[id].captured, "{id} should be removed");
    }
    for id in ["third-rank", "outside-width", "behind"] {
        assert!(!state.pieces[id].captured, "{id} should remain");
    }
}

#[test]
fn barrage_abilities_have_two_owner_turn_cooldowns() {
    for (piece_type, ability_id) in [
        ("mortar", "mortar-barrage"),
        ("machine-gunner", "machine-gun-barrage"),
    ] {
        let mut state = make_game_state(8);
        add_piece(&mut state, "wk", "white", "king", 0, 0);
        add_piece(&mut state, "bk", "black", "king", 7, 7);
        add_piece(&mut state, "actor", "white", piece_type, 3, 3);

        let ability = generate_piece_legal_ability_actions(&state, &"actor".into(), ability_id)
            .pop()
            .unwrap();
        state = submit_action(state, TurnAction::Ability(ability)).unwrap();
        assert_eq!(
            state.pieces["actor"].move_option_cooldowns[ability_id].remaining,
            2
        );

        let black_move = generate_piece_legal_move_actions(&state, &"bk".into())
            .into_iter()
            .next()
            .unwrap();
        state = submit_action(state, TurnAction::Move(black_move)).unwrap();
        assert!(
            generate_piece_legal_ability_actions(&state, &"actor".into(), ability_id).is_empty()
        );

        for expected_remaining in [1, 0] {
            let white_move = generate_piece_legal_move_actions(&state, &"wk".into())
                .into_iter()
                .next()
                .unwrap();
            state = submit_action(state, TurnAction::Move(white_move)).unwrap();
            assert_eq!(
                state.pieces["actor"]
                    .move_option_cooldowns
                    .get(ability_id)
                    .map_or(0, |cooldown| cooldown.remaining),
                expected_remaining
            );

            let black_move = generate_piece_legal_move_actions(&state, &"bk".into())
                .into_iter()
                .next()
                .unwrap();
            state = submit_action(state, TurnAction::Move(black_move)).unwrap();
        }

        // This test isolates cooldown timing; mortar now also needs a shell.
        if piece_type == "mortar" {
            state.pieces.get_mut("actor").unwrap().current_ammo = 1;
        }

        assert!(
            !generate_piece_legal_ability_actions(&state, &"actor".into(), ability_id).is_empty()
        );
    }
}

#[test]
fn mortar_barrage_king_removal_uses_normal_game_end_state() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "mortar", 3, 3);
    add_piece(&mut state, "enemy-king", "black", "king", 3, 6);
    let action = generate_piece_legal_ability_actions(&state, &"actor".into(), "mortar-barrage")
        .into_iter()
        .find(|action| action.to == Some(Square::new(3, 5)))
        .unwrap();

    let state = submit_action(state, TurnAction::Ability(action)).unwrap();
    assert_eq!(state.phase, GamePhase::Ended);
    assert_eq!(state.result.unwrap().winner, Some("white".into()));
}

#[test]
fn alternating_soldier_replaces_adjacent_friendly_piece_from_pocket() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "alternating-soldier", 3, 3);
    add_piece(&mut state, "target", "white", "bishop", 4, 4);
    add_piece(&mut state, "enemy", "black", "bishop", 2, 2);
    add_pocket_piece(&mut state, "reserve", "white", "knight");
    let actions = generate_piece_legal_ability_actions(&state, &"actor".into(), "relieve");
    assert_eq!(
        actions.len(),
        1,
        "enemy adjacent pieces are not valid relief targets"
    );
    let state = submit_action(state, TurnAction::Ability(actions[0].clone())).unwrap();
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(4, 4))
            .map(PieceId::as_str),
        Some("reserve")
    );
    assert!(state.pieces["target"].in_pocket);
    assert!(!state.pieces["reserve"].in_pocket);
}

#[test]
fn airborne_summons_only_low_score_pocket_piece_into_forward_rectangle() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "airborne", 3, 3);
    add_pocket_piece(&mut state, "low", "white", "bishop");
    add_pocket_piece(&mut state, "high", "white", "rook");
    let actions = generate_piece_legal_ability_actions(&state, &"actor".into(), "airdrop");
    assert_eq!(actions.len(), 6);
    assert!(actions.iter().all(|action| action
        .pocket_piece_id
        .as_ref()
        .is_some_and(|id| id == "low")
        && action
            .to
            .is_some_and(|sq| (4..=5).contains(&sq.rank) && (2..=4).contains(&sq.file))));
}

#[test]
fn airborne_commits_multiple_unique_deployments_in_one_turn() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "airborne", 3, 3);
    add_pocket_piece(&mut state, "first", "white", "bishop");
    add_pocket_piece(&mut state, "second", "white", "knight");
    let action = AbilityAction {
        player_id: "white".into(),
        piece_id: "actor".into(),
        ability_id: "airdrop".into(),
        target_piece_id: None,
        pocket_piece_id: None,
        to: None,
        deployments: vec![
            AbilityDeployment {
                pocket_piece_id: "first".into(),
                to: Square::new(2, 4),
            },
            AbilityDeployment {
                pocket_piece_id: "second".into(),
                to: Square::new(3, 5),
            },
        ],
    };
    let state = submit_action(state, TurnAction::Ability(action)).unwrap();
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(2, 4))
            .map(PieceId::as_str),
        Some("first")
    );
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(3, 5))
            .map(PieceId::as_str),
        Some("second")
    );
    assert_eq!(state.current_player, "black");
}

#[test]
fn green_camp_returns_enemy_to_its_owners_pocket_but_excludes_king() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "actor", "white", "green-camp", 3, 3);
    add_piece(&mut state, "enemy", "black", "rook", 4, 3);
    add_piece(&mut state, "king", "black", "king", 2, 3);
    let actions = generate_piece_legal_ability_actions(&state, &"actor".into(), "recall");
    assert_eq!(actions.len(), 1);
    let state = submit_action(state, TurnAction::Ability(actions[0].clone())).unwrap();
    assert!(state.pieces["enemy"].in_pocket);
    assert!(state.players["black"]
        .deck
        .pocket_pieces
        .iter()
        .any(|id| id == "enemy"));
    assert!(state.board.get_piece_at(&Square::new(4, 3)).is_none());
}

#[test]
fn paratrooper_drops_on_empty_square_and_cannot_move_afterward() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    let action = generate_piece_legal_drop_actions(&state, &"para".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 0))
        .unwrap();
    assert_eq!(action.captured_piece_id, None);

    let state = submit_action(state, TurnAction::Drop(action)).unwrap();
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(3, 0))
            .map(PieceId::as_str),
        Some("para")
    );
    assert!(state.players["white"].deck.pocket_pieces.is_empty());
    assert_eq!(state.current_player, "black");
    assert_eq!(state.turn_number, 2);
    assert_eq!(state.history.len(), 1);
    assert!(generate_piece_legal_move_actions(&state, &"para".into()).is_empty());
}

#[test]
fn paratrooper_captures_enemy_on_drop_and_records_capture() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    add_piece(&mut state, "enemy", "black", "knight", 3, 0);
    let action = generate_piece_legal_drop_actions(&state, &"para".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 0))
        .unwrap();
    assert_eq!(
        action.captured_piece_id.as_ref().map(PieceId::as_str),
        Some("enemy")
    );

    let state = submit_action(state, TurnAction::Drop(action)).unwrap();
    assert!(state.pieces["enemy"].captured);
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(3, 0))
            .map(PieceId::as_str),
        Some("para")
    );
    assert_eq!(state.players["white"].captured_pieces, vec!["enemy"]);
    let TurnAction::Drop(recorded) = &state.history[0].action else {
        panic!()
    };
    assert_eq!(
        recorded.captured_piece_id.as_ref().map(PieceId::as_str),
        Some("enemy")
    );
}

#[test]
fn illegal_paratrooper_drop_is_atomic_and_regular_piece_cannot_capture_on_drop() {
    let mut state = make_game_state(8);
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    add_pocket_piece(&mut state, "knight", "white", "knight");
    add_piece(&mut state, "friend", "white", "bishop", 2, 0);
    add_piece(&mut state, "enemy", "black", "bishop", 3, 0);
    let before = serde_json::to_value(&state).unwrap();
    let illegal = DropAction {
        player_id: "white".into(),
        piece_id: "para".into(),
        to: Square::new(2, 0),
        captured_piece_id: Some("friend".into()),
    };
    assert!(submit_action(state.clone(), TurnAction::Drop(illegal)).is_err());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert!(!generate_piece_legal_drop_actions(&state, &"knight".into())
        .iter()
        .any(|action| action.to == Square::new(3, 0)));
    assert!(generate_piece_legal_drop_actions(&state, &"para".into())
        .iter()
        .any(|action| action.to == Square::new(3, 0)));
}

#[test]
fn test_create_board_8x8() {
    let board = create_board(8);
    assert_eq!(board.size, 8);
    assert_eq!(board.squares.len(), 64);
    assert!(board.terrain.is_empty());
    for v in board.squares.values() {
        assert!(v.is_none());
    }
}

#[test]
fn test_create_board_10x10() {
    let board = create_board(10);
    assert_eq!(board.size, 10);
    assert_eq!(board.squares.len(), 100);
    assert!(board.terrain.is_empty());
}

#[test]
fn test_create_board_12x12_has_four_central_high_ground_squares() {
    let plain = create_board(12);
    assert!(plain.terrain.is_empty());

    let board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    assert_eq!(board.size, 12);
    assert_eq!(board.squares.len(), 144);
    assert_eq!(board.terrain.len(), 4);
    for square in [
        Square::new(5, 5),
        Square::new(6, 5),
        Square::new(5, 6),
        Square::new(6, 6),
    ] {
        assert_eq!(
            board
                .terrain
                .get(&square.to_id())
                .map(|cell| cell.type_id.as_str()),
            Some(HIGH_GROUND_TERRAIN_ID)
        );
    }
}

#[test]
fn central_high_ground_variant_rejects_non_12x12_boards() {
    let error = create_board_with_variant(10, BoardVariant::CentralHighGround).unwrap_err();
    assert!(error.contains("12x12"));
}

#[test]
fn high_ground_blocks_uphill_capture_but_not_entry_or_downhill_capture() {
    let mut uphill = make_game_state(12);
    uphill.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_piece(&mut uphill, "rook", "white", "rook", 5, 4);
    add_piece(&mut uphill, "target", "black", "knight", 5, 5);
    assert!(!generate_piece_legal_move_actions(&uphill, &"rook".into())
        .iter()
        .any(|action| action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("target")));
    assert!(
        !generate_attack_map(&uphill, &"white".into(), &HashMap::new())
            .attacked_squares
            .contains(&Square::new(5, 5).to_id())
    );

    let mut entry = make_game_state(12);
    entry.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_piece(&mut entry, "rook", "white", "rook", 5, 4);
    assert!(generate_piece_legal_move_actions(&entry, &"rook".into())
        .iter()
        .any(|action| action.to == Square::new(5, 5) && action.captured_piece_id.is_none()));

    let mut downhill = make_game_state(12);
    downhill.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_piece(&mut downhill, "rook", "white", "rook", 5, 5);
    add_piece(&mut downhill, "target", "black", "knight", 5, 4);
    assert!(generate_piece_legal_move_actions(&downhill, &"rook".into())
        .iter()
        .any(|action| action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("target")));

    let mut level = make_game_state(12);
    level.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_piece(&mut level, "rook", "white", "rook", 5, 5);
    add_piece(&mut level, "target", "black", "knight", 6, 5);
    assert!(generate_piece_legal_move_actions(&level, &"rook".into())
        .iter()
        .any(|action| action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("target")));
}

#[test]
fn pocket_capture_cannot_take_a_piece_on_high_ground() {
    let mut state = make_game_state(12);
    state.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_pocket_piece(&mut state, "para", "white", "paratrooper");
    add_piece(&mut state, "spotter", "white", "rook", 5, 6);
    add_piece(&mut state, "target", "black", "knight", 5, 5);

    assert!(!generate_piece_legal_drop_actions(&state, &"para".into())
        .iter()
        .any(|action| action.to == Square::new(5, 5)));
}

#[test]
fn low_ground_barrage_does_not_remove_a_piece_on_high_ground() {
    let mut state = make_game_state(12);
    state.board = create_board_with_variant(12, BoardVariant::CentralHighGround).unwrap();
    add_piece(&mut state, "mortar", "white", "mortar", 5, 3);
    add_piece(&mut state, "target", "black", "knight", 5, 5);
    let action = generate_piece_legal_ability_actions(&state, &"mortar".into(), "mortar-barrage")
        .into_iter()
        .find(|action| action.to == Some(Square::new(5, 5)))
        .unwrap();

    let state = submit_action(state, TurnAction::Ability(action)).unwrap();
    assert!(!state.pieces["target"].captured);
    assert_eq!(
        state
            .board
            .get_piece_at(&Square::new(5, 5))
            .map(PieceId::as_str),
        Some("target")
    );
}

#[test]
fn test_windmill_piece_state_toggle_move_modes() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wm", "white", "windmill", 3, 3);

    let bishop_mode_moves = generate_piece_legal_move_actions(&state, &"wm".into());
    assert!(bishop_mode_moves
        .iter()
        .any(|action| action.to == Square::new(4, 4)));
    assert!(bishop_mode_moves
        .iter()
        .all(|action| (action.to.file - 3).abs() == (action.to.rank - 3).abs()));
    let bishop_move = bishop_mode_moves
        .into_iter()
        .find(|action| action.to == Square::new(4, 4))
        .unwrap();
    assert_eq!(bishop_move.source_layer_ids, vec!["bishop_mode"]);
    assert_eq!(
        bishop_move.effects.piece_state_updates[0].value,
        PieceStateValue::Text("rook".into())
    );

    state = apply_move_action(state, bishop_move);
    assert_eq!(
        state.pieces["wm"].state.get("mode"),
        Some(&PieceStateValue::Text("rook".into()))
    );
    assert!(!state.global_state.contains_key("mode"));

    let rook_mode_moves = generate_piece_legal_move_actions(&state, &"wm".into());
    assert!(rook_mode_moves
        .iter()
        .any(|action| action.to == Square::new(4, 5)));
    assert!(rook_mode_moves
        .iter()
        .all(|action| action.to.file == 4 || action.to.rank == 4));
    let rook_move = rook_mode_moves
        .into_iter()
        .find(|action| action.to == Square::new(4, 5))
        .unwrap();
    assert_eq!(rook_move.source_layer_ids, vec!["rook_mode"]);

    state = apply_move_action(state, rook_move);
    assert_eq!(
        state.pieces["wm"].state.get("mode"),
        Some(&PieceStateValue::Text("bishop".into()))
    );
}

#[test]
fn windmills_own_independent_state_and_legal_queries_are_pure() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "first", "white", "windmill", 2, 2);
    add_piece(&mut state, "second", "white", "windmill", 6, 2);
    let before = state.pieces["first"].state.clone();

    let action = generate_piece_legal_move_actions(&state, &"first".into())
        .into_iter()
        .find(|action| action.to == Square::new(3, 3))
        .unwrap();
    assert_eq!(state.pieces["first"].state, before);
    assert_eq!(
        state.pieces["second"].state["mode"],
        PieceStateValue::Text("bishop".into())
    );

    state = apply_move_action(state, action);
    assert_eq!(
        state.pieces["first"].state["mode"],
        PieceStateValue::Text("rook".into())
    );
    assert_eq!(
        state.pieces["second"].state["mode"],
        PieceStateValue::Text("bishop".into())
    );
}

#[test]
fn layers_with_same_destination_and_different_effects_stay_distinct() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "layered".into(),
        name: "Layered".into(),
        score: 1,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: String::new(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: vec![
            PieceStateDefinition {
                key: "a".into(),
                default_value: PieceStateValue::Integer(0),
            },
            PieceStateDefinition {
                key: "b".into(),
                default_value: PieceStateValue::Integer(0),
            },
        ],
        move_layers: vec![
            MoveLayerDefinition {
                id: "a".into(),
                chessembly_code: "move(1, 0);".into(),
                enabled_when: Vec::new(),
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "a".into(),
                    value: PieceStateValue::Integer(1),
                }],
            },
            MoveLayerDefinition {
                id: "b".into(),
                chessembly_code: "move(1, 0);".into(),
                enabled_when: Vec::new(),
                on_commit: vec![PieceStateUpdateDefinition {
                    key: "b".into(),
                    value: PieceStateValue::Integer(1),
                }],
            },
        ],
        move_options: vec![MoveOptionDefinition {
            id: "normal".into(),
            name: "Normal".into(),
            description: String::new(),
            kind: MoveOptionKind::Normal,
            layer_ids: vec!["a".into(), "b".into()],
            execution_mode: MoveOptionExecutionMode::MoveModifier,
            contributes_to_attack_map: true,
            ammo_cost: 0,
            enabled_when: Vec::new(),
            cooldown: None,
        }],
        visual: PieceVisualDefinition {
            default_asset_key: "layered".into(),
            variants: Vec::new(),
        },
    }
    .normalize_and_validate()
    .unwrap();
    state.piece_definitions.insert("layered".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "layered", "white", "layered", 3, 3);

    let actions: Vec<_> = generate_piece_legal_move_actions(&state, &"layered".into())
        .into_iter()
        .filter(|action| action.to == Square::new(4, 3))
        .collect();
    assert_eq!(actions.len(), 2);
    assert_ne!(actions[0].effects, actions[1].effects);
}

#[test]
fn chessembly_set_state_remains_global_and_separate_from_piece_state() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "global-writer".into(),
        name: "Global writer".into(),
        score: 1,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "set-state(flag, 7) move(1, 0);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: None,
        promotion_pool: Vec::new(),
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition::default(),
    }
    .normalize_and_validate()
    .unwrap();
    state
        .piece_definitions
        .insert("global-writer".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "writer", "white", "global-writer", 3, 3);
    let action = generate_piece_legal_move_actions(&state, &"writer".into())
        .into_iter()
        .find(|action| action.to == Square::new(4, 3))
        .unwrap();
    assert_eq!(action.effects.global_state_updates[0].key, "flag");
    assert!(action.effects.piece_state_updates.is_empty());
    let state = apply_move_action(state, action);
    assert_eq!(state.global_state.get("flag"), Some(&7));
    assert!(state.pieces["writer"].state.is_empty());
}

#[test]
fn test_score_limit_8x8() {
    assert_eq!(calculate_score_limit(8), 39);
}

#[test]
fn test_score_limit_9x9() {
    assert_eq!(calculate_score_limit(9), 56);
}

#[test]
fn test_score_limit_10x10() {
    assert_eq!(calculate_score_limit(10), 75);
}

#[test]
fn test_white_base_zone_8x8() {
    let zones = get_base_zone_squares(&"white".to_string(), 8);
    assert_eq!(zones.len(), 16);
    assert!(zones.contains(&Square::new(0, 0)));
    assert!(zones.contains(&Square::new(7, 1)));
    assert!(!zones.contains(&Square::new(0, 2)));
}

#[test]
fn test_black_base_zone_8x8() {
    let zones = get_base_zone_squares(&"black".to_string(), 8);
    assert_eq!(zones.len(), 16);
    assert!(zones.contains(&Square::new(0, 6)));
    assert!(zones.contains(&Square::new(7, 7)));
    assert!(!zones.contains(&Square::new(0, 5)));
}

#[test]
fn test_base_zone_stays_two_ranks_on_9x9() {
    let white_zones = get_base_zone_squares(&"white".to_string(), 9);
    let black_zones = get_base_zone_squares(&"black".to_string(), 9);

    assert_eq!(white_zones.len(), 18);
    assert!(!white_zones.contains(&Square::new(0, 2)));
    assert_eq!(black_zones.len(), 18);
    assert!(!black_zones.contains(&Square::new(0, 6)));
}

#[test]
fn test_base_zone_uses_three_ranks_on_10x10() {
    let white_zones = get_base_zone_squares(&"white".to_string(), 10);
    let black_zones = get_base_zone_squares(&"black".to_string(), 10);

    assert_eq!(white_zones.len(), 30);
    assert!(white_zones.contains(&Square::new(9, 2)));
    assert!(!white_zones.contains(&Square::new(0, 3)));
    assert_eq!(black_zones.len(), 30);
    assert!(black_zones.contains(&Square::new(0, 7)));
    assert!(!black_zones.contains(&Square::new(0, 6)));
}

#[test]
fn test_deck_validation_no_king() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "p1", "white", "pawn-white", 0, 0);
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("King")));
}

#[test]
fn test_deck_validation_king_in_starting() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_front_pawn_line(&mut state, "white", 8);
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(result.valid, "errors: {:?}", result.errors);
}

#[test]
fn test_deck_validation_requires_every_front_rank_square() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    for file in 0..7 {
        add_piece(
            &mut state,
            &format!("p{file}"),
            "white",
            "pawn-white",
            file,
            1,
        );
    }

    let result = validate_deck(
        &state.players["white"].deck,
        8,
        &state.pieces,
        &state.piece_definitions,
    );
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|error| error.contains("앞줄") && error.contains("7/8")));

    add_piece(&mut state, "p7", "white", "pawn-white", 7, 1);
    assert!(
        validate_deck(
            &state.players["white"].deck,
            8,
            &state.pieces,
            &state.piece_definitions,
        )
        .valid
    );
}

#[test]
fn test_deck_validation_king_in_pocket_forbidden() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_pocket_piece(&mut state, "k2", "white", "king");
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("포켓")));
}

#[test]
fn test_deck_score_over_limit() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    for i in 0..5 {
        add_piece(&mut state, &format!("q{}", i), "white", "queen", i, 0);
    }
    let player = state.players.get("white").unwrap();
    let result = validate_deck(&player.deck, 8, &state.pieces, &state.piece_definitions);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("점수")));
}

#[test]
fn deployment_zone_classifies_every_builtin_and_both_player_orientations() {
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    let front_types = [
        "pawn-white",
        "pawn-black",
        "tempest-pawn-white",
        "tempest-pawn-black",
        "bouncing-pawn-white",
        "bouncing-pawn-black",
        "dozer-white",
        "dozer-black",
        "surface-to-air-missile-white",
        "surface-to-air-missile-black",
    ];

    for definition in definitions.values() {
        let expected = if front_types.contains(&definition.id.as_str()) {
            DeploymentZone::Front
        } else {
            DeploymentZone::Back
        };
        assert_eq!(definition.deployment_zone, expected, "{}", definition.id);
    }

    for (owner, front_rank, back_rank) in [("white", 1, 0), ("black", 6, 7)] {
        for type_id in front_types {
            let definition = &definitions[type_id];
            assert!(can_piece_be_placed_at_start(
                definition,
                &owner.into(),
                Square::new(0, front_rank),
                8,
            ));
            assert!(!can_piece_be_placed_at_start(
                definition,
                &owner.into(),
                Square::new(0, back_rank),
                8,
            ));
        }
        for type_id in ["knight", "bishop", "rook", "queen", "king", "paratrooper"] {
            let definition = &definitions[type_id];
            assert!(!can_piece_be_placed_at_start(
                definition,
                &owner.into(),
                Square::new(0, front_rank),
                8,
            ));
            assert!(can_piece_be_placed_at_start(
                definition,
                &owner.into(),
                Square::new(0, back_rank),
                8,
            ));
        }
    }
}

#[test]
fn deployment_zone_is_independent_of_score_and_uses_frontmost_large_board_rank() {
    let mut front_state = make_game_state(10);
    front_state
        .piece_definitions
        .get_mut("dozer-white")
        .unwrap()
        .score = 3;
    add_piece(&mut front_state, "king", "white", "king", 4, 0);
    add_front_pawn_line(&mut front_state, "white", 10);
    let displaced_pawn = front_state.board.squares[&Square::new(3, 2).to_id()]
        .clone()
        .unwrap();
    front_state
        .players
        .get_mut("white")
        .unwrap()
        .deck
        .starting_pieces
        .retain(|id| id != &displaced_pawn);
    front_state.pieces.remove(&displaced_pawn);
    add_piece(&mut front_state, "dozer", "white", "dozer-white", 3, 2);
    assert!(
        validate_deck(
            &front_state.players["white"].deck,
            10,
            &front_state.pieces,
            &front_state.piece_definitions,
        )
        .valid
    );

    let mut back_state = make_game_state(10);
    back_state
        .piece_definitions
        .get_mut("knight")
        .unwrap()
        .score = 1;
    add_piece(&mut back_state, "king", "white", "king", 4, 0);
    add_piece(&mut back_state, "knight", "white", "knight", 3, 2);
    let result = validate_deck(
        &back_state.players["white"].deck,
        10,
        &back_state.pieces,
        &back_state.piece_definitions,
    );
    assert!(!result.valid);
    assert!(result.errors.iter().any(|error| error.contains("뒷줄")));
    assert_eq!(get_frontmost_base_rank(&"white".into(), 10), Some(2));
    assert_eq!(get_frontmost_base_rank(&"black".into(), 10), Some(7));
}

#[test]
fn legacy_piece_definition_json_defaults_to_back_deployment() {
    let definition = all_default_definitions()
        .into_iter()
        .find(|definition| definition.id == "knight")
        .unwrap();
    let mut json = serde_json::to_value(definition).unwrap();
    json.as_object_mut().unwrap().remove("deployment_zone");

    let restored: PieceDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(restored.deployment_zone, DeploymentZone::Back);
}

#[test]
fn test_king_capture_ends_game() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "k2", "black", "king", 4, 1);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: Some("k2".into()),
        promotion: None,
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };

    let result_state = apply_move_action(state, action);
    assert_eq!(result_state.phase, GamePhase::Ended);
    assert_eq!(
        result_state.result.as_ref().unwrap().winner,
        Some("white".to_string())
    );
    assert_eq!(
        result_state.result.as_ref().unwrap().reason,
        GameEndReason::KingCapture
    );
}

#[test]
fn test_normal_capture_does_not_end_game() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    add_piece(&mut state, "p1", "black", "pawn-black", 4, 1);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "k1".into(),
        from: Square::new(4, 0),
        to: Square::new(4, 1),
        captured_piece_id: Some("p1".into()),
        promotion: None,
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };

    let result_state = apply_move_action(state, action);
    assert_eq!(result_state.phase, GamePhase::Playing);
    assert!(result_state.result.is_none());
}

#[test]
fn test_has_living_king() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "k1", "white", "king", 4, 0);
    assert!(has_living_king(&state, &"white".to_string()));
    assert!(!has_living_king(&state, &"black".to_string()));
}

#[test]
fn test_castling_kingside_generated_and_applied() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 7, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let legal = generate_legal_move_actions(&state);
    assert!(legal
        .iter()
        .any(|m| m.piece_id == "wk" && m.to == Square::new(6, 0)));
    assert!(generate_piece_legal_move_actions(&state, &"wk".into())
        .iter()
        .any(|m| m.to == Square::new(6, 0)));

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "wk".into(),
        from: Square::new(4, 0),
        to: Square::new(6, 0),
        captured_piece_id: None,
        promotion: None,
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, action);
    assert_eq!(
        new_state.pieces["wk"].current_square,
        Some(Square::new(6, 0))
    );
    assert_eq!(
        new_state.pieces["wr"].current_square,
        Some(Square::new(5, 0))
    );
}

#[test]
fn test_en_passant_generated_and_applied() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 4);
    add_piece(&mut state, "bp", "black", "pawn-black", 5, 6);
    state.current_player = "black".into();

    let black_double = MoveAction {
        player_id: "black".into(),
        piece_id: "bp".into(),
        from: Square::new(5, 6),
        to: Square::new(5, 4),
        captured_piece_id: None,
        promotion: None,
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let state = apply_and_advance_turn(state, TurnAction::Move(black_double));
    assert!(generate_legal_move_actions(&state)
        .iter()
        .any(|m| m.piece_id == "wp" && m.to == Square::new(5, 5)));

    let white_ep = MoveAction {
        player_id: "white".into(),
        piece_id: "wp".into(),
        from: Square::new(4, 4),
        to: Square::new(5, 5),
        captured_piece_id: Some("bp".into()),
        promotion: None,
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, white_ep);
    assert_eq!(
        new_state.pieces["wp"].current_square,
        Some(Square::new(5, 5))
    );
    assert!(new_state.pieces["bp"].captured);
    assert_eq!(new_state.board.get_piece_at(&Square::new(5, 4)), None);
    assert_eq!(new_state.en_passant_target, None);
}

#[test]
fn test_en_passant_expires_when_opponent_chooses_another_action() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "bp", "black", "pawn-black", 5, 6);
    state.current_player = "black".into();

    state = apply_and_advance_turn(
        state,
        TurnAction::Move(MoveAction {
            player_id: "black".into(),
            piece_id: "bp".into(),
            from: Square::new(5, 6),
            to: Square::new(5, 4),
            captured_piece_id: None,
            promotion: None,
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        }),
    );
    assert_eq!(state.en_passant_target, Some(Square::new(5, 5)));
    assert_eq!(state.en_passant_available_to.as_deref(), Some("white"));

    state = apply_move_action(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "wk".into(),
            from: Square::new(4, 0),
            to: Square::new(4, 1),
            captured_piece_id: None,
            promotion: None,
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        },
    );
    assert_eq!(state.en_passant_target, None);
    assert_eq!(state.en_passant_available_to, None);
}

#[test]
fn test_tempest_pawn_uses_its_distinct_movement_definition() {
    let mut tempest_state = make_game_state(8);
    add_piece(&mut tempest_state, "wk", "white", "king", 0, 0);
    add_piece(&mut tempest_state, "bk", "black", "king", 7, 7);
    add_piece(
        &mut tempest_state,
        "tp",
        "white",
        "tempest-pawn-white",
        3,
        1,
    );

    let mut tempest_moves: Vec<(i32, i32, Option<String>)> =
        generate_piece_legal_move_actions(&tempest_state, &"tp".into())
            .into_iter()
            .map(|action| (action.to.file, action.to.rank, action.promotion))
            .collect();
    tempest_moves.sort();
    assert_eq!(
        tempest_moves,
        vec![(2, 1, None), (3, 2, None), (4, 1, None)]
    );
}

#[test]
fn test_tempest_pawn_uses_its_distinct_capture_definition() {
    let mut tempest_state = make_game_state(8);
    add_piece(&mut tempest_state, "wk", "white", "king", 0, 0);
    add_piece(&mut tempest_state, "bk", "black", "king", 7, 7);
    add_piece(
        &mut tempest_state,
        "tp",
        "white",
        "tempest-pawn-white",
        3,
        3,
    );
    add_piece(&mut tempest_state, "be1", "black", "knight", 2, 4);
    add_piece(&mut tempest_state, "be2", "black", "bishop", 4, 4);
    add_piece(&mut tempest_state, "be3", "black", "rook", 3, 5);

    let mut tempest_captures: Vec<(i32, i32)> =
        generate_piece_legal_move_actions(&tempest_state, &"tp".into())
            .into_iter()
            .filter(|action| action.captured_piece_id.is_some())
            .map(|action| (action.to.file, action.to.rank))
            .collect();
    tempest_captures.sort();
    assert_eq!(tempest_captures, vec![(2, 4), (3, 5), (4, 4)]);
}

#[test]
fn test_pawn_reaching_back_rank_generates_promotion_choices() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let promotions: Vec<&MoveAction> = moves.iter().filter(|m| m.to == Square::new(4, 7)).collect();
    assert_eq!(promotions.len(), 4);
    let mut choices: Vec<String> = promotions
        .iter()
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(choices, vec!["bishop", "knight", "queen", "rook"]);
}

#[test]
fn test_tempest_pawn_reaching_back_rank_generates_tempest_promotion_choices() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"tp".into());
    let promotions: Vec<&MoveAction> = moves.iter().filter(|m| m.to == Square::new(4, 7)).collect();
    assert_eq!(promotions.len(), 4);
    let mut choices: Vec<String> = promotions
        .iter()
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(
        choices,
        vec![
            "tempest-bishop",
            "tempest-knight",
            "tempest-queen",
            "tempest-rook"
        ]
    );
}

#[test]
fn bouncing_pawns_promote_on_their_respective_back_ranks() {
    for (piece_id, owner, type_id, rank, target_rank) in [
        ("bpw", "white", "bouncing-pawn-white", 6, 7),
        ("bpb", "black", "bouncing-pawn-black", 1, 0),
    ] {
        let mut state = make_game_state(8);
        state.current_player = owner.into();
        add_piece(&mut state, "wk", "white", "king", 0, 0);
        add_piece(&mut state, "bk", "black", "king", 7, 7);
        add_piece(&mut state, piece_id, owner, type_id, 4, rank);

        let moves = generate_piece_legal_move_actions(&state, &piece_id.into());
        let mut choices: Vec<String> = moves
            .iter()
            .filter(|action| action.to == Square::new(4, target_rank))
            .filter_map(|action| action.promotion.clone())
            .collect();
        choices.sort();
        assert_eq!(
            choices,
            vec!["bouncing-bishop", "bouncing-queen", "bouncing-rook"]
        );
    }
}

#[test]
fn dozers_move_forward_and_promote_only_to_knight_or_bishop() {
    let mut white_state = make_game_state(8);
    add_piece(&mut white_state, "wk", "white", "king", 0, 0);
    add_piece(&mut white_state, "bk", "black", "king", 7, 7);
    add_piece(&mut white_state, "wd", "white", "dozer-white", 3, 6);

    let white_moves = generate_piece_legal_move_actions(&white_state, &"wd".into());
    let mut white_promotions: Vec<_> = white_moves
        .iter()
        .filter(|action| action.to.rank == 7)
        .filter_map(|action| action.promotion.clone())
        .collect();
    white_promotions.sort();
    assert_eq!(
        white_promotions,
        vec![
            "bishop", "bishop", "bishop", "bishop", "bishop", "knight", "knight", "knight",
            "knight", "knight"
        ]
    );

    let mut black_state = make_game_state(8);
    add_piece(&mut black_state, "wk", "white", "king", 0, 0);
    add_piece(&mut black_state, "bk", "black", "king", 7, 7);
    add_piece(&mut black_state, "bd", "black", "dozer-black", 3, 1);
    black_state.current_player = "black".into();

    let black_moves = generate_piece_legal_move_actions(&black_state, &"bd".into());
    let mut black_targets: Vec<_> = black_moves
        .iter()
        .filter(|action| action.to.rank == 0)
        .map(|action| (action.to.file, action.promotion.clone().unwrap()))
        .collect();
    black_targets.sort();
    assert_eq!(black_targets.len(), 10);
    assert!(black_targets
        .iter()
        .all(|(file, promotion)| (1..=5).contains(file)
            && matches!(promotion.as_str(), "bishop" | "knight")));
}

#[test]
fn test_pawn_promotion_applies_chosen_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "wp".into(),
        from: Square::new(4, 6),
        to: Square::new(4, 7),
        captured_piece_id: None,
        promotion: Some("queen".into()),
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, action);
    assert_eq!(new_state.pieces["wp"].type_id, "queen");
    assert_eq!(
        new_state.pieces["wp"].current_square,
        Some(Square::new(4, 7))
    );
}

#[test]
fn test_tempest_pawn_promotion_applies_chosen_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 6);

    let action = MoveAction {
        player_id: "white".into(),
        piece_id: "tp".into(),
        from: Square::new(4, 6),
        to: Square::new(4, 7),
        captured_piece_id: None,
        promotion: Some("tempest-queen".into()),
        move_option_id: "normal".into(),
        source_layer_ids: vec!["default".into()],
        effects: ActionEffects::default(),
    };
    let new_state = apply_move_action(state, action);
    assert_eq!(new_state.pieces["tp"].type_id, "tempest-queen");
    assert_eq!(
        new_state.pieces["tp"].current_square,
        Some(Square::new(4, 7))
    );
}

#[test]
fn test_tempest_pawn_type_survives_save_load() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "tp", "white", "tempest-pawn-white", 4, 1);

    let json = serde_json::to_string(&state).unwrap();
    let restored: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.pieces["tp"].type_id, "tempest-pawn-white");
}

#[test]
fn test_non_promoting_pawn_move_has_single_action_without_promotion() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 3);

    let moves = generate_piece_legal_move_actions(&state, &"wp".into());
    let single_step: Vec<&MoveAction> =
        moves.iter().filter(|m| m.to == Square::new(4, 4)).collect();
    assert_eq!(single_step.len(), 1);
    assert_eq!(single_step[0].promotion, None);
}

#[test]
fn test_game_catalog_overrides_a_builtin_definition() {
    let mut state = make_game_state(8);
    let mut bishop = bishop_definition();
    bishop.name = "Game Bishop".into();
    state.piece_definitions.insert("bishop".into(), bishop);

    let catalog = PieceCatalog::for_state(&state);
    assert_eq!(catalog.get("bishop").unwrap().name, "Game Bishop");
}

#[test]
fn test_custom_piece_definition_can_generate_promotion_choices() {
    let mut state = make_game_state(8);
    let definition = PieceDefinition {
        id: "promoter".into(),
        name: "Promoter".into(),
        score: 2,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "move(0, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        can_capture_on_drop: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec!["queen".into(), "knight".into()],
        state_schema: Vec::new(),
        move_layers: Vec::new(),
        move_options: Vec::new(),
        visual: PieceVisualDefinition::default(),
    }
    .normalize_and_validate()
    .unwrap();
    state
        .piece_definitions
        .insert("promoter".into(), definition);
    state.rebuild_chessembly_cache();
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "pr", "white", "promoter", 4, 6);

    let moves = generate_piece_legal_move_actions(&state, &"pr".into());
    let mut choices: Vec<String> = moves
        .iter()
        .filter(|m| m.to == Square::new(4, 7))
        .filter_map(|m| m.promotion.clone())
        .collect();
    choices.sort();
    assert_eq!(choices, vec!["knight", "queen"]);

    let promoted_state = apply_move_action(
        state,
        MoveAction {
            player_id: "white".into(),
            piece_id: "pr".into(),
            from: Square::new(4, 6),
            to: Square::new(4, 7),
            captured_piece_id: None,
            promotion: Some("knight".into()),
            move_option_id: "normal".into(),
            source_layer_ids: vec!["default".into()],
            effects: ActionEffects::default(),
        },
    );
    assert_eq!(promoted_state.pieces["pr"].type_id, "knight");
}

#[test]
fn test_chessembly_cache_preserves_legal_moves_and_attack_map() {
    let mut cached_state = make_game_state(8);
    add_piece(&mut cached_state, "wk", "white", "king", 4, 0);
    add_piece(&mut cached_state, "wr", "white", "rook", 0, 0);
    add_piece(&mut cached_state, "bk", "black", "king", 4, 7);
    add_piece(&mut cached_state, "bp", "black", "pawn-black", 0, 5);

    let rebuilt_state = cached_state.clone();
    rebuilt_state.rebuild_chessembly_cache();

    let mut cached_moves = generate_legal_move_actions(&cached_state);
    let mut rebuilt_moves = generate_legal_move_actions(&rebuilt_state);
    cached_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    rebuilt_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    assert_eq!(cached_moves.len(), rebuilt_moves.len());
    assert_eq!(
        cached_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>(),
        rebuilt_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>()
    );

    let empty_maps = HashMap::new();
    let cached_attack_map = generate_attack_map(&cached_state, &"white".into(), &empty_maps);
    let rebuilt_attack_map = generate_attack_map(&rebuilt_state, &"white".into(), &empty_maps);
    assert_eq!(
        cached_attack_map.attacked_squares,
        rebuilt_attack_map.attacked_squares
    );
    assert_eq!(cached_attack_map.source_map, rebuilt_attack_map.source_map);
}

#[test]
fn test_chessembly_cache_clone_and_deserialize_rebuild() {
    let state = make_game_state(8);
    let expected_program_count: usize = state
        .piece_definitions
        .values()
        .map(|definition| definition.move_layers.len())
        .sum();
    assert_eq!(
        state.cached_chessembly_program_count(),
        expected_program_count
    );

    let cloned = state.clone();
    let state_rook = state.chessembly_program(&"rook".to_string()).unwrap();
    let cloned_rook = cloned.chessembly_program(&"rook".to_string()).unwrap();
    assert!(Arc::ptr_eq(&state_rook, &cloned_rook));

    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("chessembly_program_cache"));
    assert!(json.contains("chessembly_code"));

    let deserialized: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.cached_chessembly_program_count(), 0);
    deserialized.ensure_chessembly_cache();
    assert_eq!(
        deserialized.cached_chessembly_program_count(),
        expected_program_count
    );
}

#[test]
fn test_piece_legal_move_actions_match_filtered_full_generator() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "wp", "white", "pawn-white", 3, 1);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_piece(&mut state, "bp", "black", "pawn-black", 0, 5);

    let mut full_rook_moves = generate_legal_move_actions(&state)
        .into_iter()
        .filter(|m| m.piece_id == "wr")
        .collect::<Vec<_>>();
    let mut piece_rook_moves = generate_piece_legal_move_actions(&state, &"wr".into());

    full_rook_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    piece_rook_moves.sort_by_key(|m| (m.piece_id.clone(), m.to.rank, m.to.file));
    assert_eq!(
        piece_rook_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>(),
        full_rook_moves
            .iter()
            .map(|m| (&m.piece_id, m.from, m.to, &m.captured_piece_id))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_piece_legal_move_actions_exclude_moved_piece() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let action = generate_piece_legal_move_actions(&state, &"wr".into())
        .into_iter()
        .find(|m| m.to == Square::new(0, 1))
        .unwrap();

    let moved_state = submit_action(state, TurnAction::Move(action)).unwrap();
    assert_eq!(moved_state.current_player, "black");
    assert_eq!(moved_state.history.len(), 1);
}

#[test]
fn test_piece_legal_drop_actions_match_filtered_full_drop_generator() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_pocket_piece(&mut state, "wn", "white", "knight");
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let mut full_piece_drops = generate_legal_drop_actions(&state)
        .into_iter()
        .filter(|d| d.piece_id == "wn")
        .collect::<Vec<_>>();
    let mut piece_drops = generate_piece_legal_drop_actions(&state, &"wn".into());

    full_piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));
    piece_drops.sort_by_key(|d| (d.piece_id.clone(), d.to.rank, d.to.file));
    assert_eq!(piece_drops, full_piece_drops);
}

#[test]
fn test_legal_action_cache_fields_are_not_serialized() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "wr", "white", "rook", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);

    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("legal_action_cache"));
    assert!(!json.contains("legal_action_cache_version"));
}

#[test]
fn test_square_id_is_copy_and_preserves_board_json_keys() {
    let id = Square::new(3, 5).to_id();
    let copied = id;
    assert_eq!(id, copied);
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"3_5\"");

    let board = create_board(8);
    let json = serde_json::to_string(&board).unwrap();
    assert!(json.contains("\"3_5\":null"));
    let decoded: Board = serde_json::from_str(&json).unwrap();
    assert!(decoded.squares.contains_key(&id));

    let legacy: Board = serde_json::from_value(serde_json::json!({
        "size": 8,
        "squares": { "3_5": null }
    }))
    .unwrap();
    assert!(legacy.terrain.is_empty());
}

#[test]
fn test_drop_candidates_are_grouped_by_piece_type() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 4, 0);
    add_piece(&mut state, "bk", "black", "king", 4, 7);
    add_pocket_piece(&mut state, "wn1", "white", "knight");
    add_pocket_piece(&mut state, "wn2", "white", "knight");
    add_pocket_piece(&mut state, "wr1", "white", "rook");

    let candidates = generate_drop_candidates_by_type(&state, &"white".to_string());
    let knight_candidates = candidates
        .iter()
        .filter(|candidate| candidate.piece_type_id == "knight")
        .collect::<Vec<_>>();
    let rook_candidates = candidates
        .iter()
        .filter(|candidate| candidate.piece_type_id == "rook")
        .collect::<Vec<_>>();

    assert!(!knight_candidates.is_empty());
    assert_eq!(knight_candidates.len(), rook_candidates.len());
    assert!(knight_candidates
        .iter()
        .all(|candidate| candidate.count == 2));
    assert!(rook_candidates.iter().all(|candidate| candidate.count == 1));
}

#[test]
fn test_cannon_rook_ability_uses_cannon_move_for_selected_move_only() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "cr", "white", "cannon-rook", 3, 3);
    add_piece(&mut state, "screen", "white", "pawn-white", 3, 4);
    add_piece(&mut state, "enemy", "black", "pawn-black", 3, 6);
    add_piece(&mut state, "screen2", "black", "pawn-black", 5, 3);
    add_piece(&mut state, "blocked", "white", "pawn-white", 6, 3);
    add_piece(&mut state, "enemy-screen", "black", "pawn-black", 2, 3);
    add_piece(
        &mut state,
        "enemy-behind-screen",
        "black",
        "pawn-black",
        0,
        3,
    );

    let base_moves = generate_piece_legal_move_actions(&state, &"cr".into());
    assert!(base_moves
        .iter()
        .any(|action| action.to == Square::new(4, 3)));
    assert!(!base_moves
        .iter()
        .any(|action| action.to == Square::new(3, 5)));

    let cannon_moves = generate_piece_legal_move_actions_with_options(
        &state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    );
    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(3, 5) && action.move_option_id == "cannon_move"
    }));
    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(3, 6)
            && action.captured_piece_id.as_ref().map(|id| id.as_str()) == Some("enemy")
            && action.move_option_id == "cannon_move"
    }));
    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(1, 3)
            && action.captured_piece_id.is_none()
            && action.move_option_id == "cannon_move"
    }));
    assert!(cannon_moves.iter().any(|action| {
        action.to == Square::new(0, 3)
            && action.captured_piece_id.as_ref().map(|id| id.as_str())
                == Some("enemy-behind-screen")
            && action.move_option_id == "cannon_move"
    }));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(2, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(4, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(5, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(6, 3)));
    assert!(!cannon_moves
        .iter()
        .any(|action| action.to == Square::new(7, 3)));

    let action = cannon_moves
        .iter()
        .find(|action| action.to == Square::new(3, 5))
        .cloned()
        .unwrap();
    let mut used_state = apply_move_action(state, action);
    let cooldown = used_state
        .pieces
        .get("cr")
        .unwrap()
        .move_option_cooldowns
        .get("cannon_move")
        .map(|cooldown| cooldown.remaining);
    assert_eq!(cooldown, Some(3));

    used_state.current_player = "white".into();
    assert!(generate_piece_legal_move_actions_with_options(
        &used_state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .is_empty());

    used_state
        .pieces
        .get_mut("cr")
        .unwrap()
        .move_option_cooldowns
        .remove("cannon_move");
    assert!(!generate_piece_legal_move_actions_with_options(
        &used_state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .is_empty());
}

#[test]
fn bouncing_bishop_reflection_is_part_of_normal_movement() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "bb", "white", "bouncing-bishop", 3, 2);

    let definition = &state.piece_definitions["bouncing-bishop"];
    assert_eq!(definition.move_options.len(), 1);
    assert_eq!(definition.move_options[0].id, "normal");
    assert!(definition.move_options[0].cooldown.is_none());

    let normal_moves = generate_piece_legal_move_actions(&state, &"bb".into());
    let reflected_action = normal_moves
        .into_iter()
        .find(|action| action.to == Square::new(6, 7))
        .expect("reflected destination must be available through normal movement");
    assert_eq!(reflected_action.move_option_id, "normal");

    state = apply_move_action(state, reflected_action);
    assert!(state.pieces["bb"].move_option_cooldowns.is_empty());

    let mut wall_state = make_game_state(8);
    add_piece(&mut wall_state, "wk", "white", "king", 0, 0);
    add_piece(&mut wall_state, "bk", "black", "king", 7, 7);
    add_piece(&mut wall_state, "bb", "white", "bouncing-bishop", 3, 3);
    add_piece(
        &mut wall_state,
        "wall",
        "black",
        "bouncing-pawn-black",
        5,
        5,
    );

    let wall_moves = generate_piece_legal_move_actions(&wall_state, &"bb".into());
    assert!(wall_moves
        .iter()
        .any(|action| action.to == Square::new(4, 4)));
    assert!(wall_moves
        .iter()
        .any(|action| action.to == Square::new(3, 5)));
    assert!(wall_moves
        .iter()
        .any(|action| action.to == Square::new(5, 3)));
    assert!(!wall_moves
        .iter()
        .any(|action| action.to == Square::new(5, 5)));
}

#[test]
fn bouncing_pawn_is_a_direct_wall_for_bouncing_rook_and_queen() {
    for type_id in ["bouncing-rook", "bouncing-queen"] {
        let mut state = make_game_state(8);
        add_piece(&mut state, "wk", "white", "king", 0, 0);
        add_piece(&mut state, "bk", "black", "king", 7, 7);
        add_piece(&mut state, "bouncer", "white", type_id, 3, 3);
        add_piece(&mut state, "wall", "black", "bouncing-pawn-black", 3, 5);

        let moves = generate_piece_legal_move_actions(&state, &"bouncer".into());
        assert!(moves.iter().any(|action| action.to == Square::new(3, 4)));
        assert!(
            moves.iter().any(|action| action.to == Square::new(1, 4)),
            "{type_id} should reflect left from the Bouncing Pawn wall"
        );
        assert!(
            moves.iter().any(|action| action.to == Square::new(5, 4)),
            "{type_id} should reflect right from the Bouncing Pawn wall"
        );
        assert!(
            !moves.iter().any(|action| action.to == Square::new(3, 5)),
            "{type_id} must not capture or enter the Bouncing Pawn wall"
        );
    }

    let mut ordinary_state = make_game_state(8);
    add_piece(&mut ordinary_state, "wk", "white", "king", 0, 0);
    add_piece(&mut ordinary_state, "bk", "black", "king", 7, 7);
    add_piece(&mut ordinary_state, "rook", "white", "rook", 3, 3);
    add_piece(
        &mut ordinary_state,
        "wall",
        "black",
        "bouncing-pawn-black",
        3,
        5,
    );
    let ordinary_moves = generate_piece_legal_move_actions(&ordinary_state, &"rook".into());
    assert!(ordinary_moves.iter().any(|action| {
        action.to == Square::new(3, 5)
            && action.captured_piece_id.as_ref().map(PieceId::as_str) == Some("wall")
    }));
    assert!(!ordinary_moves
        .iter()
        .any(|action| action.to == Square::new(1, 4)));
}

#[test]
fn owner_turn_cooldown_is_set_on_commit_and_ticks_only_after_later_owner_actions() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "cr", "white", "cannon-rook", 3, 3);
    add_piece(&mut state, "screen", "white", "pawn-white", 3, 4);

    let cannon_action = generate_piece_legal_move_actions_with_options(
        &state,
        &"cr".into(),
        &MoveGenerationOptions {
            move_option_id: Some("cannon_move".into()),
        },
    )
    .into_iter()
    .find(|action| action.to == Square::new(3, 5))
    .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(cannon_action));
    assert_eq!(state.current_player, "black");
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        3
    );

    let black_move = generate_piece_legal_move_actions(&state, &"bk".into())
        .into_iter()
        .next()
        .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(black_move));
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        3
    );

    let white_move = generate_piece_legal_move_actions(&state, &"wk".into())
        .into_iter()
        .next()
        .unwrap();
    state = apply_and_advance_turn(state, TurnAction::Move(white_move));
    assert_eq!(
        state.pieces["cr"].move_option_cooldowns["cannon_move"].remaining,
        2
    );
    assert_eq!(state.history.len(), 3);
}

#[test]
fn promotion_resets_piece_state_cooldowns_and_visual_definition() {
    let mut state = make_game_state(8);
    add_piece(&mut state, "wk", "white", "king", 0, 0);
    add_piece(&mut state, "bk", "black", "king", 7, 7);
    add_piece(&mut state, "wp", "white", "pawn-white", 4, 6);
    {
        let pawn = state.pieces.get_mut("wp").unwrap();
        pawn.state
            .insert("legacy".into(), PieceStateValue::Boolean(true));
        pawn.move_option_cooldowns
            .insert("legacy".into(), CooldownState { remaining: 9 });
    }
    let action = generate_piece_legal_move_actions(&state, &"wp".into())
        .into_iter()
        .find(|action| {
            action.to == Square::new(4, 7) && action.promotion.as_deref() == Some("knight")
        })
        .unwrap();
    let state = apply_move_action(state, action);
    let promoted = &state.pieces["wp"];
    assert_eq!(promoted.type_id, "knight");
    assert!(promoted.state.is_empty());
    assert!(promoted.move_option_cooldowns.is_empty());
    assert_eq!(
        state.piece_definitions["knight"].resolve_asset_key(promoted),
        "knight"
    );
}

#[test]
fn piece_definition_validation_rejects_unknown_references() {
    let mut definition = rook_definition();
    definition.move_options[0].layer_ids = vec!["missing".into()];
    assert!(definition.validate().unwrap_err().contains("unknown layer"));

    let mut definition = windmill_definition();
    definition.visual.variants[0].enabled_when[0].key = "typo".into();
    assert!(definition
        .validate()
        .unwrap_err()
        .contains("unknown state key"));
}
