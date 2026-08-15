use std::collections::{HashMap, HashSet};

use crate::chessembly::run_chessembly_layer_for_piece;
use crate::interaction::{destination_is_blocked_by_interaction, resolve_piece_interactions};
use crate::types::*;

/// Compute the full attack map for a player: the union of attackSquares from
/// every piece the player has on the board.
pub fn generate_attack_map(
    game_state: &GameState,
    player_id: &PlayerId,
    // Pre-computed attack maps for other players (used by `danger()` expression)
    existing_attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> AttackMap {
    crate::profiling::record_attack_map(1);
    game_state.ensure_chessembly_cache();

    let mut attacked_squares: HashSet<SquareId> = HashSet::new();
    let mut source_map: HashMap<SquareId, Vec<PieceId>> = HashMap::new();

    for (piece_id, piece) in &game_state.pieces {
        if piece.owner != *player_id || !piece.is_on_board() {
            continue;
        }
        let Some(definition) = game_state.piece_definitions.get(&piece.type_id) else {
            continue;
        };
        let options = definition.move_options.iter().filter(|option| {
            option.execution_mode == MoveOptionExecutionMode::MoveModifier
                && option.contributes_to_attack_map
                && piece
                    .move_option_cooldowns
                    .get(&option.id)
                    .is_none_or(|cooldown| cooldown.remaining == 0)
        });

        for option in options {
            let mut option_attacks = Vec::new();

            for layer_id in &option.layer_ids {
                let Some(layer) = definition
                    .move_layers
                    .iter()
                    .find(|layer| &layer.id == layer_id)
                else {
                    continue;
                };
                if !layer.is_enabled_for(piece) {
                    continue;
                }
                let chessembly_result = run_chessembly_layer_for_piece(
                    game_state,
                    piece,
                    definition,
                    layer,
                    player_id.clone(),
                    &game_state.global_state,
                    existing_attack_maps,
                );

                option_attacks.extend(chessembly_result.attack_squares.into_iter().filter(|sq| {
                    !destination_is_blocked_by_interaction(game_state, piece, *sq, &option.id)
                }));
            }

            option_attacks.extend(
                resolve_piece_interactions(game_state, piece, &option.id).attack_squares,
            );

            for sq in option_attacks {
                if !game_state.board.is_in_bounds(&sq) {
                    continue;
                }
                let sq_id = sq.to_id();
                attacked_squares.insert(sq_id);
                let sources = source_map.entry(sq_id).or_default();
                if !sources.contains(piece_id) {
                    sources.push(piece_id.clone());
                }
            }
        }
    }

    AttackMap {
        player_id: player_id.clone(),
        attacked_squares,
        source_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::default_pieces::all_default_definitions;
    use crate::rules::{calculate_score_limit, create_board};

    fn make_game_state(board_size: i32) -> GameState {
        let board = create_board(board_size);
        let defs: HashMap<String, PieceDefinition> = all_default_definitions()
            .into_iter()
            .map(|d| (d.id.clone(), d))
            .collect();
        let chessembly_program_cache = ChessemblyProgramCache::from_definitions(&defs);

        let white_deck = Deck {
            player_id: "white".into(),
            starting_pieces: Vec::new(),
            pocket_pieces: Vec::new(),
            score_limit: calculate_score_limit(board_size),
            total_score: 0,
        };
        let black_deck = Deck {
            player_id: "black".into(),
            starting_pieces: Vec::new(),
            pocket_pieces: Vec::new(),
            score_limit: calculate_score_limit(board_size),
            total_score: 0,
        };

        let mut players = HashMap::new();
        players.insert(
            "white".into(),
            Player {
                id: "white".into(),
                deck: white_deck,
                captured_pieces: Vec::new(),
            },
        );
        players.insert(
            "black".into(),
            Player {
                id: "black".into(),
                deck: black_deck,
                captured_pieces: Vec::new(),
            },
        );

        GameState {
            id: "test".into(),
            board,
            pieces: HashMap::new(),
            piece_definitions: defs,
            custom_piece_manifest: Vec::new(),
            players,
            current_player: "white".into(),
            turn_number: 1,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: Vec::new(),
            result: None,
            chessembly_program_cache,
        }
    }

    fn add_piece(
        state: &mut GameState,
        id: &str,
        owner: &str,
        type_id: &str,
        file: i32,
        rank: i32,
    ) {
        let sq = Square::new(file, rank);
        let piece = Piece {
            id: id.into(),
            owner: owner.into(),
            type_id: type_id.into(),
            current_square: Some(sq),
            in_pocket: false,
            captured: false,
            has_moved: false,
            state: state
                .piece_definitions
                .get(type_id)
                .map(PieceDefinition::initial_state)
                .unwrap_or_default(),
            move_option_cooldowns: HashMap::new(),
        };
        state.board.squares.insert(sq.to_id(), Some(id.into()));
        state.pieces.insert(id.into(), piece);
        state
            .players
            .get_mut(owner)
            .unwrap()
            .deck
            .starting_pieces
            .push(id.into());
    }

    fn add_pocket_piece(state: &mut GameState, id: &str, owner: &str, type_id: &str) {
        let piece = Piece {
            id: id.into(),
            owner: owner.into(),
            type_id: type_id.into(),
            current_square: None,
            in_pocket: true,
            captured: false,
            has_moved: false,
            state: state
                .piece_definitions
                .get(type_id)
                .map(PieceDefinition::initial_state)
                .unwrap_or_default(),
            move_option_cooldowns: HashMap::new(),
        };
        state.pieces.insert(id.into(), piece);
        state
            .players
            .get_mut(owner)
            .unwrap()
            .deck
            .pocket_pieces
            .push(id.into());
    }

    #[test]
    fn bounced_attack_squares_are_valid_placement_squares() {
        let mut state = make_game_state(8);
        add_piece(&mut state, "bouncer", "white", "bouncing-rook", 3, 3);
        add_piece(
            &mut state,
            "wall",
            "black",
            "bouncing-pawn-black",
            3,
            5,
        );
        add_pocket_piece(&mut state, "reserve", "white", "paratrooper");

        let attack_map = generate_attack_map(&state, &"white".into(), &HashMap::new());
        let reflected = Square::new(1, 4);
        let wall = Square::new(3, 5);

        assert!(attack_map.attacked_squares.contains(&reflected.to_id()));
        assert!(!attack_map.attacked_squares.contains(&wall.to_id()));
        assert!(attack_map
            .source_map
            .get(&reflected.to_id())
            .is_some_and(|sources| sources.iter().any(|id| id == "bouncer")));

        let reserve = state.pieces.get("reserve").unwrap();
        let placement =
            crate::placement::get_piece_placement_squares(&state, &"white".into(), reserve);
        assert!(placement.contains(&reflected));
        assert!(!placement.contains(&wall));
    }
}
