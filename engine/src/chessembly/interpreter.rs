//! Chessembly interpreter.
//!
//! Given a parsed `Program` and an `ExecutionContext`, the interpreter
//! runs each expression chain and accumulates the set of activated squares
//! into two separate lists: `movement_squares` and `attack_squares`.
//!
//! Key concepts:
//! - **Anchor**: the current reference position that move expressions act relative to.
//!   It starts at the piece's position and advances as expressions execute.
//! - **Chain**: resets the anchor to the piece's position at the start.
//! - **Termination**: a non-exceptional `false` return stops the chain.
//! - **Block `{ }`**: isolates `false` and restores the anchor on exit.
//! - **`do…while`**: loops while the expression inside returns `true`.

use std::collections::{HashMap, HashSet};

use crate::types::{Board, ChessemblyResult, Piece, PieceDefinition, PlayerId, Square};

use super::ast::{Expr, Program};

// ─── Context ────────────────────────────────────────────────────────────────

pub struct ExecutionContext<'a> {
    pub board: &'a Board,
    pub piece: &'a Piece,
    pub piece_definition: &'a PieceDefinition,
    /// All piece definitions keyed by type_id.
    pub all_definitions: &'a HashMap<String, PieceDefinition>,
    /// All pieces keyed by piece_id.
    pub all_pieces: &'a HashMap<String, Piece>,
    pub player: PlayerId,
    /// Global game state variables (e.g. castling rights stored as integers).
    pub global_state: &'a HashMap<String, i32>,
    /// Attack maps: player_id → set of attacked square ids.
    /// Used by `danger()` expression. May be empty if not yet computed.
    pub attack_maps: &'a HashMap<PlayerId, HashSet<String>>,
}

// ─── Interpreter ─────────────────────────────────────────────────────────────

pub fn run(program: &Program, ctx: &ExecutionContext) -> ChessemblyResult {
    let piece_pos = ctx.piece.current_square.unwrap_or(Square::new(0, 0));
    let mut result = ChessemblyResult::default();

    for chain in program {
        let mut state = ChainState::new(piece_pos);
        run_chain(chain, 0, ctx, &mut state, &mut result);
    }

    // Deduplicate
    result.movement_squares.dedup();
    result.attack_squares.dedup();
    result
}

// ─── Internal state ──────────────────────────────────────────────────────────

/// Mutable state during a single chain execution.
struct ChainState {
    /// Reference position: resets to piece position at start of each chain.
    anchor: Square,
    /// Bit registers (0..15) for `read`/`write` instructions.
    bits: [bool; 16],
    /// Tag to attach to the next activated square: piece transition.
    transition_tag: Option<String>,
    /// Tag to attach to the next activated square: set-state action.
    set_state_tag: Option<(String, i32)>,
    /// Last value returned by an expression.
    last_value: bool,
    /// Whether the previous expression was a `take` (for `take-jump` pairing).
    last_was_take: bool,
    /// If Some(sq), the preceding `take` targeted this square but it should NOT
    /// be activated yet (waiting for `jump` to decide).
    pending_take_square: Option<Square>,
    pending_take_is_attack: bool,
}

impl ChainState {
    fn new(anchor: Square) -> Self {
        Self {
            anchor,
            bits: [false; 16],
            transition_tag: None,
            set_state_tag: None,
            last_value: true,
            last_was_take: false,
            pending_take_square: None,
            pending_take_is_attack: false,
        }
    }
}

// ─── Chain runner ─────────────────────────────────────────────────────────────

/// Returned by an expression to indicate whether the chain should continue.
#[derive(Debug, PartialEq)]
enum ExprResult {
    True,
    False,
}

impl ExprResult {
    fn is_true(&self) -> bool {
        *self == ExprResult::True
    }
}

/// Run a slice of expressions starting at `start_idx`.
/// Returns `false` if the chain was terminated by a `false` result from a normal expression.
fn run_chain(
    chain: &[Expr],
    start_idx: usize,
    ctx: &ExecutionContext,
    state: &mut ChainState,
    result: &mut ChessemblyResult,
) -> bool {
    let mut i = start_idx;
    while i < chain.len() {
        let expr = &chain[i];
        let (res, consumed) = eval_expr(expr, chain, i, ctx, state, result);
        i += consumed;
        state.last_value = res.is_true();

        // `false` from a non-exceptional expression terminates the chain
        if !res.is_true() {
            // Flush pending take that was never paired with jump
            flush_pending_take(state, result);
            return false;
        }
    }
    flush_pending_take(state, result);
    true
}

/// Evaluate one expression. Returns (result, tokens_consumed_beyond_the_expression_itself).
/// `i` is the index of the current expression in `chain`.
fn eval_expr(
    expr: &Expr,
    chain: &[Expr],
    i: usize,
    ctx: &ExecutionContext,
    state: &mut ChainState,
    result: &mut ChessemblyResult,
) -> (ExprResult, usize) {
    // Clear take pending state before processing a non-take/jump expression
    // (unless the current expr is `jump`)
    match expr {
        Expr::Jump(_, _) => {} // don't clear — we need to pair with pending take
        _ => {
            if !matches!(expr, Expr::Take(_, _)) {
                flush_pending_take(state, result);
            }
        }
    }

    match expr {
        // ── Movement expressions ─────────────────────────────────────────────

        Expr::Move(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) || !ctx.board.is_empty(&target) {
                return (ExprResult::False, 1);
            }
            // Empty square: activate as movement, advance anchor
            activate_movement(target, state, result);
            state.anchor = target;
            (ExprResult::True, 1)
        }

        Expr::Take(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            let occupant = ctx.board.get_piece_at(&target);
            match occupant {
                None => {
                    // Empty square: advance anchor but DON'T activate
                    // (take only activates enemy squares)
                    state.anchor = target;
                    // Mark pending take so `jump` can pair with it
                    state.last_was_take = true;
                    state.pending_take_square = Some(target);
                    state.pending_take_is_attack = false;
                    (ExprResult::True, 1)
                }
                Some(piece_id) => {
                    let is_enemy = is_enemy_piece(piece_id, &ctx.player, ctx.all_pieces);
                    if is_enemy {
                        // Activate as attack square; anchor advances; false (chain stops after capture)
                        state.pending_take_square = None;
                        state.last_was_take = false;
                        activate_attack(target, state, result);
                        state.anchor = target;
                        (ExprResult::True, 1) // take alone continues (paired with jump or standalone)
                    } else {
                        (ExprResult::False, 1) // friendly/wall
                    }
                }
            }
        }

        Expr::TakeMove(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            let occupant = ctx.board.get_piece_at(&target);
            match occupant {
                None => {
                    activate_movement(target, state, result);
                    state.anchor = target;
                    (ExprResult::True, 1)
                }
                Some(piece_id) => {
                    let is_enemy = is_enemy_piece(piece_id, &ctx.player, ctx.all_pieces);
                    if is_enemy {
                        // Activate as both movement and attack; return false to stop chain
                        activate_movement(target, state, result);
                        activate_attack(target, state, result);
                        state.anchor = target;
                        (ExprResult::False, 1)
                    } else {
                        (ExprResult::False, 1)
                    }
                }
            }
        }

        Expr::Catch(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            let occupant = ctx.board.get_piece_at(&target);
            match occupant {
                None => {
                    // Empty square: advance anchor only (no activation)
                    state.anchor = target;
                    (ExprResult::True, 1)
                }
                Some(piece_id) => {
                    let is_enemy = is_enemy_piece(piece_id, &ctx.player, ctx.all_pieces);
                    if is_enemy {
                        // Remote capture: activate attack but DO NOT move anchor (piece stays)
                        activate_attack(target, state, result);
                        state.anchor = target;
                        (ExprResult::True, 1)
                    } else {
                        (ExprResult::False, 1)
                    }
                }
            }
        }

        Expr::Jump(dx, dy) => {
            // `jump` pairs with a preceding `take` to form a take-jump.
            // The `take` targeted one square, `jump` specifies where the piece lands.
            let jump_target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !state.last_was_take || state.pending_take_square.is_none() {
                // No preceding take: jump alone is false
                return (ExprResult::False, 1);
            }
            // Clear pending take (it should NOT be activated on its own)
            state.pending_take_square = None;
            state.last_was_take = false;

            // Jump is like `move`: only activates empty squares
            if !ctx.board.is_in_bounds(&jump_target) || !ctx.board.is_empty(&jump_target) {
                return (ExprResult::False, 1);
            }
            activate_movement(jump_target, state, result);
            state.anchor = jump_target;
            (ExprResult::True, 1)
        }

        Expr::Shift(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            let occupant = ctx.board.get_piece_at(&target);
            match occupant {
                None => {
                    activate_movement(target, state, result);
                    state.anchor = target;
                    (ExprResult::True, 1)
                }
                Some(piece_id) => {
                    let is_friendly = !is_enemy_piece(piece_id, &ctx.player, ctx.all_pieces);
                    if is_friendly {
                        // Swap with friendly: activate
                        activate_movement(target, state, result);
                        state.anchor = target;
                        (ExprResult::True, 1)
                    } else {
                        // Enemy: activate as attack
                        activate_attack(target, state, result);
                        state.anchor = target;
                        (ExprResult::True, 1)
                    }
                }
            }
        }

        // ── Position expressions ─────────────────────────────────────────────

        Expr::Anchor(dx, dy) => {
            let new_anchor = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&new_anchor) {
                return (ExprResult::False, 1);
            }
            state.anchor = new_anchor;
            (ExprResult::True, 1)
        }

        Expr::AbsoluteX(n) => {
            state.anchor = Square::new(*n, state.anchor.rank);
            if !ctx.board.is_in_bounds(&state.anchor) {
                return (ExprResult::False, 1);
            }
            (ExprResult::True, 1)
        }

        Expr::AbsoluteY(n) => {
            state.anchor = Square::new(state.anchor.file, *n);
            if !ctx.board.is_in_bounds(&state.anchor) {
                return (ExprResult::False, 1);
            }
            (ExprResult::True, 1)
        }

        // ── Conditional expressions ───────────────────────────────────────────

        Expr::Observe(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            let empty = ctx.board.is_in_bounds(&target) && ctx.board.is_empty(&target);
            if empty { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::Peek(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if ctx.board.is_in_bounds(&target) && ctx.board.is_empty(&target) {
                state.anchor = target;
                (ExprResult::True, 1)
            } else {
                (ExprResult::False, 1)
            }
        }

        Expr::Enemy(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            match ctx.board.get_piece_at(&target) {
                Some(pid) if is_enemy_piece(pid, &ctx.player, ctx.all_pieces) => {
                    (ExprResult::True, 1)
                }
                _ => (ExprResult::False, 1),
            }
        }

        Expr::Friendly(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            match ctx.board.get_piece_at(&target) {
                Some(pid) if !is_enemy_piece(pid, &ctx.player, ctx.all_pieces) => {
                    (ExprResult::True, 1)
                }
                _ => (ExprResult::False, 1),
            }
        }

        Expr::PieceOn(piece_name, dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                return (ExprResult::False, 1);
            }
            let found = ctx
                .board
                .get_piece_at(&target)
                .and_then(|pid| ctx.all_pieces.get(pid))
                .and_then(|p| ctx.all_definitions.get(&p.type_id))
                .map(|def| &def.id == piece_name || def.name.to_lowercase() == *piece_name.to_lowercase())
                .unwrap_or(false);
            if found { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::Danger(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            let opponent = opponent_id(&ctx.player);
            let is_attacked = ctx
                .attack_maps
                .get(&opponent)
                .map(|map| map.contains(&target.to_id()))
                .unwrap_or(false);
            if is_attacked { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::Check => {
            // In Brainfuck Chess there is no check, so this always returns false.
            (ExprResult::False, 1)
        }

        Expr::Bound(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            if !ctx.board.is_in_bounds(&target) {
                (ExprResult::True, 1)
            } else {
                (ExprResult::False, 1)
            }
        }

        Expr::Edge(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            let on_edge = target.file < 0
                || target.file >= ctx.board.size
                || target.rank < 0
                || target.rank >= ctx.board.size;
            if on_edge { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::Corner(dx, dy) => {
            let target = Square::new(state.anchor.file + dx, state.anchor.rank + dy);
            let in_corner = (target.file < 0 || target.file >= ctx.board.size)
                && (target.rank < 0 || target.rank >= ctx.board.size);
            if in_corner { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        // ── State expressions ────────────────────────────────────────────────

        Expr::Piece(name) => {
            let matches = ctx.piece_definition.id == *name
                || ctx.piece_definition.name.to_lowercase() == name.to_lowercase();
            if matches { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::IfState(key, n) => {
            let current = ctx.global_state.get(key).copied().unwrap_or(0);
            if current == *n { (ExprResult::True, 1) } else { (ExprResult::False, 1) }
        }

        Expr::Transition(name) => {
            state.transition_tag = Some(name.clone());
            (ExprResult::True, 1)
        }

        Expr::SetState(tag) => {
            state.set_state_tag = tag.clone();
            (ExprResult::True, 1)
        }

        // ── Control flow ─────────────────────────────────────────────────────

        Expr::Repeat(n) => {
            // `repeat(n)`: if last value is true, jump back n expressions in the chain
            if !state.last_value {
                // repeat propagates last value (which is false here) but doesn't terminate chain
                return (ExprResult::False, 1);
            }
            // Jump back n positions from the current position
            // `i` is the index of `repeat` itself, so the last expression is at `i - 1`.
            // We want to re-run the block of `n` expressions before repeat.
            // We skip consuming more tokens here; the caller (run_chain) needs special handling.
            // Since we can't jump in a simple index loop, we signal this via a special value.
            // Instead, we implement repeat by recursively calling run_chain on the sub-slice.
            // This is the simplest correct approach.
            let start = if i >= *n { i - n } else { 0 };
            // The repeat target slice is chain[start..i] (the n exprs before repeat)
            let sub_chain = &chain[start..i];
            // Run the sub-chain repeatedly until it returns false
            loop {
                let prev_anchor = state.anchor;
                let continued = run_chain(sub_chain, 0, ctx, state, result);
                if !continued {
                    break;
                }
                // Safety: if anchor didn't change, break to avoid infinite loop
                if state.anchor == prev_anchor {
                    break;
                }
            }
            // After repeat, the chain is done (we consumed the remainder by looping)
            // Signal false to stop the outer chain from continuing past this point
            // (repeat is the last meaningful expression in a chain line)
            (ExprResult::False, 1)
        }

        Expr::Do => {
            // `do` is a loop start marker; it doesn't do anything itself.
            // The actual looping is handled by `while`.
            (ExprResult::True, 1)
        }

        Expr::While => {
            // `while` is an exceptional expression: doesn't terminate chain on false.
            // If last_value is true → jump back to the matching `do`.
            if state.last_value {
                // Find the matching `do` by scanning backwards from current position `i`
                if let Some(do_idx) = find_matching_do(chain, i) {
                    // Re-run the body (do_idx+1 .. i) in a loop
                    let body = &chain[do_idx + 1..i];
                    loop {
                        let prev_anchor = state.anchor;
                        let prev_bits = state.bits;
                        let continued = run_chain(body, 0, ctx, state, result);
                        if !continued {
                            break;
                        }
                        if state.anchor == prev_anchor && state.bits == prev_bits {
                            break; // safety against infinite loop
                        }
                        if !state.last_value {
                            break;
                        }
                    }
                }
            }
            // `while` always returns true (exceptional)
            (ExprResult::True, 1)
        }

        Expr::Label(_) => {
            // Just a marker; passes through last value
            if state.last_value {
                (ExprResult::True, 1)
            } else {
                (ExprResult::True, 1) // label is exceptional: always true
            }
        }

        Expr::Jmp(n) => {
            // Jump to label(n) if last value was true
            if state.last_value {
                if let Some(label_idx) = find_label(chain, *n) {
                    // Skip to label position (labels are handled inline)
                    let skip = label_idx.saturating_sub(i);
                    return (ExprResult::True, skip + 1);
                }
            }
            (ExprResult::True, 1)
        }

        Expr::Jne(n) => {
            // Jump to label(n) if last value was false
            if !state.last_value {
                if let Some(label_idx) = find_label(chain, *n) {
                    let skip = label_idx.saturating_sub(i);
                    return (ExprResult::True, skip + 1);
                }
            }
            (ExprResult::True, 1)
        }

        Expr::Not => {
            let new_val = !state.last_value;
            state.last_value = new_val;
            if new_val { (ExprResult::True, 1) } else { (ExprResult::True, 1) }
            // `not` is exceptional: never terminates chain
        }

        Expr::True => {
            state.last_value = true;
            (ExprResult::True, 1)
        }

        Expr::False => {
            state.last_value = false;
            (ExprResult::True, 1) // exceptional: doesn't terminate chain
        }

        // ── Bit registers ────────────────────────────────────────────────────

        Expr::Read(n) => {
            let v = state.bits.get(*n).copied().unwrap_or(false);
            state.last_value = v;
            (if v { ExprResult::True } else { ExprResult::True }, 1) // exceptional
        }

        Expr::ReadAnd(n) => {
            let bit = state.bits.get(*n).copied().unwrap_or(false);
            let v = bit && state.last_value;
            state.last_value = v;
            (ExprResult::True, 1)
        }

        Expr::ReadOr(n) => {
            let bit = state.bits.get(*n).copied().unwrap_or(false);
            let v = bit || state.last_value;
            state.last_value = v;
            (ExprResult::True, 1)
        }

        Expr::ReadXor(n) => {
            let bit = state.bits.get(*n).copied().unwrap_or(false);
            let v = bit ^ state.last_value;
            state.last_value = v;
            (ExprResult::True, 1)
        }

        Expr::Write(n) => {
            if *n < 16 {
                state.bits[*n] = state.last_value;
            }
            (ExprResult::True, 1)
        }

        // ── Block ─────────────────────────────────────────────────────────────

        Expr::Block(inner) => {
            let saved_anchor = state.anchor;
            // Run inner expressions; ignore false termination (isolated)
            run_chain(inner, 0, ctx, state, result);
            // Restore anchor regardless of outcome
            state.anchor = saved_anchor;
            // Block itself returns the last value of its interior (as per spec)
            (ExprResult::True, 1) // block is not a chain terminator itself
        }

        Expr::End => {
            (ExprResult::False, 1)
        }
    }
}

// ─── Activation helpers ──────────────────────────────────────────────────────

fn activate_movement(sq: Square, _state: &ChainState, result: &mut ChessemblyResult) {
    if !result.movement_squares.contains(&sq) {
        result.movement_squares.push(sq);
    }
}

fn activate_attack(sq: Square, _state: &ChainState, result: &mut ChessemblyResult) {
    if !result.attack_squares.contains(&sq) {
        result.attack_squares.push(sq);
    }
}

fn flush_pending_take(state: &mut ChainState, result: &mut ChessemblyResult) {
    if let Some(sq) = state.pending_take_square.take() {
        if state.pending_take_is_attack {
            activate_attack(sq, state, result);
        }
        // If not marked as attack, it was an empty-square take — not activated.
    }
    state.last_was_take = false;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn is_enemy_piece(
    piece_id: &str,
    current_player: &PlayerId,
    all_pieces: &HashMap<String, Piece>,
) -> bool {
    all_pieces
        .get(piece_id)
        .map(|p| p.owner != *current_player)
        .unwrap_or(false)
}

fn opponent_id(player: &PlayerId) -> PlayerId {
    if player == "white" {
        "black".to_string()
    } else {
        "white".to_string()
    }
}

fn find_matching_do(chain: &[Expr], while_idx: usize) -> Option<usize> {
    // Scan backwards from `while_idx` for the nearest `do`
    for j in (0..while_idx).rev() {
        if chain[j] == Expr::Do {
            return Some(j);
        }
    }
    None
}

fn find_label(chain: &[Expr], n: usize) -> Option<usize> {
    chain
        .iter()
        .position(|e| matches!(e, Expr::Label(label_n) if *label_n == n))
}
