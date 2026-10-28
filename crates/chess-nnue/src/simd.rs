//! Vectorized kernels of NNUE inference, on rows whose length is a multiple
//! of 16 (the architecture schema guarantees it for accumulators).
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

/// `i16` lanes of one AVX2 register.
pub(crate) const LANES: usize = 16;

type Chunk = [i16; LANES];

/// The `LANES`-wide chunks of a row (its length is a multiple of `LANES`).
#[inline]
fn chunks(row: &[i16]) -> &[Chunk] {
    row.as_chunks::<LANES>().0
}

/// `T` chunks of `row` from chunk `first`, as an array (one bound check).
#[inline]
fn block<const T: usize>(row: &[Chunk], first: usize) -> Option<&[Chunk; T]> {
    row.get(first..first + T)?.try_into().ok()
}

/// `out = base + Σ add − Σ sub`, wrapping like the `i16` accumulators. All
/// rows have the length of `out`.
///
/// Common accumulator sizes run kernels compiled for their exact length
/// (fully unrolled, no bound checks); the others run the slice kernels.
pub(crate) fn update(out: &mut [i16], base: &[i16], add: &[&[i16]], sub: &[&[i16]]) {
    match out.len() {
        256 => update_fixed::<256>(out, base, add, sub),
        512 => update_fixed::<512>(out, base, add, sub),
        768 => update_fixed::<768>(out, base, add, sub),
        1024 => update_fixed::<1024>(out, base, add, sub),
        1536 => update_fixed::<1536>(out, base, add, sub),
        2048 => update_fixed::<2048>(out, base, add, sub),
        _ => update_slices(out, base, add, sub),
    }
}

/// [`update`] for `N` values, a multiple of 128 (one AVX2 tile), on at
/// most 32 rows of each kind.
fn update_fixed<const N: usize>(out: &mut [i16], base: &[i16], add: &[&[i16]], sub: &[&[i16]]) {
    let (Ok(out), Ok(base)) = (<&mut [i16; N]>::try_from(out), <&[i16; N]>::try_from(base)) else {
        return;
    };
    // A move changes at most 4 features; a refresh adds up to 32.
    if add.len() <= 4 && sub.len() <= 4 {
        let ((add, n_add), (sub, n_sub)) = (arrays::<N, 4>(add, base), arrays::<N, 4>(sub, base));
        update_arrays(out, base, &add[..n_add], &sub[..n_sub]);
    } else {
        let ((add, n_add), (sub, n_sub)) = (arrays::<N, 32>(add, base), arrays::<N, 32>(sub, base));
        update_arrays(out, base, &add[..n_add], &sub[..n_sub]);
    }
}

/// Up to `K` `rows` as arrays (`fill` pads the rest), and their count.
fn arrays<'a, const N: usize, const K: usize>(
    rows: &[&'a [i16]],
    fill: &'a [i16; N],
) -> ([&'a [i16; N]; K], usize) {
    let mut arrays = [fill; K];
    let mut count = 0;
    for &row in rows {
        if let (Ok(row), Some(slot)) = (<&[i16; N]>::try_from(row), arrays.get_mut(count)) {
            *slot = row;
            count += 1;
        }
    }
    (arrays, count)
}

/// [`update`] on rows of a size known at compile time (a multiple of 128).
pub(crate) fn update_arrays<const N: usize>(
    out: &mut [i16; N],
    base: &[i16; N],
    add: &[&[i16; N]],
    sub: &[&[i16; N]],
) {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        unsafe { avx2::update_fixed(out, base, add, sub) };
        return;
    }
    portable::update_fixed(out, base, add, sub);
}

fn update_slices(out: &mut [i16], base: &[i16], add: &[&[i16]], sub: &[&[i16]]) {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        unsafe { avx2::update(out, base, add, sub) };
        return;
    }
    portable::update(out, base, add, sub);
}

/// `Σ clamp(acc, 0, qa) · weights`: a clipped ReLU through one row of a
/// dense layer. The caller guarantees that the sum fits in an `i32`.
pub(crate) fn crelu_dot(acc: &[i16], weights: &[i16], qa: i16) -> i32 {
    match acc.len() {
        256 => crelu_dot_fixed::<256>(acc, weights, qa),
        512 => crelu_dot_fixed::<512>(acc, weights, qa),
        768 => crelu_dot_fixed::<768>(acc, weights, qa),
        1024 => crelu_dot_fixed::<1024>(acc, weights, qa),
        _ => crelu_dot_slices(acc, weights, qa),
    }
}

fn crelu_dot_fixed<const N: usize>(acc: &[i16], weights: &[i16], qa: i16) -> i32 {
    let (Ok(acc), Ok(weights)) = (<&[i16; N]>::try_from(acc), <&[i16; N]>::try_from(weights))
    else {
        return 0;
    };
    crelu_dot_arrays(acc, weights, qa)
}

/// [`crelu_dot`] on rows of a size known at compile time (a multiple of 16).
pub(crate) fn crelu_dot_arrays<const N: usize>(acc: &[i16; N], weights: &[i16; N], qa: i16) -> i32 {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        return unsafe { avx2::crelu_dot_fixed(acc, weights, qa) };
    }
    portable::crelu_dot(acc, weights, qa)
}

fn crelu_dot_slices(acc: &[i16], weights: &[i16], qa: i16) -> i32 {
    #[cfg(target_arch = "x86_64")]
    if avx2::available() {
        // SAFETY: this CPU supports AVX2 (checked just above).
        return unsafe { avx2::crelu_dot(acc, weights, qa) };
    }
    portable::crelu_dot(acc, weights, qa)
}

/// `Σ a · w` over two rows of any (equal) length. The caller guarantees
/// that the sum fits in an `i32`.
pub(crate) fn dot(a: &[i16], w: &[i16]) -> i32 {
    let (head, tail) = a.as_chunks::<LANES>();
    let (w_head, w_tail) = w.as_chunks::<LANES>();
    let mut sums = [0_i32; LANES];
    for (a, w) in head.iter().zip(w_head) {
        for ((s, &a), &w) in sums.iter_mut().zip(a).zip(w) {
            *s += i32::from(a) * i32::from(w);
        }
    }
    let tail: i32 = tail
        .iter()
        .zip(w_tail)
        .map(|(&a, &w)| i32::from(a) * i32::from(w))
        .sum();
    sums.iter().sum::<i32>() + tail
}

mod portable {
    use super::{Chunk, LANES, block, chunks};

    /// Tiles of 4 chunks (64 values: 8 SSE2 or NEON registers) that the
    /// compiler keeps in registers, then single chunks.
    pub(super) fn update(out: &mut [i16], base: &[i16], add: &[&[i16]], sub: &[&[i16]]) {
        let n = out.len() / LANES;
        let out = out.as_chunks_mut::<LANES>().0;
        let base = chunks(base);
        let mut first = 0;
        while first < n {
            if n - first >= 4 {
                tile::<4>(out, base, add, sub, first);
                first += 4;
            } else {
                tile::<1>(out, base, add, sub, first);
                first += 1;
            }
        }
    }

    fn tile<const T: usize>(
        out: &mut [Chunk],
        base: &[Chunk],
        add: &[&[i16]],
        sub: &[&[i16]],
        first: usize,
    ) {
        let Some(mut values) = block::<T>(base, first).copied() else {
            return;
        };
        for row in add {
            if let Some(row) = block::<T>(chunks(row), first) {
                for (v, w) in values.as_flattened_mut().iter_mut().zip(row.as_flattened()) {
                    *v = v.wrapping_add(*w);
                }
            }
        }
        for row in sub {
            if let Some(row) = block::<T>(chunks(row), first) {
                for (v, w) in values.as_flattened_mut().iter_mut().zip(row.as_flattened()) {
                    *v = v.wrapping_sub(*w);
                }
            }
        }
        if let Some(out) = out.get_mut(first..first + T) {
            out.copy_from_slice(&values);
        }
    }

    /// [`update`] on rows of exactly `N` values, in tiles of 64.
    pub(super) fn update_fixed<const N: usize>(
        out: &mut [i16; N],
        base: &[i16; N],
        add: &[&[i16; N]],
        sub: &[&[i16; N]],
    ) {
        const T: usize = 64;
        let tile = |row: &[i16; N], t: usize| row.as_chunks::<T>().0[t];
        for t in 0..N / T {
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
            out.as_chunks_mut::<T>().0[t] = values;
        }
    }

    pub(super) fn crelu_dot(acc: &[i16], weights: &[i16], qa: i16) -> i32 {
        let mut sums = [0_i32; LANES];
        for (a, w) in chunks(acc).iter().zip(chunks(weights)) {
            for ((s, &a), &w) in sums.iter_mut().zip(a).zip(w) {
                *s += i32::from(a.clamp(0, qa)) * i32::from(w);
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

    use super::{Chunk, LANES, block, chunks};

    /// Registers updated together: the tile stays in registers while every
    /// added and removed row is applied to it, instead of looping over the
    /// rows once per register (which is bound by branches, not arithmetic).
    const TILE: usize = 8;

    /// Whether this CPU supports AVX2 (detected once, then cached by std).
    pub(super) fn available() -> bool {
        std::arch::is_x86_feature_detected!("avx2")
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    pub(super) fn load(values: &Chunk) -> __m256i {
        // SAFETY: `values` is 32 readable bytes; the load is unaligned.
        unsafe { _mm256_loadu_si256(values.as_ptr().cast()) }
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    pub(super) fn store(values: &mut Chunk, v: __m256i) {
        // SAFETY: `values` is 32 writable bytes; the store is unaligned.
        unsafe { _mm256_storeu_si256(values.as_mut_ptr().cast(), v) }
    }

    #[inline]
    fn chunk<const N: usize>(row: &[i16; N], k: usize) -> &Chunk {
        &row.as_chunks::<LANES>().0[k]
    }

    /// [`update`] on rows of exactly `N` values (a multiple of `TILE` chunks).
    #[target_feature(enable = "avx2")]
    pub(super) fn update_fixed<const N: usize>(
        out: &mut [i16; N],
        base: &[i16; N],
        add: &[&[i16; N]],
        sub: &[&[i16; N]],
    ) {
        for first in (0..N / LANES).step_by(TILE) {
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
                store(&mut out.as_chunks_mut::<LANES>().0[first + i], v);
            }
        }
    }

    #[target_feature(enable = "avx2")]
    pub(super) fn update(out: &mut [i16], base: &[i16], add: &[&[i16]], sub: &[&[i16]]) {
        let n = out.len() / LANES;
        let out = out.as_chunks_mut::<LANES>().0;
        let base = chunks(base);
        let mut first = 0;
        while first < n {
            if n - first >= TILE {
                tile::<TILE>(out, base, add, sub, first);
                first += TILE;
            } else {
                tile::<1>(out, base, add, sub, first);
                first += 1;
            }
        }
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    fn tile<const T: usize>(
        out: &mut [Chunk],
        base: &[Chunk],
        add: &[&[i16]],
        sub: &[&[i16]],
        first: usize,
    ) {
        let Some(base) = block::<T>(base, first) else {
            return;
        };
        let mut tile = [_mm256_setzero_si256(); T];
        for (v, chunk) in tile.iter_mut().zip(base) {
            *v = load(chunk);
        }
        for row in add {
            if let Some(row) = block::<T>(chunks(row), first) {
                for (v, chunk) in tile.iter_mut().zip(row) {
                    *v = _mm256_add_epi16(*v, load(chunk));
                }
            }
        }
        for row in sub {
            if let Some(row) = block::<T>(chunks(row), first) {
                for (v, chunk) in tile.iter_mut().zip(row) {
                    *v = _mm256_sub_epi16(*v, load(chunk));
                }
            }
        }
        if let Some(out) = out.get_mut(first..first + T) {
            for (chunk, v) in out.iter_mut().zip(tile) {
                store(chunk, v);
            }
        }
    }

    /// [`crelu_dot`] on rows of exactly `N` values.
    #[target_feature(enable = "avx2")]
    pub(super) fn crelu_dot_fixed<const N: usize>(
        acc: &[i16; N],
        weights: &[i16; N],
        qa: i16,
    ) -> i32 {
        let zero = _mm256_setzero_si256();
        let one = _mm256_set1_epi16(qa);
        let mut sum = _mm256_setzero_si256();
        for k in 0..N / LANES {
            let clipped = _mm256_min_epi16(_mm256_max_epi16(load(chunk(acc, k)), zero), one);
            sum = _mm256_add_epi32(sum, _mm256_madd_epi16(clipped, load(chunk(weights, k))));
        }
        horizontal_sum(sum)
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    fn horizontal_sum(sum: __m256i) -> i32 {
        let half = _mm_add_epi32(
            _mm256_castsi256_si128(sum),
            _mm256_extracti128_si256::<1>(sum),
        );
        let quarter = _mm_add_epi32(half, _mm_shuffle_epi32::<0b01_00_11_10>(half));
        let total = _mm_add_epi32(quarter, _mm_shuffle_epi32::<0b10_11_00_01>(quarter));
        _mm_cvtsi128_si32(total)
    }

    #[target_feature(enable = "avx2")]
    pub(super) fn crelu_dot(acc: &[i16], weights: &[i16], qa: i16) -> i32 {
        let zero = _mm256_setzero_si256();
        let one = _mm256_set1_epi16(qa);
        let mut sum = _mm256_setzero_si256();
        for (a, w) in chunks(acc).iter().zip(chunks(weights)) {
            let clipped = _mm256_min_epi16(_mm256_max_epi16(load(a), zero), one);
            // Pairs of products summed into i32 lanes.
            sum = _mm256_add_epi32(sum, _mm256_madd_epi16(clipped, load(w)));
        }
        horizontal_sum(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QA: i16 = 255;

    fn random_row(seed: &mut u64, len: usize) -> Vec<i16> {
        (0..len)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 7;
                *seed ^= *seed << 17;
                *seed as i16
            })
            .collect()
    }

    #[test]
    fn dispatched_kernels_match_the_portable_ones() {
        let mut seed = 0xDEAD_BEEF_u64;
        // Tiles only, tiles and single chunks, single chunks only.
        for len in [256, 512, 16 * 11, 48] {
            let extreme: Vec<i16> = (0..len)
                .map(|i| [i16::MIN, i16::MAX, -1, 0, QA, QA + 1][i % 6])
                .collect();
            for _ in 0..20 {
                let [base, a, b, c] = [(); 4].map(|()| random_row(&mut seed, len));
                let (mut fast, mut reference) = (vec![0; len], vec![0; len]);
                update(&mut fast, &base, &[&a, &extreme], &[&b, &c]);
                portable::update(&mut reference, &base, &[&a, &extreme], &[&b, &c]);
                assert_eq!(fast, reference);
                let expected: Vec<i16> = (0..len)
                    .map(|i| {
                        base[i]
                            .wrapping_add(a[i])
                            .wrapping_add(extreme[i])
                            .wrapping_sub(b[i])
                            .wrapping_sub(c[i])
                    })
                    .collect();
                assert_eq!(fast, expected);
                if len == 256 {
                    fn row(r: &[i16]) -> &[i16; 256] {
                        r.try_into().unwrap()
                    }
                    let mut fixed = [0; 256];
                    let (add, sub) = ([row(&a), row(&extreme)], [row(&b), row(&c)]);
                    portable::update_fixed(&mut fixed, row(&base), &add, &sub);
                    assert_eq!(fixed.as_slice(), expected);
                }

                // Small weights, so that the i32 sum cannot overflow.
                let small: Vec<i16> = a.iter().map(|w| w / 512).collect();
                for acc in [&base, &extreme, &b] {
                    let expected: i64 = acc
                        .iter()
                        .zip(&small)
                        .map(|(&x, &w)| i64::from(x.clamp(0, QA)) * i64::from(w))
                        .sum();
                    assert_eq!(i64::from(crelu_dot(acc, &small, QA)), expected);
                    assert_eq!(i64::from(portable::crelu_dot(acc, &small, QA)), expected);
                    let plain: i64 = acc
                        .iter()
                        .zip(&small)
                        .map(|(&x, &w)| i64::from(x / 4) * i64::from(w))
                        .sum();
                    let quarter: Vec<i16> = acc.iter().map(|x| x / 4).collect();
                    assert_eq!(i64::from(dot(&quarter, &small)), plain);
                }
            }
        }
    }

    #[test]
    fn dot_handles_any_length() {
        let a: Vec<i16> = (0..37).map(|i| i - 18).collect();
        let w: Vec<i16> = (0..37).map(|i| 3 * i - 50).collect();
        let expected: i32 = a
            .iter()
            .zip(&w)
            .map(|(&a, &w)| i32::from(a) * i32::from(w))
            .sum();
        assert_eq!(dot(&a, &w), expected);
    }

    #[test]
    fn worst_case_legacy_dot_product_does_not_overflow() {
        let acc = [QA; 256];
        let weights = [i16::MIN; 256];
        let expected = i64::from(QA) * i64::from(i16::MIN) * 256;
        assert_eq!(i64::from(crelu_dot(&acc, &weights, QA)), expected);
        assert_eq!(i64::from(portable::crelu_dot(&acc, &weights, QA)), expected);
    }
}
