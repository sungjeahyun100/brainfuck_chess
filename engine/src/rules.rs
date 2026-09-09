use crate::types::*;
use std::collections::HashMap;

pub const HIGH_GROUND_TERRAIN_ID: &str = "high-ground";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardMapDefinition {
    pub id: &'static str,
    pub board_size: i32,
    pub variant: BoardVariant,
}

pub const BOARD_MAPS: [BoardMapDefinition; 6] = [
    BoardMapDefinition {
        id: "standard-8x8",
        board_size: 8,
        variant: BoardVariant::Plain,
    },
    BoardMapDefinition {
        id: "standard-9x9",
        board_size: 9,
        variant: BoardVariant::Plain,
    },
    BoardMapDefinition {
        id: "standard-10x10",
        board_size: 10,
        variant: BoardVariant::Plain,
    },
    BoardMapDefinition {
        id: "standard-11x11",
        board_size: 11,
        variant: BoardVariant::Plain,
    },
    BoardMapDefinition {
        id: "standard-12x12",
        board_size: 12,
        variant: BoardVariant::Plain,
    },
    BoardMapDefinition {
        id: "central-high-ground-12x12",
        board_size: 12,
        variant: BoardVariant::CentralHighGround,
    },
];

pub fn board_map_definition(id: &str) -> Option<BoardMapDefinition> {
    BOARD_MAPS.iter().copied().find(|map| map.id == id)
}

pub fn standard_board_map_id(size: i32) -> Option<&'static str> {
    BOARD_MAPS
        .iter()
        .find(|map| map.board_size == size && map.variant == BoardVariant::Plain)
        .map(|map| map.id)
}

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
    Board {
        size,
        squares,
        air_squares: HashMap::new(),
        terrain: HashMap::new(),
    }
}

pub fn create_board_with_variant(size: i32, variant: BoardVariant) -> Result<Board, String> {
    let mut board = create_board(size);
    match variant {
        BoardVariant::Plain => {}
        BoardVariant::CentralHighGround => {
            if size != 12 {
                return Err("중앙 고지 보드는 12x12에서만 사용할 수 있습니다.".into());
            }
            for square in [
                Square::new(5, 5),
                Square::new(6, 5),
                Square::new(5, 6),
                Square::new(6, 6),
            ] {
                board.terrain.insert(
                    square.to_id(),
                    TerrainCell {
                        type_id: HIGH_GROUND_TERRAIN_ID.into(),
                    },
                );
            }
        }
    }
    Ok(board)
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
/// - Every Front deployment square is occupied (the full frontmost rank in Legacy)
pub fn validate_deck(
    deck: &Deck,
    board_size: i32,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
) -> ValidationResult {
    validate_deck_with_ruleset(deck, board_size, pieces, definitions, DeckRuleset::Legacy)
}

/// Validate using the selected format’s deployment and home geometry.
pub fn validate_deck_with_ruleset(
    deck: &Deck,
    board_size: i32,
    pieces: &HashMap<PieceId, Piece>,
    definitions: &HashMap<PieceTypeId, PieceDefinition>,
    ruleset: DeckRuleset,
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

    let front_zone = get_front_zone_squares_with_ruleset(&deck.player_id, board_size, ruleset);
    let occupied_front_squares = deck
        .starting_pieces
        .iter()
        .filter_map(|piece_id| pieces.get(piece_id))
        .filter(|piece| {
            ruleset == DeckRuleset::Legacy
                || definitions
                    .get(&piece.type_id)
                    .is_some_and(|definition| definition.deployment_zone == DeploymentZone::Front)
        })
        .filter_map(|piece| piece.current_square)
        .filter(|square| front_zone.contains(square))
        .collect::<std::collections::HashSet<_>>()
        .len();
    if occupied_front_squares != front_zone.len() {
        errors.push(format!(
            "덱의 앞줄은 모든 칸에 기물이 배치되어야 합니다. ({}/{})",
            occupied_front_squares,
            front_zone.len()
        ));
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
            if !can_piece_be_placed_at_start_with_ruleset(
                definition,
                &deck.player_id,
                square,
                board_size,
                ruleset,
            ) {
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
    get_base_zone_squares_with_ruleset(player_id, board_size, DeckRuleset::Legacy)
}

/// Shared home/base boundary for deployment, drops, abilities and ammunition.
/// Base is the union of Front and Back; runtime home rules do not restrict by piece type.
pub fn get_base_zone_squares_with_ruleset(
    player_id: &PlayerId,
    board_size: i32,
    ruleset: DeckRuleset,
) -> Vec<Square> {
    if ruleset == DeckRuleset::Standard {
        return standard_zone_squares(player_id, board_size, None);
    }
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

/// Authoritative Standard geometry, independent of map/terrain and piece metadata.
fn standard_zone_squares(
    player_id: &PlayerId,
    board_size: i32,
    zone: Option<DeploymentZone>,
) -> Vec<Square> {
    let center_width = if board_size % 2 == 0 { 2 } else { 3 };
    let center_start = (board_size - center_width) / 2;
    let center_end = center_start + center_width - 1;
    let mut squares = Vec::new();
    for depth in 0..2 {
        for file in center_start - 1..=center_end + 1 {
            let is_back = depth == 0 && (center_start..=center_end).contains(&file);
            let square_zone = if is_back {
                DeploymentZone::Back
            } else {
                DeploymentZone::Front
            };
            if zone.is_none_or(|zone| zone == square_zone) {
                let rank = if player_id == "white" {
                    depth
                } else {
                    board_size - 1 - depth
                };
                squares.push(Square::new(file, rank));
            }
        }
    }
    squares
}

/// Initial deployment area. Runtime Drop/home rules must use the full Base API.
pub fn get_deployment_zone_squares_with_ruleset(
    player_id: &PlayerId,
    board_size: i32,
    zone: DeploymentZone,
    ruleset: DeckRuleset,
) -> Vec<Square> {
    if ruleset == DeckRuleset::Standard {
        return standard_zone_squares(player_id, board_size, Some(zone));
    }
    let front_rank = get_frontmost_base_rank(player_id, board_size);
    get_base_zone_squares(player_id, board_size)
        .into_iter()
        .filter(|square| (Some(square.rank) == front_rank) == (zone == DeploymentZone::Front))
        .collect()
}

pub fn get_front_zone_squares_with_ruleset(
    player_id: &PlayerId,
    board_size: i32,
    ruleset: DeckRuleset,
) -> Vec<Square> {
    get_deployment_zone_squares_with_ruleset(player_id, board_size, DeploymentZone::Front, ruleset)
}

pub fn get_back_zone_squares_with_ruleset(
    player_id: &PlayerId,
    board_size: i32,
    ruleset: DeckRuleset,
) -> Vec<Square> {
    get_deployment_zone_squares_with_ruleset(player_id, board_size, DeploymentZone::Back, ruleset)
}

/// Rank in the setup zone closest to the opposing side (the ordinary pawn rank).
/// This is derived from the setup zone and player orientation rather than board
/// coordinates so it also follows wider setup zones on larger boards.
pub fn get_frontmost_base_rank(player_id: &PlayerId, board_size: i32) -> Option<i32> {
    get_frontmost_base_rank_with_ruleset(player_id, board_size, DeckRuleset::Legacy)
}

/// Returns only the foremost rank value, not the complete Front deployment area.
pub fn get_frontmost_base_rank_with_ruleset(
    player_id: &PlayerId,
    board_size: i32,
    ruleset: DeckRuleset,
) -> Option<i32> {
    let forward = if player_id == "white" { 1 } else { -1 };
    get_base_zone_squares_with_ruleset(player_id, board_size, ruleset)
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
    can_piece_be_placed_at_start_with_ruleset(
        definition,
        player_id,
        square,
        board_size,
        DeckRuleset::Legacy,
    )
}

pub fn can_piece_be_placed_at_start_with_ruleset(
    definition: &PieceDefinition,
    player_id: &PlayerId,
    square: Square,
    board_size: i32,
    ruleset: DeckRuleset,
) -> bool {
    get_deployment_zone_squares_with_ruleset(
        player_id,
        board_size,
        definition.deployment_zone,
        ruleset,
    )
    .contains(&square)
}

/// Direction in which a player's pieces advance toward the opposing side.
pub fn player_forward_direction(player_id: &PlayerId) -> i32 {
    if player_id == "white" {
        1
    } else {
        -1
    }
}
