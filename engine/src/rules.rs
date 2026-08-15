use crate::types::*;
use std::collections::HashMap;

/// Create an empty n×n board with all squares initialized to empty.
pub fn create_board(size: i32) -> Board {
    assert!(size >= 8, "Board size must be at least 8");
    let mut squares = HashMap::new();
    for rank in 0..size {
        for file in 0..size {
            let sq = Square::new(file, rank);
            squares.insert(sq.to_id(), None);
        }
    }
    Board { size, squares }
}

/// Deck score limit: scoreLimit = n*n - 25
pub fn calculate_score_limit(board_size: i32) -> u32 {
    (board_size * board_size - 25).max(0) as u32
}

/// Sum the scores of all non-king pieces in a deck.
pub fn calculate_deck_score(
    deck: &Deck,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> u32 {
    let all_piece_ids = deck.starting_pieces.iter().chain(deck.pocket_pieces.iter());
    all_piece_ids
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| !def.is_king)
        .map(|def| def.score)
        .sum()
}

/// Validate a deck:
/// - Exactly one King in starting pieces
/// - No King in pocket
/// - Total score ≤ score limit
/// - At least one piece besides King
pub fn validate_deck(
    deck: &Deck,
    board_size: i32,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> ValidationResult {
    let mut errors = Vec::new();

    let score_limit = calculate_score_limit(board_size);

    // Count kings in starting pieces
    let king_count = deck
        .starting_pieces
        .iter()
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| def.is_king)
        .count();

    if king_count == 0 {
        errors.push("덱에 King이 없습니다. 기본 진영에 King 1개를 포함해야 합니다.".into());
    } else if king_count > 1 {
        errors.push(format!(
            "King이 {}개입니다. King은 1개만 허용됩니다.",
            king_count
        ));
    }

    // No king in pocket
    let pocket_king_count = deck
        .pocket_pieces
        .iter()
        .filter_map(|pid| pieces.get(pid))
        .filter_map(|p| definitions.get(&p.type_id))
        .filter(|def| def.is_king)
        .count();

    if pocket_king_count > 0 {
        errors.push("King은 포켓에 넣을 수 없습니다.".into());
    }

    // Score check
    let total_score = calculate_deck_score(deck, pieces, definitions);
    if total_score > score_limit {
        errors.push(format!(
            "덱 점수 {}점이 상한 {}점을 초과합니다.",
            total_score, score_limit
        ));
    }

    for piece in deck
        .starting_pieces
        .iter()
        .filter_map(|piece_id| pieces.get(piece_id))
    {
        if let (Some(square), Some(definition)) =
            (piece.current_square, definitions.get(&piece.type_id))
        {
            if !can_piece_be_placed_at_start(definition, &deck.player_id, square, board_size) {
                errors.push(format!(
                    "{}은(는) {} 초기 배치 구역에만 배치할 수 있습니다: {}",
                    definition.name,
                    match definition.deployment_zone {
                        DeploymentZone::Front => "앞줄",
                        DeploymentZone::Back => "뒷줄",
                    },
                    square.to_id()
                ));
            }
        }
    }

    if errors.is_empty() {
        ValidationResult::ok()
    } else {
        ValidationResult::fail(errors)
    }
}

/// Return the base zone squares for a player.
/// Boards smaller than 10 use two ranks; boards 10 or larger use three.
/// White starts from rank 0, while Black starts from the opposite edge.
pub fn get_base_zone_squares(player_id: &PlayerId, board_size: i32) -> Vec<Square> {
    let zone_depth = if board_size >= 10 { 3 } else { 2 };
    let ranks: Vec<i32> = if player_id == "white" {
        (0..zone_depth).collect()
    } else {
        (board_size - zone_depth..board_size).collect()
    };
    let mut squares = Vec::new();
    for rank in ranks {
        for file in 0..board_size {
            squares.push(Square::new(file, rank));
        }
    }
    squares
}

/// Rank in the setup zone closest to the opposing side (the ordinary pawn rank).
/// This is derived from the setup zone and player orientation rather than board
/// coordinates so it also follows wider setup zones on larger boards.
pub fn get_frontmost_base_rank(player_id: &PlayerId, board_size: i32) -> Option<i32> {
    let forward = if player_id == "white" { 1 } else { -1 };
    get_base_zone_squares(player_id, board_size)
        .into_iter()
        .map(|square| square.rank)
        .max_by_key(|rank| rank * forward)
}

/// Apply the piece definition's setup-zone contract to one starting square.
pub fn can_piece_be_placed_at_start(
    definition: &PieceDefinition,
    player_id: &PlayerId,
    square: Square,
    board_size: i32,
) -> bool {
    let base_zone = get_base_zone_squares(player_id, board_size);
    if !base_zone.contains(&square) {
        return false;
    }
    let is_front = get_frontmost_base_rank(player_id, board_size) == Some(square.rank);
    matches!(
        (definition.deployment_zone, is_front),
        (DeploymentZone::Front, true) | (DeploymentZone::Back, false)
    )
}

/// Direction in which a player's pieces advance toward the opposing side.
pub fn player_forward_direction(player_id: &PlayerId) -> i32 {
    if player_id == "white" {
        1
    } else {
        -1
    }
}
