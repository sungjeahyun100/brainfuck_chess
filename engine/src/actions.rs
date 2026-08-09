use crate::endgame::apply_and_advance_turn;
use crate::legal_moves::{
    generate_piece_legal_drop_actions, generate_piece_legal_move_actions_with_options,
    is_legal_ability_action, MoveGenerationOptions,
};
use crate::types::{GamePhase, GameState, TurnAction};

/// Validate and apply every public gameplay action through one boundary.
pub fn submit_action(state: GameState, action: TurnAction) -> Result<GameState, String> {
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return Err("게임이 이미 종료되었습니다.".into());
    }

    let legal = match &action {
        TurnAction::Move(action) => {
            action.player_id == state.current_player
                && generate_piece_legal_move_actions_with_options(
                    &state,
                    &action.piece_id,
                    &MoveGenerationOptions {
                        move_option_id: Some(action.move_option_id.clone()),
                    },
                )
                .into_iter()
                .any(|candidate| candidate == *action)
        }
        TurnAction::Drop(action) => {
            action.player_id == state.current_player
                && generate_piece_legal_drop_actions(&state, &action.piece_id)
                    .into_iter()
                    .any(|candidate| candidate == *action)
        }
        TurnAction::Ability(action) => is_legal_ability_action(&state, action),
    };
    if !legal {
        return Err("canonical legal action이 아닙니다.".into());
    }
    Ok(apply_canonical_action(state, action))
}

/// Apply a canonical action produced by the engine itself.
///
/// This deliberately stays crate-private: untrusted/public actions must pass
/// through `submit_action` and its canonical validation first.
pub(crate) fn apply_canonical_action(state: GameState, action: TurnAction) -> GameState {
    crate::profiling::record_action_application(1);
    apply_and_advance_turn(state, action)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::legal_moves::{
        generate_legal_ability_actions, generate_legal_drop_actions, generate_legal_move_actions,
    };
    use crate::pieces::default_pieces::all_default_definitions;
    use crate::rules::create_board;
    use crate::types::*;

    fn state_with_all_action_kinds() -> GameState {
        let definitions: HashMap<_, _> = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect();
        let players = ["white", "black"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    Player {
                        id: id.into(),
                        deck: Deck {
                            player_id: id.into(),
                            starting_pieces: Vec::new(),
                            pocket_pieces: Vec::new(),
                            score_limit: 39,
                            total_score: 0,
                        },
                        captured_pieces: Vec::new(),
                    },
                )
            })
            .collect();
        let mut state = GameState {
            id: "canonical-apply-parity".into(),
            board: create_board(8),
            pieces: HashMap::new(),
            chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
            piece_definitions: definitions,
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
        };
        add_piece(
            &mut state,
            "camp",
            "white",
            "green-camp",
            Some(Square::new(3, 3)),
        );
        add_piece(
            &mut state,
            "enemy",
            "black",
            "rook",
            Some(Square::new(4, 3)),
        );
        add_piece(&mut state, "reserve", "white", "knight", None);
        state
    }

    fn add_piece(
        state: &mut GameState,
        id: &str,
        owner: &str,
        type_id: &str,
        square: Option<Square>,
    ) {
        let piece_id: PieceId = id.into();
        if let Some(square) = square {
            state
                .board
                .squares
                .insert(square.to_id(), Some(piece_id.clone()));
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .starting_pieces
                .push(piece_id.clone());
        } else {
            state
                .players
                .get_mut(owner)
                .unwrap()
                .deck
                .pocket_pieces
                .push(piece_id.clone());
        }
        state.pieces.insert(
            piece_id.clone(),
            Piece {
                id: piece_id,
                owner: owner.into(),
                type_id: type_id.into(),
                current_square: square,
                in_pocket: square.is_none(),
                captured: false,
                has_moved: false,
                state: HashMap::new(),
                move_option_cooldowns: HashMap::new(),
            },
        );
    }

    fn assert_apply_parity(state: &GameState, action: TurnAction) {
        let public = submit_action(state.clone(), action.clone()).unwrap();
        let internal = apply_canonical_action(state.clone(), action);
        assert_eq!(
            serde_json::to_value(public).unwrap(),
            serde_json::to_value(internal).unwrap()
        );
    }

    #[test]
    fn generated_move_public_and_internal_apply_match() {
        let state = state_with_all_action_kinds();
        let action = generate_legal_move_actions(&state)
            .into_iter()
            .next()
            .unwrap();
        assert_apply_parity(&state, TurnAction::Move(action));
    }

    #[test]
    fn generated_drop_public_and_internal_apply_match() {
        let state = state_with_all_action_kinds();
        let action = generate_legal_drop_actions(&state)
            .into_iter()
            .next()
            .unwrap();
        assert_apply_parity(&state, TurnAction::Drop(action));
    }

    #[test]
    fn generated_ability_public_and_internal_apply_match() {
        let state = state_with_all_action_kinds();
        let action = generate_legal_ability_actions(&state)
            .into_iter()
            .next()
            .unwrap();
        assert_apply_parity(&state, TurnAction::Ability(action));
    }
}
