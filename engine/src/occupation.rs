use crate::types::{OccupationZone, Square};

// Each entry is (judgment circles, arrow squares). The sets are separate on purpose.
const ZONES_8: &[(&[&str], &[&str])] = &[
    (&["b8"], &["a8", "a7", "b7", "c7", "c8"]),
    (&["g8"], &["f8", "f7", "g7", "h7", "h8"]),
    (&["a5", "a4"], &["a6", "b6", "b5", "b4", "b3", "a3"]),
    (
        &["d5", "e5", "d4", "e4"],
        &[
            "d6", "e6", "f6", "c6", "c5", "c4", "c3", "d3", "e3", "f3", "f4", "f5",
        ],
    ),
    (&["h5", "h4"], &["h6", "g6", "g5", "g4", "g3", "h3"]),
    (&["b1"], &["a1", "a2", "b2", "c2", "c1"]),
    (&["g1"], &["f1", "f2", "g2", "h2", "h1"]),
];

const ZONES_12: &[(&[&str], &[&str])] = &[
    (&["f12", "g12"], &["e12", "e11", "f11", "g11", "h11", "h12"]),
    (
        &["c11"],
        &["b12", "c12", "d12", "b11", "d11", "b10", "c10", "d10"],
    ),
    (
        &["j11"],
        &["i12", "j12", "k12", "i11", "k11", "i10", "j10", "k10"],
    ),
    (
        &["b7", "c7", "b6", "c6"],
        &[
            "a8", "b8", "c8", "d8", "a7", "d7", "a6", "d6", "a5", "b5", "c5", "d5",
        ],
    ),
    (
        &["f7", "g7", "f6", "g6"],
        &[
            "e8", "f8", "g8", "h8", "e7", "h7", "e6", "h6", "e5", "f5", "g5", "h5",
        ],
    ),
    (
        &["j7", "k7", "j6", "k6"],
        &[
            "i8", "j8", "k8", "l8", "i7", "l7", "i6", "l6", "i5", "j5", "k5", "l5",
        ],
    ),
    (&["c2"], &["b3", "c3", "d3", "b2", "d2", "b1", "c1", "d1"]),
    (&["j2"], &["i3", "j3", "k3", "i2", "k2", "i1", "j1", "k1"]),
    (&["f1", "g1"], &["e1", "e2", "f2", "g2", "h2", "h1"]),
];

fn square(coordinate: &str) -> Square {
    Square::new(
        (coordinate.as_bytes()[0] - b'a') as i32,
        coordinate[1..].parse::<i32>().expect("fixed zone rank") - 1,
    )
}

pub fn zones_for_board(size: i32) -> Option<Vec<OccupationZone>> {
    let zones = match size {
        8 => ZONES_8,
        12 => ZONES_12,
        _ => return None,
    };
    Some(
        zones
            .iter()
            .map(|(circles, arrows)| OccupationZone {
                squares: circles.iter().map(|label| square(label)).collect(),
                effect_squares: arrows.iter().map(|label| square(label)).collect(),
                owner: None,
            })
            .collect(),
    )
}
