//! Sequential backend: the always-compiled correctness baseline that every
//! other backend is differential-tested against (spec 5.5).
//!
//! At M1 the algorithms run directly; the M3 dispatcher routes public
//! operations through here (and through the parallel/SIMD backends) with
//! identical results.

use crate::algos::Scratch;

/// Run a pure algorithm closure with a fresh scratch arena.
pub fn run<T>(f: impl FnOnce(&mut Scratch) -> T) -> T {
    let mut scratch = Scratch::new();
    f(&mut scratch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_provides_scratch() {
        let v = run(|s| {
            let buf = s.get(8);
            buf.len()
        });
        assert_eq!(v, 8);
    }
}
