use std::collections::HashMap;

use brainfuck_chess_engine::{
    catalog::PieceCatalog,
    movegen::{
        ChessemblyBackend, NativeBackend, PieceMoveBackend, PieceMoveContext, PieceMovePattern,
    },
    rules::create_board,
    types::*,
};

fn empty_player(id: &str) -> Player {
    Player {
        id: id.into(),
        deck: Deck {
            player_id: id.into(),
            starting_pieces: Vec::new(),
            pocket_pieces: Vec::new(),
            score_limit: 39,
            total_score: 0,
        },
        captured_pieces: Vec::new(),
    }
}

fn add_piece(
    state: &mut GameState,
    id: &str,
    owner: PlayerId,
    type_id: PieceTypeId,
    square: Square,
) {
    let piece_id = PieceId::from(id);
    state
        .board
        .squares
        .insert(square.to_id(), Some(piece_id.clone()));
    state.pieces.insert(
        piece_id.clone(),
        Piece {
            id: piece_id,
            owner,
            type_id,
            current_square: Some(square),
            in_pocket: false,
            captured: false,
            has_moved: false,
            active_ability: None,
            ability_cooldowns: HashMap::new(),
        },
    );
}

fn parity_state(piece_type: &str) -> (GameState, PieceId) {
    let mut players = HashMap::new();
    players.insert("white".into(), empty_player("white"));
    players.insert("black".into(), empty_player("black"));

    let mut state = GameState {
        id: format!("{piece_type}-parity"),
        board: create_board(8),
        pieces: HashMap::new(),
        players,
        current_player: "white".into(),
        turn_number: 1,
        phase: GamePhase::Playing,
        en_passant_target: None,
        en_passant_available_to: None,
        turn_state: TurnState::new(),
        result: None,
    };

    let subject_id = PieceId::from("subject");
    add_piece(
        &mut state,
        subject_id.as_str(),
        "white".into(),
        piece_type.into(),
        Square::new(3, 3),
    );

    add_piece(
        &mut state,
        "friendly_east_blocker",
        "white".into(),
        "pawn-white".into(),
        Square::new(5, 3),
    );
    add_piece(
        &mut state,
        "enemy_north_blocker",
        "black".into(),
        "pawn-black".into(),
        Square::new(3, 5),
    );
    add_piece(
        &mut state,
        "friendly_northeast_blocker",
        "white".into(),
        "pawn-white".into(),
        Square::new(4, 4),
    );
    add_piece(
        &mut state,
        "enemy_southwest_blocker",
        "black".into(),
        "pawn-black".into(),
        Square::new(1, 1),
    );
    add_piece(
        &mut state,
        "friendly_knight_target",
        "white".into(),
        "pawn-white".into(),
        Square::new(4, 5),
    );
    add_piece(
        &mut state,
        "enemy_knight_target",
        "black".into(),
        "pawn-black".into(),
        Square::new(5, 4),
    );

    (state, subject_id)
}

fn normalize(mut pattern: PieceMovePattern) -> PieceMovePattern {
    pattern
        .movement_squares
        .sort_by_key(|square| (square.rank, square.file));
    pattern
        .attack_squares
        .sort_by_key(|square| (square.rank, square.file));
    pattern
}

fn assert_native_matches_chessembly(piece_type: &str) {
    let (state, piece_id) = parity_state(piece_type);
    let catalog = PieceCatalog::default_catalog();
    let definition = catalog.get(&piece_type.to_string()).unwrap();
    let piece = state.pieces.get(&piece_id).unwrap();
    let player_id = "white".to_string();

    let chessembly = ChessemblyBackend.generate(PieceMoveContext {
        state: &state,
        piece,
        definition,
        player_id: &player_id,
    });
    let native = NativeBackend.generate(PieceMoveContext {
        state: &state,
        piece,
        definition,
        player_id: &player_id,
    });

    assert_eq!(
        normalize(chessembly),
        normalize(native),
        "{piece_type} native backend should match Chessembly backend"
    );
}

#[test]
fn king_native_backend_matches_chessembly_backend() {
    assert_native_matches_chessembly("king");
}

#[test]
fn queen_native_backend_matches_chessembly_backend() {
    assert_native_matches_chessembly("queen");
}

#[test]
fn rook_native_backend_matches_chessembly_backend() {
    assert_native_matches_chessembly("rook");
}

#[test]
fn bishop_native_backend_matches_chessembly_backend() {
    assert_native_matches_chessembly("bishop");
}

#[test]
fn knight_native_backend_matches_chessembly_backend() {
    assert_native_matches_chessembly("knight");
}
