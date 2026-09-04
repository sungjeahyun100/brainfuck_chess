use crate::legal_moves::{generate_drop_candidates_by_type, generate_legal_move_actions};
use crate::types::{GamePhase, GameState, PlayerId};

pub const WIN_SCORE: i32 = 1_000_000;
const KING_CAPTURE_THREAT: i32 = 100_000;
const MATERIAL_WEIGHT: i32 = 100;
const DROP_MOBILITY_WEIGHT: i32 = 2;
const MOVE_MOBILITY_WEIGHT: i32 = 2;

fn player_view(state: &GameState, player_id: &PlayerId) -> GameState {
    if &state.current_player == player_id {
        return state.clone();
    }

    let mut view = state.clone();
    view.current_player = player_id.clone();
    view
}

fn mobility(state: &GameState, player_id: &PlayerId) -> (usize, usize, bool) {
    let view = player_view(state, player_id);
    let moves = generate_legal_move_actions(&view);
    let can_capture_king = moves.iter().any(|action| {
        action
            .captured_piece_id
            .as_ref()
            .and_then(|id| view.pieces.get(id))
            .and_then(|piece| view.piece_definitions.get(&piece.type_id))
            .is_some_and(|definition| definition.is_king)
    });
    (
        moves.len(),
        generate_drop_candidates_by_type(&view, player_id).len(),
        can_capture_king,
    )
}

pub fn evaluate(state: &GameState, bot_player_id: &PlayerId) -> i32 {
    evaluate_internal(state, bot_player_id, true)
}

pub(crate) fn evaluate_without_king_capture_threat(
    state: &GameState,
    bot_player_id: &PlayerId,
) -> i32 {
    evaluate_internal(state, bot_player_id, false)
}

fn evaluate_internal(
    state: &GameState,
    bot_player_id: &PlayerId,
    include_king_capture_threat: bool,
) -> i32 {
    crate::profiling::record_evaluation(1);
    if state.phase == GamePhase::Ended || state.result.is_some() {
        return match state
            .result
            .as_ref()
            .and_then(|result| result.winner.as_ref())
        {
            Some(winner) if winner == bot_player_id => WIN_SCORE,
            Some(_) => -WIN_SCORE,
            None => 0,
        };
    }

    let mut score = material_balance(state, bot_player_id) * i64::from(MATERIAL_WEIGHT);

    let opponent_id = state
        .players
        .keys()
        .find(|player_id| *player_id != bot_player_id)
        .cloned()
        .unwrap_or_else(|| {
            if bot_player_id == "white" {
                "black".to_string()
            } else {
                "white".to_string()
            }
        });
    let (bot_moves, bot_drops, bot_king_capture) = mobility(state, bot_player_id);
    let (opponent_moves, opponent_drops, opponent_king_capture) = mobility(state, &opponent_id);

    score += (bot_moves as i64 - opponent_moves as i64) * i64::from(MOVE_MOBILITY_WEIGHT);
    score += (bot_drops as i64 - opponent_drops as i64) * i64::from(DROP_MOBILITY_WEIGHT);
    if include_king_capture_threat {
        if bot_king_capture {
            score += i64::from(KING_CAPTURE_THREAT);
        }
        if opponent_king_capture {
            score -= i64::from(KING_CAPTURE_THREAT);
        }
    }
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn material_balance(state: &GameState, bot_player_id: &PlayerId) -> i64 {
    let mut material = 0_i64;
    for piece in state.pieces.values() {
        if piece.captured {
            continue;
        }
        let Some(definition) = state.piece_definitions.get(&piece.type_id) else {
            continue;
        };
        if definition.is_king {
            continue;
        }

        let sign = if &piece.owner == bot_player_id { 1 } else { -1 };
        let value = if piece.in_pocket {
            definition.ai_pocket_value()
        } else if piece.is_on_board() {
            definition.ai_board_value()
        } else {
            0
        };
        material += i64::from(sign) * i64::from(value);
    }
    material
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::material_balance;
    use crate::pieces::default_pieces::{
        all_default_definitions, knight_definition, paratrooper_definition,
    };
    use crate::rules::{calculate_deck_score, create_board};
    use crate::types::{
        ChessemblyProgramCache, Deck, GamePhase, GameState, Piece, PieceDefinition, PieceLayer,
        Square,
    };

    fn test_state() -> GameState {
        let definitions: HashMap<_, _> = all_default_definitions()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect();
        GameState {
            id: "material-test".into(),
            board: create_board(8),
            pieces: HashMap::new(),
            chessembly_program_cache: ChessemblyProgramCache::from_definitions(&definitions),
            piece_definitions: definitions,
            custom_piece_manifest: Vec::new(),
            players: HashMap::new(),
            current_player: "white".into(),
            turn_number: 1,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: Vec::new(),
            result: None,
        }
    }

    fn test_piece(id: &str, type_id: &str, in_pocket: bool) -> Piece {
        Piece {
            id: id.into(),
            owner: "white".into(),
            type_id: type_id.into(),
            current_square: (!in_pocket).then(|| Square::new(0, 0)),
            in_pocket,
            captured: false,
            has_moved: false,
            current_ammo: 0,
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: HashMap::new(),
            move_option_cooldowns: HashMap::new(),
        }
    }

    #[test]
    fn unspecified_ai_values_fall_back_to_rule_score() {
        let knight = knight_definition();
        assert_eq!(knight.score, 3);
        assert_eq!(knight.ai_board_value, None);
        assert_eq!(knight.ai_pocket_value, None);
        assert_eq!(knight.ai_board_value(), 3);
        assert_eq!(knight.ai_pocket_value(), 3);

        let legacy_json = serde_json::to_value(&knight).unwrap();
        assert!(legacy_json.get("ai_board_value").is_none());
        assert!(legacy_json.get("ai_pocket_value").is_none());
        let restored: PieceDefinition = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(restored.ai_board_value(), 3);
        assert_eq!(restored.ai_pocket_value(), 3);
    }

    #[test]
    fn paratrooper_declares_distinct_board_and_pocket_values_without_changing_cost() {
        let paratrooper = paratrooper_definition();
        assert_eq!(paratrooper.score, 3);
        assert_eq!(paratrooper.ai_board_value(), 0);
        assert_eq!(paratrooper.ai_pocket_value(), 3);

        let definitions = HashMap::from([("paratrooper".into(), paratrooper)]);
        let pieces = HashMap::from([("piece".into(), test_piece("piece", "paratrooper", true))]);
        let deck = Deck {
            player_id: "white".into(),
            starting_pieces: Vec::new(),
            pocket_pieces: vec!["piece".into()],
            score_limit: 39,
            total_score: 3,
        };
        assert_eq!(calculate_deck_score(&deck, &pieces, &definitions), 3);
    }

    #[test]
    fn paratrooper_material_drops_by_three_after_deployment() {
        let mut state = test_state();
        state
            .pieces
            .insert("piece".into(), test_piece("piece", "paratrooper", true));
        let before = material_balance(&state, &"white".into());

        let piece = state.pieces.get_mut("piece").unwrap();
        piece.in_pocket = false;
        piece.current_square = Some(Square::new(0, 0));
        let after = material_balance(&state, &"white".into());

        assert_eq!(before, 3);
        assert_eq!(after, 0);
        assert_eq!(before - after, 3);
    }

    #[test]
    fn ordinary_piece_material_is_unchanged_after_deployment() {
        let mut state = test_state();
        state
            .pieces
            .insert("piece".into(), test_piece("piece", "knight", true));
        let before = material_balance(&state, &"white".into());

        let piece = state.pieces.get_mut("piece").unwrap();
        piece.in_pocket = false;
        piece.current_square = Some(Square::new(0, 0));
        let after = material_balance(&state, &"white".into());

        assert_eq!(before, 3);
        assert_eq!(after, 3);
    }

    #[test]
    fn board_and_pocket_material_are_both_included() {
        let mut state = test_state();
        state
            .pieces
            .insert("knight".into(), test_piece("knight", "knight", false));
        state
            .pieces
            .insert("queen".into(), test_piece("queen", "queen", true));

        assert_eq!(material_balance(&state, &"white".into()), 12);
    }
}
