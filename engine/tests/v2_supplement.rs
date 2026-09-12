use std::collections::HashMap;

use brainfuck_chess_engine::actions::submit_action;
use brainfuck_chess_engine::legal_moves::{
    generate_legal_ability_actions, generate_piece_legal_ability_actions,
    generate_piece_legal_drop_actions, generate_piece_legal_move_actions,
};
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;

fn make_state() -> GameState {
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    GameState {
        ruleset: Default::default(),
        id: "v2-supplement".into(),
        board: create_board(8),
        pieces: HashMap::new(),
        chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
        piece_definitions: definitions,
        custom_piece_manifest: Vec::new(),
        players: ["white", "black"]
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
                            score_limit: 39,
                            total_score: 0,
                        },
                        captured_pieces: Vec::new(),
                    },
                )
            })
            .collect(),
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
    layer: PieceLayer,
) {
    let piece_id: PieceId = id.into();
    let mut piece = Piece {
        id: piece_id.clone(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: square,
        in_pocket: square.is_none(),
        captured: false,
        has_moved: false,
        current_ammo: 0,
        layer,
        remaining_flight_turns: 0,
        state: HashMap::new(),
        move_option_cooldowns: HashMap::new(),
    };
    piece.initialize_from_definition(&state.piece_definitions[type_id]);
    piece.layer = layer;
    if let Some(square) = square {
        state
            .board
            .set_piece_at_layer(square, layer, Some(piece_id.clone()));
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
    state.pieces.insert(piece_id, piece);
}

fn score(state: &GameState, piece_id: &PieceId) -> u32 {
    let piece = &state.pieces[piece_id];
    state.piece_definitions[&piece.type_id].score
}

#[test]
fn supplement_definitions_are_registered_with_documented_metadata() {
    let state = make_state();
    for (id, expected_score, zone) in [
        ("shell", 3, DeploymentZone::Back),
        ("sacrificial-shrine", 8, DeploymentZone::Back),
        ("sacrificial-lamb", 1, DeploymentZone::Back),
        ("fanatic", 2, DeploymentZone::Front),
        ("wall", 1, DeploymentZone::Front),
        ("repairman", 4, DeploymentZone::Back),
    ] {
        let definition = &state.piece_definitions[id];
        assert_eq!(definition.score, expected_score, "{id}");
        assert_eq!(definition.deployment_zone, zone, "{id}");
        assert!(!definition.is_king, "{id}");
    }
}

#[test]
fn wall_blocks_ordinary_capture_but_special_explosion_removes_it() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "rook",
        "white",
        "rook",
        Some(Square::new(0, 0)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "wall",
        "black",
        "wall",
        Some(Square::new(3, 0)),
        PieceLayer::Ground,
    );
    assert!(!generate_piece_legal_move_actions(&state, &"rook".into())
        .iter()
        .any(|action| action.to == Square::new(3, 0)));
    add_piece(
        &mut state,
        "pocket-shell",
        "white",
        "shell",
        None,
        PieceLayer::Ground,
    );
    assert!(
        !generate_piece_legal_drop_actions(&state, &"pocket-shell".into())
            .iter()
            .any(|action| action.to == Square::new(3, 0))
    );

    state
        .board
        .set_piece_at_layer(Square::new(0, 0), PieceLayer::Ground, None);
    state.pieces.get_mut("rook").unwrap().captured = true;
    state.pieces.get_mut("rook").unwrap().current_square = None;
    add_piece(
        &mut state,
        "shell",
        "white",
        "shell",
        Some(Square::new(2, 0)),
        PieceLayer::Ground,
    );
    let detonate = generate_piece_legal_ability_actions(&state, &"shell".into(), "detonate")
        .into_iter()
        .next()
        .unwrap();
    let next = submit_action(state, TurnAction::Ability(detonate)).unwrap();
    assert!(next.pieces["wall"].captured);
    assert!(next.pieces["shell"].captured);
}

#[test]
fn shell_rejects_occupied_drops_and_explodes_immediately_on_empty_square() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "rook",
        "white",
        "rook",
        Some(Square::new(0, 0)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "enemy",
        "black",
        "bishop",
        Some(Square::new(1, 1)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "shell",
        "white",
        "shell",
        None,
        PieceLayer::Ground,
    );
    let drops = generate_piece_legal_drop_actions(&state, &"shell".into());
    for (to, occupant) in [(Square::new(0, 0), "rook"), (Square::new(1, 1), "enemy")] {
        assert!(!drops.iter().any(|action| action.to == to));
        for captured_piece_id in [None, Some(occupant.into())] {
            let request = DropAction {
                sacrifice_piece_ids: Vec::new(),
                player_id: "white".into(),
                piece_id: "shell".into(),
                to,
                captured_piece_id,
            };
            assert!(submit_action(state.clone(), TurnAction::Drop(request)).is_err());
        }
    }
    let drop = drops
        .into_iter()
        .find(|action| action.to == Square::new(0, 1))
        .unwrap();
    let next = submit_action(state, TurnAction::Drop(drop)).unwrap();
    assert!(next.pieces["enemy"].captured);
    assert!(next.pieces["shell"].captured);
    assert!(next.pieces["rook"].captured);
    assert!(next.board.is_empty(&Square::new(1, 1)));
}

#[test]
fn sacrifice_uses_ground_non_king_allies_and_validates_selected_budget() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "shrine",
        "white",
        "sacrificial-shrine",
        Some(Square::new(3, 3)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "lamb",
        "white",
        "sacrificial-lamb",
        Some(Square::new(2, 3)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "bishop-sacrifice",
        "white",
        "bishop",
        Some(Square::new(3, 4)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "friendly-king",
        "white",
        "king",
        Some(Square::new(4, 3)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "friendly-air",
        "white",
        "bomber",
        Some(Square::new(4, 3)),
        PieceLayer::Air,
    );
    add_piece(
        &mut state,
        "enemy-rook",
        "black",
        "bishop",
        Some(Square::new(7, 0)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "enemy-king",
        "black",
        "king",
        Some(Square::new(7, 7)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "enemy-air",
        "black",
        "bomber",
        Some(Square::new(6, 6)),
        PieceLayer::Air,
    );

    let singles = generate_piece_legal_ability_actions(&state, &"shrine".into(), "sacrifice");
    assert!(singles
        .iter()
        .any(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy-rook")));
    assert!(!singles
        .iter()
        .any(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy-king")));
    assert!(!singles
        .iter()
        .any(|action| action.target_piece_id.as_ref().map(PieceId::as_str) == Some("enemy-air")));

    let action = AbilityAction {
        player_id: "white".into(),
        piece_id: "shrine".into(),
        ability_id: "sacrifice".into(),
        target_piece_id: None,
        target_piece_ids: vec!["enemy-rook".into()],
        pocket_piece_id: None,
        to: None,
        deployments: Vec::new(),
    };
    let next = submit_action(state, TurnAction::Ability(action)).unwrap();
    assert!(next.pieces["lamb"].captured);
    assert!(next.pieces["bishop-sacrifice"].captured);
    assert!(next.pieces["friendly-air"].captured);
    assert!(!next.pieces["friendly-king"].captured);
    assert!(next.pieces["enemy-rook"].captured);
    assert_eq!(
        next.pieces["shrine"].move_option_cooldowns["sacrifice"].remaining,
        2
    );
}

#[test]
fn eight_lamb_ring_allows_sacrificing_the_enemy_king() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "shrine",
        "white",
        "sacrificial-shrine",
        Some(Square::new(3, 3)),
        PieceLayer::Ground,
    );
    for (index, (dx, dy)) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ]
    .into_iter()
    .enumerate()
    {
        add_piece(
            &mut state,
            &format!("lamb-{index}"),
            "white",
            "sacrificial-lamb",
            Some(Square::new(3 + dx, 3 + dy)),
            PieceLayer::Ground,
        );
    }
    add_piece(
        &mut state,
        "enemy-king",
        "black",
        "king",
        Some(Square::new(7, 7)),
        PieceLayer::Ground,
    );

    let action = generate_legal_ability_actions(&state)
        .into_iter()
        .find(|action| action.ability_id == "sacrifice")
        .unwrap();
    assert_eq!(action.target_piece_ids, vec![PieceId::from("enemy-king")]);
    let next = submit_action(state, TurnAction::Ability(action)).unwrap();
    assert_eq!(next.result.unwrap().winner.as_deref(), Some("white"));
}

#[test]
fn ai_sacrifice_selection_is_one_bounded_maximum_value_action() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "shrine",
        "white",
        "sacrificial-shrine",
        Some(Square::new(3, 3)),
        PieceLayer::Ground,
    );
    for (index, square) in [Square::new(2, 3), Square::new(3, 4), Square::new(4, 3)]
        .into_iter()
        .enumerate()
    {
        add_piece(
            &mut state,
            &format!("lamb-{index}"),
            "white",
            "sacrificial-lamb",
            Some(square),
            PieceLayer::Ground,
        );
    }
    add_piece(
        &mut state,
        "enemy-knight",
        "black",
        "knight",
        Some(Square::new(7, 7)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "enemy-pawn",
        "black",
        "pawn-black",
        Some(Square::new(6, 7)),
        PieceLayer::Ground,
    );

    let actions = generate_legal_ability_actions(&state)
        .into_iter()
        .filter(|action| action.ability_id == "sacrifice")
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]
            .target_piece_ids
            .iter()
            .map(|id| score(&state, id))
            .sum::<u32>(),
        3
    );
}

#[test]
fn fanatic_death_removes_only_the_capturer() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "rook",
        "white",
        "rook",
        Some(Square::new(0, 0)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "fanatic",
        "black",
        "fanatic",
        Some(Square::new(2, 0)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "air-piece",
        "black",
        "bomber",
        Some(Square::new(2, 0)),
        PieceLayer::Air,
    );
    let capture = generate_piece_legal_move_actions(&state, &"rook".into())
        .into_iter()
        .find(|action| action.to == Square::new(2, 0))
        .unwrap();
    let next = submit_action(state, TurnAction::Move(capture)).unwrap();
    assert!(next.pieces["fanatic"].captured);
    assert!(next.pieces["rook"].captured);
    assert!(!next.pieces["air-piece"].captured);
    assert!(next.board.get_piece_at(&Square::new(2, 0)).is_none());
    assert_eq!(
        next.board
            .get_piece_at_layer(&Square::new(2, 0), PieceLayer::Air),
        Some(&PieceId::from("air-piece"))
    );
}

#[test]
fn repairman_fills_empty_front_squares_when_an_allied_wall_exists() {
    let mut state = make_state();
    add_piece(
        &mut state,
        "repairman",
        "white",
        "repairman",
        Some(Square::new(3, 3)),
        PieceLayer::Ground,
    );
    add_piece(
        &mut state,
        "existing-wall",
        "white",
        "wall",
        Some(Square::new(3, 4)),
        PieceLayer::Ground,
    );
    let repair = generate_piece_legal_ability_actions(&state, &"repairman".into(), "repair-walls")
        .into_iter()
        .next()
        .unwrap();
    let next = submit_action(state, TurnAction::Ability(repair)).unwrap();
    for square in [Square::new(2, 4), Square::new(3, 4), Square::new(4, 4)] {
        let piece_id = next.board.get_piece_at(&square).unwrap();
        assert_eq!(next.pieces[piece_id].type_id, "wall");
        assert_eq!(next.pieces[piece_id].owner, "white");
    }
}
