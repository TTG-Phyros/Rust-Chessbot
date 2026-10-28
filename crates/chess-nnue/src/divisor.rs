//! Division by a divisor fixed when a network is loaded, without the
//! division instruction (40 to 90 cycles for 64 bits on many CPUs):
//! Granlund and Montgomery, "Division by invariant integers using
//! multiplication" (1994), theorem 4.2. Results are exactly those of `/`.

/// Dividends must stay below this magnitude (2⁶²); every sum of a valid
/// architecture does, by far.
const DIVIDEND_BITS: u32 = 62;

/// A positive divisor and its reciprocal.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Divisor {
    multiplier: u64,
    shift: u32,
}

impl Divisor {
    /// `divisor` must be positive (and below 2³²).
    pub(crate) fn new(divisor: u64) -> Self {
        let divisor = divisor.max(1);
        // 2^(l-1) < divisor ≤ 2^l
        let l = u64::BITS - (divisor - 1).leading_zeros();
        let shift = DIVIDEND_BITS + l;
        // ⌈2^shift / divisor⌉ < 2^63
        let multiplier = (1_u128 << shift).div_ceil(u128::from(divisor)) as u64;
        Self { multiplier, shift }
    }

    /// `value / divisor`, truncated toward zero like `/`.
    #[inline]
    pub(crate) fn divide(self, value: i64) -> i64 {
        let magnitude = u128::from(value.unsigned_abs());
        let quotient = ((magnitude * u128::from(self.multiplier)) >> self.shift) as i64;
        if value < 0 { -quotient } else { quotient }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_division_operator() {
        let mut seed = 0x0123_4567_89AB_CDEF_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let limit = (1_i64 << DIVIDEND_BITS) - 1;
        for divisor in [
            1_u64,
            2,
            3,
            7,
            64,
            255,
            400,
            16_320,
            65_536,
            1_073_676_289,
            (1 << 32) - 1,
        ] {
            let d = Divisor::new(divisor);
            let edge = divisor as i64;
            for value in [
                0,
                1,
                -1,
                edge - 1,
                edge,
                edge + 1,
                -edge,
                -edge - 1,
                limit,
                -limit,
            ] {
                assert_eq!(d.divide(value), value / edge, "{value} / {divisor}");
            }
            for _ in 0..20_000 {
                let value = (next() as i64) >> (64 - DIVIDEND_BITS - 1);
                let value = value.clamp(-limit, limit);
                assert_eq!(d.divide(value), value / edge, "{value} / {divisor}");
                let small = (next() % 2_000_001) as i64 - 1_000_000;
                assert_eq!(d.divide(small), small / edge, "{small} / {divisor}");
            }
        }
    }
}
