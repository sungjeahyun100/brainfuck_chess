//! Chessembly backward-compatibility tests.
//!
//! These tests verify that standard Chessembly examples from the official docs
//! produce the expected movement / attack squares.

use std::collections::HashMap;

use brainfuck_chess_engine::chessembly::interpreter::{run, ExecutionContext};
use brainfuck_chess_engine::chessembly::parser::parse;
use brainfuck_chess_engine::pieces::default_pieces::*;
use brainfuck_chess_engine::rules::create_board;
use brainfuck_chess_engine::types::*;

fn make_piece(id: &str, owner: &str, type_id: &str, file: i32, rank: i32) -> Piece {
    Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: Some(Square::new(file, rank)),
        in_pocket: false,
        captured: false,
        has_moved: false,
        state: HashMap::new(),
        move_option_cooldowns: HashMap::new(),
    }
}

fn run_code(
    code: &str,
    piece: &Piece,
    board: &Board,
    all_pieces: &HashMap<PieceId, Piece>,
    def: &PieceDefinition,
) -> ChessemblyResult {
    let program = parse(code);
    let initial_square = piece.current_square.unwrap_or(Square::new(0, 0));
    let mut board = board.clone();
    board
        .squares
        .insert(initial_square.to_id(), Some(piece.id.clone()));
    let definitions = HashMap::from([(def.id.clone(), def.clone())]);
    let ctx = ExecutionContext {
        board: &board,
        initial_square,
        all_definitions: &definitions,
        all_pieces,
        player: piece.owner.clone(),
        global_state: &HashMap::new(),
        attack_maps: &HashMap::new(),
    };
    run(&program, &ctx)
}

fn sorted(mut squares: Vec<Square>) -> Vec<Square> {
    squares.sort_by_key(|s| (s.rank, s.file));
    squares
}

#[test]
fn test_wazir_center() {
    let board = create_board(8);
    let piece = make_piece("w1", "white", "wazir", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("w1".into(), piece.clone());

    let def = PieceDefinition {
        id: "wazir".into(),
        name: "Wazir".into(),
        score: 1,
        chessembly_code: "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);"
            .into(),
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    let moves = sorted(result.movement_squares.clone());
    assert!(moves.contains(&Square::new(4, 3)), "right");
    assert!(moves.contains(&Square::new(2, 3)), "left");
    assert!(moves.contains(&Square::new(3, 4)), "up");
    assert!(moves.contains(&Square::new(3, 2)), "down");
    assert_eq!(moves.len(), 4);
    assert!(result.attack_squares.contains(&Square::new(4, 3)));
    assert!(result.attack_squares.contains(&Square::new(2, 3)));
    assert!(result.attack_squares.contains(&Square::new(3, 4)));
    assert!(result.attack_squares.contains(&Square::new(3, 2)));
}

#[test]
fn test_rook_slide_open_board() {
    let board = create_board(8);
    let def = rook_definition();
    let piece = make_piece("r1", "white", "rook", 0, 0);
    let mut pieces = HashMap::new();
    pieces.insert("r1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert_eq!(result.movement_squares.len(), 14);
}

#[test]
fn test_rook_blocked_by_friendly() {
    let mut board = create_board(8);
    let piece = make_piece("r1", "white", "rook", 0, 0);
    let blocker = make_piece("p1", "white", "pawn-white", 3, 0);
    board
        .squares
        .insert(blocker.current_square.unwrap().to_id(), Some("p1".into()));
    let mut pieces = HashMap::new();
    pieces.insert("r1".into(), piece.clone());
    pieces.insert("p1".into(), blocker);

    let def = rook_definition();
    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(1, 0)));
    assert!(result.movement_squares.contains(&Square::new(2, 0)));
    assert!(!result.movement_squares.contains(&Square::new(3, 0)));
    assert!(!result.movement_squares.contains(&Square::new(4, 0)));
}

#[test]
fn test_rook_can_capture_enemy() {
    let mut board = create_board(8);
    let piece = make_piece("r1", "white", "rook", 0, 0);
    let enemy = make_piece("e1", "black", "rook", 3, 0);
    board
        .squares
        .insert(enemy.current_square.unwrap().to_id(), Some("e1".into()));
    let mut pieces = HashMap::new();
    pieces.insert("r1".into(), piece.clone());
    pieces.insert("e1".into(), enemy);

    let def = rook_definition();
    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.attack_squares.contains(&Square::new(3, 0)));
    assert!(!result.movement_squares.contains(&Square::new(4, 0)));
    assert!(!result.attack_squares.contains(&Square::new(4, 0)));
}

#[test]
fn test_knightrider_slide() {
    let board = create_board(8);
    let piece = make_piece("kr1", "white", "knightrider", 0, 0);
    let mut pieces = HashMap::new();
    pieces.insert("kr1".into(), piece.clone());

    let def = PieceDefinition {
        id: "knightrider".into(),
        name: "Knightrider".into(),
        score: 5,
        chessembly_code: "\
take-move(1, 2) repeat(1);
take-move(2, 1) repeat(1);
take-move(2, -1) repeat(1);
take-move(1, -2) repeat(1);
take-move(-1, -2) repeat(1);
take-move(-2, -1) repeat(1);
take-move(-2, 1) repeat(1);
take-move(-1, 2) repeat(1);"
            .into(),
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(1, 2)));
    assert!(result.movement_squares.contains(&Square::new(2, 4)));
    assert!(result.movement_squares.contains(&Square::new(3, 6)));
    assert!(!result.movement_squares.contains(&Square::new(4, 8)));
}

#[test]
fn test_variant_piece_definitions_are_registered() {
    let definitions = all_default_definitions();
    let find = |id: &str| definitions.iter().find(|def| def.id == id).unwrap();

    assert_eq!(find("amazon").score, 13);
    assert_eq!(find("guhang").score, 25);
    assert_eq!(find("tempest-queen").score, 10);
    assert_eq!(find("tempest-rook").score, 8);
    assert_eq!(find("tempest-bishop").score, 5);
    assert_eq!(find("tempest-knight").score, 5);
    assert_eq!(find("bouncing-bishop").score, 7);
    assert_eq!(find("bouncing-rook").score, 6);
    assert_eq!(find("bouncing-queen").score, 13);
    assert_eq!(find("bouncing-pawn-white").score, 2);
    assert_eq!(find("bouncing-pawn-black").score, 2);
    assert_eq!(find("nightrider").score, 5);
    assert_eq!(find("dozer-white").score, 2);
    assert_eq!(find("dozer-black").score, 2);
    assert_eq!(find("tempest-pawn-white").score, 1);
    assert_eq!(find("tempest-pawn-black").score, 1);
}

#[test]
fn test_guhang_executes_all_four_orthogonal_fan_chains() {
    let board = create_board(8);
    let def = guhang_definition();
    let piece = make_piece("g1", "white", "guhang", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("g1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    for square in [
        Square::new(4, 3),
        Square::new(4, 7),
        Square::new(4, 0),
        Square::new(2, 3),
        Square::new(2, 7),
        Square::new(2, 0),
        Square::new(3, 4),
        Square::new(7, 4),
        Square::new(0, 4),
        Square::new(3, 2),
        Square::new(7, 2),
        Square::new(0, 2),
    ] {
        assert!(result.movement_squares.contains(&square), "missing {square:?}");
    }
}

#[test]
fn test_nightrider_repeats_knight_leaps_until_blocked() {
    let mut board = create_board(8);
    let def = nightrider_definition();
    let piece = make_piece("nr1", "white", "nightrider", 0, 0);
    let mut pieces = HashMap::new();
    pieces.insert("nr1".into(), piece.clone());
    pieces.insert(
        "blocker".into(),
        make_piece("blocker", "white", "pawn-white", 4, 2),
    );
    board
        .squares
        .insert(Square::new(4, 2).to_id(), Some("blocker".into()));

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(2, 1)));
    assert!(!result.movement_squares.contains(&Square::new(4, 2)));
    assert!(!result.movement_squares.contains(&Square::new(6, 3)));
    assert!(result.movement_squares.contains(&Square::new(1, 2)));
    assert!(result.movement_squares.contains(&Square::new(2, 4)));
    assert!(result.movement_squares.contains(&Square::new(3, 6)));
}

#[test]
fn test_amazon_combines_queen_and_knight() {
    let board = create_board(8);
    let def = amazon_definition();
    let piece = make_piece("a1", "white", "amazon", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("a1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(3, 7)));
    assert!(result.movement_squares.contains(&Square::new(7, 7)));
    assert!(result.movement_squares.contains(&Square::new(5, 4)));
    assert!(result.movement_squares.contains(&Square::new(1, 2)));
}

#[test]
fn test_tempest_rook_steps_diagonal_then_rays_outward() {
    let board = create_board(8);
    let def = tempest_rook_definition();
    let piece = make_piece("tr1", "white", "tempest-rook", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("tr1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(4, 4)));
    assert!(result.movement_squares.contains(&Square::new(7, 4)));
    assert!(result.movement_squares.contains(&Square::new(4, 7)));
    assert!(result.movement_squares.contains(&Square::new(0, 2)));
    assert!(result.movement_squares.contains(&Square::new(2, 0)));
    assert!(!result.movement_squares.contains(&Square::new(3, 4)));
}

#[test]
fn test_tempest_knight_executes_diagonal_chain_and_three_step_jumps() {
    let board = create_board(8);
    let def = tempest_knight_definition();
    let piece = make_piece("tn1", "white", "tempest-knight", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("tn1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(4, 4)));
    assert!(result.movement_squares.contains(&Square::new(6, 5)));
    assert!(result.movement_squares.contains(&Square::new(5, 6)));
    assert!(result.movement_squares.contains(&Square::new(6, 3)));
    assert!(result.movement_squares.contains(&Square::new(3, 6)));
}

#[test]
fn test_bouncing_bishop_reflects_from_edges_in_ability_layer() {
    let board = create_board(8);
    let def = bouncing_bishop_definition();
    let piece = make_piece("bb1", "white", "bouncing-bishop", 3, 2);
    let mut pieces = HashMap::new();
    pieces.insert("bb1".into(), piece.clone());

    assert_eq!(def.move_options.len(), 2);
    assert_eq!(def.normal_move_option().unwrap().id, "normal");
    let bounce_option = def
        .move_options
        .iter()
        .find(|option| option.id == "bounce_move")
        .unwrap();
    assert_eq!(bounce_option.kind, MoveOptionKind::Ability);
    assert_eq!(bounce_option.cooldown.as_ref().unwrap().turns, 2);
    let bounce_layer = def
        .move_layers
        .iter()
        .find(|layer| layer.id == "bounce_move")
        .unwrap();

    let result = run_code(
        &bounce_layer.chessembly_code,
        &piece,
        &board,
        &pieces,
        &def,
    );
    assert!(result.movement_squares.contains(&Square::new(7, 6)));
    assert!(result.movement_squares.contains(&Square::new(6, 7)));
    assert!(result.movement_squares.contains(&Square::new(7, 2)));
    assert!(result.movement_squares.contains(&Square::new(0, 5)));
    assert!(result.movement_squares.contains(&Square::new(2, 7)));
}

#[test]
fn test_bouncing_rook_turns_at_edges() {
    let board = create_board(8);
    let def = bouncing_rook_definition();
    let piece = make_piece("br1", "white", "bouncing-rook", 3, 2);
    let mut pieces = HashMap::new();
    pieces.insert("br1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(7, 2)));
    assert!(result.movement_squares.contains(&Square::new(7, 7)));
    assert!(result.movement_squares.contains(&Square::new(7, 0)));
    assert!(result.movement_squares.contains(&Square::new(3, 7)));
    assert!(result.movement_squares.contains(&Square::new(0, 7)));
}

#[test]
fn test_bouncing_queen_combines_bishop_and_rook_bounces() {
    let board = create_board(8);
    let def = bouncing_queen_definition();
    let piece = make_piece("bq1", "white", "bouncing-queen", 3, 2);
    let mut pieces = HashMap::new();
    pieces.insert("bq1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(7, 6)));
    assert!(result.movement_squares.contains(&Square::new(6, 7)));
    assert!(result.movement_squares.contains(&Square::new(7, 2)));
    assert!(result.movement_squares.contains(&Square::new(7, 7)));
}

#[test]
fn test_piece_on_remains_a_general_condition() {
    let mut board = create_board(8);
    let def = rook_definition();
    let piece = make_piece("r1", "white", "rook", 3, 3);
    let target = make_piece("target", "white", "pawn-white", 4, 3);
    board
        .squares
        .insert(Square::new(4, 3).to_id(), Some(target.id.clone()));
    let pieces = HashMap::from([
        (piece.id.clone(), piece.clone()),
        (target.id.clone(), target),
    ]);

    let result = run_code(
        "piece-on(pawn-white, 1, 0) move(0, 1);",
        &piece,
        &board,
        &pieces,
        &def,
    );
    assert!(result.movement_squares.contains(&Square::new(3, 4)));
}

#[test]
fn test_white_pawn_movement_and_attack_separated() {
    let board = create_board(8);
    let def = pawn_white_definition();
    let piece = make_piece("pw1", "white", "pawn-white", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("pw1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(3, 4)));
    assert!(result.attack_squares.contains(&Square::new(4, 4)));
    assert!(result.attack_squares.contains(&Square::new(2, 4)));
    assert!(!result.attack_squares.contains(&Square::new(3, 4)));
}

#[test]
fn test_white_pawn_attack_captures_enemy() {
    let mut board = create_board(8);
    let def = pawn_white_definition();
    let piece = make_piece("pw1", "white", "pawn-white", 3, 3);
    let enemy = make_piece("e1", "black", "pawn-black", 4, 4);
    board
        .squares
        .insert(enemy.current_square.unwrap().to_id(), Some("e1".into()));
    let mut pieces = HashMap::new();
    pieces.insert("pw1".into(), piece.clone());
    pieces.insert("e1".into(), enemy);

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.attack_squares.contains(&Square::new(4, 4)));
}

#[test]
fn test_king_moves() {
    let board = create_board(8);
    let def = king_definition();
    let piece = make_piece("k1", "white", "king", 4, 4);
    let mut pieces = HashMap::new();
    pieces.insert("k1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert_eq!(result.movement_squares.len(), 8);
}

#[test]
fn test_scope_block_y_move() {
    let board = create_board(8);
    let piece = make_piece("t1", "white", "test", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("t1".into(), piece.clone());

    let def = PieceDefinition {
        id: "test".into(),
        name: "Test".into(),
        score: 1,
        chessembly_code: "move(0, 1) { move(1, 1) } move(-1, 1);".into(),
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(3, 4)));
    assert!(result.movement_squares.contains(&Square::new(4, 5)));
    assert!(result.movement_squares.contains(&Square::new(2, 5)));
}

#[test]
fn test_catch_scans_and_marks_threatened_squares() {
    let mut board = create_board(8);
    let piece = make_piece("c1", "white", "cannon", 0, 0);
    let enemy = make_piece("e1", "black", "rook", 3, 0);
    board
        .squares
        .insert(enemy.current_square.unwrap().to_id(), Some("e1".into()));
    let mut pieces = HashMap::new();
    pieces.insert("c1".into(), piece.clone());
    pieces.insert("e1".into(), enemy);

    let def = PieceDefinition {
        id: "cannon".into(),
        name: "Cannon".into(),
        score: 4,
        chessembly_code: "catch(1, 0) repeat(1);".into(),
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.attack_squares.contains(&Square::new(1, 0)));
    assert!(result.attack_squares.contains(&Square::new(2, 0)));
    assert!(result.attack_squares.contains(&Square::new(3, 0)));
    assert!(result.attack_squares.contains(&Square::new(4, 0)));
}
