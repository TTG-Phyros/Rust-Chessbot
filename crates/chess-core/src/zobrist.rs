//! Zobrist keys, generated at compile time from a fixed seed so that hashes
//! are identical across runs and machines (transposition tables, datasets).

use crate::{Piece, Square};

pub(crate) struct Keys {
    pieces: [[u64; 64]; 12],
    castling: [u64; 16],
    en_passant_file: [u64; 8],
    pub(crate) side: u64,
}

pub(crate) static KEYS: Keys = generate();

impl Keys {
    #[inline]
    pub(crate) fn piece(&self, piece: Piece, square: Square) -> u64 {
        self.pieces[piece.index()][square.index()]
    }

    #[inline]
    pub(crate) fn castling(&self, rights: u8) -> u64 {
        self.castling[(rights & 15) as usize]
    }

    #[inline]
    pub(crate) fn en_passant(&self, square: Square) -> u64 {
        self.en_passant_file[square.file().index() as usize]
    }
}

/// SplitMix64: tiny, well-distributed, and usable in `const fn`.
const fn split_mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const fn generate() -> Keys {
    let mut state = 0x00C0_FFEE_D15E_A5E5;
    let mut pieces = [[0; 64]; 12];
    let mut piece = 0;
    while piece < 12 {
        let mut square = 0;
        while square < 64 {
            pieces[piece][square] = split_mix(&mut state);
            square += 1;
        }
        piece += 1;
    }

    // Castling keys are the XOR of one key per right, so that
    // `castling(a) ^ castling(b)` is the key of the rights that changed.
    let mut single = [0; 4];
    let mut i = 0;
    while i < 4 {
        single[i] = split_mix(&mut state);
        i += 1;
    }
    let mut castling = [0; 16];
    let mut rights = 0;
    while rights < 16 {
        let mut bit = 0;
        while bit < 4 {
            if rights & (1 << bit) != 0 {
                castling[rights] ^= single[bit];
            }
            bit += 1;
        }
        rights += 1;
    }

    let mut en_passant_file = [0; 8];
    let mut file = 0;
    while file < 8 {
        en_passant_file[file] = split_mix(&mut state);
        file += 1;
    }

    Keys {
        pieces,
        castling,
        en_passant_file,
        side: split_mix(&mut state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_distinct_and_non_zero() {
        let mut all: Vec<u64> = KEYS.pieces.iter().flatten().copied().collect();
        all.extend(&KEYS.en_passant_file);
        all.extend(&KEYS.castling[1..]);
        all.push(KEYS.side);
        let count = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), count);
        assert!(all.iter().all(|&key| key != 0));
        assert_eq!(KEYS.castling(0), 0);
    }
}
