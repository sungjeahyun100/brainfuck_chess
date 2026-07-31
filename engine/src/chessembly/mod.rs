pub mod ast;
pub mod interpreter;
pub mod parser;

use std::collections::{HashMap, HashSet};

use crate::types::{
    ChessemblyResult, GameState, MoveLayerDefinition, Piece, PieceDefinition, PlayerId, Square,
    SquareId,
};

use self::interpreter::{run, ExecutionContext};
use crate::custom_pieces::CustomPieceError;

pub fn run_chessembly_layer_for_piece(
    game_state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    layer: &MoveLayerDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
) -> ChessemblyResult {
    let program = game_state.chessembly_layer_program(&piece.type_id, layer);
    let ctx = ExecutionContext {
        board: &game_state.board,
        initial_square: piece.current_square.unwrap_or(Square::new(0, 0)),
        all_definitions: &game_state.piece_definitions,
        all_pieces: &game_state.pieces,
        player,
        global_state,
        attack_maps,
    };
    let _ = definition;
    run(program.as_ref(), &ctx)
}

#[allow(clippy::too_many_arguments)]
pub fn run_chessembly_layer_for_piece_checked(
    game_state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    layer: &MoveLayerDefinition,
    player: PlayerId,
    global_state: &HashMap<String, i32>,
    attack_maps: &HashMap<PlayerId, HashSet<SquareId>>,
    max_execution_steps: u64,
) -> Result<ChessemblyResult, CustomPieceError> {
    let program = game_state.chessembly_layer_program(&piece.type_id, layer);
    let ctx = ExecutionContext {
        board: &game_state.board,
        initial_square: piece.current_square.unwrap_or(Square::new(0, 0)),
        all_definitions: &game_state.piece_definitions,
        all_pieces: &game_state.pieces,
        player,
        global_state,
        attack_maps,
    };
    let _ = definition;
    interpreter::run_checked(program.as_ref(), &ctx, max_execution_steps)
        .map_err(|_| CustomPieceError::ExecutionLimitExceeded("execution_steps"))
}
