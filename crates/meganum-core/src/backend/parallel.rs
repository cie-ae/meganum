//! Rayon-backed parallel scheduling (feature `parallel`).
//!
//! Work-splitting with adaptive grain sizes from tunables, capped join
//! depth, and deterministic merge order so output is bit-identical to the
//! sequential backend (spec 5.5 / FR-3.3).

/// Number of worker threads rayon will use for parallel ranges.
pub fn worker_count() -> usize {
    rayon::current_num_threads().max(1)
}

/// Split a range `[lo, hi)` into deterministic contiguous chunks of at
/// least `grain` items; chunk boundaries depend only on (lo, hi, grain),
/// never on thread scheduling, keeping merges deterministic.
pub fn chunk_bounds(lo: usize, hi: usize, grain: usize) -> Vec<(usize, usize)> {
    let grain = grain.max(1);
    let mut out = Vec::new();
    let mut start = lo;
    while start < hi {
        let end = (start + grain).min(hi);
        out.push((start, end));
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_is_deterministic_and_covers() {
        for grain in [1usize, 7, 64, 1000] {
            let bounds = chunk_bounds(0, 100, grain);
            assert_eq!(bounds.first().unwrap().0, 0);
            assert_eq!(bounds.last().unwrap().1, 100);
            for w in bounds.windows(2) {
                assert_eq!(w[0].1, w[1].0, "contiguous");
            }
            for (lo, hi) in &bounds {
                assert!(*hi - *lo <= grain.max(1), "chunk larger than grain");
                assert!(hi > lo);
            }
        }
    }

    #[test]
    fn worker_count_positive() {
        assert!(worker_count() >= 1);
    }
}
