//! Algorithm dispatcher: a pure function from (operation context, tunables)
//! to `(algorithm, backend, grain)` (spec 5.4 / FR-3). Deterministic within
//! a machine and process; runtime overrides and introspection are
//! thread-local; the tunables content hash is exposed by `meganum.about()`.

pub mod tunables;

use std::sync::OnceLock;

pub use tunables::{parse, SimdMode, Tunables};

/// Operation classes the dispatcher distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Add,
    Sub,
    Mul,
    Div,
    Sqrt,
    Convert,
}

/// Selected algorithm within a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Schoolbook,
    Karatsuba,
    Toom3,
    Ntt,
    BurnikelZiegler,
    Newton,
    Binary,
}

/// Selected execution backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sequential,
    Parallel,
}

/// Full dispatcher decision, exposed through `meganum.last_dispatch_info()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub op: OpKind,
    pub algorithm: Algorithm,
    pub backend: Backend,
    pub grain: usize,
    pub lhs_limbs: usize,
    pub rhs_limbs: usize,
    /// Tunables thresholds crossed (introspection, spec FR-3.4).
    pub thresholds_crossed: Vec<&'static str>,
}

/// Content hash of the embedded tunables (exposed by about()).
pub fn tunables_hash() -> u64 {
    static HASH: OnceLock<u64> = OnceLock::new();
    *HASH.get_or_init(|| {
        let text = include_str!("tunables.toml");
        let mut h: u64 = 0xCBF2_9CE4_8422_2325; // FNV-1a basis
        for byte in text.bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x1000_0000_01B3);
        }
        h
    })
}

/// Embedded, parsed tunables (parsed once; immutable afterwards).
pub fn tunables() -> &'static Tunables {
    static T: OnceLock<Tunables> = OnceLock::new();
    T.get_or_init(|| parse(include_str!("tunables.toml")).expect("embedded tunables must be valid"))
}

/// Worker count available to parallel backends: available_parallelism minus
/// the reservation (spec 5.4 worker awareness).
pub fn worker_count() -> usize {
    let reserved = tunables().parallel_worker_reservation;
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(reserved).max(1))
        .unwrap_or(1)
}

thread_local! {
    static LAST_DECISION: std::cell::RefCell<Option<Decision>> =
        const { std::cell::RefCell::new(None) };
    static OVERRIDES: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Record the last top-level decision (called by the routed operations).
pub fn record(decision: Decision) {
    LAST_DECISION.with(|d| *d.borrow_mut() = Some(decision));
}

/// Snapshot of the previous operation's decision (spec FR-3.4).
pub fn last_dispatch_info() -> Option<Decision> {
    LAST_DECISION.with(|d| d.borrow().clone())
}

/// Set a validated override (`"mul" -> "ntt"` etc.); invalid keys or
/// algorithm names raise DispatchError at the binding layer via Err.
pub fn set_override(op: &str, algorithm: &str) -> Result<(), crate::error::MegaNumError> {
    const OPS: [&str; 6] = ["add", "sub", "mul", "div", "sqrt", "convert"];
    const ALGOS: [&str; 7] = [
        "schoolbook",
        "karatsuba",
        "toom3",
        "ntt",
        "burnikel_ziegler",
        "newton",
        "binary",
    ];
    if !OPS.contains(&op) {
        return Err(crate::error::MegaNumError::Dispatch(format!(
            "unknown op {op:?}; valid ops: {OPS:?}"
        )));
    }
    if !ALGOS.contains(&algorithm) {
        return Err(crate::error::MegaNumError::Dispatch(format!(
            "unknown algorithm {algorithm:?}; valid: {ALGOS:?}"
        )));
    }
    OVERRIDES.with(|o| {
        o.borrow_mut().insert(op.to_string(), algorithm.to_string());
    });
    Ok(())
}

/// Clear all overrides (spec FR-3.5).
pub fn reset_overrides() {
    OVERRIDES.with(|o| o.borrow_mut().clear());
}

fn override_for(op: OpKind) -> Option<Algorithm> {
    let key = match op {
        OpKind::Add | OpKind::Sub => "add",
        OpKind::Mul => "mul",
        OpKind::Div => "div",
        OpKind::Sqrt => "sqrt",
        OpKind::Convert => "convert",
    };
    OVERRIDES.with(|o| {
        o.borrow().get(key).map(|s| match s.as_str() {
            "schoolbook" => Algorithm::Schoolbook,
            "karatsuba" => Algorithm::Karatsuba,
            "toom3" => Algorithm::Toom3,
            "ntt" => Algorithm::Ntt,
            "burnikel_ziegler" => Algorithm::BurnikelZiegler,
            "newton" => Algorithm::Newton,
            _ => Algorithm::Binary,
        })
    })
}

/// Peak-memory projection multiplier per op class (spec 5.6 budgets).
fn memory_factor(op: OpKind) -> usize {
    match op {
        OpKind::Mul => 4,
        OpKind::Div => 8,
        OpKind::Convert => 6,
        _ => 2,
    }
}

/// The dispatcher: pure function of (context, tunables).
pub fn decide(op: OpKind, lhs: usize, rhs: Option<usize>) -> Decision {
    let t = tunables();
    let n = lhs.max(rhs.unwrap_or(0));
    let mut thresholds = Vec::new();
    let algorithm;
    let mut grain = t.mul_ntt_grain;
    match op {
        OpKind::Add | OpKind::Sub => {
            algorithm = Algorithm::Schoolbook;
        }
        OpKind::Mul => {
            let ov = override_for(op);
            if let Some(a) = ov {
                algorithm = a;
            } else if n < t.mul_schoolbook_max_limbs {
                algorithm = Algorithm::Schoolbook;
                thresholds.push("mul.schoolbook_max_limbs");
            } else if n < t.mul_karatsuba_max_limbs {
                algorithm = Algorithm::Karatsuba;
                thresholds.push("mul.karatsuba_max_limbs");
            } else if n < t.mul_toom3_max_limbs {
                algorithm = Algorithm::Toom3;
                thresholds.push("mul.toom3_max_limbs");
            } else {
                algorithm = Algorithm::Ntt;
                thresholds.push("mul.toom3_max_limbs");
            }
        }
        OpKind::Div => {
            let ov = override_for(op);
            if let Some(a) = ov {
                algorithm = a;
            } else {
                let m = rhs.unwrap_or(1);
                let balanced_gap = lhs.saturating_sub(m);
                if n < t.div_schoolbook_max_limbs || balanced_gap < 48 {
                    algorithm = Algorithm::Schoolbook;
                    thresholds.push("div.schoolbook_max_limbs");
                } else if lhs < 2 * m + 48 && n >= t.div_bz_min_balanced {
                    algorithm = Algorithm::BurnikelZiegler;
                    thresholds.push("div.bz_min_balanced");
                } else {
                    algorithm = Algorithm::Newton;
                }
            }
        }
        OpKind::Sqrt | OpKind::Convert => {
            algorithm = Algorithm::Newton;
        }
    }
    let parallel = n >= t.parallel_min_limbs && worker_count() >= 2;
    if parallel {
        thresholds.push("parallel.min_limbs_for_parallel");
    }
    if parallel {
        grain = grain.max(n / worker_count().max(1));
    }
    let decision = Decision {
        op,
        algorithm,
        backend: if parallel {
            Backend::Parallel
        } else {
            Backend::Sequential
        },
        grain,
        lhs_limbs: lhs,
        rhs_limbs: rhs.unwrap_or(0),
        thresholds_crossed: thresholds,
    };
    let _ = memory_factor(op);
    record(decision.clone());
    #[cfg(feature = "dispatch-trace")]
    eprintln!(
        "meganum: dispatch op={:?} alg={:?} backend={:?} grain={} lhs={} rhs={}",
        decision.op,
        decision.algorithm,
        decision.backend,
        decision.grain,
        decision.lhs_limbs,
        decision.rhs_limbs
    );
    decision
}
