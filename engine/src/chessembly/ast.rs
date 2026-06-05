/// Abstract Syntax Tree for Chessembly DSL.
///
/// One `Program` is a sequence of `Chain`s separated by `;`.
/// Each `Chain` is a sequence of `Expr`s executed in order.
/// The anchor (reference position) resets to the piece's square at the start
/// of every new chain.

// ─── Top level ──────────────────────────────────────────────────────────────

/// A complete Chessembly program: zero or more expression chains.
pub type Program = Vec<Chain>;

/// An expression chain ending with `;`. Expressions are evaluated left-to-right.
pub type Chain = Vec<Expr>;

// ─── Expressions ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // ── Movement expressions ────────────────────────────────────────────────
    /// Move to empty squares only.
    Move(i32, i32),
    /// Capture enemy squares only (piece stays in place for catch variant).
    Take(i32, i32),
    /// Move or capture.
    TakeMove(i32, i32),
    /// Remote capture — piece stays in place.
    Catch(i32, i32),
    /// Jump move (pairs with a preceding Take to form take-jump).
    Jump(i32, i32),
    /// Swap with another piece (friendly/enemy depending on target).
    Shift(i32, i32),

    // ── Position expressions ─────────────────────────────────────────────────
    /// Move anchor without activating any square.
    Anchor(i32, i32),
    /// Set anchor to absolute file coordinate.
    AbsoluteX(i32),
    /// Set anchor to absolute rank coordinate.
    AbsoluteY(i32),

    // ── Conditional expressions ──────────────────────────────────────────────
    /// True if (dx,dy) from anchor is empty.
    Observe(i32, i32),
    /// True if (dx,dy) is empty; also moves anchor if true.
    Peek(i32, i32),
    /// True if (dx,dy) contains an enemy piece.
    Enemy(i32, i32),
    /// True if (dx,dy) contains a friendly piece.
    Friendly(i32, i32),
    /// True if (dx,dy) contains the named piece type.
    PieceOn(String, i32, i32),
    /// True if (dx,dy) is threatened by any enemy piece.
    Danger(i32, i32),
    /// True if current player's king is in check (standard chess concept — rarely used in BFC).
    Check,
    /// True if (dx,dy) is out of board bounds.
    Bound(i32, i32),
    /// True if (dx,dy) is beyond the board edge.
    Edge(i32, i32),
    /// True if (dx,dy) is beyond the board corner.
    Corner(i32, i32),

    // ── State expressions ────────────────────────────────────────────────────
    /// True if this piece's type matches `name`.
    Piece(String),
    /// True if global state `key` equals `n`.
    IfState(String, i32),
    /// Tag subsequent activations with a piece-transition action.
    Transition(String),
    /// Tag subsequent activations with a set-state action.
    SetState(Option<(String, i32)>),

    // ── Control flow ─────────────────────────────────────────────────────────
    /// Repeat the previous `n` expressions while the last value was true.
    Repeat(usize),
    /// Loop body start marker.
    Do,
    /// Loop back to `Do` if last value was true.
    While,
    /// Jump to `label(n)` if last value was true.
    Jmp(usize),
    /// Jump to `label(n)` if last value was false.
    Jne(usize),
    /// Loop / jump destination marker.
    Label(usize),
    /// Invert the last value.
    Not,
    /// Force true regardless of last value.
    True,
    /// Force false regardless of last value.
    False,

    // ── Bit registers ────────────────────────────────────────────────────────
    Read(usize),
    ReadAnd(usize),
    ReadOr(usize),
    ReadXor(usize),
    Write(usize),

    // ── Block ────────────────────────────────────────────────────────────────
    /// `{ ... }` scope block: isolates false termination and restores anchor.
    Block(Vec<Expr>),

    /// Sentinel — used when parse error converts an invalid token to `end`.
    End,
}
