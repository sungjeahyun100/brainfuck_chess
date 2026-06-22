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

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_piece(id: &str, owner: &str, type_id: &str, file: i32, rank: i32) -> Piece {
    Piece {
        id: id.into(),
        owner: owner.into(),
        type_id: type_id.into(),
        current_square: Some(Square::new(file, rank)),
        in_pocket: false,
        captured: false,
        move_stack: 1,
        has_moved: false,
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
    let ctx = ExecutionContext {
        board,
        piece,
        piece_definition: def,
        all_definitions: &HashMap::new(),
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

// ─── Wazir ────────────────────────────────────────────────────────────────────

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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    let moves = sorted(result.movement_squares.clone());
    // All 4 neighbours should be reachable from (3,3)
    assert!(moves.contains(&Square::new(4, 3)), "right");
    assert!(moves.contains(&Square::new(2, 3)), "left");
    assert!(moves.contains(&Square::new(3, 4)), "up");
    assert!(moves.contains(&Square::new(3, 2)), "down");
    assert_eq!(moves.len(), 4);
    assert!(
        result.attack_squares.contains(&Square::new(4, 3)),
        "right attack"
    );
    assert!(
        result.attack_squares.contains(&Square::new(2, 3)),
        "left attack"
    );
    assert!(
        result.attack_squares.contains(&Square::new(3, 4)),
        "up attack"
    );
    assert!(
        result.attack_squares.contains(&Square::new(3, 2)),
        "down attack"
    );
}

// ─── Rook slide ───────────────────────────────────────────────────────────────

#[test]
fn test_rook_slide_open_board() {
    let board = create_board(8);
    let def = rook_definition();
    let piece = make_piece("r1", "white", "rook", 0, 0);
    let mut pieces = HashMap::new();
    pieces.insert("r1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    // From (0,0), rook can reach rank 0 files 1-7 and file 0 ranks 1-7
    let moves = result.movement_squares;
    assert_eq!(moves.len(), 14, "Rook from corner should have 14 moves");
}

#[test]
fn test_rook_blocked_by_friendly() {
    let mut board = create_board(8);
    let piece = make_piece("r1", "white", "rook", 0, 0);
    let blocker = make_piece("p1", "white", "pawn-white", 3, 0);

    // Place blocker on board
    board
        .squares
        .insert(blocker.current_square.unwrap().to_id(), Some("p1".into()));

    let mut pieces = HashMap::new();
    pieces.insert("r1".into(), piece.clone());
    pieces.insert("p1".into(), blocker);

    let def = rook_definition();
    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    // Horizontal: (1,0) and (2,0) reachable, (3,0) blocked (friendly), beyond not reachable
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

    // (3,0) should appear as an attack square (enemy capture)
    assert!(result.attack_squares.contains(&Square::new(3, 0)));
    // (4,0) and beyond should NOT be reachable
    assert!(!result.movement_squares.contains(&Square::new(4, 0)));
    assert!(!result.attack_squares.contains(&Square::new(4, 0)));
}

// ─── Knightrider ─────────────────────────────────────────────────────────────

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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    // From (0,0), the (1,2) direction gives: (1,2), (2,4), (3,6)
    assert!(result.movement_squares.contains(&Square::new(1, 2)));
    assert!(result.movement_squares.contains(&Square::new(2, 4)));
    assert!(result.movement_squares.contains(&Square::new(3, 6)));
    // (4,8) is out of bounds
    assert!(!result.movement_squares.contains(&Square::new(4, 8)));
}

// ─── Variant defaults ───────────────────────────────────────────────────────

#[test]
fn test_variant_piece_definitions_are_registered() {
    let definitions = all_default_definitions();
    let find = |id: &str| definitions.iter().find(|def| def.id == id).unwrap();

    assert_eq!(find("amazon").score, 13);
    assert_eq!(find("tempest-rook").score, 8);
    assert_eq!(find("bouncing-bishop").score, 7);
}

#[test]
fn test_amazon_combines_queen_and_knight() {
    let board = create_board(8);
    let def = amazon_definition();
    let piece = make_piece("a1", "white", "amazon", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("a1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    assert!(
        result.movement_squares.contains(&Square::new(3, 7)),
        "queen file slide"
    );
    assert!(
        result.movement_squares.contains(&Square::new(7, 7)),
        "queen diagonal slide"
    );
    assert!(
        result.movement_squares.contains(&Square::new(5, 4)),
        "knight jump"
    );
    assert!(
        result.movement_squares.contains(&Square::new(1, 2)),
        "knight jump"
    );
}

#[test]
fn test_tempest_rook_steps_diagonal_then_rays_outward() {
    let board = create_board(8);
    let def = tempest_rook_definition();
    let piece = make_piece("tr1", "white", "tempest-rook", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("tr1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    assert!(
        result.movement_squares.contains(&Square::new(4, 4)),
        "diagonal step"
    );
    assert!(
        result.movement_squares.contains(&Square::new(7, 4)),
        "east ray from diagonal"
    );
    assert!(
        result.movement_squares.contains(&Square::new(4, 7)),
        "north ray from diagonal"
    );
    assert!(
        result.movement_squares.contains(&Square::new(0, 2)),
        "west ray from diagonal"
    );
    assert!(
        result.movement_squares.contains(&Square::new(2, 0)),
        "south ray from diagonal"
    );
    assert!(
        !result.movement_squares.contains(&Square::new(3, 4)),
        "no direct rook move"
    );
}

#[test]
fn test_bouncing_bishop_reflects_from_edges() {
    let board = create_board(8);
    let def = bouncing_bishop_definition();
    let piece = make_piece("bb1", "white", "bouncing-bishop", 3, 2);
    let mut pieces = HashMap::new();
    pieces.insert("bb1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    assert!(
        result.movement_squares.contains(&Square::new(7, 6)),
        "initial northeast diagonal"
    );
    assert!(
        result.movement_squares.contains(&Square::new(6, 7)),
        "reflection from right edge"
    );
    assert!(
        result.movement_squares.contains(&Square::new(7, 2)),
        "reflection from bottom edge"
    );
    assert!(
        result.movement_squares.contains(&Square::new(0, 5)),
        "initial northwest diagonal"
    );
    assert!(
        result.movement_squares.contains(&Square::new(2, 7)),
        "reflection from left edge"
    );
}

// ─── Pawn ──────────────────────────────────────────────────────────────────

#[test]
fn test_white_pawn_movement_and_attack_separated() {
    let board = create_board(8);
    let def = pawn_white_definition();
    // Place pawn at (3, 3) — not in starting rank
    let piece = make_piece("pw1", "white", "pawn-white", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("pw1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    // Forward move: (3,4) should be a movement square
    assert!(
        result.movement_squares.contains(&Square::new(3, 4)),
        "forward move"
    );
    // Diagonal attacks should be recorded as threatened squares even when empty.
    assert!(
        result.attack_squares.contains(&Square::new(4, 4)),
        "right diagonal threat"
    );
    assert!(
        result.attack_squares.contains(&Square::new(2, 4)),
        "left diagonal threat"
    );
    assert!(
        !result.attack_squares.contains(&Square::new(3, 4)),
        "forward should not be an attack square"
    );

    // 2-step: from rank 3 (not rank 1), observe(0,1) is truthy but the 2-step chain starts
    // from (3,3): observe(0,1) checks (3,4) which is empty → true, then move(0,2) tries (3,5).
    // This is intentional: the 2-step is allowed whenever the path is clear.
    // (The rule engine enforces "only from starting rank" separately via hasMoved.)
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

    // (4,4) has an enemy → should be an attack square
    assert!(
        result.attack_squares.contains(&Square::new(4, 4)),
        "diagonal capture"
    );
}

// ─── King ─────────────────────────────────────────────────────────────────────

#[test]
fn test_king_moves() {
    let board = create_board(8);
    let def = king_definition();
    let piece = make_piece("k1", "white", "king", 4, 4);
    let mut pieces = HashMap::new();
    pieces.insert("k1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert_eq!(
        result.movement_squares.len(),
        8,
        "King from center has 8 moves"
    );
}

// ─── Scope block ─────────────────────────────────────────────────────────────

#[test]
fn test_scope_block_y_move() {
    // Y-shaped movement: move(0, 1) { move(1, 1) } move(-1, 1)
    // From (3, 3): activates (3,4), (4,5), (2,4)
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    // From (3,3): move(0,1) → (3,4); block saves (3,4), move(1,1) → (4,5), restores (3,4);
    // then move(-1,1) from (3,4) → (2,5)
    assert!(
        result.movement_squares.contains(&Square::new(3, 4)),
        "(3,4)"
    );
    assert!(
        result.movement_squares.contains(&Square::new(4, 5)),
        "(4,5)"
    );
    assert!(
        result.movement_squares.contains(&Square::new(2, 5)),
        "(2,5)"
    );
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
    };

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);

    assert!(
        result.attack_squares.contains(&Square::new(1, 0)),
        "empty scan square 1"
    );
    assert!(
        result.attack_squares.contains(&Square::new(2, 0)),
        "empty scan square 2"
    );
    assert!(
        result.attack_squares.contains(&Square::new(3, 0)),
        "enemy capture square"
    );
    assert!(
        result.attack_squares.contains(&Square::new(4, 0)),
        "catch chain continues scanning after contact"
    );
}
