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
    Wizard,
    WizardCadet,
    WizardRook,
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
    pub categories: Vec<InteractionTag>,
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
        "wizard-rook" => InteractionProfile {
            categories: vec![InteractionTag::Wizard, InteractionTag::WizardRook],
            ..InteractionProfile::default()
        },
        "wizard-king" | "wizard-queen" | "wizard-knight" | "wizard-bishop" => {
            InteractionProfile {
                categories: vec![InteractionTag::Wizard],
                ..InteractionProfile::default()
            }
        },
        "wizard-cadet" | "wizard-cadet-black" => InteractionProfile {
            categories: vec![InteractionTag::Wizard, InteractionTag::WizardCadet],
            ..InteractionProfile::default()
        },
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

/// Semantic membership lives in the existing interaction registry, never names.
pub fn has_category(piece: &Piece, category: InteractionTag) -> bool {
    profile_for_piece_type(&piece.type_id)
        .categories
        .contains(&category)
}

pub fn friendly_wizards<'a>(state: &'a GameState, actor: &Piece) -> Vec<&'a Piece> {
    state
        .pieces
        .values()
        .filter(|target| {
            target.owner == actor.owner
                && target.is_on_board()
                && target.current_square.is_some_and(|square| {
                    state.board.is_in_bounds(&square)
                        && state.board.get_piece_at_layer(&square, target.layer) == Some(&target.id)
                })
                && has_category(target, InteractionTag::Wizard)
        })
        .collect()
}

pub fn surrounded_by_cadets(state: &GameState, actor: &Piece) -> bool {
    let Some(origin) = actor.current_square else {
        return false;
    };
    [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| {
        let square = Square::new(origin.file + dx, origin.rank + dy);
        state.board.is_in_bounds(&square)
            && state
                .board
                .get_piece_at(&square)
                .and_then(|id| state.pieces.get(id))
                .is_some_and(|piece| {
                    piece.is_on_board()
                        && piece.current_square == Some(square)
                        && piece.owner == actor.owner
                        && has_category(piece, InteractionTag::WizardCadet)
                })
    })
}

pub const KNIGHT_OFFSETS: [(i32, i32); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

pub const QUEEN_DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

const DIAGONAL_DIRECTIONS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Every in-bounds ordinary Knight destination must contain a Wizard.
/// Ownership is intentionally irrelevant to this formation condition.
pub fn knight_destinations_are_wizards(state: &GameState, actor: &Piece) -> bool {
    let Some(origin) = actor.current_square else {
        return false;
    };
    KNIGHT_OFFSETS
        .into_iter()
        .map(|(dx, dy)| Square::new(origin.file + dx, origin.rank + dy))
        .filter(|square| state.board.is_in_bounds(square))
        .all(|square| {
            state
                .board
                .get_piece_at_layer(&square, actor.layer)
                .and_then(|id| state.pieces.get(id))
                .is_some_and(|piece| has_category(piece, InteractionTag::Wizard))
        })
}

/// First occupied square on each Queen ray, restricted to enemy pieces.
pub fn wizard_knight_catch_targets<'a>(state: &'a GameState, actor: &Piece) -> Vec<&'a Piece> {
    let Some(origin) = actor.current_square else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for (dx, dy) in QUEEN_DIRECTIONS {
        let mut square = Square::new(origin.file + dx, origin.rank + dy);
        while state.board.is_in_bounds(&square) {
            if let Some(target) = state
                .board
                .get_piece_at_layer(&square, actor.layer)
                .and_then(|id| state.pieces.get(id))
            {
                if target.owner != actor.owner {
                    targets.push(target);
                }
                break;
            }
            square = Square::new(square.file + dx, square.rank + dy);
        }
    }
    targets
}

/// All four orthogonal neighbors must be cadets. Ownership is irrelevant;
/// the requirement is the exact semantic piece category.
pub fn surrounded_by_any_cadets(state: &GameState, actor: &Piece) -> bool {
    let Some(origin) = actor.current_square else {
        return false;
    };
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .all(|(dx, dy)| {
            let square = Square::new(origin.file + dx, origin.rank + dy);
            state.board.is_in_bounds(&square)
                && state
                    .board
                    .get_piece_at_layer(&square, actor.layer)
                    .and_then(|id| state.pieces.get(id))
                    .is_some_and(|piece| has_category(piece, InteractionTag::WizardCadet))
        })
}

/// On each diagonal, jump the first occupied square and inspect only the
/// immediately following square for an enemy capture.
pub fn wizard_bishop_jump_targets<'a>(state: &'a GameState, actor: &Piece) -> Vec<&'a Piece> {
    let Some(origin) = actor.current_square else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for (dx, dy) in DIAGONAL_DIRECTIONS {
        let mut square = Square::new(origin.file + dx, origin.rank + dy);
        while state.board.is_in_bounds(&square)
            && state.board.is_empty_at_layer(&square, actor.layer)
        {
            square = Square::new(square.file + dx, square.rank + dy);
        }
        if !state.board.is_in_bounds(&square) {
            continue;
        }
        let behind = Square::new(square.file + dx, square.rank + dy);
        let Some(target) = state
            .board
            .is_in_bounds(&behind)
            .then(|| state.board.get_piece_at_layer(&behind, actor.layer))
            .flatten()
            .and_then(|id| state.pieces.get(id))
        else {
            continue;
        };
        if target.owner != actor.owner {
            targets.push(target);
        }
    }
    targets
}


/// The first two occupied cells on each orthogonal ray must each contain one
/// friendly wizard rook. Both physical layers count as obstacles.
pub fn alekhines_gun_targets<'a>(state: &'a GameState, actor: &Piece) -> Vec<&'a Piece> {
    let Some(origin) = actor.current_square else { return Vec::new() };
    if !state.board.is_in_bounds(&origin)
        || state.board.get_piece_at_layer(&origin, actor.layer) != Some(&actor.id) {
        return Vec::new();
    }
    let mut targets = Vec::new();
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut square = Square::new(origin.file + dx, origin.rank + dy);
        let mut rooks = 0;
        while state.board.is_in_bounds(&square) {
            let occupants: Vec<_> = [crate::types::PieceLayer::Ground, crate::types::PieceLayer::Air]
                .into_iter().filter_map(|layer| state.board.get_piece_at_layer(&square, layer)).collect();
            if !occupants.is_empty() {
                if occupants.len() != 1 { break; }
                let Some(piece) = state.pieces.get(occupants[0]) else { break; };
                if !piece.is_on_board() || piece.current_square != Some(square)
                    || state.board.get_piece_at_layer(&square, piece.layer) != Some(&piece.id)
                    || piece.owner != actor.owner || !has_category(piece, InteractionTag::WizardRook) {
                    break;
                }
                rooks += 1;
                if rooks == 2 { targets.push(piece); break; }
            }
            square = Square::new(square.file + dx, square.rank + dy);
        }
    }
    targets
}


/// Side cadets are adjacent on the caster's physical layer. The passenger is
/// immediately behind the caster, relative to its owner's forward direction.
pub fn transfer_circle_passengers<'a>(state: &'a GameState, actor: &Piece) -> Vec<&'a Piece> {
    let Some(origin) = actor.current_square else {
        return Vec::new();
    };
    let occupant = |square: Square, layer| {
        state
            .board
            .is_in_bounds(&square)
            .then(|| state.board.get_piece_at_layer(&square, layer))
            .flatten()
            .and_then(|id| state.pieces.get(id))
            .filter(|piece| {
                piece.is_on_board()
                    && piece.current_square == Some(square)
                    && piece.layer == layer
                    && piece.owner == actor.owner
            })
    };
    if occupant(origin, actor.layer).map(|piece| &piece.id) != Some(&actor.id)
        || ![-1, 1].into_iter().all(|dx| {
            occupant(Square::new(origin.file + dx, origin.rank), actor.layer)
                .is_some_and(|piece| has_category(piece, InteractionTag::WizardCadet))
        })
    {
        return Vec::new();
    }
    let behind = Square::new(
        origin.file,
        origin.rank - crate::rules::player_forward_direction(&actor.owner),
    );
    [
        crate::types::PieceLayer::Ground,
        crate::types::PieceLayer::Air,
    ]
    .into_iter()
    .filter_map(|layer| occupant(behind, layer))
    .collect()
}
