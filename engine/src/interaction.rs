use crate::types::{GameState, Piece, PieceId, Square};

/// Resolve occupied squares in the eight-cell Moore neighborhood around a
/// board piece. Ability code owns policy (ally/enemy/royal); interaction owns
/// board proximity and occupancy detection.
pub fn neighboring_pieces<'a>(game_state: &'a GameState, piece: &Piece) -> Vec<&'a Piece> {
    let Some(origin) = piece.current_square else {
        return Vec::new();
    };
    let mut neighbors = Vec::new();
    for rank_offset in -1..=1 {
        for file_offset in -1..=1 {
            if file_offset == 0 && rank_offset == 0 {
                continue;
            }
            let square = Square::new(origin.file + file_offset, origin.rank + rank_offset);
            let Some(target_id) = game_state.board.get_piece_at(&square) else {
                continue;
            };
            if let Some(target) = game_state.pieces.get(target_id) {
                neighbors.push(target);
            }
        }
    }
    neighbors
}

/// Semantic interaction categories. Movement code asks whether a target blocks
/// one of its tags instead of checking concrete piece type ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionTag {
    Bouncing,
}

/// Geometry owned by the moving piece. The obstacle only says that it blocks a
/// movement tag; the mover decides how a blocked ray is reflected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BounceGeometry {
    Orthogonal,
    Diagonal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InteractionProfile {
    pub movement_tags: Vec<InteractionTag>,
    pub blocks: Vec<InteractionTag>,
    pub bounce_geometries: Vec<BounceGeometry>,
    pub required_move_option: Option<&'static str>,
}

impl InteractionProfile {
    fn moves_with(&self, tag: InteractionTag) -> bool {
        self.movement_tags.contains(&tag)
    }

    fn blocks(&self, tag: InteractionTag) -> bool {
        self.blocks.contains(&tag)
    }

    fn is_enabled_for_option(&self, move_option_id: &str) -> bool {
        self.required_move_option
            .is_none_or(|required| required == move_option_id)
    }
}

/// First built-in interaction registry. Keeping the mapping here makes the
/// interaction engine independent from Chessembly and keeps concrete piece ids
/// out of the interpreter. Custom-piece and terrain registries can provide the
/// same profiles later without changing the resolver.
pub fn profile_for_piece_type(type_id: &str) -> InteractionProfile {
    match type_id {
        "bouncing-pawn-white" | "bouncing-pawn-black" => InteractionProfile {
            blocks: vec![InteractionTag::Bouncing],
            ..InteractionProfile::default()
        },
        "bouncing-rook" => InteractionProfile {
            movement_tags: vec![InteractionTag::Bouncing],
            bounce_geometries: vec![BounceGeometry::Orthogonal],
            ..InteractionProfile::default()
        },
        "bouncing-bishop" => InteractionProfile {
            movement_tags: vec![InteractionTag::Bouncing],
            bounce_geometries: vec![BounceGeometry::Diagonal],
            ..InteractionProfile::default()
        },
        "bouncing-queen" => InteractionProfile {
            movement_tags: vec![InteractionTag::Bouncing],
            bounce_geometries: vec![BounceGeometry::Orthogonal, BounceGeometry::Diagonal],
            ..InteractionProfile::default()
        },
        _ => InteractionProfile::default(),
    }
}

/// True when an occupied destination is a wall for the mover's active
/// interaction profile. Legal-move generation uses this to suppress an ordinary
/// Chessembly capture/move that would otherwise bypass the interaction layer.
pub fn destination_is_blocked_by_interaction(
    game_state: &GameState,
    piece: &Piece,
    to: Square,
    move_option_id: &str,
) -> bool {
    let mover_profile = profile_for_piece_type(&piece.type_id);
    if mover_profile.movement_tags.is_empty()
        || !mover_profile.is_enabled_for_option(move_option_id)
    {
        return false;
    }

    let Some(target_id) = game_state.board.get_piece_at(&to) else {
        return false;
    };
    let Some(target) = game_state.pieces.get(target_id) else {
        return false;
    };
    let target_profile = profile_for_piece_type(&target.type_id);

    mover_profile
        .movement_tags
        .iter()
        .copied()
        .any(|tag| target_profile.blocks(tag))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionMoveCandidate {
    pub to: Square,
    pub captured_piece_id: Option<PieceId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InteractionResult {
    pub moves: Vec<InteractionMoveCandidate>,
    pub attack_squares: Vec<Square>,
}

/// Resolve movement produced specifically by board-object interactions.
///
/// The ordinary Chessembly result is still responsible for the mover's normal
/// path. This resolver only contributes destinations created after a collision.
/// For the first interaction, a Bouncing Pawn blocks the `Bouncing` movement
/// tag and the moving piece reflects from the square immediately before it.
pub fn resolve_piece_interactions(
    game_state: &GameState,
    piece: &Piece,
    move_option_id: &str,
) -> InteractionResult {
    let profile = profile_for_piece_type(&piece.type_id);
    if !profile.moves_with(InteractionTag::Bouncing)
        || !profile.is_enabled_for_option(move_option_id)
        || !piece.is_on_board()
    {
        return InteractionResult::default();
    }

    let mut result = InteractionResult::default();
    for geometry in profile.bounce_geometries {
        for &incoming in incoming_directions(geometry) {
            resolve_first_bouncing_wall(game_state, piece, incoming, geometry, &mut result);
        }
    }
    result
}

fn resolve_first_bouncing_wall(
    game_state: &GameState,
    piece: &Piece,
    incoming: (i32, i32),
    geometry: BounceGeometry,
    result: &mut InteractionResult,
) {
    let Some(origin) = piece.current_square else {
        return;
    };

    let mut cursor = origin;
    loop {
        let next = Square::new(cursor.file + incoming.0, cursor.rank + incoming.1);
        if !game_state.board.is_in_bounds(&next) {
            return;
        }

        let Some(blocker_id) = game_state.board.get_piece_at(&next) else {
            cursor = next;
            continue;
        };
        let Some(blocker) = game_state.pieces.get(blocker_id) else {
            return;
        };
        if !profile_for_piece_type(&blocker.type_id).blocks(InteractionTag::Bouncing) {
            return;
        }

        for outgoing in reflected_directions(incoming, geometry) {
            trace_reflected_ray(game_state, piece, cursor, outgoing, result);
        }
        return;
    }
}

fn trace_reflected_ray(
    game_state: &GameState,
    piece: &Piece,
    start: Square,
    direction: (i32, i32),
    result: &mut InteractionResult,
) {
    let mut cursor = start;
    loop {
        let next = Square::new(cursor.file + direction.0, cursor.rank + direction.1);
        if !game_state.board.is_in_bounds(&next) {
            return;
        }

        match game_state.board.get_piece_at(&next) {
            None => {
                push_attack_square(result, next);
                push_move_candidate(result, next, None);
                cursor = next;
            }
            Some(target_id) => {
                let Some(target) = game_state.pieces.get(target_id) else {
                    return;
                };
                if profile_for_piece_type(&target.type_id).blocks(InteractionTag::Bouncing) {
                    return;
                }
                if target.owner != piece.owner {
                    push_attack_square(result, next);
                    push_move_candidate(result, next, Some(target_id.clone()));
                }
                return;
            }
        }
    }
}

fn push_attack_square(result: &mut InteractionResult, square: Square) {
    if !result.attack_squares.contains(&square) {
        result.attack_squares.push(square);
    }
}

fn push_move_candidate(
    result: &mut InteractionResult,
    to: Square,
    captured_piece_id: Option<PieceId>,
) {
    if result
        .moves
        .iter()
        .any(|candidate| candidate.to == to && candidate.captured_piece_id == captured_piece_id)
    {
        return;
    }
    result.moves.push(InteractionMoveCandidate {
        to,
        captured_piece_id,
    });
}

fn incoming_directions(geometry: BounceGeometry) -> &'static [(i32, i32)] {
    match geometry {
        BounceGeometry::Orthogonal => &[(1, 0), (-1, 0), (0, 1), (0, -1)],
        BounceGeometry::Diagonal => &[(1, 1), (-1, 1), (1, -1), (-1, -1)],
    }
}

fn reflected_directions(incoming: (i32, i32), geometry: BounceGeometry) -> [(i32, i32); 2] {
    match geometry {
        BounceGeometry::Orthogonal if incoming.0 == 0 => [(-1, 0), (1, 0)],
        BounceGeometry::Orthogonal => [(0, -1), (0, 1)],
        BounceGeometry::Diagonal => [(-incoming.0, incoming.1), (incoming.0, -incoming.1)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orthogonal_reflection_turns_left_and_right() {
        assert_eq!(
            reflected_directions((0, 1), BounceGeometry::Orthogonal),
            [(-1, 0), (1, 0)]
        );
        assert_eq!(
            reflected_directions((1, 0), BounceGeometry::Orthogonal),
            [(0, -1), (0, 1)]
        );
    }

    #[test]
    fn diagonal_reflection_flips_one_axis() {
        assert_eq!(
            reflected_directions((1, 1), BounceGeometry::Diagonal),
            [(-1, 1), (1, -1)]
        );
    }
}
