use crate::types::*;

/// 국무총리: moves through the supplied Chessembly edge and corner chains.
pub fn prime_minister_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "prime-minister".into(),
        name: "국무총리".into(),
        score: 7,
        max_ammo: 0,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
take-move(0, 1) { take-move(1, 1) } { take-move(0, 1) } { take-move(-1, 1) };
take-move(1, 0) { take-move(1, 1) } { take-move(1, 0) } { take-move(1, -1) };
take-move(0, -1) { take-move(1, -1) } { take-move(0, -1) } { take-move(-1, -1) };
take-move(-1, 0) { take-move(-1, 1) } { take-move(-1, 0) } { take-move(-1, -1) };

take-move(1, 1) { take-move(1, 1) } { take-move(0, 1) } { take-move(-1, 1) } { take-move(1, 0) } { take-move(1, -1) };
take-move(1, -1) { take-move(1, -1) } { take-move(1, 0) } { take-move(1, 1) } { take-move(0, -1) } { take-move(-1, -1) };
take-move(-1, 1) { take-move(-1, 1) } { take-move(0, 1) } { take-move(-1, 1) } { take-move(-1, 0) } { take-move(-1, -1) };
take-move(-1, -1) { take-move(-1, -1) } { take-move(0, -1) } { take-move(1, -1) } { take-move(-1, 0) } { take-move(-1, 1) };"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::chessembly::{
        interpreter::{run, ExecutionContext},
        parser::parse,
    };
    use crate::rules::create_board;

    #[test]
    fn definition_has_the_requested_score_and_no_ability() {
        let definition = prime_minister_definition();

        assert_eq!(definition.name, "국무총리");
        assert_eq!(definition.score, 7);
        assert_eq!(definition.move_options.len(), 1);
        assert_eq!(definition.move_options[0].kind, MoveOptionKind::Normal);
    }

    #[test]
    fn supplied_program_reaches_all_squares_in_the_surrounding_five_by_five_area() {
        let definition = prime_minister_definition();
        let piece = Piece {
            id: "pm".into(),
            owner: "white".into(),
            type_id: definition.id.clone(),
            current_square: Some(Square::new(3, 3)),
            in_pocket: false,
            captured: false,
            has_moved: false,
            current_ammo: 0,
            layer: PieceLayer::Ground,
            remaining_flight_turns: 0,
            state: HashMap::new(),
            move_option_cooldowns: HashMap::new(),
        };
        let mut board = create_board(8);
        board
            .squares
            .insert(Square::new(3, 3).to_id(), Some(piece.id.clone()));
        let pieces = HashMap::from([(piece.id.clone(), piece.clone())]);
        let definitions = HashMap::from([(definition.id.clone(), definition.clone())]);
        let global_state = HashMap::new();
        let attack_maps = HashMap::new();
        let program = parse(&definition.chessembly_code);
        let context = ExecutionContext {
            board: &board,
            layer: PieceLayer::Ground,
            initial_square: Square::new(3, 3),
            all_definitions: &definitions,
            all_pieces: &pieces,
            player: piece.owner,
            global_state: &global_state,
            attack_maps: &attack_maps,
        };

        let result = run(&program, &context);

        assert_eq!(result.movement_squares.len(), 24);
        for file in 1..=5 {
            for rank in 1..=5 {
                if file != 3 || rank != 3 {
                    assert!(
                        result.movement_squares.contains(&Square::new(file, rank)),
                        "missing ({file}, {rank})"
                    );
                }
            }
        }
    }
}
