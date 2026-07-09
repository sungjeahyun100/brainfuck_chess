use crate::types::*;

use super::{PieceMoveContext, PieceMovePattern};

pub fn generate(ctx: PieceMoveContext<'_>) -> PieceMovePattern {
    match ctx.definition.id.as_str() {
        "king" => step_pattern(
            ctx,
            &[
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ],
        ),
        "queen" => slide_pattern(
            ctx,
            &[
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ],
        ),
        "rook" => slide_pattern(ctx, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
        "bishop" => slide_pattern(ctx, &[(1, 1), (1, -1), (-1, 1), (-1, -1)]),
        "knight" => step_pattern(
            ctx,
            &[
                (1, 2),
                (2, 1),
                (2, -1),
                (1, -2),
                (-1, -2),
                (-2, -1),
                (-2, 1),
                (-1, 2),
            ],
        ),
        _ => PieceMovePattern::default(),
    }
}

fn step_pattern(ctx: PieceMoveContext<'_>, directions: &[(i32, i32)]) -> PieceMovePattern {
    let mut pattern = PieceMovePattern::default();
    let Some(from) = ctx.piece.current_square else {
        return pattern;
    };

    for (dx, dy) in directions {
        let target = Square::new(from.file + dx, from.rank + dy);
        push_take_move_target(&mut pattern, ctx.state, ctx.player_id, target);
    }

    pattern
}

fn slide_pattern(ctx: PieceMoveContext<'_>, directions: &[(i32, i32)]) -> PieceMovePattern {
    let mut pattern = PieceMovePattern::default();
    let Some(from) = ctx.piece.current_square else {
        return pattern;
    };

    for (dx, dy) in directions {
        let mut target = Square::new(from.file + dx, from.rank + dy);
        while ctx.state.board.is_in_bounds(&target) {
            if !push_take_move_target(&mut pattern, ctx.state, ctx.player_id, target) {
                break;
            }
            target = Square::new(target.file + dx, target.rank + dy);
        }
    }

    pattern
}

fn push_take_move_target(
    pattern: &mut PieceMovePattern,
    state: &GameState,
    player_id: &PlayerId,
    target: Square,
) -> bool {
    if !state.board.is_in_bounds(&target) {
        return false;
    }

    match state.board.get_piece_at(&target) {
        None => {
            push_unique(&mut pattern.movement_squares, target);
            push_unique(&mut pattern.attack_squares, target);
            true
        }
        Some(piece_id) => {
            let is_enemy = state
                .pieces
                .get(piece_id)
                .is_some_and(|piece| piece.owner != *player_id);
            if is_enemy {
                push_unique(&mut pattern.movement_squares, target);
                push_unique(&mut pattern.attack_squares, target);
            }
            false
        }
    }
}

fn push_unique(squares: &mut Vec<Square>, square: Square) {
    if !squares.contains(&square) {
        squares.push(square);
    }
}
