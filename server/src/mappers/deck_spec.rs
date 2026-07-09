use std::collections::HashMap;

use brainfuck_chess_engine::types::PieceId;

use crate::dto::game::{PlayerDeckSpec, StartingPieceSpec};

pub fn resolve_piece_type(player_id: &str, raw_piece_type: &str) -> Option<String> {
    match raw_piece_type {
        "king" | "queen" | "rook" | "bishop" | "knight" | "amazon" | "cannon-rook"
        | "tempest-queen" | "tempest-rook" | "tempest-knight" | "bouncing-bishop" => {
            Some(raw_piece_type.into())
        }
        "cannon_rook" => Some("cannon-rook".into()),
        "pawn" | "pawn-white" | "pawn-black" => Some(if player_id == "white" {
            "pawn-white".into()
        } else {
            "pawn-black".into()
        }),
        "tempest-pawn" | "tempest-pawn-white" | "tempest-pawn-black" => {
            Some(if player_id == "white" {
                "tempest-pawn-white".into()
            } else {
                "tempest-pawn-black".into()
            })
        }
        _ => None,
    }
}

pub fn make_piece_id(
    player_id: &str,
    piece_type: &str,
    counters: &mut HashMap<String, u32>,
) -> PieceId {
    let next = counters.entry(piece_type.into()).or_insert(0);
    *next += 1;
    format!("{}_{}_{}", player_id, piece_type.replace('-', "_"), next).into()
}

pub fn materialize_neutral_deck(
    spec: &PlayerDeckSpec,
    player_id: &str,
    board_size: i32,
) -> PlayerDeckSpec {
    if player_id == "white" {
        return spec.clone();
    }

    PlayerDeckSpec {
        starting: spec
            .starting
            .iter()
            .map(|piece| StartingPieceSpec {
                piece_type: piece.piece_type.clone(),
                square: brainfuck_chess_engine::types::Square {
                    file: piece.square.file,
                    rank: board_size - 1 - piece.square.rank,
                },
            })
            .collect(),
        pocket: spec.pocket.clone(),
    }
}
