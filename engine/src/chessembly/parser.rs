use super::ast::{Chain, Expr, Program};

/// Parse a Chessembly program string into an AST.
///
/// Tokenisation rules (from the official docs):
/// - Case-sensitive
/// - Spaces are significant: identifiers and `(` must be separated properly
/// - Opening `(` is attached to the preceding identifier (no space)
/// - `;` terminates a chain
/// - `{` and `}` are individual tokens that must be surrounded by spaces
/// - Invalid tokens are converted to `End`
pub fn parse(source: &str) -> Program {
    let tokens = tokenise(source);
    let mut pos = 0;
    let mut program: Program = Vec::new();

    while pos < tokens.len() {
        let (chain, new_pos) = parse_chain(&tokens, pos);
        pos = new_pos;
        // Skip empty chains (consecutive semicolons)
        if !chain.is_empty() {
            program.push(chain);
        }
    }

    program
}

// ─── Tokeniser ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// `identifier` (no `(` immediately after)
    Ident(String),
    /// `identifier(` — function call start
    Call(String),
    /// Integer literal (may be negative)
    Int(i32),
    Comma,
    RParen,
    LBrace,
    RBrace,
    Semi,
}

fn tokenise(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            ';' => {
                tokens.push(Token::Semi);
                i += 1;
            }
            '{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '-' if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() => {
                // Negative number
                i += 1;
                let (n, new_i) = read_int(&chars, i);
                tokens.push(Token::Int(-n));
                i = new_i;
            }
            c if c.is_ascii_digit() => {
                let (n, new_i) = read_int(&chars, i);
                tokens.push(Token::Int(n));
                i = new_i;
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let (name, new_i) = read_ident(&chars, i);
                i = new_i;
                // Check for immediately following `(`
                if i < chars.len() && chars[i] == '(' {
                    tokens.push(Token::Call(name));
                    i += 1; // consume `(`
                } else {
                    tokens.push(Token::Ident(name));
                }
            }
            _ => {
                // Unknown character — skip
                i += 1;
            }
        }
    }

    tokens
}

fn read_int(chars: &[char], mut i: usize) -> (i32, usize) {
    let mut s = String::new();
    while i < chars.len() && chars[i].is_ascii_digit() {
        s.push(chars[i]);
        i += 1;
    }
    (s.parse().unwrap_or(0), i)
}

fn read_ident(chars: &[char], mut i: usize) -> (String, usize) {
    let mut s = String::new();
    while i < chars.len()
        && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '-')
    {
        s.push(chars[i]);
        i += 1;
    }
    (s, i)
}

// ─── Parser ─────────────────────────────────────────────────────────────────

/// Parse one chain (until `;` or EOF). Returns (chain, new_pos).
fn parse_chain(tokens: &[Token], mut pos: usize) -> (Chain, usize) {
    let mut chain: Chain = Vec::new();

    while pos < tokens.len() {
        match &tokens[pos] {
            Token::Semi => {
                pos += 1;
                break;
            }
            Token::RBrace => {
                // Let the block parser handle this
                break;
            }
            _ => {
                if let Some((expr, new_pos)) = parse_expr(tokens, pos) {
                    pos = new_pos;
                    if expr == Expr::End {
                        // End terminates the current chain
                        break;
                    }
                    chain.push(expr);
                } else {
                    pos += 1;
                }
            }
        }
    }

    (chain, pos)
}

/// Parse a single expression starting at `pos`. Returns (expr, new_pos) or None.
fn parse_expr(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    match tokens.get(pos)? {
        // ── Block ────────────────────────────────────────────────────────────
        Token::LBrace => {
            let (block_exprs, new_pos) = parse_block(tokens, pos + 1);
            Some((Expr::Block(block_exprs), new_pos))
        }

        // ── Keywords / bare identifiers ──────────────────────────────────────
        Token::Ident(name) => {
            let expr = match name.as_str() {
                "do" => Expr::Do,
                "while" => Expr::While,
                "not" => Expr::Not,
                "true" => Expr::True,
                "false" => Expr::False,
                "check" => Expr::Check,
                "end" => Expr::End,
                _ => Expr::End, // unknown bare identifier
            };
            Some((expr, pos + 1))
        }

        // ── Function calls ───────────────────────────────────────────────────
        Token::Call(name) => {
            match name.as_str() {
                "move" => parse_xy(tokens, pos + 1, |x, y| Expr::Move(x, y)),
                "take" => parse_xy(tokens, pos + 1, |x, y| Expr::Take(x, y)),
                "take-move" => parse_xy(tokens, pos + 1, |x, y| Expr::TakeMove(x, y)),
                "catch" => parse_xy(tokens, pos + 1, |x, y| Expr::Catch(x, y)),
                "jump" => parse_xy(tokens, pos + 1, |x, y| Expr::Jump(x, y)),
                "shift" => parse_xy(tokens, pos + 1, |x, y| Expr::Shift(x, y)),
                "anchor" => parse_xy(tokens, pos + 1, |x, y| Expr::Anchor(x, y)),
                "absolute-x" => parse_int_arg(tokens, pos + 1, Expr::AbsoluteX),
                "absolute-y" => parse_int_arg(tokens, pos + 1, Expr::AbsoluteY),
                "observe" => parse_xy(tokens, pos + 1, |x, y| Expr::Observe(x, y)),
                "peek" => parse_xy(tokens, pos + 1, |x, y| Expr::Peek(x, y)),
                "enemy" => parse_xy(tokens, pos + 1, |x, y| Expr::Enemy(x, y)),
                "friendly" => parse_xy(tokens, pos + 1, |x, y| Expr::Friendly(x, y)),
                "bound" => parse_xy(tokens, pos + 1, |x, y| Expr::Bound(x, y)),
                "edge" => parse_xy(tokens, pos + 1, |x, y| Expr::Edge(x, y)),
                "corner" => parse_xy(tokens, pos + 1, |x, y| Expr::Corner(x, y)),
                "danger" => parse_xy(tokens, pos + 1, |x, y| Expr::Danger(x, y)),
                "repeat" => parse_usize_arg(tokens, pos + 1, Expr::Repeat),
                "jmp" => parse_usize_arg(tokens, pos + 1, Expr::Jmp),
                "jne" => parse_usize_arg(tokens, pos + 1, Expr::Jne),
                "label" => parse_usize_arg(tokens, pos + 1, Expr::Label),
                "read" => parse_usize_arg(tokens, pos + 1, Expr::Read),
                "read-and" => parse_usize_arg(tokens, pos + 1, Expr::ReadAnd),
                "read-or" => parse_usize_arg(tokens, pos + 1, Expr::ReadOr),
                "read-xor" => parse_usize_arg(tokens, pos + 1, Expr::ReadXor),
                "write" => parse_usize_arg(tokens, pos + 1, Expr::Write),
                "piece" => parse_string_arg(tokens, pos + 1, Expr::Piece),
                "if-state" => parse_state_condition(tokens, pos + 1),
                "transition" => parse_string_arg(tokens, pos + 1, Expr::Transition),
                "set-state" => parse_set_state(tokens, pos + 1),
                "piece-on" => parse_piece_on(tokens, pos + 1),
                _ => Some((Expr::End, pos + 1)), // unknown call
            }
        }

        _ => Some((Expr::End, pos + 1)),
    }
}

/// Parse a `{ ... }` block. `pos` points to the first token after `{`.
fn parse_block(tokens: &[Token], mut pos: usize) -> (Vec<Expr>, usize) {
    let mut exprs = Vec::new();
    while pos < tokens.len() {
        match &tokens[pos] {
            Token::RBrace => {
                pos += 1;
                break;
            }
            Token::Semi => {
                // semicolons inside a block are ignored as chain separators
                // (the block itself is one expression inside a chain)
                pos += 1;
            }
            _ => {
                if let Some((expr, new_pos)) = parse_expr(tokens, pos) {
                    pos = new_pos;
                    exprs.push(expr);
                } else {
                    pos += 1;
                }
            }
        }
    }
    (exprs, pos)
}

// ─── Argument parsers ────────────────────────────────────────────────────────

/// Expect `int, int )` and build an `Expr` from the two ints.
fn parse_xy<F>(tokens: &[Token], pos: usize, f: F) -> Option<(Expr, usize)>
where
    F: Fn(i32, i32) -> Expr,
{
    let (x, pos) = expect_int(tokens, pos)?;
    let pos = expect_comma(tokens, pos)?;
    let (y, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((f(x, y), pos))
}

fn parse_int_arg<F>(tokens: &[Token], pos: usize, f: F) -> Option<(Expr, usize)>
where
    F: Fn(i32) -> Expr,
{
    let (n, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((f(n), pos))
}

fn parse_usize_arg<F>(tokens: &[Token], pos: usize, f: F) -> Option<(Expr, usize)>
where
    F: Fn(usize) -> Expr,
{
    let (n, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((f(n.max(0) as usize), pos))
}

fn parse_string_arg<F>(tokens: &[Token], pos: usize, f: F) -> Option<(Expr, usize)>
where
    F: Fn(String) -> Expr,
{
    let (s, pos) = expect_ident_or_call_name(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((f(s), pos))
}

/// `if-state(key, n)` — key is an identifier
fn parse_state_condition(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let (key, pos) = expect_ident_or_call_name(tokens, pos)?;
    let pos = expect_comma(tokens, pos)?;
    let (n, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((Expr::IfState(key, n), pos))
}

/// `set-state` with no args (clears last tag) or `set-state(key, n)`
fn parse_set_state(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    // The `(` was already consumed by the tokeniser as part of Call token.
    // If next token is RParen → no-arg form
    if matches!(tokens.get(pos), Some(Token::RParen)) {
        return Some((Expr::SetState(None), pos + 1));
    }
    let (key, pos) = expect_ident_or_call_name(tokens, pos)?;
    let pos = expect_comma(tokens, pos)?;
    let (n, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((Expr::SetState(Some((key, n))), pos))
}

/// `piece-on(piece_name, dx, dy)`
fn parse_piece_on(tokens: &[Token], pos: usize) -> Option<(Expr, usize)> {
    let (name, pos) = expect_ident_or_call_name(tokens, pos)?;
    let pos = expect_comma(tokens, pos)?;
    let (dx, pos) = expect_int(tokens, pos)?;
    let pos = expect_comma(tokens, pos)?;
    let (dy, pos) = expect_int(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Some((Expr::PieceOn(name, dx, dy), pos))
}

// ─── Token helpers ───────────────────────────────────────────────────────────

fn expect_int(tokens: &[Token], pos: usize) -> Option<(i32, usize)> {
    if let Some(Token::Int(n)) = tokens.get(pos) {
        Some((*n, pos + 1))
    } else {
        None
    }
}

fn expect_comma(tokens: &[Token], pos: usize) -> Option<usize> {
    if matches!(tokens.get(pos), Some(Token::Comma)) {
        Some(pos + 1)
    } else {
        None
    }
}

fn expect_rparen(tokens: &[Token], pos: usize) -> Option<usize> {
    if matches!(tokens.get(pos), Some(Token::RParen)) {
        Some(pos + 1)
    } else {
        None
    }
}

/// Accept either a bare `Ident` or the name part of a `Call` token as a string.
fn expect_ident_or_call_name(tokens: &[Token], pos: usize) -> Option<(String, usize)> {
    match tokens.get(pos) {
        Some(Token::Ident(s)) => Some((s.clone(), pos + 1)),
        // A Call token means the next token opened a `(`, but here we need just the name.
        // This handles cases like `piece-on(rook, 0, 0)` where `rook` itself might be
        // tokenised as a bare Ident.
        Some(Token::Call(s)) => Some((s.clone(), pos + 1)),
        _ => None,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chessembly::ast::Expr;

    #[test]
    fn test_parse_wazir() {
        let prog = parse("take-move(1, 0); take-move(-1, 0); take-move(0, 1); take-move(0, -1);");
        assert_eq!(prog.len(), 4);
        assert_eq!(prog[0], vec![Expr::TakeMove(1, 0)]);
        assert_eq!(prog[3], vec![Expr::TakeMove(0, -1)]);
    }

    #[test]
    fn test_parse_rook_slide() {
        let prog = parse("take-move(1, 0) repeat(1);");
        assert_eq!(prog.len(), 1);
        assert_eq!(prog[0], vec![Expr::TakeMove(1, 0), Expr::Repeat(1)]);
    }

    #[test]
    fn test_parse_block() {
        let prog = parse("move(0, 1) { move(1, 1) } move(-1, 1);");
        assert_eq!(prog.len(), 1);
        assert_eq!(
            prog[0],
            vec![
                Expr::Move(0, 1),
                Expr::Block(vec![Expr::Move(1, 1)]),
                Expr::Move(-1, 1),
            ]
        );
    }

    #[test]
    fn test_parse_do_while() {
        let prog = parse("do take-move(1, 1) while");
        // No semicolon: whole thing is one chain
        assert_eq!(prog.len(), 1);
        assert_eq!(
            prog[0],
            vec![Expr::Do, Expr::TakeMove(1, 1), Expr::While]
        );
    }
}
