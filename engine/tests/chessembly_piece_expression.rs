use std::collections::{HashMap, HashSet};

use brainfuck_chess_engine::chessembly::interpreter::{run, ExecutionContext};
use brainfuck_chess_engine::chessembly::parser::parse;
use brainfuck_chess_engine::pieces::default_pieces::all_default_definitions;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::{Piece, PieceId, PlayerId, Square, SquareId};

fn piece(id: &str, type_id: &str, square: Square) -> Piece {
    Piece {
        id: id.into(),
        owner: "white".into(),
        type_id: type_id.into(),
        current_square: Some(square),
        in_pocket: false,
        captured: false,
        has_moved: false,
        state: HashMap::new(),
        move_option_cooldowns: HashMap::new(),
    }
}

fn context_parts() -> (
    Square,
    brainfuck_chess_engine::types::Board,
    HashMap<PieceId, Piece>,
    HashMap<String, brainfuck_chess_engine::types::PieceDefinition>,
) {
    let initial_square = Square::new(3, 3);
    let rook = piece("rook-1", "rook", initial_square);
    let mut board = create_board(8);
    board
        .squares
        .insert(initial_square.to_id(), Some(rook.id.clone()));
    let pieces = HashMap::from([(PieceId::from("rook-1"), rook)]);
    let definitions = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    (initial_square, board, pieces, definitions)
}

#[test]
fn piece_expression_resolves_the_piece_on_the_initial_square() {
    let (initial_square, board, pieces, definitions) = context_parts();
    let global_state = HashMap::new();
    let attack_maps: HashMap<PlayerId, HashSet<SquareId>> = HashMap::new();
    let context = ExecutionContext {
        board: &board,
        initial_square,
        all_definitions: &definitions,
        all_pieces: &pieces,
        player: "white".into(),
        global_state: &global_state,
        attack_maps: &attack_maps,
    };

    let result = run(&parse("move(1, 0) piece(rook) move(1, 0);"), &context);

    assert!(result.movement_squares.contains(&Square::new(4, 3)));
    assert!(result.movement_squares.contains(&Square::new(5, 3)));
}

#[test]
fn piece_expression_rejects_a_different_piece_type() {
    let (initial_square, board, pieces, definitions) = context_parts();
    let global_state = HashMap::new();
    let attack_maps: HashMap<PlayerId, HashSet<SquareId>> = HashMap::new();
    let context = ExecutionContext {
        board: &board,
        initial_square,
        all_definitions: &definitions,
        all_pieces: &pieces,
        player: "white".into(),
        global_state: &global_state,
        attack_maps: &attack_maps,
    };

    let result = run(&parse("piece(knight) move(1, 0);"), &context);

    assert!(result.movement_squares.is_empty());
}
