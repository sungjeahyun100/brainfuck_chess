//! Built-in chess piece registry.
//!
//! Each piece owns its definition and Chessembly movement program in a separate
//! module so its movement can evolve without coupling it to another piece.

use crate::types::PieceDefinition;

/// Keeps simple built-in definitions compact while preserving the validated
/// default layer/normal option model used by legacy single-code pieces.
macro_rules! legacy_piece_definition {
    ($($field:tt)*) => {{
        PieceDefinition {
            $($field)*
            state_schema: Vec::new(),
            move_layers: Vec::new(),
            move_options: Vec::new(),
            visual: PieceVisualDefinition::default(),
            can_capture_on_drop: false,
        }
        .normalize_and_validate()
        .expect("built-in piece definition must be valid")
    }};
}

mod airborne;
mod alternating_soldier;
mod amazon;
mod bishop;
mod bouncing_bishop;
mod bouncing_pawn_black;
mod bouncing_pawn_white;
mod bouncing_queen;
mod bouncing_rook;
mod cannon_rook;
mod dozer_black;
mod dozer_white;
mod green_camp;
mod guhang;
mod king;
mod knight;
mod machine_gunner;
mod mortar;
mod nightrider;
mod paratrooper;
mod pawn_black;
mod pawn_white;
mod queen;
mod rook;
mod tempest_bishop;
mod tempest_knight;
mod tempest_pawn_black;
mod tempest_pawn_white;
mod tempest_queen;
mod tempest_rook;
mod windmill;

pub use airborne::airborne_definition;
pub use alternating_soldier::alternating_soldier_definition;
pub use amazon::amazon_definition;
pub use bishop::bishop_definition;
pub use bouncing_bishop::bouncing_bishop_definition;
pub use bouncing_pawn_black::bouncing_pawn_black_definition;
pub use bouncing_pawn_white::bouncing_pawn_white_definition;
pub use bouncing_queen::bouncing_queen_definition;
pub use bouncing_rook::{bouncing_rook_chessembly_code, bouncing_rook_definition};
pub use cannon_rook::cannon_rook_definition;
pub use dozer_black::dozer_black_definition;
pub use dozer_white::dozer_white_definition;
pub use green_camp::green_camp_definition;
pub use guhang::guhang_definition;
pub use king::king_definition;
pub use knight::knight_definition;
pub use machine_gunner::machine_gunner_definition;
pub(crate) use machine_gunner::MACHINE_GUN_BARRAGE_ABILITY_ID;
pub use mortar::mortar_definition;
pub(crate) use mortar::MORTAR_BARRAGE_ABILITY_ID;
pub use nightrider::nightrider_definition;
pub use paratrooper::paratrooper_definition;
pub use pawn_black::pawn_black_definition;
pub use pawn_white::pawn_white_definition;
pub use queen::queen_definition;
pub use rook::rook_definition;
pub use tempest_bishop::tempest_bishop_definition;
pub use tempest_knight::tempest_knight_definition;
pub use tempest_pawn_black::tempest_pawn_black_definition;
pub use tempest_pawn_white::tempest_pawn_white_definition;
pub use tempest_queen::tempest_queen_definition;
pub use tempest_rook::tempest_rook_definition;
pub use windmill::windmill_definition;

/// Return all standard piece definitions in the established registration order.
pub fn all_default_definitions() -> Vec<PieceDefinition> {
    vec![
        king_definition(),
        queen_definition(),
        rook_definition(),
        bishop_definition(),
        windmill_definition(),
        knight_definition(),
        nightrider_definition(),
        paratrooper_definition(),
        alternating_soldier_definition(),
        airborne_definition(),
        green_camp_definition(),
        mortar_definition(),
        machine_gunner_definition(),
        amazon_definition(),
        guhang_definition(),
        cannon_rook_definition(),
        tempest_rook_definition(),
        bouncing_bishop_definition(),
        bouncing_rook_definition(),
        bouncing_queen_definition(),
        pawn_white_definition(),
        pawn_black_definition(),
        bouncing_pawn_white_definition(),
        bouncing_pawn_black_definition(),
        dozer_white_definition(),
        dozer_black_definition(),
        tempest_pawn_white_definition(),
        tempest_pawn_black_definition(),
        tempest_queen_definition(),
        tempest_knight_definition(),
        tempest_bishop_definition(),
    ]
}
