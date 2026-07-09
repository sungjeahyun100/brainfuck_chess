use crate::types::*;

pub(crate) fn is_pawn_type(type_id: &str) -> bool {
    matches!(
        type_id,
        "pawn-white" | "pawn-black" | "tempest-pawn-white" | "tempest-pawn-black"
    )
}

pub(crate) fn pawn_forward_dir(type_id: &str) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" => Some(-1),
        _ => None,
    }
}

pub fn rejects_illegal_double_step(
    piece: &Piece,
    from: Square,
    to: Square,
    board_size: i32,
) -> bool {
    let Some(dir) = pawn_forward_dir(&piece.type_id) else {
        return false;
    };
    let Some(start_rank) = pawn_start_rank(&piece.type_id, board_size) else {
        return false;
    };

    to.file == from.file
        && to.rank - from.rank == 2 * dir
        && (from.rank != start_rank || piece.has_moved)
}

pub(crate) fn pawn_start_rank(type_id: &str, board_size: i32) -> Option<i32> {
    match type_id {
        "pawn-white" | "tempest-pawn-white" => Some(1),
        "pawn-black" | "tempest-pawn-black" => Some(board_size - 2),
        _ => None,
    }
}
