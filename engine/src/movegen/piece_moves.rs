use std::collections::{HashMap, HashSet};
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::chessembly::run_effective_chessembly_for_context;
use crate::context::GameContext;
use crate::types::*;

use super::ability::{
    can_use_selected_ability, run_selected_ability_for_piece, MoveGenerationOptions,
};
use super::special::{castling, en_passant, pawn, promotion};

struct PieceMovePattern {
    result: ChessemblyResult,
    ability_id: Option<String>,
}

pub fn generate_piece_legal_move_actions(
    context: &GameContext<'_>,
    piece_id: &PieceId,
) -> Vec<MoveAction> {
    generate_piece_legal_move_actions_with_options(
        context,
        piece_id,
        &MoveGenerationOptions::default(),
    )
}

pub fn generate_piece_legal_move_actions_with_options(
    context: &GameContext<'_>,
    piece_id: &PieceId,
    options: &MoveGenerationOptions,
) -> Vec<MoveAction> {
    context.ensure_chessembly_cache();
    let state = context.state;

    if state.turn_state.mode == TurnMode::Drop || !context.can_generate_move_or_drop() {
        return Vec::new();
    }

    let Some(piece) = state.pieces.get(piece_id) else {
        return Vec::new();
    };

    if piece.owner != state.current_player || !piece.is_on_board() {
        return Vec::new();
    }

    let Some(definition) = context.catalog.get(&piece.type_id) else {
        return Vec::new();
    };

    let Some(pattern) =
        run_piece_backend_or_ability(context, piece, definition, &state.current_player, options)
    else {
        return Vec::new();
    };

    let from = piece.current_square.unwrap();
    let ability_id = pattern.ability_id.as_deref();
    let mut actions = build_standard_move_actions(
        state,
        piece,
        definition,
        piece_id,
        &state.current_player,
        from,
        &pattern,
    );

    actions.extend(en_passant::generate_en_passant_actions(
        state,
        piece,
        definition,
        piece_id,
        &state.current_player,
        from,
        ability_id,
    ));

    actions.extend(castling::generate_castling_actions(
        state,
        piece,
        definition,
        piece_id,
        &state.current_player,
        from,
        ability_id,
    ));

    actions
}

fn run_piece_backend_or_ability(
    context: &GameContext<'_>,
    piece: &Piece,
    definition: &PieceDefinition,
    player_id: &PlayerId,
    options: &MoveGenerationOptions,
) -> Option<PieceMovePattern> {
    let empty_maps: HashMap<PlayerId, HashSet<SquareId>> = HashMap::new();
    let empty_global_state = HashMap::new();

    let selected_ability = options
        .ability_id
        .as_deref()
        .and_then(|ability_id| can_use_selected_ability(context, piece, definition, ability_id));
    if options.ability_id.is_some() && selected_ability.is_none() {
        return None;
    }

    let result = if let Some(ability) = selected_ability.as_ref() {
        run_selected_ability_for_piece(
            context,
            piece,
            definition,
            ability,
            player_id,
            &empty_global_state,
            &empty_maps,
        )
    } else {
        run_effective_chessembly_for_context(
            context,
            piece,
            definition,
            player_id.clone(),
            &empty_global_state,
            &empty_maps,
        )
    };

    Some(PieceMovePattern {
        result,
        ability_id: selected_ability.map(|ability| ability.id),
    })
}

fn build_standard_move_actions(
    state: &GameState,
    piece: &Piece,
    definition: &PieceDefinition,
    piece_id: &PieceId,
    player_id: &PlayerId,
    from: Square,
    pattern: &PieceMovePattern,
) -> Vec<MoveAction> {
    let mut actions = Vec::new();
    let ability_id = pattern.ability_id.as_deref();

    for to in pattern.result.movement_squares.iter().copied() {
        if !state.board.is_in_bounds(&to) {
            continue;
        }

        if pawn::rejects_illegal_double_step(piece, from, to, state.board.size) {
            continue;
        }

        let captured_piece_id = state.board.get_piece_at(&to).cloned();

        if let Some(ref cap_id) = captured_piece_id {
            if let Some(cap_piece) = state.pieces.get(cap_id) {
                if cap_piece.owner == *player_id {
                    continue;
                }
            }
        }

        promotion::push_move_or_promotions(
            &mut actions,
            definition,
            state.board.size,
            player_id,
            piece_id,
            from,
            to,
            captured_piece_id,
            ability_id,
        );
    }

    for to in pattern.result.attack_squares.iter().copied() {
        if !state.board.is_in_bounds(&to) {
            continue;
        }

        let Some(captured_piece_id) = state.board.get_piece_at(&to).cloned() else {
            continue;
        };
        let Some(captured_piece) = state.pieces.get(&captured_piece_id) else {
            continue;
        };
        if captured_piece.owner == *player_id {
            continue;
        }

        promotion::push_move_or_promotions(
            &mut actions,
            definition,
            state.board.size,
            player_id,
            piece_id,
            from,
            to,
            Some(captured_piece_id),
            ability_id,
        );
    }

    actions
}

pub fn generate_legal_move_actions(context: &GameContext<'_>) -> Vec<MoveAction> {
    #[cfg(feature = "profiling")]
    let started = Instant::now();
    let state = context.state;
    let player_id = &state.current_player;

    if state.turn_state.mode == TurnMode::Drop || !context.can_generate_move_or_drop() {
        return Vec::new();
    }

    let mut piece_ids = state
        .pieces
        .iter()
        .filter_map(|(piece_id, piece)| {
            if piece.owner == *player_id {
                Some(piece_id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    piece_ids.sort();

    let actions = piece_ids
        .into_iter()
        .flat_map(|piece_id| generate_piece_legal_move_actions(context, &piece_id))
        .collect::<Vec<_>>();
    #[cfg(feature = "profiling")]
    crate::profiling::record_legal_moves(started.elapsed(), actions.len());
    actions
}
