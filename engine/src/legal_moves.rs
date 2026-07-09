use crate::catalog::PieceCatalog;
use crate::context::GameContext;
pub use crate::movegen::ability::MoveGenerationOptions;
use crate::runtime::RuntimeResources;
use crate::types::*;

fn with_game_context<R>(state: &GameState, run: impl FnOnce(&GameContext<'_>) -> R) -> R {
    let catalog = PieceCatalog::default_catalog();
    let runtime = RuntimeResources::from_catalog(&catalog);
    let context = GameContext {
        state,
        catalog: &catalog,
        runtime: &runtime,
    };
    run(&context)
}

pub fn generate_piece_attack_squares(state: &GameState, piece_id: &PieceId) -> Vec<Square> {
    with_game_context(state, |context| {
        crate::movegen::attack_squares::generate_piece_attack_squares(context, piece_id)
    })
}

pub fn generate_piece_legal_move_actions(state: &GameState, piece_id: &PieceId) -> Vec<MoveAction> {
    with_game_context(state, |context| {
        crate::movegen::piece_moves::generate_piece_legal_move_actions(context, piece_id)
    })
}

pub fn generate_piece_legal_move_actions_with_options(
    state: &GameState,
    piece_id: &PieceId,
    options: &MoveGenerationOptions,
) -> Vec<MoveAction> {
    with_game_context(state, |context| {
        crate::movegen::piece_moves::generate_piece_legal_move_actions_with_options(
            context, piece_id, options,
        )
    })
}

pub fn generate_legal_move_actions(state: &GameState) -> Vec<MoveAction> {
    with_game_context(state, |context| {
        crate::movegen::piece_moves::generate_legal_move_actions(context)
    })
}

pub fn generate_piece_legal_drop_actions(state: &GameState, piece_id: &PieceId) -> Vec<DropAction> {
    with_game_context(state, |context| {
        crate::movegen::drops::generate_piece_legal_drop_actions(context, piece_id)
    })
}

pub fn generate_legal_drop_actions(state: &GameState) -> Vec<DropAction> {
    with_game_context(state, |context| {
        crate::movegen::drops::generate_legal_drop_actions(context)
    })
}

pub fn generate_drop_candidates_by_type(
    state: &GameState,
    player_id: &PlayerId,
) -> Vec<DropCandidateByType> {
    with_game_context(state, |context| {
        crate::movegen::drops::generate_drop_candidates_by_type(context, player_id)
    })
}
