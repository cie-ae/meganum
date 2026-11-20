//! Subquadratic base conversion (parsing and formatting), radix 2..=62.
//!
//! * Radices that are powers of two use direct bit-group paths (O(n)).
//! * Other radices use divide-and-conquer over fixed-size digit chunks with
//!   a precomputed power ladder: parsing combines chunks with
//!   `mul_best` (Karatsuba/NTT), formatting splits with the fast division
//!   from [`crate::algos::div`]. Both are O(M(n) log n) (spec 6).
//!
//! Digit alphabet (spec FR-1.1): 0-9, then a-z (10-35), then A-Z (36-61).
//! Parsing accepts both cases for values < 36.

use crate::algos::div;
use crate::algos::mul;
use crate::algos::Scratch;
use crate::biguint::BigUInt;
use crate::error::{MegaNumError, Result};
use crate::limb::Limb;

/// Maximum supported radix.
pub const MAX_RADIX: u32 = 62;

/// Digit value with radix-aware case handling: for radix <= 35 both cases
/// map to 10-35 (CPython-compatible, e.g. "FF" in hex); for radix > 35 the
/// spec alphabet applies: a-z = 10-35, A-Z = 36-61.
fn digit_value_for(c: u8, radix: u32) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 10),
        b'A'..=b'Z' => {
            // Radix <= 36 is CPython territory: case-insensitive letters
            // (A == a == 10). From radix 37 the uppercase letters carry the
            // extended digit values 36..=61 (MegaNum radix 37..=62).
            if radix > 36 {
                Some((c - b'A') as u32 + 36)
            } else {
                Some((c - b'A') as u32 + 10)
            }
        }
        _ => None,
    }
}

fn digit_char(v: u32) -> char {
    match v {
        0..=9 => (b'0' + v as u8) as char,
        10..=35 => (b'a' + (v - 10) as u8) as char,
        36..=61 => (b'A' + (v - 36) as u8) as char,
        _ => unreachable!("digit value out of range"),
    }
}

/// Largest `d` with `radix^d <= u64::MAX`, and that power.
fn chunk_params(radix: u32) -> (usize, u64) {
    let r = radix as u64;
    let mut d = 1usize;
    let mut p = r;
    while let Some(next) = p.checked_mul(r) {
        p = next;
        d += 1;
    }
    (d, p)
}

/// Parse a magnitude (no sign, no prefix, no underscores) in `radix`.
pub fn parse_mag_radix(s: &str, radix: u32) -> Result<BigUInt> {
    if !(2..=MAX_RADIX).contains(&radix) {
        return Err(MegaNumError::Parse(format!(
            "radix {radix} out of range 2..={MAX_RADIX}"
        )));
    }
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(MegaNumError::Parse("empty digit string".to_string()));
    }
    for (i, &c) in bytes.iter().enumerate() {
        let v = digit_value_for(c, radix)
            .ok_or_else(|| MegaNumError::Parse(format!("invalid digit {c:?} at position {i}")))?;
        if v >= radix {
            return Err(MegaNumError::Parse(format!(
                "digit {c:?} at position {i} invalid for radix {radix}"
            )));
        }
    }
    let start = bytes
        .iter()
        .take_while(|&&c| digit_value_for(c, radix) == Some(0))
        .count();
    let digits = &bytes[start..];
    if digits.is_empty() {
        return Ok(BigUInt::zero());
    }
    if radix.is_power_of_two() {
        Ok(parse_bits(digits, radix))
    } else {
        let mut scratch = Scratch::new();
        Ok(parse_chunks(digits, radix, &mut scratch))
    }
}

/// Direct bit-group parse for radix 2^k.
fn parse_bits(digits: &[u8], radix: u32) -> BigUInt {
    let k = radix.trailing_zeros();
    let n = digits.len();
    let total_bits = n as u64 * k as u64;
    let limb_count = total_bits.div_ceil(64) as usize;
    let mut limbs = vec![0u64; limb_count];
    for (i, &c) in digits.iter().rev().enumerate() {
        let v = digit_value_for(c, radix).unwrap_or(0) as u64;
        let pos = i as u64 * k as u64;
        let li = (pos / 64) as usize;
        let off = (pos % 64) as u32;
        limbs[li] |= v << off;
        if off + k > 64 && li + 1 < limb_count {
            limbs[li + 1] |= v >> (64 - off);
        }
    }
    BigUInt::from_limbs(limbs)
}

/// Divide-and-conquer chunk parse for non-power-of-two radices.
fn parse_chunks(digits: &[u8], radix: u32, scratch: &mut Scratch) -> BigUInt {
    let (d, base) = chunk_params(radix);
    let mut chunks: Vec<u64> = Vec::with_capacity(digits.len() / d + 1);
    let mut idx = digits.len();
    while idx > 0 {
        let start = idx.saturating_sub(d);
        let mut val: u64 = 0;
        for &c in &digits[start..idx] {
            val = val * radix as u64 + digit_value_for(c, radix).unwrap_or(0) as u64;
        }
        chunks.push(val);
        idx = start;
    }
    while chunks.last() == Some(&0) {
        chunks.pop();
    }
    if chunks.is_empty() {
        return BigUInt::zero();
    }
    // Pad chunk count to a power of two so spans are exact powers.
    let padded = chunks.len().next_power_of_two();
    chunks.resize(padded, 0);

    // Power ladder: pow[i] = radix^(d * 2^i).
    let levels = padded.trailing_zeros() as usize;
    let mut pow: Vec<BigUInt> = Vec::with_capacity(levels + 1);
    pow.push(BigUInt::from(base));
    for i in 0..levels {
        let sq = mul::mul_best(pow[i].limbs(), pow[i].limbs(), scratch);
        pow.push(BigUInt::from_limbs(sq));
    }

    fn combine(chunks: &[u64], lo: usize, hi: usize, pow: &[BigUInt]) -> BigUInt {
        if hi - lo == 1 {
            return BigUInt::from(chunks[lo]);
        }
        let mid = lo + (hi - lo) / 2;
        // chunks[] is least-significant first: combine(lo, mid) holds the
        // low digits, combine(mid, hi) the high digits.
        let low = combine(chunks, lo, mid, pow);
        let high = combine(chunks, mid, hi, pow);
        if high.is_zero() {
            return low;
        }
        let shifted = high.mul(&pow[(mid - lo).trailing_zeros() as usize]);
        BigUInt::from_limbs(crate::algos::add::add_slices(shifted.limbs(), low.limbs()))
    }

    combine(&chunks, 0, padded, &pow)
}

/// Render a canonical magnitude in `radix` (2..=62), lowercase for < 36.
pub fn to_string_radix(limbs: &[Limb], radix: u32) -> Result<String> {
    if !(2..=MAX_RADIX).contains(&radix) {
        return Err(MegaNumError::Parse(format!(
            "radix {radix} out of range 2..={MAX_RADIX}"
        )));
    }
    if limbs.is_empty() {
        return Ok("0".to_string());
    }
    if radix.is_power_of_two() {
        return Ok(format_bits(limbs, radix.trailing_zeros()));
    }
    let (d, _base) = chunk_params(radix);
    let mut chunks: Vec<u64> = Vec::new(); // least-significant first
    let mut scratch = Scratch::new();
    collect_chunks(limbs, radix, d, &mut chunks, &mut scratch);
    if chunks.is_empty() {
        return Ok("0".to_string());
    }
    debug_assert!(*chunks.last().unwrap() != 0, "top chunk must be nonzero");
    let r64 = radix as u64;
    let mut s = String::with_capacity(chunks.len() * d + 8);
    let mut buf = [0u8; 64];
    let last = chunks.len() - 1;
    for (i, c) in chunks.iter().enumerate().rev() {
        let mut v = *c;
        let mut written = 0;
        while v > 0 {
            buf[written] = digit_char((v % r64) as u32) as u8;
            v /= r64;
            written += 1;
        }
        if i != last {
            // Zero-pad all but the most significant chunk to full width.
            while written < d {
                buf[written] = b'0';
                written += 1;
            }
        }
        for j in (0..written).rev() {
            s.push(buf[j] as char);
        }
    }
    Ok(s)
}

/// Recursive split into base^d chunks (most significant first pushed last).
fn collect_chunks(limbs: &[Limb], radix: u32, d: usize, out: &mut Vec<u64>, scratch: &mut Scratch) {
    if limbs.len() <= SMALL_FORMAT_LIMBS {
        let mut cur = limbs.to_vec();
        loop {
            if cur.is_empty() {
                break;
            }
            let (q, r) = div::schoolbook::divmod_limb(&cur, small_base(radix, d));
            out.push(r);
            cur = q;
        }
        return;
    }
    // Choose the split power closest to half the bit length (balanced).
    let n = BigUInt::from_limbs(limbs.to_vec());
    let half_bits = n.bit_length() / 2;
    let (_dd, base) = chunk_params(radix);
    // Build powers ladder until exceeding half bits.
    let mut pow = BigUInt::from(base);
    let mut ladder: Vec<BigUInt> = vec![pow.clone()];
    while (pow.bit_length() as i64) < half_bits as i64 {
        let sq = mul::mul_best(pow.limbs(), pow.limbs(), scratch);
        pow = BigUInt::from_limbs(sq);
        ladder.push(pow.clone());
    }
    // Use the largest ladder entry with bit_length <= half_bits (or the
    // first entry if none fits, which cannot happen at this size).
    let mut pick = 0;
    for (i, p) in ladder.iter().enumerate() {
        if p.bit_length() <= half_bits {
            pick = i;
        }
    }
    let split = ladder.swap_remove(pick);
    let (q, r) = div::divmod_best(limbs, split.limbs());
    // Least-significant first: the low remainder chunks come before the
    // high quotient chunks.
    collect_chunks(&r, radix, d, out, scratch);
    collect_chunks(&q, radix, d, out, scratch);
}

/// Below this many limbs, repeated single-limb division wins.
const SMALL_FORMAT_LIMBS: usize = 24;

/// radix^d for small-format repeated division (d from chunk_params).
fn small_base(radix: u32, d: usize) -> u64 {
    let mut p = 1u64;
    for _ in 0..d {
        p *= radix as u64;
    }
    p
}

/// Direct bit-group render for radix 2^k.
fn format_bits(limbs: &[Limb], k: u32) -> String {
    let top = *limbs.last().unwrap();
    let bit_len = (limbs.len() - 1) * 64 + crate::limb::limb_bits(top) as usize;
    let digit_count = bit_len.div_ceil(k as usize);
    let mut s = String::with_capacity(digit_count);
    for i in (0..digit_count).rev() {
        let pos = i * k as usize;
        let li = pos / 64;
        let off = (pos % 64) as u32;
        let lo = limbs.get(li).copied().unwrap_or(0);
        let hi = limbs.get(li + 1).copied().unwrap_or(0);
        let group =
            ((lo >> off) | if off + k > 64 { hi << (64 - off) } else { 0 }) & ((1u64 << k) - 1);
        s.push(digit_char(group as u32));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limb::{DEC_CHUNK_BASE, DEC_CHUNK_DIGITS};

    #[test]
    fn chunk_params_decimal() {
        let (d, base) = chunk_params(10);
        assert_eq!(d, DEC_CHUNK_DIGITS);
        assert_eq!(base, DEC_CHUNK_BASE);
    }

    #[test]
    fn roundtrip_decimal_millionish() {
        let s = "9".repeat(5000);
        let n = parse_mag_radix(&s, 10).unwrap();
        assert_eq!(n.to_string_radix(10).unwrap(), s);
    }

    #[test]
    fn roundtrip_various_radices() {
        for radix in [2u32, 3, 7, 10, 16, 36, 62] {
            let s = match radix {
                62 => "Z".repeat(200),
                36 => "zz".repeat(100),
                16 => "f".repeat(400),
                10 => "9".repeat(300),
                _ => std::iter::repeat_n(digit_char(radix - 1), 300).collect(),
            };
            let n = parse_mag_radix(&s, radix).unwrap();
            assert_eq!(n.to_string_radix(radix).unwrap(), s, "radix {radix}");
        }
    }

    #[test]
    fn known_decimal_values() {
        assert_eq!(
            parse_mag_radix("1000000", 10).unwrap(),
            BigUInt::from(1_000_000u64)
        );
        let n = parse_mag_radix("123456789012345678901234567890", 10).unwrap();
        assert_eq!(
            n.to_string_radix(10).unwrap(),
            "123456789012345678901234567890"
        );
        assert_eq!(n.to_string_radix(16).unwrap(), "18ee90ff6c373e0ee4e3f0ad2");
    }

    #[test]
    fn invalid_digits_rejected() {
        assert!(parse_mag_radix("12x3", 10).is_err());
        assert!(parse_mag_radix("", 10).is_err());
        assert!(parse_mag_radix("8", 8).is_err());
        // Radix <= 36 is CPython territory: letters are case-insensitive,
        // so 'Z' == 'z' == 35 at radix 36.
        assert_eq!(
            parse_mag_radix("Z", 36).unwrap(),
            parse_mag_radix("z", 36).unwrap()
        );
        assert!(parse_mag_radix("Z", 62).is_ok());
        // From radix 37 the uppercase letters carry 36..=61 ('A' == 36).
        assert!(parse_mag_radix("A", 37).is_ok());
        assert!(parse_mag_radix("Z", 61).is_err()); // 61 >= radix 61
        assert!(parse_mag_radix("+", 62).is_err());
        assert!(parse_mag_radix("@", 62).is_err());
    }

    #[test]
    fn leading_zeros_ok() {
        assert_eq!(
            parse_mag_radix("000123", 10).unwrap(),
            BigUInt::from(123u64)
        );
        assert_eq!(to_string_radix(BigUInt::zero().limbs(), 10).unwrap(), "0");
    }

    #[test]
    fn hex_bit_path_matches_decimal_path() {
        // 300 limbs of pseudo-random data through both paths.
        let mut s: u64 = 0xC0FFEE;
        let limbs: Vec<Limb> = (0..300)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            })
            .collect();
        let n = BigUInt::from_limbs(limbs);
        let hex = n.to_string_radix(16).unwrap();
        let back = parse_mag_radix(&hex, 16).unwrap();
        assert_eq!(back, n);
        let dec = n.to_string_radix(10).unwrap();
        let back_dec = parse_mag_radix(&dec, 10).unwrap();
        assert_eq!(back_dec, n);
    }
}
