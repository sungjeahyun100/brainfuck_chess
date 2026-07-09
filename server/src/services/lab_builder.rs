use std::collections::{HashMap, HashSet};

use brainfuck_chess_engine::{
    rules::{calculate_score_limit, create_board},
    types::{Deck, GamePhase, GameState, Piece, PieceId, Player, TurnState},
};

use crate::dto::lab::LabPieceOptionsRequest;
use crate::mappers::deck_spec::resolve_piece_type;

pub fn build_lab_game_state(req: &LabPieceOptionsRequest) -> Result<GameState, String> {
    if !(8..=12).contains(&req.board_size) {
        return Err("보드 크기는 8부터 12까지 선택할 수 있습니다.".into());
    }

    let mut board = create_board(req.board_size);
    let mut pieces = HashMap::new();
    let mut white_starting = Vec::new();
    let mut black_starting = Vec::new();
    let mut seen_piece_ids = HashSet::new();

    for lab_piece in &req.pieces {
        if lab_piece.owner != "white" && lab_piece.owner != "black" {
            return Err("기물 owner는 white 또는 black이어야 합니다.".into());
        }
        if !seen_piece_ids.insert(lab_piece.id.clone()) {
            return Err(format!("중복된 테스트 기물 id입니다: {}", lab_piece.id));
        }
        if !board.is_in_bounds(&lab_piece.square) {
            return Err(format!(
                "{} 배치가 보드 밖입니다.",
                lab_piece.square.to_id()
            ));
        }
        if !board.is_empty(&lab_piece.square) {
            return Err(format!(
                "{} 칸에 이미 기물이 있습니다.",
                lab_piece.square.to_id()
            ));
        }

        let type_id = resolve_piece_type(&lab_piece.owner, &lab_piece.piece_type)
            .ok_or_else(|| format!("알 수 없는 기물 타입입니다: {}", lab_piece.piece_type))?;
        let piece_id = PieceId::from(lab_piece.id.clone());
        let piece = Piece {
            id: piece_id.clone(),
            owner: lab_piece.owner.clone(),
            type_id,
            current_square: Some(lab_piece.square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
        };

        board
            .squares
            .insert(lab_piece.square.to_id(), Some(piece_id.clone()));
        if lab_piece.owner == "white" {
            white_starting.push(piece_id.clone());
        } else {
            black_starting.push(piece_id.clone());
        }
        pieces.insert(piece_id, piece);
    }

    let selected_piece_id = PieceId::from(req.selected_piece_id.clone());
    let selected_piece = pieces
        .get(&selected_piece_id)
        .ok_or_else(|| "선택한 테스트 기물을 찾을 수 없습니다.".to_string())?;
    let current_player = selected_piece.owner.clone();

    let white_deck = Deck {
        player_id: "white".into(),
        starting_pieces: white_starting,
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(req.board_size),
        total_score: 0,
    };
    let black_deck = Deck {
        player_id: "black".into(),
        starting_pieces: black_starting,
        pocket_pieces: Vec::new(),
        score_limit: calculate_score_limit(req.board_size),
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

    Ok(GameState {
        id: "piece-lab".into(),
        board,
        pieces,
        players,
        current_player,
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        turn_state: TurnState::new(),
        result: None,
    })
}
