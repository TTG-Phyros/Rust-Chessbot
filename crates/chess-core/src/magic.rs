//! Magic bitboards for sliding pieces ("fancy" layout: one shared table,
//! one variable-size slice per square).
//!
//! `index = ((occupied & mask) * magic) >> (64 - bits)` maps every relevant
//! occupancy of a square to a slot holding its attack set. The magic numbers
//! below were found by the `find_magics` generator (run it with
//! `cargo test -p chess-core find_magics -- --ignored --nocapture`); a test
//! checks that none of them has a harmful collision.

use std::sync::LazyLock;

use crate::Bitboard;
use crate::attacks::{BISHOP_DIRECTIONS, Direction, ROOK_DIRECTIONS, sliding_attacks};

/// Slider attack tables, built on first use (about 800 KiB).
pub(crate) static SLIDERS: LazyLock<Sliders> = LazyLock::new(Sliders::new);

#[derive(Debug, Clone, Copy)]
struct Magic {
    mask: u64,
    magic: u64,
    shift: u32,
    offset: usize,
}

impl Magic {
    #[inline]
    fn index(&self, occupied: u64) -> usize {
        self.offset + ((occupied & self.mask).wrapping_mul(self.magic) >> self.shift) as usize
    }
}

pub(crate) struct Sliders {
    rook: [Magic; 64],
    bishop: [Magic; 64],
    attacks: Vec<Bitboard>,
}

impl Sliders {
    fn new() -> Self {
        let mut attacks = Vec::with_capacity(102_400 + 5_248);
        let rook = build(&ROOK_MAGICS, &ROOK_DIRECTIONS, &mut attacks);
        let bishop = build(&BISHOP_MAGICS, &BISHOP_DIRECTIONS, &mut attacks);
        Self {
            rook,
            bishop,
            attacks,
        }
    }

    #[inline]
    pub(crate) fn rook(&self, square: usize, occupied: u64) -> Bitboard {
        self.lookup(&self.rook, square, occupied)
    }

    #[inline]
    pub(crate) fn bishop(&self, square: usize, occupied: u64) -> Bitboard {
        self.lookup(&self.bishop, square, occupied)
    }

    #[inline]
    fn lookup(&self, magics: &[Magic; 64], square: usize, occupied: u64) -> Bitboard {
        // Both lookups are in bounds by construction; `get` avoids any panic path.
        magics
            .get(square)
            .and_then(|magic| self.attacks.get(magic.index(occupied)))
            .copied()
            .unwrap_or_default()
    }
}

fn build(magics: &[u64; 64], directions: &[Direction], attacks: &mut Vec<Bitboard>) -> [Magic; 64] {
    std::array::from_fn(|square| {
        let mask = relevant_mask(square, directions);
        let bits = mask.count_ones();
        let magic = Magic {
            mask,
            magic: magics[square],
            shift: 64 - bits,
            offset: attacks.len(),
        };
        attacks.resize(magic.offset + (1 << bits), Bitboard::EMPTY);
        for subset in subsets(mask) {
            attacks[magic.index(subset)] = Bitboard(sliding_attacks(square, subset, directions));
        }
        magic
    })
}

/// Squares whose occupancy matters for a slider on `square`: its rays, minus
/// the last square of each ray (a blocker there changes nothing).
fn relevant_mask(square: usize, directions: &[Direction]) -> u64 {
    let mut mask = 0;
    for &(df, dr) in directions {
        let (mut file, mut rank) = ((square % 8) as i8, (square / 8) as i8);
        loop {
            file += df;
            rank += dr;
            let next_on_board = (0..8).contains(&(file + df)) && (0..8).contains(&(rank + dr));
            if !next_on_board {
                break;
            }
            mask |= 1 << (rank * 8 + file);
        }
    }
    mask
}

/// Every subset of `mask` (Carry-Rippler enumeration), starting with the empty set.
fn subsets(mask: u64) -> impl Iterator<Item = u64> {
    let mut next = Some(0_u64);
    std::iter::from_fn(move || {
        let current = next?;
        let following = current.wrapping_sub(mask) & mask;
        next = (following != 0).then_some(following);
        Some(current)
    })
}

#[rustfmt::skip]
const ROOK_MAGICS: [u64; 64] = [
    0x1080004008801020,
    0x0840092002c03000,
    0x1900200010400900,
    0x0880100008000480,
    0x4200100420080200,
    0x8100020100080400,
    0x0200040110886200,
    0x0200008040220411,
    0x0404800084400220,
    0x0000401000402000,
    0x0086001081220440,
    0x0408800800100280,
    0x000a001201040820,
    0x8848800200840080,
    0x4001000100040200,
    0x0442000102105084,
    0x9080010020804100,
    0x0040404000201009,
    0x0000808010002009,
    0x2200090021d00100,
    0x0008008008040080,
    0x0004004002010040,
    0x0011040008015042,
    0x00000a0001768104,
    0x0000800080204009,
    0x2010004140002001,
    0x9800200280100080,
    0x1000100080080080,
    0x0442000a00049020,
    0x2100040080020080,
    0x0800120400900148,
    0x0010040a00128541,
    0x2800804000800030,
    0x1010002000400041,
    0x4000200011004100,
    0x0610008410800800,
    0x0400802402800800,
    0xc100020080800400,
    0x0002000802000401,
    0x0182085882000401,
    0x0220204000808000,
    0x2860100040024022,
    0x0001002004110040,
    0x99101042000a0020,
    0x0004080004008080,
    0x0010040002008080,
    0x2012004881020004,
    0x8300842444820011,
    0x0088403882010200,
    0x0820400080210100,
    0x0110910040a00300,
    0x0801100280080480,
    0x0242009008200600,
    0x1002000489500200,
    0x0040800200010080,
    0x0091800041000080,
    0x0000209300488001,
    0x04c1002414824001,
    0x020020000b001041,
    0x7000100004200901,
    0x8002002004100802,
    0x30010002084c0007,
    0x0888221800813004,
    0x4000002840840112,
];

#[rustfmt::skip]
const BISHOP_MAGICS: [u64; 64] = [
    0xa010041108003100,
    0x006082020a002900,
    0x6810010619200000,
    0x08281a0520000408,
    0x0001104001000400,
    0x0018901008048400,
    0x00040a0210245280,
    0x000200210808a402,
    0x9140048410821200,
    0x0800091010820041,
    0x20504804832202c0,
    0x0100091401081000,
    0x8021011140000012,
    0x0810020804450400,
    0x208b0542109008a2,
    0x0080084a08040204,
    0x0040e2a80811244c,
    0x2505022008008108,
    0x0430220100420040,
    0x010a040420220040,
    0x1105000290400000,
    0x0093001200822120,
    0x4000a62048043004,
    0x280120048a015004,
    0x006090002a020814,
    0x44042000240800d0,
    0x01102800040a4400,
    0x1004080080220040,
    0x0001001011004024,
    0x0010044000805040,
    0x0914041200820100,
    0x0004821012821480,
    0x0024040500c05021,
    0x0088611002080200,
    0x0116080a00040020,
    0x4000020080080080,
    0x2450450140840040,
    0x0000880201484100,
    0x0222020404020092,
    0x8081110600002e00,
    0x2842101105000801,
    0x1100809008001025,
    0x00020202221c0400,
    0x0422014022009020,
    0x0210046102100c00,
    0xc004008082029102,
    0x00aa461801101200,
    0x0404080080201108,
    0x020542108c205002,
    0x0410544804100100,
    0x0040910841100000,
    0x0400200042021100,
    0x00004204850400c0,
    0x0200100410a42102,
    0x1040020801210102,
    0x0805040410420000,
    0x2884804130100200,
    0x800c262201242000,
    0x1058000194108800,
    0x0014221054420204,
    0x0104000012a02200,
    0x0200881003300100,
    0x0140400202840100,
    0x0402020801010201,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `magic` maps every occupancy of `square` to a slot that holds
    /// the right attacks (collisions are fine when the attacks are equal).
    fn is_valid(square: usize, directions: &[Direction], magic: u64) -> bool {
        let mask = relevant_mask(square, directions);
        let bits = mask.count_ones();
        let mut table = vec![None; 1 << bits];
        subsets(mask).all(|subset| {
            let index = ((subset.wrapping_mul(magic)) >> (64 - bits)) as usize;
            let attacks = sliding_attacks(square, subset, directions);
            match table[index] {
                None => {
                    table[index] = Some(attacks);
                    true
                }
                Some(existing) => existing == attacks,
            }
        })
    }

    #[test]
    fn stored_magics_are_valid() {
        for square in 0..64 {
            assert!(
                is_valid(square, &ROOK_DIRECTIONS, ROOK_MAGICS[square]),
                "rook {square}"
            );
            assert!(
                is_valid(square, &BISHOP_DIRECTIONS, BISHOP_MAGICS[square]),
                "bishop {square}"
            );
        }
    }

    #[test]
    fn mask_sizes() {
        assert_eq!(relevant_mask(0, &ROOK_DIRECTIONS).count_ones(), 12);
        assert_eq!(relevant_mask(27, &ROOK_DIRECTIONS).count_ones(), 10);
        assert_eq!(relevant_mask(0, &BISHOP_DIRECTIONS).count_ones(), 6);
        assert_eq!(relevant_mask(27, &BISHOP_DIRECTIONS).count_ones(), 9);
        assert_eq!(subsets(0b101).collect::<Vec<_>>(), vec![0, 1, 4, 5]);
    }

    #[test]
    #[ignore = "generator: prints new magic constants"]
    fn find_magics() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut random = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        for (name, directions) in [("ROOK", &ROOK_DIRECTIONS), ("BISHOP", &BISHOP_DIRECTIONS)] {
            println!("const {name}_MAGICS: [u64; 64] = [");
            for square in 0..64 {
                let mask = relevant_mask(square, directions);
                let magic = loop {
                    let candidate = random() & random() & random();
                    // Cheap filter: good magics spread the mask over the top byte.
                    if (mask.wrapping_mul(candidate) >> 56).count_ones() < 6 {
                        continue;
                    }
                    if is_valid(square, directions, candidate) {
                        break candidate;
                    }
                };
                println!("    {magic:#018x},");
            }
            println!("];");
        }
    }
}
