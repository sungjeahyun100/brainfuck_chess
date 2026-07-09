pub mod ability;
mod action_builder;
pub mod attack_squares;
pub mod backend;
mod context;
pub mod drops;
pub mod piece_moves;
mod special;

pub use ability::MoveGenerationOptions;
pub use attack_squares::generate_piece_attack_squares;
pub use backend::{
    ChessemblyBackend, MoveIntent, NativeBackend, PieceMoveBackend, PieceMoveContext,
    PieceMovePattern,
};
pub use context::MovegenContext;
pub use drops::{
    generate_drop_candidates_by_type, generate_legal_drop_actions,
    generate_piece_legal_drop_actions,
};
pub use piece_moves::{
    generate_legal_move_actions, generate_piece_legal_move_actions,
    generate_piece_legal_move_actions_with_options,
};
