pub mod ast;
pub mod interpreter;
pub mod parser;

use std::collections::{HashMap, HashSet};

use crate::context::GameContext;
use crate::types::{ChessemblyResult, GameState, Piece, PieceDefinition, PlayerId, SquareId};

use self::interpreter::{run, ExecutionContext};

pub fn run_effective_chessembly_for_piece_with_context(
    context: &GameContext<'_>,
    piece: &Piece,
    definition: &PieceDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    let cache = &context.runtime.chessembly_programs;
    let program = piece
        .active_ability
        .as_ref()
        .and_then(|active| {
            definition
                .abilities
                .iter()
                .find(|ability| ability.id == active.ability_id)
                .map(|ability| cache.get_or_parse_ability(&definition.id, ability))
        })
        .unwrap_or_else(|| cache.get_or_parse(&definition.id, definition));

    let ctx = ExecutionContext {
        board: &context.state.board,
        piece,
        piece_definition: definition,
        all_definitions: context.catalog.definitions(),
        all_pieces: &context.state.pieces,
        player,
        global_state,
        attack_maps,
    };

    run(program.as_ref(), &ctx)
}

/// Compatibility facade. Multi-step engine flows should create one
/// `GameContext` and call the context-aware variant instead.
pub fn run_effective_chessembly_for_piece(
    game_state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    let context = GameContext::new(game_state);
    run_effective_chessembly_for_piece_with_context(
        &context,
        piece,
        definition,
        player,
        global_state,
        attack_maps,
    )
}
