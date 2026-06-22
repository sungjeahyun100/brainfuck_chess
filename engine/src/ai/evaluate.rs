use crate::legal_moves::{generate_legal_drop_actions, generate_legal_move_actions};
use crate::rules::grant_move_stacks;
use crate::types::{GamePhase, GameState, PlayerId, TurnState};

pub const WIN_SCORE: i32 = 1_000_000;
const KING_CAPTURE_THREAT: i32 = 100_000;
const MATERIAL_WEIGHT: i32 = 100;
const POCKET_MATERIAL_WEIGHT: i32 = 60;
const DROP_MOBILITY_WEIGHT: i32 = 5;
const MOVE_MOBILITY_WEIGHT: i32 = 2;
const TURN_MOMENTUM_WEIGHT: i32 = 3;

fn player_view(state: &GameState, player_id: &PlayerId) -> GameState {
    if &state.current_player == player_id {
        return state.clone();
    }

    let mut view = state.clone();
    view.current_player = player_id.clone();
    view.turn_state = TurnState::new();
    grant_move_stacks(&mut view);
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
        generate_legal_drop_actions(&view).len(),
        can_capture_king,
    )
}

pub fn evaluate(state: &GameState, bot_player_id: &PlayerId) -> i32 {
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

    let mut score = 0_i64;
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
        let weight = if piece.in_pocket {
            POCKET_MATERIAL_WEIGHT
        } else if piece.is_on_board() {
            MATERIAL_WEIGHT
        } else {
            0
        };
        score += i64::from(sign * weight) * i64::from(definition.score);
    }

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
    if bot_king_capture {
        score += i64::from(KING_CAPTURE_THREAT);
    }
    if opponent_king_capture {
        score -= i64::from(KING_CAPTURE_THREAT);
    }
    if &state.current_player == bot_player_id {
        let remaining = state
            .pieces
            .values()
            .filter(|piece| {
                piece.owner == *bot_player_id
                    && piece.is_on_board()
                    && piece.move_stack > 0
                    && !state.turn_state.moved_piece_ids.contains(&piece.id)
            })
            .count();
        score += remaining as i64 * i64::from(TURN_MOMENTUM_WEIGHT);
    }

    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
