//! Toom-Cook-3 multiplication, O(n^1.465), for the 1,024..8,192 limb band.
//!
//! Evaluation at points {0, 1, -1, 2, infinity} with exact interpolation
//! (divisions by 2 and 3 are exact in Z). Sub-products use Karatsuba
//! directly - never `mul_best` - so there is no dispatch recursion. A small
//! signed wrapper carries the interpolation intermediates, whose signs can
//! be negative even though all final coefficients are non-negative.

use crate::algos::add::{add_slices, cmp_slices, sub_slices};
use crate::algos::mul::karatsuba;
use crate::algos::Scratch;
use crate::limb::{Limb, LIMB_BITS};

/// Signed magnitude helper over canonical limb vectors.
#[derive(Clone)]
struct SBig {
    neg: bool,
    mag: Vec<Limb>,
}

impl SBig {
    fn zero() -> SBig {
        SBig {
            neg: false,
            mag: Vec::new(),
        }
    }

    fn new(neg: bool, mag: Vec<Limb>) -> SBig {
        let mut mag = mag;
        while mag.last() == Some(&0) {
            mag.pop();
        }
        if mag.is_empty() {
            SBig { neg: false, mag }
        } else {
            SBig { neg, mag }
        }
    }

    fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    fn neg(&self) -> SBig {
        SBig {
            neg: !self.neg,
            mag: self.mag.clone(),
        }
    }

    fn add(&self, other: &SBig) -> SBig {
        if self.neg == other.neg {
            SBig::new(self.neg, add_slices(&self.mag, &other.mag))
        } else {
            match cmp_slices(&self.mag, &other.mag) {
                std::cmp::Ordering::Equal => SBig::zero(),
                std::cmp::Ordering::Greater => {
                    SBig::new(self.neg, sub_slices(&self.mag, &other.mag))
                }
                std::cmp::Ordering::Less => SBig::new(other.neg, sub_slices(&other.mag, &self.mag)),
            }
        }
    }

    fn sub(&self, other: &SBig) -> SBig {
        self.add(&other.neg())
    }

    fn mul(&self, other: &SBig, scratch: &mut Scratch) -> SBig {
        let mag = karatsuba::mul(&self.mag, &other.mag, scratch);
        SBig::new(self.neg != other.neg, mag)
    }

    /// Exact division by 2 (value must be even).
    fn half(&self) -> SBig {
        debug_assert!(
            self.is_zero() || self.mag[0] & 1 == 0,
            "half(): value not divisible by 2"
        );
        let half = crate::algos::div::schoolbook::shr_slices(&self.mag, 1);
        SBig::new(self.neg, half)
    }

    /// Exact division by 3 (value must be divisible).
    fn third(&self) -> SBig {
        let (q, r) = crate::algos::div::schoolbook::divmod_limb(&self.mag, 3);
        debug_assert_eq!(r, 0, "third(): value not divisible by 3");
        SBig::new(self.neg, q)
    }

    /// Shifted-weight evaluation: self + 2^k * part.
    fn add_weighted(&self, part: &[Limb], k: u32) -> SBig {
        let shifted = crate::algos::div::schoolbook::shl_slices(part, k);
        // shl_slices returns len+1 limbs (may end with zero) - canonicalize.
        let s = SBig::new(false, shifted);
        self.add(&s)
    }
}

/// Multiply canonical slices with Toom-3.
pub fn mul(a: &[Limb], b: &[Limb], scratch: &mut Scratch) -> Vec<Limb> {
    let n = a.len().max(b.len());
    if n < 64 || a.is_empty() || b.is_empty() {
        // Below the crossover, Karatsuba wins.
        return karatsuba::mul(a, b, scratch);
    }
    let m = n.div_ceil(3);

    let (a0, a1, a2) = split3(a, m);
    let (b0, b1, b2) = split3(b, m);

    // Evaluation.
    let sa0 = SBig::new(false, a0.to_vec());
    let sa1 = SBig::new(false, a1.to_vec());
    let sa2 = SBig::new(false, a2.to_vec());
    let sb0 = SBig::new(false, b0.to_vec());
    let sb1 = SBig::new(false, b1.to_vec());
    let sb2 = SBig::new(false, b2.to_vec());

    let a1d = sa1.neg();
    let b1d = sb1.neg();

    let pe0 = sa0.add(&sa2).add(&a1d); // a0 - a1 + a2
    let pe1 = sa0.add(&sa1).add(&sa2); // a0 + a1 + a2
    let pe2 = sa0.add_weighted(&sa1.mag, 1).add_weighted(&sa2.mag, 2); // a0 + 2a1 + 4a2
    let qe0 = sb0.add(&sb2).add(&b1d); // b0 - b1 + b2
    let qe1 = sb0.add(&sb1).add(&sb2);
    let qe2 = sb0.add_weighted(&sb1.mag, 1).add_weighted(&sb2.mag, 2);

    let v0 = SBig::new(false, a0.to_vec()).mul(&SBig::new(false, b0.to_vec()), scratch);
    let v1 = pe1.mul(&qe1, scratch);
    let vm1 = pe0.mul(&qe0, scratch);
    let v2 = pe2.mul(&qe2, scratch);
    let vinf = sa2.mul(&sb2, scratch);

    // Interpolation (all divisions exact).
    let c0 = v0.clone();
    let c4 = vinf.clone();
    let c2 = v1.add(&vm1).half().sub(&v0).sub(&vinf);
    let s2 = v1.sub(&vm1).half(); // c1 + c3
                                  // t = (v2 - c0 - 4*c2 - 16*c4)/2 = c1 + 4*c3
    let c2x4 = SBig::new(false, crate::algos::div::schoolbook::shl_slices(&c2.mag, 2));
    let c4x16 = SBig::new(false, crate::algos::div::schoolbook::shl_slices(&c4.mag, 4));
    let t = v2.sub(&v0).sub(&c2x4).sub(&c4x16).half();
    // t - s2 = 3*c3
    let c3 = t.sub(&s2).third();
    let c1 = s2.sub(&c3);

    // Recomposition: result = c0 + c1*B + c2*B^2 + c3*B^3 + c4*B^4.
    let mut out = vec![0u64; a.len() + b.len()];
    add_coeff(&mut out, &c0, 0);
    add_coeff(&mut out, &c1, m);
    add_coeff(&mut out, &c2, 2 * m);
    add_coeff(&mut out, &c3, 3 * m);
    add_coeff(&mut out, &c4, 4 * m);
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

fn add_coeff(out: &mut [Limb], c: &SBig, off: usize) {
    debug_assert!(!c.neg, "product coefficients must be non-negative");
    let mut carry = 0u64;
    for (i, &x) in c.mag.iter().enumerate() {
        let t = out[off + i] as u128 + x as u128 + carry as u128;
        out[off + i] = t as u64;
        carry = (t >> LIMB_BITS) as u64;
    }
    let mut k = off + c.mag.len();
    while carry != 0 && k < out.len() {
        let t = out[k] as u128 + carry as u128;
        out[k] = t as u64;
        carry = (t >> LIMB_BITS) as u64;
        k += 1;
    }
    debug_assert_eq!(carry, 0, "toom3 recomposition overflow");
}

fn split3(v: &[Limb], m: usize) -> (&[Limb], &[Limb], &[Limb]) {
    let mid = m.min(v.len());
    let hi_start = (2 * m).min(v.len());
    (&v[0..mid], &v[mid..hi_start], &v[hi_start..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::mul::schoolbook;

    fn pseudo_random_vec(len: usize, seed: u64) -> Vec<Limb> {
        let mut s = seed | 1;
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            })
            .collect()
    }

    #[test]
    fn toom3_matches_schoolbook() {
        let mut scratch = Scratch::new();
        for (n1, n2) in [
            (64usize, 64usize),
            (100, 70),
            (127, 1),
            (200, 200),
            (333, 111),
            (512, 512),
            (1000, 500),
            (1023, 1023),
            (1024, 1024),
            (1100, 900),
        ] {
            let a = pseudo_random_vec(n1, 0x777 + n1 as u64);
            let b = pseudo_random_vec(n2, 0x888 + n2 as u64);
            let got = mul(&a, &b, &mut scratch);
            let want = schoolbook::mul(&a, &b);
            assert_eq!(got, want, "toom3 {n1}x{n2}");
        }
    }

    #[test]
    fn toom3_handles_zero_and_small() {
        let mut scratch = Scratch::new();
        assert!(mul(&[], &[1], &mut scratch).is_empty());
        let a = pseudo_random_vec(200, 12);
        assert!(mul(&a, &[], &mut scratch).is_empty());
        let z = vec![0u64; 300];
        assert!(mul(&a, &z, &mut scratch).is_empty());
    }
}
