pub mod ast;
pub mod interpreter;
pub mod parser;

use std::collections::{HashMap, HashSet};

use crate::catalog::PieceCatalog;
use crate::context::GameContext;
use crate::runtime::RuntimeResources;
use crate::types::{ChessemblyResult, GameState, Piece, PieceDefinition, PlayerId, SquareId};

use self::interpreter::{run, ExecutionContext};

pub fn run_effective_chessembly_for_piece(
    game_state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    let mut definitions = PieceCatalog::default_catalog().definitions().clone();
    definitions.insert(definition.id.clone(), definition.clone());
    let catalog = PieceCatalog::from_definitions(definitions);
    let runtime = RuntimeResources::from_catalog(&catalog);
    let context = GameContext {
        state: game_state,
        catalog: &catalog,
        runtime: &runtime,
    };
    run_effective_chessembly_for_context(
        &context,
        piece,
        definition,
        player,
        global_state,
        attack_maps,
    )
}

pub fn run_effective_chessembly_for_context(
    context: &GameContext<'_>,
    piece: &Piece,
    definition: &PieceDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    context.ensure_chessembly_cache();

    let Some(program) = context.effective_chessembly_program(piece, definition) else {
        return ChessemblyResult::default();
    };

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
