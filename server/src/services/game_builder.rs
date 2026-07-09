use std::collections::{HashMap, HashSet};

use brainfuck_chess_engine::{
    pieces::default_pieces::all_default_definitions,
    rules::{
        calculate_deck_score, calculate_score_limit, create_board, get_base_zone_squares,
        validate_deck,
    },
    types::{
        Board, Deck, GamePhase, GameState, Piece, PieceDefinition, PieceId, PieceTypeId, Player,
        SquareId, TurnState,
    },
};

use crate::dto::game::PlayerDeckSpec;
use crate::mappers::deck_spec::{make_piece_id, resolve_piece_type};

fn build_player_deck(
    player_id: &str,
    spec: &PlayerDeckSpec,
    board_size: i32,
    board: &mut Board,
    pieces: &mut HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> Result<Deck, String> {
    let base_zone: HashSet<SquareId> = get_base_zone_squares(&player_id.to_string(), board_size)
        .into_iter()
        .map(|sq| sq.to_id())
        .collect();

    let mut counters = HashMap::new();
    let mut starting_pieces = Vec::new();
    let mut pocket_pieces = Vec::new();

    for placement in &spec.starting {
        if !board.is_in_bounds(&placement.square) {
            return Err(format!("{} 시작 기물 배치가 보드 밖입니다.", player_id));
        }
        if !base_zone.contains(&placement.square.to_id()) {
            return Err(format!(
                "{} 시작 기물은 기본 진영에만 배치할 수 있습니다.",
                player_id
            ));
        }
        if !board.is_empty(&placement.square) {
            return Err(format!(
                "{} 배치 칸이 이미 사용 중입니다.",
                placement.square.to_id()
            ));
        }

        let type_id = resolve_piece_type(player_id, &placement.piece_type)
            .ok_or_else(|| format!("알 수 없는 기물 타입입니다: {}", placement.piece_type))?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);

        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: Some(placement.square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
        };

        board
            .squares
            .insert(placement.square.to_id(), Some(piece_id.clone()));
        pieces.insert(piece_id.clone(), piece);
        starting_pieces.push(piece_id);
    }

    for pocket_piece in &spec.pocket {
        let type_id = resolve_piece_type(player_id, pocket_piece)
            .ok_or_else(|| format!("알 수 없는 포켓 기물 타입입니다: {}", pocket_piece))?;
        let piece_id = make_piece_id(player_id, &type_id, &mut counters);
        let piece = Piece {
            id: piece_id.clone(),
            owner: player_id.into(),
            type_id: type_id.clone(),
            current_square: None,
            in_pocket: true,
            captured: false,
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
        };

        pieces.insert(piece_id.clone(), piece);
        pocket_pieces.push(piece_id);
    }

    let mut deck = Deck {
        player_id: player_id.into(),
        starting_pieces,
        pocket_pieces,
        score_limit: calculate_score_limit(board_size),
        total_score: 0,
    };

    deck.total_score = calculate_deck_score(&deck, pieces, definitions);

    let validation = validate_deck(&deck, board_size, pieces, definitions);
    if !validation.valid {
        return Err(validation.errors.join(" "));
    }

    Ok(deck)
}

pub fn build_game_state(
    id: String,
    board_size: i32,
    white_spec: &PlayerDeckSpec,
    black_spec: &PlayerDeckSpec,
) -> Result<GameState, String> {
    if board_size < 8 {
        return Err("보드 크기는 최소 8이어야 합니다.".into());
    }

    let mut board = create_board(board_size);
    let defs: HashMap<String, PieceDefinition> = all_default_definitions()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let mut pieces = HashMap::new();

    let white_deck = build_player_deck(
        "white",
        white_spec,
        board_size,
        &mut board,
        &mut pieces,
        &defs,
    )?;
    let black_deck = build_player_deck(
        "black",
        black_spec,
        board_size,
        &mut board,
        &mut pieces,
        &defs,
    )?;

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

    Ok(GameState {
        id,
        board,
        pieces,
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        turn_state: TurnState::new(),
        result: None,
    })
}
