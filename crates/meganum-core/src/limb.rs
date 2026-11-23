//! `u64` limb primitives in base 2^64.
//!
//! These are the only arithmetic primitives allowed to exist at this level;
//! everything above composes them. Half-open carry semantics: carries and
//! borrows are always 0 or 1 so they can be chained cheaply.

/// One digit of the internal base-2^64 representation.
pub type Limb = u64;

/// Number of bits per limb.
pub const LIMB_BITS: u32 = 64;

/// Maximum decimal digits that fit in one limb chunk for fast parsing:
/// 10^19 < 2^64 < 10^20.
pub const DEC_CHUNK_DIGITS: usize = 19;

/// 10^19, the largest power of ten below 2^64; one decimal chunk.
pub const DEC_CHUNK_BASE: Limb = 10_000_000_000_000_000_000;

/// Full adder: returns `(sum, carry_out)` with `carry` in `{0, 1}`.
#[inline]
pub fn adc(a: Limb, b: Limb, carry: Limb) -> (Limb, Limb) {
    debug_assert!(carry <= 1);
    let t = a as u128 + b as u128 + carry as u128;
    (t as Limb, (t >> LIMB_BITS) as Limb)
}

/// Full subtractor: returns `(difference, borrow_out)` with `borrow` in `{0, 1}`.
#[inline]
pub fn sbb(a: Limb, b: Limb, borrow: Limb) -> (Limb, Limb) {
    debug_assert!(borrow <= 1);
    let t = (a as u128).wrapping_sub(b as u128 + borrow as u128);
    (t as Limb, ((t >> 127) as Limb))
}

/// Wide multiply: returns `(low, high)` of the 128-bit product.
#[inline]
pub fn mul_wide(a: Limb, b: Limb) -> (Limb, Limb) {
    let t = a as u128 * b as u128;
    (t as Limb, (t >> LIMB_BITS) as Limb)
}

/// Combined multiply-add with carry: `ret = a*b + c + carry`, returns
/// `(low, high)` of the 128-bit result.
#[inline]
pub fn mul_add_carry(a: Limb, b: Limb, c: Limb, carry: Limb) -> (Limb, Limb) {
    let t = a as u128 * b as u128 + c as u128 + carry as u128;
    (t as Limb, (t >> LIMB_BITS) as Limb)
}

/// Dividend setup for Knuth division of `u128` by `Limb`.
/// Returns `(quotient, remainder)` for `hi:lo / d` where `d > 0` and
/// `hi < d` (caller must normalize).
#[inline]
pub fn div_wide(hi: Limb, lo: Limb, d: Limb) -> (Limb, Limb) {
    debug_assert!(d != 0, "div_wide divisor must be nonzero");
    debug_assert!(hi < d, "div_wide requires hi < d for exact quotient");
    let t = ((hi as u128) << LIMB_BITS) | lo as u128;
    ((t / d as u128) as Limb, (t % d as u128) as Limb)
}

/// Number of significant bits of a limb (0 for 0).
#[inline]
pub fn limb_bits(x: Limb) -> u32 {
    LIMB_BITS - x.leading_zeros()
}

/// Number of trailing zero bits of a limb (64 for 0).
#[inline]
pub fn limb_trailing_zeros(x: Limb) -> u32 {
    if x == 0 {
        LIMB_BITS
    } else {
        x.trailing_zeros()
    }
}

/// Population count of a limb.
#[inline]
pub fn limb_bit_count(x: Limb) -> u32 {
    x.count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adc_chain_overflows() {
        let (s, c) = adc(Limb::MAX, 1, 0);
        assert_eq!((s, c), (0, 1));
        let (s, c) = adc(Limb::MAX, Limb::MAX, 1);
        assert_eq!((s, c), (Limb::MAX, 1));
        assert_eq!(adc(1, 2, 0), (3, 0));
    }

    #[test]
    fn sbb_chain_borrows() {
        assert_eq!(sbb(0, 1, 0), (Limb::MAX, 1));
        assert_eq!(sbb(0, 0, 1), (Limb::MAX, 1));
        assert_eq!(sbb(5, 3, 1), (1, 0));
    }

    #[test]
    fn mul_wide_roundtrip() {
        assert_eq!(mul_wide(0, 0), (0, 0));
        // (2^64-1)^2 = 2^128 - 2^65 + 1: low limb 1, high limb 2^64-2.
        let (lo, hi) = mul_wide(Limb::MAX, Limb::MAX);
        assert_eq!(lo, 1);
        assert_eq!(hi, Limb::MAX - 1);
        assert_eq!(mul_wide(1 << 32, 1 << 32), (0, 1));
    }

    #[test]
    fn div_wide_exact() {
        assert_eq!(div_wide(0, 10, 3), (3, 1));
        assert_eq!(div_wide(1, 0, 2), ((1u64 << 63), 0));
    }

    #[test]
    fn bit_helpers() {
        assert_eq!(limb_bits(0), 0);
        assert_eq!(limb_bits(1), 1);
        assert_eq!(limb_bits(Limb::MAX), 64);
        assert_eq!(limb_trailing_zeros(0), 64);
        assert_eq!(limb_trailing_zeros(1 << 40), 40);
        assert_eq!(limb_bit_count(0b1011), 3);
    }
}
