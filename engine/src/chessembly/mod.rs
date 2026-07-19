pub mod ast;
pub mod interpreter;
pub mod parser;

use std::collections::{HashMap, HashSet};

use crate::types::{
    ChessemblyResult, GameState, MoveLayerDefinition, Piece, PieceDefinition, PlayerId, SquareId,
};

use self::interpreter::{run, ExecutionContext};

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
        piece,
        piece_definition: definition,
        all_definitions: &game_state.piece_definitions,
        all_pieces: &game_state.pieces,
        player,
        global_state,
        attack_maps,
    };
    run(program.as_ref(), &ctx)
}
