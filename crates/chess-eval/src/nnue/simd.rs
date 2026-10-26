//! Vectorized kernels of NNUE inference.
//!
//! Each kernel has a portable version, written in fixed-size chunks so that
//! the compiler vectorizes it for the build target (SSE2 on x86-64, NEON on
//! AArch64), and an AVX2 version picked at runtime on x86-64 CPUs that have it.
//! Both give bit-identical results.
//!
//! # Safety
//!
//! This is the only `unsafe` code of the crate, for two reasons:
//!
//! - calling an AVX2 function from code compiled without AVX2, which is done
//!   only after `is_x86_feature_detected!("avx2")` returned true;
//! - unaligned 256-bit loads and stores, which go through [`avx2::load`] and
//!   [`avx2::store`]: they take a reference to exactly 16 `i16`, so they can
//!   never touch memory outside it.
#![allow(unsafe_code)]

use super::{HIDDEN, QA};

/// One perspective of an accumulator, or one row of weights.
pub(super) type Row = [i16; HIDDEN];

/// `i16` lanes of one AVX2 register.
const LANES: usize = 16;
const CHUNKS: usize = HIDDEN / LANES;
/// Registers updated together: the tile stays in registers while every
/// added and removed row is applied to it, instead of looping over the rows
/// once per register (which is bound by branches, not arithmetic).
const TILE: usize = 8;
const _: () = assert!(HIDDEN.is_multiple_of(LANES * TILE));

#[inline]
fn chunk(row: &Row, k: usize) -> &[i16; LANES] {
    &row.as_chunks::<LANES>().0[k]
}

/// `out = base + Σ add − Σ sub`, wrapping like the `i16` accumulators.
pub(super) fn update(out: &mut Row, base: &Row, add: &[&Row], sub: &[&Row]) {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        unsafe { avx2::update(out, base, add, sub) };
        return;
    }
    portable::update(out, base, add, sub);
}

/// `Σ clamp(acc, 0, QA) · weights`: the clipped ReLU of one accumulator
/// perspective through its half of the output layer. Exact in `i32` for any
/// inputs, since `|Σ| ≤ 256 · 255 · 32768 < 2³¹`.
pub(super) fn crelu_dot(acc: &Row, weights: &Row) -> i32 {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        return unsafe { avx2::crelu_dot(acc, weights) };
    }
    portable::crelu_dot(acc, weights)
}

mod portable {
    use super::{CHUNKS, HIDDEN, LANES, QA, Row, chunk};

    /// Same tiling as the AVX2 version, in 64-value tiles (8 SSE2 or NEON
    /// registers) that the compiler keeps in registers.
    pub(super) fn update(out: &mut Row, base: &Row, add: &[&Row], sub: &[&Row]) {
        const TILE: usize = 64;
        let tile = |row: &Row, t: usize| row.as_chunks::<TILE>().0[t];
        for t in 0..HIDDEN / TILE {
            let mut values = tile(base, t);
            for row in add {
                for (v, w) in values.iter_mut().zip(tile(row, t)) {
                    *v = v.wrapping_add(w);
                }
            }
            for row in sub {
                for (v, w) in values.iter_mut().zip(tile(row, t)) {
                    *v = v.wrapping_sub(w);
                }
            }
            out.as_chunks_mut::<TILE>().0[t] = values;
        }
    }

    pub(super) fn crelu_dot(acc: &Row, weights: &Row) -> i32 {
        let mut sums = [0_i32; LANES];
        for k in 0..CHUNKS {
            for ((s, &a), &w) in sums.iter_mut().zip(chunk(acc, k)).zip(chunk(weights, k)) {
                *s += i32::from(a.clamp(0, QA as i16)) * i32::from(w);
            }
        }
        sums.iter().sum()
    }
}

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use std::arch::x86_64::{
        __m256i, _mm_add_epi32, _mm_cvtsi128_si32, _mm_shuffle_epi32, _mm256_add_epi16,
        _mm256_add_epi32, _mm256_castsi256_si128, _mm256_extracti128_si256, _mm256_loadu_si256,
        _mm256_madd_epi16, _mm256_max_epi16, _mm256_min_epi16, _mm256_set1_epi16,
        _mm256_setzero_si256, _mm256_storeu_si256, _mm256_sub_epi16,
    };

    use super::{CHUNKS, LANES, QA, Row, TILE, chunk};

    #[inline]
    fn chunk_mut(row: &mut Row, k: usize) -> &mut [i16; LANES] {
        &mut row.as_chunks_mut::<LANES>().0[k]
    }

    /// Whether this CPU supports AVX2 (detected once, then cached by std).
    pub(super) fn available() -> bool {
        std::arch::is_x86_feature_detected!("avx2")
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    pub(super) fn load(values: &[i16; LANES]) -> __m256i {
        // SAFETY: `values` is 32 readable bytes; the load is unaligned.
        unsafe { _mm256_loadu_si256(values.as_ptr().cast()) }
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    pub(super) fn store(values: &mut [i16; LANES], v: __m256i) {
        // SAFETY: `values` is 32 writable bytes; the store is unaligned.
        unsafe { _mm256_storeu_si256(values.as_mut_ptr().cast(), v) }
    }

    #[target_feature(enable = "avx2")]
    pub(super) fn update(out: &mut Row, base: &Row, add: &[&Row], sub: &[&Row]) {
        for first in (0..CHUNKS).step_by(TILE) {
            let mut tile = [_mm256_setzero_si256(); TILE];
            for (i, v) in tile.iter_mut().enumerate() {
                *v = load(chunk(base, first + i));
            }
            for row in add {
                for (i, v) in tile.iter_mut().enumerate() {
                    *v = _mm256_add_epi16(*v, load(chunk(row, first + i)));
                }
            }
            for row in sub {
                for (i, v) in tile.iter_mut().enumerate() {
                    *v = _mm256_sub_epi16(*v, load(chunk(row, first + i)));
                }
            }
            for (i, v) in tile.into_iter().enumerate() {
                store(chunk_mut(out, first + i), v);
            }
        }
    }

    #[target_feature(enable = "avx2")]
    pub(super) fn crelu_dot(acc: &Row, weights: &Row) -> i32 {
        let zero = _mm256_setzero_si256();
        let one = _mm256_set1_epi16(QA as i16);
        let mut sum = _mm256_setzero_si256();
        for k in 0..CHUNKS {
            let clipped = _mm256_min_epi16(_mm256_max_epi16(load(chunk(acc, k)), zero), one);
            // Pairs of products summed into i32 lanes: |a·w + a'·w'| ≤ 2 · 255 · 32768.
            sum = _mm256_add_epi32(sum, _mm256_madd_epi16(clipped, load(chunk(weights, k))));
        }
        let half = _mm_add_epi32(
            _mm256_castsi256_si128(sum),
            _mm256_extracti128_si256::<1>(sum),
        );
        let quarter = _mm_add_epi32(half, _mm_shuffle_epi32::<0b01_00_11_10>(half));
        let total = _mm_add_epi32(quarter, _mm_shuffle_epi32::<0b10_11_00_01>(quarter));
        _mm_cvtsi128_si32(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_row(seed: &mut u64) -> Box<Row> {
        let mut row = Box::new([0_i16; HIDDEN]);
        for value in row.iter_mut() {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            *value = *seed as i16;
        }
        row
    }

    #[test]
    fn dispatched_kernels_match_the_portable_ones() {
        let mut seed = 0xDEAD_BEEF_u64;
        let mut extreme = Box::new([0_i16; HIDDEN]);
        for (i, value) in extreme.iter_mut().enumerate() {
            *value = [i16::MIN, i16::MAX, -1, 0, QA as i16, QA as i16 + 1][i % 6];
        }
        for _ in 0..50 {
            let (base, a, b, c) = (
                random_row(&mut seed),
                random_row(&mut seed),
                random_row(&mut seed),
                random_row(&mut seed),
            );
            let (mut fast, mut reference) = ([0; HIDDEN], [0; HIDDEN]);
            update(&mut fast, &base, &[&a, &extreme], &[&b, &c]);
            portable::update(&mut reference, &base, &[&a, &extreme], &[&b, &c]);
            assert_eq!(fast, reference);

            for (acc, weights) in [(&base, &a), (&extreme, &b), (&a, &extreme)] {
                let expected: i64 = acc
                    .iter()
                    .zip(weights.iter())
                    .map(|(&x, &w)| i64::from(x.clamp(0, QA as i16)) * i64::from(w))
                    .sum();
                assert_eq!(i64::from(crelu_dot(acc, weights)), expected);
                assert_eq!(i64::from(portable::crelu_dot(acc, weights)), expected);
            }
        }
    }

    #[test]
    fn worst_case_dot_product_does_not_overflow() {
        let acc = [QA as i16; HIDDEN];
        let weights = [i16::MIN; HIDDEN];
        let expected = i64::from(QA) * i64::from(i16::MIN) * HIDDEN as i64;
        assert_eq!(i64::from(crelu_dot(&acc, &weights)), expected);
        assert_eq!(i64::from(portable::crelu_dot(&acc, &weights)), expected);
    }
}
