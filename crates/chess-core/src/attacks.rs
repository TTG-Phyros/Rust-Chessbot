//! Attack tables.
//!
//! Leaper attacks (knight, king, pawn) and the `between`/`line` tables are
//! computed at compile time. Slider attacks (bishop, rook, queen) use magic
//! bitboards, built once on first use (see the `magic` module).

use crate::magic::SLIDERS;
use crate::{Bitboard, Color, Square};

pub(crate) type Direction = (i8, i8);

const KNIGHT_DELTAS: [Direction; 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING_DELTAS: [Direction; 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const WHITE_PAWN_DELTAS: [Direction; 2] = [(-1, 1), (1, 1)];
const BLACK_PAWN_DELTAS: [Direction; 2] = [(-1, -1), (1, -1)];

pub(crate) const ROOK_DIRECTIONS: [Direction; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub(crate) const BISHOP_DIRECTIONS: [Direction; 4] = [(1, 1), (-1, -1), (1, -1), (-1, 1)];
/// Eight directions, ordered so that `ALL_DIRECTIONS[d ^ 1]` is the opposite of `d`.
const ALL_DIRECTIONS: [Direction; 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

static KNIGHT: [Bitboard; 64] = leaper_table(&KNIGHT_DELTAS);
static KING: [Bitboard; 64] = leaper_table(&KING_DELTAS);
static PAWN: [[Bitboard; 64]; 2] = [
    leaper_table(&WHITE_PAWN_DELTAS),
    leaper_table(&BLACK_PAWN_DELTAS),
];
static BETWEEN: [[Bitboard; 64]; 64] = ray_tables().0;
static LINE: [[Bitboard; 64]; 64] = ray_tables().1;

/// Squares attacked by a knight on `square`.
#[must_use]
pub fn knight_attacks(square: Square) -> Bitboard {
    KNIGHT[square.index()]
}

/// Squares attacked by a king on `square`.
#[must_use]
pub fn king_attacks(square: Square) -> Bitboard {
    KING[square.index()]
}

/// Squares attacked by a pawn of `color` on `square`.
#[must_use]
pub fn pawn_attacks(color: Color, square: Square) -> Bitboard {
    PAWN[color.index()][square.index()]
}

/// Squares attacked by a bishop on `square`, given the board `occupied`.
#[must_use]
pub fn bishop_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    SLIDERS.bishop(square.index(), occupied.0)
}

/// Squares attacked by a rook on `square`, given the board `occupied`.
#[must_use]
pub fn rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    SLIDERS.rook(square.index(), occupied.0)
}

/// Squares attacked by a queen on `square`, given the board `occupied`.
#[must_use]
pub fn queen_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    bishop_attacks(square, occupied) | rook_attacks(square, occupied)
}

/// Squares strictly between `a` and `b`; empty unless they share a line or diagonal.
#[must_use]
pub fn between(a: Square, b: Square) -> Bitboard {
    BETWEEN[a.index()][b.index()]
}

/// The whole line or diagonal through `a` and `b` (both included); empty if
/// they are not aligned.
#[must_use]
pub fn line(a: Square, b: Square) -> Bitboard {
    LINE[a.index()][b.index()]
}

const fn on_board(file: i8, rank: i8) -> bool {
    file >= 0 && file < 8 && rank >= 0 && rank < 8
}

const fn bit(file: i8, rank: i8) -> u64 {
    1 << (rank * 8 + file) as u32
}

const fn leaper_table(deltas: &[Direction]) -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut square = 0;
    while square < 64 {
        let (file, rank) = ((square % 8) as i8, (square / 8) as i8);
        let mut bits = 0;
        let mut i = 0;
        while i < deltas.len() {
            let (df, dr) = deltas[i];
            if on_board(file + df, rank + dr) {
                bits |= bit(file + df, rank + dr);
            }
            i += 1;
        }
        table[square] = Bitboard(bits);
        square += 1;
    }
    table
}

/// Reference slider attacks by walking rays; used to build and test the magic tables.
pub(crate) const fn sliding_attacks(square: usize, occupied: u64, directions: &[Direction]) -> u64 {
    let mut attacks = 0;
    let mut d = 0;
    while d < directions.len() {
        let (df, dr) = directions[d];
        let (mut file, mut rank) = ((square % 8) as i8, (square / 8) as i8);
        loop {
            file += df;
            rank += dr;
            if !on_board(file, rank) {
                break;
            }
            attacks |= bit(file, rank);
            if occupied & bit(file, rank) != 0 {
                break;
            }
        }
        d += 1;
    }
    attacks
}

/// Ray from `square` towards `direction`, excluding `square`, up to the edge.
const fn ray(square: usize, direction: Direction) -> u64 {
    sliding_attacks(square, 0, &[direction])
}

const fn ray_tables() -> ([[Bitboard; 64]; 64], [[Bitboard; 64]; 64]) {
    let mut between = [[Bitboard::EMPTY; 64]; 64];
    let mut line = [[Bitboard::EMPTY; 64]; 64];
    let mut a = 0;
    while a < 64 {
        let mut d = 0;
        while d < 8 {
            let (df, dr) = ALL_DIRECTIONS[d];
            let full_line = ray(a, ALL_DIRECTIONS[d]) | ray(a, ALL_DIRECTIONS[d ^ 1]) | (1 << a);
            let (mut file, mut rank) = ((a % 8) as i8, (a / 8) as i8);
            let mut walked = 0;
            loop {
                file += df;
                rank += dr;
                if !on_board(file, rank) {
                    break;
                }
                let b = (rank * 8 + file) as usize;
                between[a][b] = Bitboard(walked);
                line[a][b] = Bitboard(full_line);
                walked |= 1 << b;
            }
            d += 1;
        }
        a += 1;
    }
    (between, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn squares(names: &[&str]) -> Bitboard {
        names
            .iter()
            .map(|name| name.parse::<Square>().unwrap())
            .collect()
    }

    #[test]
    fn knight_and_king() {
        assert_eq!(knight_attacks(Square::A1), squares(&["b3", "c2"]));
        assert_eq!(knight_attacks(Square::E4).count(), 8);
        assert_eq!(king_attacks(Square::H8), squares(&["g8", "g7", "h7"]));
        assert_eq!(king_attacks(Square::E4).count(), 8);
    }

    #[test]
    fn pawns_do_not_wrap_around() {
        assert_eq!(pawn_attacks(Color::White, Square::A2), squares(&["b3"]));
        assert_eq!(pawn_attacks(Color::White, Square::H2), squares(&["g3"]));
        assert_eq!(
            pawn_attacks(Color::Black, Square::E5),
            squares(&["d4", "f4"])
        );
        assert!(pawn_attacks(Color::White, Square::E8).is_empty());
    }

    #[test]
    fn sliders_stop_on_blockers() {
        let occupied = squares(&["e6", "c4", "e2"]);
        assert_eq!(
            rook_attacks(Square::E4, occupied),
            squares(&["e5", "e6", "e3", "e2", "d4", "c4", "f4", "g4", "h4"])
        );
        assert_eq!(bishop_attacks(Square::A1, Bitboard::EMPTY).count(), 7);
        assert_eq!(queen_attacks(Square::D4, Bitboard::EMPTY).count(), 27);
    }

    #[test]
    fn between_and_line() {
        assert_eq!(between(Square::A1, Square::D4), squares(&["b2", "c3"]));
        assert_eq!(between(Square::D4, Square::A1), squares(&["b2", "c3"]));
        assert_eq!(between(Square::E1, Square::E2), Bitboard::EMPTY);
        assert_eq!(between(Square::A1, Square::B3), Bitboard::EMPTY);
        assert_eq!(line(Square::B2, Square::C3).count(), 8);
        assert!(line(Square::B2, Square::C3).contains(Square::H8));
        assert_eq!(line(Square::A1, Square::B3), Bitboard::EMPTY);
        assert_eq!(
            line(Square::E2, Square::E7),
            Bitboard::file(Square::E1.file())
        );
    }

    #[test]
    fn magic_lookups_match_reference_on_random_boards() {
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        for _ in 0..20_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            // Sparse boards, like real positions.
            let occupied = state & (state >> 3) & (state >> 7);
            let square = Square::from_u6((state >> 58) as u8);
            assert_eq!(
                rook_attacks(square, Bitboard(occupied)).0,
                sliding_attacks(square.index(), occupied, &ROOK_DIRECTIONS)
            );
            assert_eq!(
                bishop_attacks(square, Bitboard(occupied)).0,
                sliding_attacks(square.index(), occupied, &BISHOP_DIRECTIONS)
            );
        }
    }
}
