//! Algorithm layer: pure functions over limb slices.
//!
//! Rules (spec 5.3): no threads, no SIMD intrinsics, no allocator tricks
//! inside algorithm bodies - those live in the backend layer. Every
//! optimized algorithm has an obviously-correct reference twin in the
//! cfg-gated `_ref` modules, used by differential tests.

pub mod add;
pub mod convert;
pub mod div;
pub mod gcd;
pub mod mul;
pub mod number_theory;
pub mod pow;
pub mod sqrt;

#[cfg(any(test, feature = "ref-impls"))]
pub mod _ref;

#[cfg(test)]
mod differential_tests;

use crate::limb::Limb;

/// Recyclable scratch buffer pool shared across algorithm calls within one
/// operation. Buffers handed out are zero-initialized fresh or reused
/// (reuse overwrites, so callers must treat contents as uninitialized).
///
/// The pool keeps peak memory bounded (spec 5.6: pooled scratch, no
/// allocation in inner loops of basecase kernels).
#[derive(Default)]
pub struct Scratch {
    pool: Vec<Vec<Limb>>,
}

impl Scratch {
    /// Create an empty pool.
    pub fn new() -> Self {
        Scratch { pool: Vec::new() }
    }

    /// Get a buffer with capacity >= `len`; contents undefined.
    pub fn get(&mut self, len: usize) -> Vec<Limb> {
        match self.pool.pop() {
            Some(mut v) => {
                v.clear();
                v.resize(len, 0);
                v
            }
            None => vec![0; len],
        }
    }

    /// Return a buffer to the pool for reuse.
    pub fn put(&mut self, v: Vec<Limb>) {
        if self.pool.len() < 64 {
            self.pool.push(v);
        }
    }
}

/// Temporary alias so backend code can reference the pool type uniformly.
pub type ScratchArena = Scratch;

/// SIMD-chunked add (scalar fallback when the feature or CPU lacks kernels).
#[cfg(feature = "simd")]
pub fn backend_add(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    crate::backend::simd::add(a, b)
}

/// SIMD-chunked subtract (a >= b).
#[cfg(feature = "simd")]
pub fn backend_sub(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    crate::backend::simd::sub(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_reuses_buffers() {
        let mut s = Scratch::new();
        let a = s.get(10);
        assert_eq!(a.len(), 10);
        s.put(a);
        let b = s.get(4);
        assert_eq!(b.len(), 4);
        s.put(b);
        // Pool was populated, so the next get comes from reuse (capacity kept).
        let c = s.get(3);
        assert_eq!(c.len(), 3);
    }
}
