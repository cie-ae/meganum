//! `BigInt`: sign-and-magnitude arbitrary-precision signed integers with
//! CPython `int` semantics for floor division, remainder, bitwise operators
//! (infinite two's-complement interpretation) and arithmetic shifts.

use crate::algos;
use crate::biguint::BigUInt;
use crate::error::{MegaNumError, Result};
use crate::limb::LIMB_BITS;

/// Sign of a `BigInt`; zero is canonically `Positive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    /// Value >= 0.
    Positive,
    /// Value < 0.
    Negative,
}

impl Sign {
    /// Multiplication/XOR-like combination of two signs.
    pub fn xor(a: Sign, b: Sign) -> Sign {
        if a == b {
            Sign::Positive
        } else {
            Sign::Negative
        }
    }

    /// -1 raised to the sign: Positive -> +1, Negative -> -1.
    pub fn apply(a: Sign, mag: BigUInt) -> BigInt {
        BigInt::new(a, mag)
    }
}

/// Signed arbitrary-precision integer.
#[derive(Clone, PartialEq, Eq)]
pub struct BigInt {
    /// Sign; `Positive` iff `mag` is zero (canonical).
    pub(crate) sign: Sign,
    /// Magnitude, canonical (no trailing zero limbs).
    pub(crate) mag: BigUInt,
}

impl BigInt {
    /// Construct with canonicalization (zero gets a positive sign).
    pub fn new(sign: Sign, mag: BigUInt) -> BigInt {
        if mag.is_zero() {
            BigInt {
                sign: Sign::Positive,
                mag,
            }
        } else {
            BigInt { sign, mag }
        }
    }

    pub fn zero() -> BigInt {
        BigInt {
            sign: Sign::Positive,
            mag: BigUInt::zero(),
        }
    }

    pub fn one() -> BigInt {
        BigInt {
            sign: Sign::Positive,
            mag: BigUInt::one(),
        }
    }

    pub fn from_i64(v: i64) -> BigInt {
        if v >= 0 {
            BigInt {
                sign: Sign::Positive,
                mag: BigUInt::from(v as u64),
            }
        } else {
            let m = (v as i128).unsigned_abs() as u64;
            BigInt {
                sign: Sign::Negative,
                mag: BigUInt::from(m),
            }
        }
    }

    pub fn from_u64(v: u64) -> BigInt {
        BigInt {
            sign: Sign::Positive,
            mag: BigUInt::from(v),
        }
    }

    /// Sign component (Positive for zero).
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// Magnitude component.
    pub fn mag(&self) -> &BigUInt {
        &self.mag
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.sign == Sign::Negative
    }

    pub fn is_positive(&self) -> bool {
        self.sign == Sign::Positive && !self.mag.is_zero()
    }

    /// Negate (zero stays positive).
    pub fn neg(&self) -> BigInt {
        if self.is_zero() {
            self.clone()
        } else {
            BigInt {
                sign: Sign::xor(self.sign, Sign::Negative),
                mag: self.mag.clone(),
            }
        }
    }

    pub fn abs(&self) -> BigInt {
        BigInt {
            sign: Sign::Positive,
            mag: self.mag.clone(),
        }
    }

    /// Addition.
    pub fn add(&self, other: &BigInt) -> BigInt {
        if self.sign == other.sign {
            BigInt::new(self.sign, self.mag.add(&other.mag))
        } else {
            match self.mag.cmp(&other.mag) {
                std::cmp::Ordering::Equal => BigInt::zero(),
                std::cmp::Ordering::Greater => BigInt::new(self.sign, self.mag.sub(&other.mag)),
                std::cmp::Ordering::Less => BigInt::new(other.sign, other.mag.sub(&self.mag)),
            }
        }
    }

    /// Subtraction.
    pub fn sub(&self, other: &BigInt) -> BigInt {
        self.add(&other.neg())
    }

    /// Multiplication.
    pub fn mul(&self, other: &BigInt) -> BigInt {
        BigInt::new(Sign::xor(self.sign, other.sign), self.mag.mul(&other.mag))
    }

    /// Exponentiation with a non-negative exponent; sign follows parity.
    pub fn pow_u64(&self, exp: u64) -> BigInt {
        let mag = pow_mag(&self.mag, exp);
        let sign = if self.is_negative() && exp % 2 == 1 {
            Sign::Negative
        } else {
            Sign::Positive
        };
        BigInt::new(sign, mag)
    }

    /// Floor division and remainder (Python semantics):
    /// `self = q*other + r`, sign of `r` follows sign of `other`,
    /// `0 <= |r| < |other|`.
    pub fn divmod_floor(&self, other: &BigInt) -> Result<(BigInt, BigInt)> {
        if other.is_zero() {
            return Err(MegaNumError::DivisionByZero);
        }
        let (tq, tr) = self.mag.divmod(&other.mag)?;
        let same_sign = self.sign == other.sign;
        if tr.is_zero() {
            let q = BigInt::new(Sign::xor(self.sign, other.sign), tq);
            return Ok((q, BigInt::zero()));
        }
        if same_sign {
            // Quotient of same-sign operands is non-negative.
            Ok((
                BigInt::new(Sign::xor(self.sign, other.sign), tq),
                BigInt::new(other.sign, tr),
            ))
        } else {
            // Floor adjustment: q = -(tq + 1), r = |other| - tr.
            let q_mag = tq.add(&BigUInt::one());
            let r_mag = other.mag.sub(&tr);
            Ok((
                BigInt::new(Sign::Negative, q_mag),
                BigInt::new(other.sign, r_mag),
            ))
        }
    }

    /// Floor division (`//`).
    pub fn floordiv(&self, other: &BigInt) -> Result<BigInt> {
        self.divmod_floor(other).map(|(q, _)| q)
    }

    /// Remainder with sign of divisor (`%`).
    pub fn rem_floor(&self, other: &BigInt) -> Result<BigInt> {
        self.divmod_floor(other).map(|(_, r)| r)
    }

    /// Arithmetic left shift (sign preserved).
    pub fn shl(&self, n: usize) -> BigInt {
        BigInt::new(self.sign, self.mag.shl(n))
    }

    /// Arithmetic right shift with floor semantics
    /// (-3 >> 1 == -2, like CPython).
    pub fn shr(&self, n: usize) -> BigInt {
        if self.is_zero() || n == 0 {
            return self.clone();
        }
        if !self.is_negative() {
            return BigInt::new(Sign::Positive, self.mag.shr(n));
        }
        // floor(-A / 2^n) = -ceil(A / 2^n) = -((A + 2^n - 1) >> n)
        let mask = BigUInt::one().shl(n).sub(&BigUInt::one());
        let mag = self.mag.add(&mask).shr(n);
        BigInt::new(Sign::Negative, mag)
    }

    /// Bitwise AND with infinite two's-complement semantics.
    pub fn bit_and(&self, other: &BigInt) -> BigInt {
        use Sign::*;
        match (self.sign, other.sign) {
            (Positive, Positive) => BigInt::new(Positive, self.mag.bit_and(&other.mag)),
            (Negative, Positive) => BigInt::new(
                Positive,
                other.mag.bit_and_not(&self.mag.sub(&BigUInt::one())),
            ),
            (Positive, Negative) => BigInt::new(
                Positive,
                self.mag.bit_and_not(&other.mag.sub(&BigUInt::one())),
            ),
            (Negative, Negative) => {
                let c = self
                    .mag
                    .sub(&BigUInt::one())
                    .bit_or(&other.mag.sub(&BigUInt::one()));
                BigInt::new(Negative, c.add(&BigUInt::one()))
            }
        }
    }

    /// Bitwise OR with infinite two's-complement semantics.
    pub fn bit_or(&self, other: &BigInt) -> BigInt {
        use Sign::*;
        match (self.sign, other.sign) {
            (Positive, Positive) => BigInt::new(Positive, self.mag.bit_or(&other.mag)),
            (Negative, Positive) => {
                let c = self.mag.sub(&BigUInt::one()).bit_and_not(&other.mag);
                BigInt::new(Negative, c.add(&BigUInt::one()))
            }
            (Positive, Negative) => {
                let c = other.mag.sub(&BigUInt::one()).bit_and_not(&self.mag);
                BigInt::new(Negative, c.add(&BigUInt::one()))
            }
            (Negative, Negative) => {
                let c = self
                    .mag
                    .sub(&BigUInt::one())
                    .bit_and(&other.mag.sub(&BigUInt::one()));
                BigInt::new(Negative, c.add(&BigUInt::one()))
            }
        }
    }

    /// Bitwise XOR with infinite two's-complement semantics.
    pub fn bit_xor(&self, other: &BigInt) -> BigInt {
        // xor = or - and (holds for two's-complement values).
        self.bit_or(other).sub(&self.bit_and(other))
    }

    /// Bitwise NOT: `~a == -a - 1`.
    pub fn bit_not(&self) -> BigInt {
        if self.is_negative() {
            BigInt::new(Sign::Positive, self.mag.sub(&BigUInt::one()))
        } else {
            BigInt::new(Sign::Negative, self.mag.add(&BigUInt::one()))
        }
    }

    /// Number of significant bits of the magnitude.
    pub fn bit_length(&self) -> usize {
        self.mag.bit_length()
    }

    /// Population count of set bits in the magnitude.
    pub fn bit_count(&self) -> u32 {
        self.mag.bit_count()
    }

    /// Trailing zero bits of the magnitude; `None` for zero.
    pub fn trailing_zeros(&self) -> Option<usize> {
        self.mag.trailing_zeros()
    }

    /// Leading zero bits within the minimal limb-aligned representation:
    /// `64 * limb_count - bit_length` (0 for zero). Documented MegaNum
    /// extension (CPython has no such method).
    pub fn leading_zeros(&self) -> usize {
        if self.mag.is_zero() {
            return 0;
        }
        let limbs = self.mag.limbs().len();
        limbs * LIMB_BITS as usize - self.mag.bit_length()
    }

    /// Parse with CPython-style auto radix: optional sign, optional
    /// `0x`/`0o`/`0b` prefix, `_` separators between digits.
    pub fn parse(s: &str) -> Result<BigInt> {
        BigInt::parse_radix(s, 0)
    }

    /// Parse with explicit radix (0 = auto-detect from prefix, default 10).
    /// Prefixes are accepted when they match the radix (or with auto).
    pub fn parse_radix(s: &str, radix: u32) -> Result<BigInt> {
        let t = s.trim();
        if t.is_empty() {
            return Err(MegaNumError::Parse("empty input".to_string()));
        }
        let (sign, rest) = match t.as_bytes()[0] {
            b'-' => (Sign::Negative, &t[1..]),
            b'+' => (Sign::Positive, &t[1..]),
            _ => (Sign::Positive, t),
        };
        if rest.is_empty() {
            return Err(MegaNumError::Parse("no digits after sign".to_string()));
        }
        // Underscore separators: strip single underscores that sit between
        // two digit characters.
        let cleaned = strip_underscores(rest)?;
        let bytes = cleaned.as_bytes();
        let mut eff_radix = if radix == 0 { 10u32 } else { radix };
        let mut digits = bytes;
        if matches!(radix, 0 | 16 | 8 | 2) && bytes.len() >= 2 && bytes[0] == b'0' {
            let (pfx, r) = match bytes[1] {
                b'x' | b'X' => (2usize, 16u32),
                b'o' | b'O' => (2usize, 8u32),
                b'b' | b'B' => (2usize, 2u32),
                _ => (0usize, 0u32),
            };
            if pfx != 0 {
                if radix != 0 && radix != r {
                    return Err(MegaNumError::Parse(format!(
                        "prefix 0{} not allowed for radix {radix}",
                        bytes[1] as char
                    )));
                }
                eff_radix = r;
                digits = &bytes[2..];
                if digits.is_empty() {
                    return Err(MegaNumError::Parse("no digits after prefix".to_string()));
                }
            }
        }
        let mag = BigUInt::parse_mag_radix(
            std::str::from_utf8(digits)
                .map_err(|_| MegaNumError::Parse("input is not valid UTF-8".to_string()))?,
            eff_radix,
        )?;
        Ok(BigInt::new(sign, mag))
    }

    /// Render in `radix` (2..=62) with sign; decimal by default elsewhere.
    pub fn to_string_radix(&self, radix: u32) -> Result<String> {
        let mag = self.mag.to_string_radix(radix)?;
        if self.is_negative() {
            Ok(format!("-{mag}"))
        } else {
            Ok(mag)
        }
    }

    /// Two's-complement decode from bytes.
    pub fn from_bytes_le(bytes: &[u8], signed: bool) -> BigInt {
        let raw = BigUInt::from_bytes_le(bytes);
        if !signed || bytes.is_empty() || (bytes[bytes.len() - 1] & 0x80) == 0 {
            return BigInt::new(Sign::Positive, raw);
        }
        // Negative: value = raw - 2^(8*len); magnitude = 2^(8*len) - raw.
        let modulus = BigUInt::one().shl(bytes.len() * 8);
        BigInt::new(Sign::Negative, modulus.sub(&raw))
    }

    pub fn from_bytes_be(bytes: &[u8], signed: bool) -> BigInt {
        let rev: Vec<u8> = bytes.iter().rev().copied().collect();
        BigInt::from_bytes_le(&rev, signed)
    }

    /// Two's-complement encode; `length == None` selects the minimal number
    /// of bytes (0 for zero, as in CPython `to_bytes(0)`).
    pub fn to_bytes_le(&self, length: Option<usize>, signed: bool) -> Result<Vec<u8>> {
        let minimal = minimal_bytes_le(self, signed)?;
        match length {
            None => Ok(minimal),
            Some(len) => {
                if minimal.len() > len {
                    return Err(MegaNumError::Overflow(format!(
                        "value needs {} bytes but {} requested",
                        minimal.len(),
                        len
                    )));
                }
                let fill = match self.sign {
                    Sign::Positive => 0u8,
                    Sign::Negative => 0xFF,
                };
                let mut out = minimal;
                out.resize(len, fill);
                Ok(out)
            }
        }
    }

    pub fn to_bytes_be(&self, length: Option<usize>, signed: bool) -> Result<Vec<u8>> {
        let mut out = self.to_bytes_le(length, signed)?;
        out.reverse();
        Ok(out)
    }

    /// Convert to `i64` if in range.
    pub fn to_i64(&self) -> Option<i64> {
        let m = u64::try_from(&self.mag).ok()?;
        if self.sign == Sign::Positive {
            if m <= i64::MAX as u64 {
                Some(m as i64)
            } else {
                None
            }
        } else if m <= (i64::MAX as u64) + 1 {
            Some(-(m as i128) as i64)
        } else {
            None
        }
    }

    /// Exact integer square root of the magnitude; errors on negatives
    /// (M2 implements the Newton fast path; schoolbook fallback for now).
    pub fn isqrt(&self) -> Result<BigInt> {
        if self.is_negative() {
            return Err(MegaNumError::Conversion(
                "isqrt of a negative number".to_string(),
            ));
        }
        Ok(BigInt::new(
            Sign::Positive,
            algos::sqrt::isqrt_mag(&self.mag),
        ))
    }

    // ----- number theory (spec FR-1.4): thin wrappers over the algorithm
    // layer; dispatch/backend selection stays inside `algos` -----

    /// Greatest common divisor of the magnitudes; always non-negative,
    /// `gcd(0, 0) == 0` (FR-1.4).
    pub fn gcd(&self, other: &BigInt) -> BigInt {
        BigInt::new(Sign::Positive, algos::gcd::gcd(self.mag(), other.mag()))
    }

    /// Least common multiple of the magnitudes: `|a*b| / gcd(a,b)`;
    /// `lcm(0, x) == 0`; always non-negative.
    pub fn lcm(&self, other: &BigInt) -> BigInt {
        if self.is_zero() || other.is_zero() {
            return BigInt::zero();
        }
        let g = algos::gcd::gcd(self.mag(), other.mag());
        let prod = self.mag().mul(other.mag());
        let (q, _) = prod.divmod(&g).expect("gcd divides the product");
        BigInt::new(Sign::Positive, q)
    }

    /// `(self ** exp) mod modulus` with CPython `pow` semantics: a nonzero
    /// result carries the sign of the modulus, and a negative exponent is
    /// honored through the modular inverse. Zero modulus maps to `Parse`
    /// (CPython raises `ValueError` for `pow(x, y, 0)`), a non-invertible
    /// base maps to `Parse` as well (`ValueError` in CPython).
    pub fn modpow(&self, exp: &BigInt, modulus: &BigInt) -> Result<BigInt> {
        if modulus.is_zero() {
            return Err(MegaNumError::Parse(
                "pow() 3rd argument cannot be 0".to_string(),
            ));
        }
        let m = modulus.mag();
        // Reduce the signed base into [0, |m|) first (CPython parity:
        // pow(-2, 3, 5) == 2).
        let (_, red) = self.mag().divmod(m).expect("m nonzero");
        let base = if self.is_negative() && !red.is_zero() {
            m.sub(&red)
        } else {
            red
        };
        let (base, exp_mag) = if exp.is_negative() {
            // CPython pow(b, -e, m): invert the base, then exponentiate.
            let inv = algos::pow::mod_inverse(&base, m).ok_or_else(|| {
                MegaNumError::Parse("base is not invertible for the given modulus".to_string())
            })?;
            (inv, exp.mag().clone())
        } else {
            (base, exp.mag().clone())
        };
        let mut r = {
            let mut scratch = algos::Scratch::new();
            algos::pow::modpow_big(&base, &exp_mag, m, &mut scratch)
        };
        if modulus.is_negative() && !r.is_zero() {
            // CPython parity: representative in (modulus, 0].
            r = m.sub(&r);
            return Ok(BigInt::new(Sign::Negative, r));
        }
        Ok(BigInt::new(Sign::Positive, r))
    }

    /// Modular inverse of `self` modulo `modulus`, CPython
    /// `pow(self, -1, modulus)` semantics: result in the modulus's sign
    /// domain; zero modulus or non-invertible base -> `Parse` error.
    pub fn mod_inverse(&self, modulus: &BigInt) -> Result<BigInt> {
        if modulus.is_zero() {
            return Err(MegaNumError::Parse(
                "pow() 3rd argument cannot be 0".to_string(),
            ));
        }
        let m = modulus.mag();
        // Signed reduction into [0, |m|) first (pow(-3, -1, 7) == 2).
        let (_, red) = self.mag().divmod(m).expect("m nonzero");
        let base = if self.is_negative() && !red.is_zero() {
            m.sub(&red)
        } else {
            red
        };
        let inv = algos::pow::mod_inverse(&base, m).ok_or_else(|| {
            MegaNumError::Parse("base is not invertible for the given modulus".to_string())
        })?;
        if modulus.is_negative() && !inv.is_zero() {
            let r = m.sub(&inv);
            return Ok(BigInt::new(Sign::Negative, r));
        }
        Ok(BigInt::new(Sign::Positive, inv))
    }

    /// Jacobi symbol `(self / n)` for an odd, positive modulus `n`;
    /// returns -1, 0 or 1. A negative `self` applies the `-1` twist iff
    /// `n % 4 == 3`. Even or non-positive moduli are rejected.
    pub fn jacobi(&self, n: &BigInt) -> Result<i8> {
        if n.is_zero() || n.is_negative() || n.mag().is_even() {
            return Err(MegaNumError::Parse(
                "jacobi modulus must be a positive odd integer".to_string(),
            ));
        }
        let mut j = algos::number_theory::jacobi(self.mag(), n.mag());
        if self.is_negative() {
            // (-1/n) == -1 iff n % 4 == 3.
            let (_, r) = n.mag().divmod(&BigUInt::from(4u64)).expect("4 != 0");
            if r.limbs()[0] & 3 == 3 {
                j = -j;
            }
        }
        Ok(j)
    }

    /// Miller-Rabin probable-prime test on the magnitude with `rounds`
    /// pseudorandom bases after the deterministic prechecks. Probabilistic
    /// for magnitudes beyond the deterministic witness set; documented as
    /// such (FR-1.4).
    pub fn is_probable_prime(&self, rounds: u32) -> bool {
        algos::number_theory::is_probable_prime(self.mag(), rounds)
    }
}

/// Strip single underscores that separate digit characters (CPython rule).
fn strip_underscores(s: &str) -> Result<String> {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' {
            let prev_ok = i > 0 && (b[i - 1] as char).is_ascii_alphanumeric();
            let next_ok = i + 1 < b.len() && (b[i + 1] as char).is_ascii_alphanumeric();
            if !prev_ok || !next_ok {
                return Err(MegaNumError::Parse(
                    "misplaced underscore separator".to_string(),
                ));
            }
        } else {
            out.push(c as char);
        }
    }
    Ok(out)
}

/// Square-and-multiply on magnitudes.
pub(crate) fn pow_mag(mag: &BigUInt, exp: u64) -> BigUInt {
    if exp == 0 {
        return BigUInt::one();
    }
    let mut result = BigUInt::one();
    let mut base = mag.clone();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.mul(&base);
        }
        e >>= 1;
        if e > 0 {
            base = base.mul(&base);
        }
    }
    result
}

/// Minimal little-endian two's-complement bytes.
fn minimal_bytes_le(v: &BigInt, signed: bool) -> Result<Vec<u8>> {
    if !signed {
        return Ok(v.mag.to_bytes_le());
    }
    if v.is_zero() {
        return Ok(Vec::new());
    }
    if !v.is_negative() {
        let mut b = v.mag.to_bytes_le();
        if b.is_empty() {
            b.push(0);
        }
        if b[b.len() - 1] & 0x80 != 0 {
            b.push(0);
        }
        Ok(b)
    } else {
        // -mag: two's complement = !(mag - 1); prepend 0xFF if needed.
        let m1 = v.mag.sub(&BigUInt::one());
        let mut b: Vec<u8> = m1.to_bytes_le().into_iter().map(|x| !x).collect();
        if b.is_empty() {
            b.push(0xFF);
        }
        if b[b.len() - 1] & 0x80 == 0 {
            b.push(0xFF);
        }
        Ok(b)
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self.is_negative(), other.is_negative()) {
            (false, false) => self.mag.cmp(&other.mag),
            (true, true) => other.mag.cmp(&self.mag),
            (false, true) => Greater,
            (true, false) => Less,
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl std::fmt::Display for BigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_string_radix(10) {
            Ok(s) => f.write_str(&s),
            Err(_) => Err(std::fmt::Error),
        }
    }
}

impl std::fmt::Debug for BigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BigInt({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(v: i64) -> BigInt {
        BigInt::from_i64(v)
    }

    #[test]
    fn python_floor_semantics() {
        // (a, b) -> (a // b, a % b) per CPython.
        let cases: &[(i64, i64, i64, i64)] = &[
            (7, 2, 3, 1),
            (-7, 2, -4, 1),
            (7, -2, -4, -1),
            (-7, -2, 3, -1),
            (6, 3, 2, 0),
            (-6, 3, -2, 0),
            (1, 3, 0, 1),
            (-1, 3, -1, 2),
            (0, 5, 0, 0),
            (0, -5, 0, 0),
        ];
        for &(a, b, q, r) in cases {
            let got = i(a).divmod_floor(&i(b)).unwrap();
            assert_eq!(got.0.to_i64(), Some(q), "{a} // {b}");
            assert_eq!(got.1.to_i64(), Some(r), "{a} % {b}");
            // invariant a == q*b + r
            let recomposed = got.0.mul(&i(b)).add(&got.1);
            assert_eq!(recomposed, i(a));
        }
        assert!(i(1).divmod_floor(&i(0)).is_err());
    }

    #[test]
    fn bitwise_matches_i64() {
        let cases: &[(i64, i64)] = &[
            (0x0F0F, 0x00FF),
            (-1, 5),
            (-2, -3),
            (123, -456),
            (-789, 0),
            (0, 0),
            (i64::MAX, 1),
            (-1024, 1023),
        ];
        for &(a, b) in cases {
            assert_eq!(i(a).bit_and(&i(b)).to_i64(), Some(a & b), "{a} & {b}");
            assert_eq!(i(a).bit_or(&i(b)).to_i64(), Some(a | b), "{a} | {b}");
            assert_eq!(i(a).bit_xor(&i(b)).to_i64(), Some(a ^ b), "{a} ^ {b}");
        }
        for a in [0i64, 5, -1, -2, i64::MAX, i64::MIN + 1] {
            assert_eq!(i(a).bit_not().to_i64(), Some(!a), "!{a}");
        }
    }

    #[test]
    fn shifts_match_i64() {
        for a in [5i64, -5, -1, 1024, -1024, i64::MAX, -12345678] {
            for s in [0usize, 1, 3, 8, 40] {
                let got = i(a).shr(s).to_i64().unwrap();
                assert_eq!(got, a >> s, "{a} >> {s}");
            }
        }
        let big = i(1).shl(100);
        assert_eq!(big.bit_length(), 101);
        assert_eq!(big.shr(100), i(1));
        assert_eq!(i(-1).shr(200), i(-1));
    }

    #[test]
    fn parse_prefixes_and_underscores() {
        assert_eq!(BigInt::parse("12345").unwrap(), i(12345));
        assert_eq!(BigInt::parse("-42").unwrap(), i(-42));
        assert_eq!(BigInt::parse("+7").unwrap(), i(7));
        assert_eq!(BigInt::parse("0x1F").unwrap(), i(31));
        assert_eq!(BigInt::parse("0o17").unwrap(), i(15));
        assert_eq!(BigInt::parse("0b1011").unwrap(), i(11));
        assert_eq!(BigInt::parse("1_000_000").unwrap(), i(1_000_000));
        assert_eq!(BigInt::parse("0xff_ff").unwrap(), i(0xffff));
        assert_eq!(BigInt::parse_radix("z", 36).unwrap(), i(35));
        assert!(BigInt::parse("1__0").is_err());
        assert!(BigInt::parse("_10").is_err());
        assert!(BigInt::parse("10_").is_err());
        assert!(BigInt::parse("-").is_err());
        assert!(BigInt::parse_radix("0x", 16).is_err());
        assert!(BigInt::parse_radix("0x1f", 10).is_err());
    }

    #[test]
    fn format_radix_negative() {
        assert_eq!(i(-255).to_string_radix(16).unwrap(), "-ff");
        assert_eq!(i(-255).to_string_radix(10).unwrap(), "-255");
    }

    #[test]
    fn bytes_roundtrip_signed() {
        for v in [
            0i64,
            1,
            -1,
            127,
            128,
            -128,
            -129,
            255,
            256,
            -256,
            i64::MAX,
            i64::MIN,
        ] {
            let b = i(v).to_bytes_le(None, true).unwrap();
            assert_eq!(BigInt::from_bytes_le(&b, true).to_i64(), Some(v), "le {v}");
            let bb = i(v).to_bytes_be(None, true).unwrap();
            assert_eq!(BigInt::from_bytes_be(&bb, true).to_i64(), Some(v), "be {v}");
        }
        // explicit lengths with padding
        let b = i(-2).to_bytes_le(Some(4), true).unwrap();
        assert_eq!(b, vec![0xFE, 0xFF, 0xFF, 0xFF]);
        let b = i(2).to_bytes_le(Some(4), true).unwrap();
        assert_eq!(b, vec![2, 0, 0, 0]);
        assert!(i(300).to_bytes_le(Some(1), false).is_err());
    }

    #[test]
    fn bytes_roundtrip_large() {
        let mag = BigUInt::from_limbs(vec![0xFEDC_BA98_7654_3210, 1, 0xAB]);
        let v = BigInt::new(Sign::Negative, mag.clone());
        let b = v.to_bytes_le(None, true).unwrap();
        let back = BigInt::from_bytes_le(&b, true);
        assert_eq!(back, v);
        let u = BigInt::new(Sign::Positive, mag);
        let bu = u.to_bytes_be(None, false).unwrap();
        assert_eq!(BigInt::from_bytes_be(&bu, false), u);
    }

    #[test]
    fn pow_signs_and_values() {
        assert_eq!(i(-2).pow_u64(3), i(-8));
        assert_eq!(i(-2).pow_u64(4), i(16));
        assert_eq!(i(0).pow_u64(0), i(1));
        assert_eq!(i(3).pow_u64(5), i(243));
    }

    #[test]
    fn cmp_and_helpers() {
        assert!(i(-5) < i(3));
        assert!(i(-5) < i(-4));
        assert!(BigInt::zero() < i(1));
        assert_eq!(i(7).cmp(&i(7)), std::cmp::Ordering::Equal);
        assert_eq!(i(-9).bit_count(), 2); // 9 = 1001
        assert_eq!(i(4096).trailing_zeros(), Some(12));
        assert_eq!(BigInt::zero().trailing_zeros(), None);
        assert_eq!(i(1).leading_zeros(), 63);
    }

    #[test]
    fn mul_signs() {
        assert_eq!(i(-3).mul(&i(4)), i(-12));
        assert_eq!(i(-3).mul(&i(-4)), i(12));
        assert_eq!(i(3).mul(&i(-4)), i(-12));
        assert_eq!(BigInt::zero().mul(&i(999)), BigInt::zero());
    }

    #[test]
    fn large_floor_division() {
        // Cross-check big operands against the truncating magnitude quotient.
        let a = BigInt::parse("1000000000000000000000000000000000000007").unwrap();
        let b = BigInt::parse("-12345678901234567890123456789").unwrap();
        let (q, r) = a.divmod_floor(&b).unwrap();
        let recomposed = q.mul(&b).add(&r);
        assert_eq!(recomposed, a);
        assert!(r.is_negative() && r.mag.cmp(&b.mag) == std::cmp::Ordering::Less);
    }
}
