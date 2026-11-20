//! `BigUInt`: non-negative arbitrary-precision integers as little-endian
//! `u64` limb vectors, base 2^64.
//!
//! Canonical form: no trailing zero limbs; zero is the empty vector. All
//! constructors enforce the invariant; hot loops rely on it (debug builds
//! re-check via `debug_assert!`).

use crate::algos;
use crate::error::{MegaNumError, Result};
use crate::limb::{Limb, LIMB_BITS};

/// Non-negative big integer; canonical little-endian limb vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigUInt(pub(crate) Vec<Limb>);

impl Default for BigUInt {
    fn default() -> Self {
        Self::zero()
    }
}

impl BigUInt {
    /// Canonical zero (empty limb vector).
    pub fn zero() -> Self {
        BigUInt(Vec::new())
    }

    /// Canonical one.
    pub fn one() -> Self {
        BigUInt(vec![1])
    }

    /// Build from limbs, trimming trailing zeros to restore canonical form.
    pub fn from_limbs(mut limbs: Vec<Limb>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        BigUInt(limbs)
    }

    /// Limb slice, little-endian.
    pub fn limbs(&self) -> &[Limb] {
        &self.0
    }

    /// Number of stored limbs (0 for zero).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there are no limbs (canonical zero).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Consume and return the raw limb vector (canonical).
    pub fn into_limbs(self) -> Vec<Limb> {
        self.0
    }

    /// True if the value is zero.
    pub fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    /// True if the value is exactly one.
    pub fn is_one(&self) -> bool {
        self.0.len() == 1 && self.0[0] == 1
    }

    /// True if the value is exactly two.
    pub fn is_two(&self) -> bool {
        self.0.len() == 1 && self.0[0] == 2
    }

    /// True if the value is even.
    pub fn is_even(&self) -> bool {
        self.0.first().copied().unwrap_or(0) & 1 == 0
    }

    /// True if the value equals the small constant `v`.
    pub fn is_one_value(&self, v: u64) -> bool {
        self.0.len() == 1 && self.0[0] == v
    }

    /// Number of significant bits (0 for zero).
    pub fn bit_length(&self) -> usize {
        match self.0.last() {
            None => 0,
            Some(&top) => {
                (self.0.len() - 1) * LIMB_BITS as usize + crate::limb::limb_bits(top) as usize
            }
        }
    }

    /// Number of trailing zero bits; `None` for zero (undefined, like CPython).
    pub fn trailing_zeros(&self) -> Option<usize> {
        let mut idx = 0;
        while idx < self.0.len() && self.0[idx] == 0 {
            idx += 1;
        }
        self.0
            .get(idx)
            .map(|&l| idx * LIMB_BITS as usize + crate::limb::limb_trailing_zeros(l) as usize)
    }

    /// Population count of set bits.
    pub fn bit_count(&self) -> u32 {
        self.0.iter().map(|l| l.count_ones()).sum()
    }

    /// debug-only canonical form check (trailing zeros trimmed).
    pub(crate) fn is_canonical(&self) -> bool {
        !self.0.last().is_some_and(|l| *l == 0)
    }

    fn assert_canonical(&self) {
        debug_assert!(self.is_canonical(), "BigUInt must stay canonical");
    }

    /// Addition; the dispatcher records the decision and the SIMD backend
    /// (AVX2/NEON chunked kernels with a scalar fallback) executes it.
    pub fn add(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        let _ =
            crate::dispatch::decide(crate::dispatch::OpKind::Add, self.len(), Some(other.len()));
        #[cfg(feature = "simd")]
        return BigUInt::from_limbs(algos::backend_add(&self.0, &other.0));
        #[cfg(not(feature = "simd"))]
        BigUInt::from_limbs(algos::add::add_slices(&self.0, &other.0))
    }

    /// Subtraction; panics in debug if `self < other` (caller guarantees).
    /// Routes through the dispatcher like `add`.
    pub fn sub(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        other.assert_canonical();
        debug_assert!(
            self.cmp(other) != std::cmp::Ordering::Less,
            "BigUInt::sub requires self >= other"
        );
        let _ =
            crate::dispatch::decide(crate::dispatch::OpKind::Sub, self.len(), Some(other.len()));
        #[cfg(feature = "simd")]
        return BigUInt::from_limbs(algos::backend_sub(&self.0, &other.0));
        #[cfg(not(feature = "simd"))]
        BigUInt::from_limbs(algos::add::sub_slices(&self.0, &other.0))
    }

    /// Multiply via the best available algorithm for the operand sizes.
    pub fn mul(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        other.assert_canonical();
        let mut scratch = algos::Scratch::new();
        BigUInt::from_limbs(algos::mul::mul_best(&self.0, &other.0, &mut scratch))
    }

    /// Floor division by a nonzero divisor; returns (quotient, remainder).
    pub fn divmod(&self, v: &BigUInt) -> Result<(BigUInt, BigUInt)> {
        self.assert_canonical();
        v.assert_canonical();
        if v.is_zero() {
            return Err(MegaNumError::DivisionByZero);
        }
        let (q, r) = algos::div::divmod_best(&self.0, &v.0);
        Ok((BigUInt::from_limbs(q), BigUInt::from_limbs(r)))
    }

    /// Shift left by `n` bits.
    pub fn shl(&self, n: usize) -> BigUInt {
        self.assert_canonical();
        if self.is_zero() {
            return BigUInt::zero();
        }
        let limbs = n / LIMB_BITS as usize;
        let bits = (n % LIMB_BITS as usize) as u32;
        let mut out = vec![0u64; limbs];
        if bits == 0 {
            out.extend_from_slice(&self.0);
        } else {
            let mut carry = 0u64;
            for &l in &self.0 {
                out.push((l << bits) | carry);
                carry = l >> (LIMB_BITS - bits);
            }
            if carry != 0 {
                out.push(carry);
            }
        }
        BigUInt::from_limbs(out)
    }

    /// Shift right by `n` bits (floor; self is non-negative so exact floor).
    pub fn shr(&self, n: usize) -> BigUInt {
        self.assert_canonical();
        let limbs = n / LIMB_BITS as usize;
        if limbs >= self.0.len() {
            return BigUInt::zero();
        }
        let bits = (n % LIMB_BITS as usize) as u32;
        let src = &self.0[limbs..];
        if bits == 0 {
            BigUInt::from_limbs(src.to_vec())
        } else {
            let mut out = Vec::with_capacity(src.len());
            for i in 0..src.len() {
                let hi = if i + 1 < src.len() {
                    src[i + 1] << (LIMB_BITS - bits)
                } else {
                    0
                };
                out.push((src[i] >> bits) | hi);
            }
            BigUInt::from_limbs(out)
        }
    }

    /// Bitwise AND of non-negative values.
    pub fn bit_and(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        BigUInt::from_limbs(
            self.0
                .iter()
                .zip(other.0.iter())
                .map(|(a, b)| a & b)
                .collect(),
        )
    }

    /// Bitwise OR of non-negative values.
    pub fn bit_or(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        let n = self.0.len().max(other.0.len());
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let a = self.0.get(i).copied().unwrap_or(0);
            let b = other.0.get(i).copied().unwrap_or(0);
            out.push(a | b);
        }
        BigUInt::from_limbs(out)
    }

    /// Bitwise AND-NOT: `self & !other` over non-negative values.
    pub fn bit_and_not(&self, other: &BigUInt) -> BigUInt {
        self.assert_canonical();
        BigUInt::from_limbs(
            self.0
                .iter()
                .enumerate()
                .map(|(i, a)| a & !other.0.get(i).copied().unwrap_or(0))
                .collect(),
        )
    }

    /// Bit at position `idx` (false beyond the top).
    pub fn bit(&self, idx: usize) -> bool {
        let limb = idx / LIMB_BITS as usize;
        let off = (idx % LIMB_BITS as usize) as u32;
        self.0.get(limb).is_some_and(|l| (l >> off) & 1 == 1)
    }

    /// Parse a magnitude in `radix` (2..=62) from `s` (no sign, no prefix).
    pub fn parse_mag_radix(s: &str, radix: u32) -> Result<BigUInt> {
        algos::convert::parse_mag_radix(s, radix)
    }

    /// Render the magnitude in `radix` (2..=62), lowercase digits.
    pub fn to_string_radix(&self, radix: u32) -> Result<String> {
        algos::convert::to_string_radix(&self.0, radix)
    }

    /// Decode from little-endian bytes (unsigned).
    pub fn from_bytes_le(bytes: &[u8]) -> BigUInt {
        let mut limbs = bytes
            .chunks(8)
            .map(|c| {
                let mut buf = [0u8; 8];
                buf[..c.len()].copy_from_slice(c);
                u64::from_le_bytes(buf)
            })
            .collect::<Vec<Limb>>();
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        BigUInt(limbs)
    }

    /// Minimal little-endian byte encoding (empty for zero).
    pub fn to_bytes_le(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.0.len() * 8);
        for l in &self.0 {
            out.extend_from_slice(&l.to_le_bytes());
        }
        while out.last() == Some(&0) {
            out.pop();
        }
        out
    }

    /// Decode from big-endian bytes (unsigned).
    pub fn from_bytes_be(bytes: &[u8]) -> BigUInt {
        let rev: Vec<u8> = bytes.iter().rev().copied().collect();
        BigUInt::from_bytes_le(&rev)
    }

    /// Minimal big-endian byte encoding (empty for zero; non-empty keeps a
    /// leading byte even if it is zero-padded by callers via `length`).
    pub fn to_bytes_be(&self) -> Vec<u8> {
        let mut out = self.to_bytes_le();
        out.reverse();
        out
    }
}

impl std::fmt::Display for BigUInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_string_radix(10) {
            Ok(s) => f.write_str(&s),
            Err(_) => Err(std::fmt::Error),
        }
    }
}

impl From<u64> for BigUInt {
    fn from(v: u64) -> Self {
        if v == 0 {
            BigUInt::zero()
        } else {
            BigUInt(vec![v])
        }
    }
}

impl From<u32> for BigUInt {
    fn from(v: u32) -> Self {
        BigUInt::from(v as u64)
    }
}

impl TryFrom<&BigUInt> for u64 {
    type Error = MegaNumError;
    fn try_from(v: &BigUInt) -> Result<u64> {
        match v.0.len() {
            0 => Ok(0),
            1 => Ok(v.0[0]),
            _ => Err(MegaNumError::Overflow(
                "value does not fit in u64".to_string(),
            )),
        }
    }
}

impl Ord for BigUInt {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.assert_canonical();
        other.assert_canonical();
        match self.0.len().cmp(&other.0.len()) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        for i in (0..self.0.len()).rev() {
            match self.0[i].cmp(&other.0[i]) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl PartialOrd for BigUInt {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::add::{add_slices, sub_slices};

    fn b(v: Vec<u64>) -> BigUInt {
        BigUInt::from_limbs(v)
    }

    #[test]
    fn canonical_zero_and_from_limbs() {
        assert!(BigUInt::zero().is_zero());
        // Low zero limbs are legitimate; only trailing (top) zeros trim.
        assert_eq!(BigUInt::from_limbs(vec![5, 0, 0]).limbs(), &[5]);
        assert_eq!(BigUInt::from_limbs(vec![0, 0, 5]).limbs(), &[0, 0, 5]);
        assert_eq!(BigUInt::from_limbs(vec![0, 0]), BigUInt::zero());
    }

    #[test]
    fn add_sub_roundtrip() {
        let a = b(vec![u64::MAX, 3, 7]);
        let c = b(vec![2, 9]);
        let s = a.add(&c);
        assert_eq!(s.limbs()[0], 1);
        assert_eq!(s.limbs()[1], 13);
        assert_eq!(s.limbs()[2], 7);
        assert_eq!(s.sub(&c), a);
        assert_eq!(add_slices(&[u64::MAX], &[1]), vec![0, 1]);
        assert_eq!(sub_slices(&[0, 1], &[1]), vec![u64::MAX]);
    }

    #[test]
    fn cmp_ordering() {
        assert_eq!(
            b(vec![1, 1]).cmp(&b(vec![u64::MAX])),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            b(vec![u64::MAX, u64::MAX]).cmp(&b(vec![0, u64::MAX, 1])),
            std::cmp::Ordering::Less
        );
        assert_eq!(b(vec![3, 4]).cmp(&b(vec![3, 4])), std::cmp::Ordering::Equal);
    }

    #[test]
    fn shifts() {
        let a = b(vec![1]);
        assert_eq!(a.shl(64).limbs(), &[0, 1]);
        assert_eq!(a.shl(65).limbs(), &[0, 2]);
        assert_eq!(a.shl(3).limbs(), &[8]);
        assert_eq!(b(vec![0, 2]).shr(65), BigUInt::one());
        assert_eq!(b(vec![8]).shr(3), BigUInt::one());
        assert_eq!(b(vec![3]).shr(1).limbs(), &[1]);
        assert!(b(vec![1]).shl(1000).shr(1000) == BigUInt::one());
        assert!(BigUInt::zero().shl(5).is_zero());
    }

    #[test]
    fn bit_ops() {
        let a = b(vec![0b1010, 0b11]);
        let c = b(vec![0b0110, 0b01]);
        assert_eq!(a.bit_and(&c), b(vec![0b0010, 0b01]));
        assert_eq!(a.bit_or(&c), b(vec![0b1110, 0b11]));
        assert_eq!(a.bit_and_not(&c), b(vec![0b1000, 0b10]));
        assert_eq!(a.bit_and(&BigUInt::zero()), BigUInt::zero());
        assert_eq!(a.bit_or(&BigUInt::zero()), a);
    }

    #[test]
    fn bit_helpers() {
        assert_eq!(BigUInt::zero().bit_length(), 0);
        assert_eq!(BigUInt::one().bit_length(), 1);
        assert_eq!(b(vec![0, 1]).bit_length(), 65);
        assert_eq!(b(vec![0b1100]).bit_count(), 2);
        assert_eq!(b(vec![0, 0, 0b0100]).trailing_zeros(), Some(130));
        assert_eq!(BigUInt::zero().trailing_zeros(), None);
        assert!(b(vec![0, 1]).bit(64));
        assert!(!b(vec![0, 1]).bit(65));
    }

    #[test]
    fn bytes_roundtrip() {
        let a = b(vec![0x0102_0304_0506_0708, 0xF1F2_F3F4_F5F6_F7F8]);
        let le = a.to_bytes_le();
        assert_eq!(BigUInt::from_bytes_le(&le), a);
        let be = a.to_bytes_be();
        assert_eq!(be.len(), 16);
        assert_eq!(BigUInt::from_bytes_be(&be), a);
        assert!(BigUInt::zero().to_bytes_le().is_empty());
        assert!(BigUInt::zero().to_bytes_be().is_empty());
        assert_eq!(BigUInt::from_bytes_be(&[1, 0]), b(vec![0x0100]));
    }

    #[test]
    fn small_parse_format_decimal() {
        let a = BigUInt::parse_mag_radix("123456789012345678901234567890", 10).unwrap();
        assert_eq!(
            a.to_string_radix(10).unwrap(),
            "123456789012345678901234567890"
        );
        assert_eq!(BigUInt::parse_mag_radix("0", 10).unwrap(), BigUInt::zero());
        assert_eq!(
            a.divmod(&b(vec![7]))
                .unwrap()
                .0
                .to_string_radix(10)
                .unwrap(),
            "17636684144620811271604938270"
        );
        assert_eq!(a.divmod(&b(vec![7])).unwrap().1, BigUInt::zero());
    }

    #[test]
    fn mul_small_matches_schoolbook() {
        let a = b(vec![u64::MAX, u64::MAX >> 1]);
        let c = a.mul(&a);
        let direct = algos::mul::schoolbook::mul(&a.0, &a.0);
        assert_eq!(c.into_limbs(), BigUInt::from_limbs(direct).into_limbs());
    }

    #[test]
    fn try_from_u64() {
        assert_eq!(u64::try_from(&b(vec![42])).unwrap(), 42);
        assert_eq!(u64::try_from(&BigUInt::zero()).unwrap(), 0);
        assert!(u64::try_from(&b(vec![1, 1])).is_err());
    }

    #[test]
    fn divmod_basics() {
        let a = b(vec![10, 1]);
        let (q, r) = a.divmod(&b(vec![3])).unwrap();
        assert_eq!(r.limbs(), &[2]); // (2^64 + 10) mod 3 = 2
        assert!(q.mul(&b(vec![3])).add(&r) == a);
        let (q, r) = BigUInt::from(100u64).divmod(&BigUInt::from(10u64)).unwrap();
        assert_eq!(q, BigUInt::from(10u64));
        assert!(r.is_zero());
        assert!(a.divmod(&BigUInt::zero()).is_err());
    }
}
