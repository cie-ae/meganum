//! Embedded tunables file + strict-subset TOML parser (ADR-0004: no
//! external TOML dependency, per the spec 2 dependency budget).
//!
//! Supported subset: `[table]` headers, `key = value` with integer, boolean
//! or quoted-string values, `#` comments. Anything else is a hard error at
//! first use, so malformed tunables fail fast rather than being guessed.

/// Parsed tunables (mirrors dispatch/tunables.toml).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunables {
    pub mul_schoolbook_max_limbs: usize,
    pub mul_karatsuba_max_limbs: usize,
    pub mul_toom3_max_limbs: usize,
    pub mul_ntt_grain: usize,
    pub div_schoolbook_max_limbs: usize,
    pub div_bz_min_balanced: usize,
    pub parallel_min_limbs: usize,
    pub parallel_worker_reservation: usize,
    pub parallel_memory_lean_factor: usize,
    pub simd_enable_avx2: bool,
    pub simd_enable_avx512: SimdMode,
    pub simd_enable_neon: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdMode {
    Auto,
    On,
    Off,
}

/// Parse the strict TOML subset into `Tunables`. Panics only through a
/// typed error path: returns Err on any structural deviation (fail fast).
pub fn parse(text: &str) -> Result<Tunables, String> {
    let mut section = String::new();
    let mut ints: Vec<(&str, i128)> = Vec::new();
    let mut bools: Vec<(&str, bool)> = Vec::new();
    let mut strings: Vec<(&str, String)> = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw
            .split('#')
            .next()
            .expect("split '#' always yields one")
            .trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(format!("line {lineno}: malformed section header"));
            }
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {lineno}: expected key = value"));
        };
        let key = key.trim();
        let value = value.trim();
        let full_key = format!("{section}.{key}");
        let owned = Box::leak(full_key.into_boxed_str());
        if let Some(stripped) = value.strip_prefix('"') {
            let inner = stripped
                .strip_suffix('"')
                .ok_or_else(|| format!("line {lineno}: unterminated string"))?;
            strings.push((owned, inner.to_string()));
        } else if value == "true" || value == "false" {
            bools.push((owned, value == "true"));
        } else {
            let v: i128 = value.parse().map_err(|_| {
                format!("line {lineno}: value {value:?} is not an integer, bool or string")
            })?;
            ints.push((owned, v));
        }
    }
    let int_of = |name: &str| -> Result<usize, String> {
        ints.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| *v as usize)
            .ok_or_else(|| format!("missing key {name}"))
    };
    let bool_of = |name: &str| -> Result<bool, String> {
        bools
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("missing key {name}"))
    };
    let string_of = |name: &str| -> Result<String, String> {
        strings
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| format!("missing key {name}"))
    };
    let avx512 = match string_of("simd.enable_avx512")?.as_str() {
        "auto" => SimdMode::Auto,
        "on" => SimdMode::On,
        "off" => SimdMode::Off,
        other => return Err(format!("simd.enable_avx512: unknown mode {other:?}")),
    };
    Ok(Tunables {
        mul_schoolbook_max_limbs: int_of("mul.schoolbook_max_limbs")?,
        mul_karatsuba_max_limbs: int_of("mul.karatsuba_max_limbs")?,
        mul_toom3_max_limbs: int_of("mul.toom3_max_limbs")?,
        mul_ntt_grain: int_of("mul.ntt_grain")?,
        div_schoolbook_max_limbs: int_of("div.schoolbook_max_limbs")?,
        div_bz_min_balanced: int_of("div.bz_min_balanced")?,
        parallel_min_limbs: int_of("parallel.min_limbs_for_parallel")?,
        parallel_worker_reservation: int_of("parallel.worker_reservation")?,
        parallel_memory_lean_factor: int_of("parallel.memory_lean_factor")?,
        simd_enable_avx2: bool_of("simd.enable_avx2")?,
        simd_enable_avx512: avx512,
        simd_enable_neon: bool_of("simd.enable_neon")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tunables_parse() {
        let t = parse(include_str!("tunables.toml")).expect("embedded tunables must parse");
        assert_eq!(t.mul_schoolbook_max_limbs, 40);
        assert_eq!(t.mul_karatsuba_max_limbs, 1024);
        assert_eq!(t.mul_toom3_max_limbs, 8192);
        assert_eq!(t.mul_ntt_grain, 512);
        assert_eq!(t.div_schoolbook_max_limbs, 48);
        assert_eq!(t.div_bz_min_balanced, 128);
        assert_eq!(t.parallel_min_limbs, 4096);
        assert_eq!(t.parallel_worker_reservation, 1);
        assert_eq!(t.parallel_memory_lean_factor, 4);
        assert!(t.simd_enable_avx2);
        assert_eq!(t.simd_enable_avx512, SimdMode::Auto);
        assert!(t.simd_enable_neon);
    }

    #[test]
    fn hostile_inputs_rejected() {
        assert!(parse("garbage").is_err());
        assert!(parse("[mul\nfoo = 1").is_err());
        assert!(parse("[mul]\nfoo = 'bar'").is_err());
        assert!(parse("[mul]\nfoo = 1.5").is_err());
        assert!(parse("[mul]\nfoo = true # ok\nbar = 2").is_err());
        assert!(parse("[mul]\nschoolbook_max_limbs = 40").is_err()); // missing keys
                                                                     // Comments and whitespace are tolerated.
        assert!(parse("# header\n[mul] # table\nschoolbook_max_limbs = 40\n").is_err());
        // other keys missing
    }

    #[test]
    fn partial_document_reports_missing_keys() {
        let doc = "[mul]\nschoolbook_max_limbs = 40\nkaratsuba_max_limbs = 1\ntoom3_max_limbs = 2\nntt_grain = 3\n";
        assert!(parse(doc).is_err());
    }
}
