//! MegaInt Python bindings: a thin adapter over `meganum_core::bigint::BigInt`
//! with CPython-int semantics for every dunder (spec 8.2, 8.6).
//!
//! GIL discipline: every Python-object coercion happens with the GIL held;
//! only pure core arithmetic runs inside `py.detach` with Send payloads.

use crate::ffi_guard::{gil_hold, gil_release, map_err};
use meganum_core::bigint::{BigInt, Sign};
use meganum_core::biguint::BigUInt;
use meganum_core::error::MegaNumError as CoreErr;
use pyo3::exceptions::{PyOverflowError, PyTypeError};
use pyo3::prelude::*;
use pyo3::pyclass::CompareOp;

/// Arbitrary-precision signed integer with CPython `int` semantics.
#[pyclass(frozen, module = "meganum._native")]
#[derive(Clone)]
pub struct MegaInt {
    pub(crate) inner: BigInt,
}

impl MegaInt {
    pub fn from_bigint(v: BigInt) -> Self {
        MegaInt { inner: v }
    }
}

/// Coerce a Python object (MegaInt or int) into a BigInt. Large Python
/// ints travel through `int.to_bytes` (unsigned magnitude + explicit sign) -
/// a limb path never affected by `sys.set_int_max_str_digits` (FR-5.3).
/// GIL must be held.
fn coerce(_py: Python<'_>, ob: &Bound<'_, PyAny>) -> PyResult<BigInt> {
    if let Ok(m) = ob.cast::<MegaInt>() {
        return Ok(m.get().inner.clone());
    }
    if let Ok(v) = ob.extract::<i64>() {
        return Ok(BigInt::from_i64(v));
    }
    if ob.is_instance_of::<pyo3::types::PyInt>() {
        let is_neg = ob.call_method1("__lt__", (0,))?.is_truthy()?;
        let abs = if is_neg {
            ob.call_method0("__abs__")?
        } else {
            ob.clone()
        };
        let length = abs
            .call_method0("bit_length")?
            .call_method1("__add__", (7,))?
            .call_method1("__floordiv__", (8,))?;
        let length: usize = length.extract()?;
        let length = length.max(1);
        let bytes: Vec<u8> = abs.call_method1("to_bytes", (length, "big"))?.extract()?;
        let mag = BigUInt::from_bytes_be(&bytes);
        let v = BigInt::new(
            if is_neg {
                Sign::Negative
            } else {
                Sign::Positive
            },
            mag,
        );
        return Ok(v);
    }
    Err(PyTypeError::new_err(
        "operand must be MegaInt or int (float operands are only accepted by comparisons)",
    ))
}

/// Exact comparison between a BigInt and an f64 (CPython parity, FR-5.2).
///
/// The float is decomposed into its exact IEEE-754 value `±sig * 2^e` and
/// compared against the integer without any precision loss:
///
/// * `e >= 0`     : the float is an exact integer `sig << e`;
/// * `e < 0`      : compare `|a| * 2^-e` against `sig` exactly (an integer
///   equals a float only when the float is integral).
///
/// Truncating the fractional part would misorder `5 < 5.5` and make
/// `5 == 5.5` true, so it is never done.
fn cmp_bigint_f64(a: &BigInt, f: f64) -> std::cmp::Ordering {
    let f_neg = f.is_sign_negative();
    let a_neg = a.is_negative();
    let a_zero = a.is_zero();
    let f_zero = f == 0.0; // covers -0.0 too
    match (a_zero, f_zero) {
        (true, true) => return std::cmp::Ordering::Equal,
        (true, false) => {
            return if f_neg {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }
        (false, true) => {
            return if a_neg {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }
        _ => {}
    }
    let bits = f.to_bits();
    let exp_field = ((bits >> 52) & 0x7FF) as i64;
    let (sig, e): (u64, i64) = if exp_field == 0 {
        (bits & ((1u64 << 52) - 1), -1074) // subnormal
    } else {
        ((bits & ((1u64 << 52) - 1)) | (1u64 << 52), exp_field - 1075)
    };
    let mag_cmp = if e >= 0 {
        let f_mag = BigUInt::from(sig).shl(e as usize);
        a.mag().cmp(&f_mag)
    } else {
        // |a| * 2^-e  vs  sig: exact because both sides are integers.
        let a_scaled = a.mag().shl((-e) as usize);
        a_scaled.cmp(&BigUInt::from(sig))
    };
    if a_neg == f_neg {
        if a_neg {
            mag_cmp.reverse()
        } else {
            mag_cmp
        }
    } else if a_neg {
        std::cmp::Ordering::Less
    } else {
        std::cmp::Ordering::Greater
    }
}

/// Exact conversion to f64 with round-half-even; errors past IEEE range.
fn float_of(v: &BigInt) -> Result<f64, String> {
    if v.is_zero() {
        return Ok(0.0);
    }
    let mag = v.mag();
    let bl = mag.bit_length();
    if bl > 1025 {
        return Err(format!("integer too large to convert to float ({bl} bits)"));
    }
    if bl <= 53 {
        // Exactly representable (u64 cast is safe for bl <= 53).
        let t = u64::try_from(mag).unwrap_or(0);
        let value = t as f64;
        return Ok(if v.is_negative() { -value } else { value });
    }
    // Keep 54 bits: 53 significand bits + one guard bit; stickiness from
    // the dropped remainder decides the round-half-even case.
    let shift = bl - 54;
    let top = mag.shr(shift);
    let sticky = !mag
        .bit_and(&BigUInt::one().shl(shift).sub(&BigUInt::one()))
        .is_zero();
    let t = u64::try_from(&top).unwrap_or(0);
    let dropped = t & 1;
    let mut rest = t >> 1; // 53 bits
    if dropped == 1 && (sticky || rest & 1 == 1) {
        rest += 1; // may carry into 2^53; the exponent keeps the value right
    }
    let value = (rest as f64) * 2f64.powi(shift as i32 + 1);
    if value.is_infinite() {
        return Err("integer too large to convert to float".to_string());
    }
    Ok(if v.is_negative() { -value } else { value })
}

/// CPython-compatible long hash: |n| mod (2^61 - 1), sign applied, -1 -> -2.
fn hash_of(v: &BigInt) -> i64 {
    const P: u64 = (1u64 << 61) - 1;
    let mut h: u64 = 0;
    for &limb in v.mag().limbs().iter().rev() {
        // h = (h * 2^64 + limb) mod P; h < P < 2^61 so the product fits u128.
        let t = ((h as u128) << 64) | limb as u128;
        h = (t % P as u128) as u64;
    }
    let signed = if v.is_negative() {
        -(h as i64)
    } else {
        h as i64
    };
    if signed == -1 {
        -2
    } else {
        signed
    }
}

fn repr_of(v: &BigInt) -> String {
    let s = v.to_string_radix(10).unwrap_or_default();
    if s.len() <= 80 {
        format!("MegaInt('{s}')")
    } else {
        let head = &s[..30];
        let tail = &s[s.len() - 30..];
        let digits = s.len() - if v.is_negative() { 1 } else { 0 };
        format!("MegaInt('{head}...{tail}') (digits={digits})")
    }
}

mod format_spec {
    use meganum_core::bigint::BigInt;
    use meganum_core::error::{MegaNumError, Result};

    /// CPython integer format mini-language subset:
    /// `[[fill]align][sign][#][0][width][grouping][type]` with
    /// `align` in `<>=^`, `sign` in `+ -`, `grouping` in `, _` and
    /// `type` in `d n x X o b` (documented in docs/api-megaint.md).
    ///
    /// CPython-order details honored here:
    /// * grouping comes AFTER width (`>12,d`, not `>12d,`);
    /// * grouping with a non-decimal type raises (CPython `ValueError`);
    /// * the numeric default alignment is `>` (right);
    /// * the bare `0` flag sets the fill to `0` and the default alignment
    ///   to `=`; with an explicit alignment it only overrides the fill
    ///   (`'<08d'` -> `'42000000'`, `'=+07d'` -> `'+000007'`);
    /// * zero fill with grouping pads the RAW digits to the smallest count
    ///   whose grouped form reaches the field width, then inserts the
    ///   separators (the result may exceed the width: `'012,d'` on
    ///   1234567 -> `'0,001,234,567'`);
    /// * `#` adds the `0b`/`0o`/`0x`/`0X` prefix unconditionally for
    ///   radix types (including zero: `format(0, "#x")` == `"0x0"`).
    pub fn format(v: &BigInt, spec: &str) -> Result<String> {
        let cs: Vec<char> = spec.chars().collect();
        let mut i = 0usize;
        let mut fill = ' ';
        let mut align: char = '>';
        let mut align_seen = false;
        let mut fill_seen = false;
        if cs.len() >= 2 && matches!(cs[1], '<' | '>' | '^' | '=') {
            fill = cs[0];
            align = cs[1];
            align_seen = true;
            fill_seen = true;
            i = 2;
        } else if !cs.is_empty() && matches!(cs[0], '<' | '>' | '^' | '=') {
            align = cs[0];
            align_seen = true;
            i = 1;
        }
        let mut sign = '-';
        if i < cs.len() && matches!(cs[i], '+' | '-' | ' ') {
            sign = cs[i];
            i += 1;
        }
        let mut alt = false;
        if i < cs.len() && cs[i] == '#' {
            alt = true;
            i += 1;
        }
        let mut zero_flag = false;
        if i < cs.len() && cs[i] == '0' {
            zero_flag = true;
            i += 1;
        }
        let mut width: Option<usize> = None;
        let mut w = String::new();
        while i < cs.len() && cs[i].is_ascii_digit() {
            w.push(cs[i]);
            i += 1;
        }
        if !w.is_empty() {
            width = w.parse().ok();
        }
        let mut grouping: Option<char> = None;
        if i < cs.len() && (cs[i] == ',' || cs[i] == '_') {
            grouping = Some(cs[i]);
            i += 1;
        }
        if i < cs.len() && cs[i] == '.' {
            return Err(MegaNumError::Parse(
                "precision not allowed for integers".to_string(),
            ));
        }
        let mut ty = 'd';
        if i < cs.len() {
            ty = cs[i];
            i += 1;
        }
        if i != cs.len() {
            return Err(MegaNumError::Parse(format!(
                "unsupported format spec {spec:?}"
            )));
        }
        let decimal = matches!(ty, 'd' | 'n');
        let radix = match ty {
            'd' | 'n' => 10u32,
            'x' | 'X' => 16,
            'o' => 8,
            'b' => 2,
            other => {
                return Err(MegaNumError::Parse(format!(
                    "format type {other:?} not supported (d, n, x, X, o, b)"
                )))
            }
        };
        let mut digits = v
            .to_string_radix(radix)
            .map_err(|e| MegaNumError::Parse(e.to_string()))?;
        let negative = digits.starts_with('-');
        if negative {
            digits.remove(0);
        }
        if ty == 'X' {
            digits = digits.to_uppercase();
        }
        // `#` prefixes apply unconditionally for radix types (CPython:
        // format(0, "#x") == "0x0").
        let prefix = if alt {
            match ty {
                'x' => "0x",
                'X' => "0X",
                'o' => "0o",
                'b' => "0b",
                _ => "",
            }
        } else {
            ""
        };
        let sign_str = if negative {
            "-"
        } else {
            match sign {
                '+' => "+",
                ' ' => " ",
                _ => "",
            }
        };
        let head = format!("{sign_str}{prefix}");
        let head_len = head.chars().count();

        // Effective alignment / fill (CPython): numeric default align is '>';
        // the bare `0` flag defaults the alignment to '=' and the fill to
        // '0'. With an explicit alignment it still overrides an implied
        // fill ('<08d' -> '42000000'), but never an explicit one.
        let eff_align = if align_seen {
            align
        } else if zero_flag {
            '='
        } else {
            '>'
        };
        let eff_fill = if fill_seen {
            fill
        } else if zero_flag {
            '0'
        } else {
            ' '
        };
        // Sign-aware zero fill pads the digits themselves (before
        // grouping); every other combination pads the rendered body.
        let zero_pad = eff_align == '=' && eff_fill == '0';

        let digits = if let Some(g) = grouping {
            if !decimal {
                return Err(MegaNumError::Parse(format!(
                    "Cannot specify '{g}' with '{ty}'"
                )));
            }
            if zero_pad {
                if let Some(wd) = width {
                    // Smallest raw-digit count whose grouped form reaches
                    // the field width; the result may exceed the width.
                    loop {
                        let grouped_len = group_every_3(&digits, g).chars().count();
                        if head_len + grouped_len >= wd {
                            break;
                        }
                        digits.insert(0, '0');
                    }
                }
            }
            group_every_3(&digits, g)
        } else {
            if zero_pad {
                if let Some(wd) = width {
                    let target = wd.saturating_sub(head_len);
                    while digits.chars().count() < target {
                        digits.insert(0, '0');
                    }
                }
            }
            digits
        };

        let body_len = head_len + digits.chars().count();
        Ok(match width {
            Some(wd) if body_len < wd => {
                let pad = wd - body_len;
                match eff_align {
                    // '=' is sign-aware: fill goes between head and digits.
                    '=' => format!("{head}{}{digits}", eff_fill.to_string().repeat(pad)),
                    '>' => format!("{}{head}{digits}", eff_fill.to_string().repeat(pad)),
                    '<' => format!("{head}{digits}{}", eff_fill.to_string().repeat(pad)),
                    _ => {
                        let left = pad / 2;
                        format!(
                            "{left_fill}{head}{digits}{right_fill}",
                            left_fill = eff_fill.to_string().repeat(left),
                            right_fill = eff_fill.to_string().repeat(pad - left)
                        )
                    }
                }
            }
            _ => format!("{head}{digits}"),
        })
    }

    /// Insert `sep` every three digits from the right (decimal grouping).
    fn group_every_3(digits: &str, sep: char) -> String {
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
        let chars: Vec<char> = digits.chars().collect();
        let n = chars.len();
        for (k, c) in chars.iter().enumerate() {
            if k > 0 && (n - k) % 3 == 0 {
                grouped.push(sep);
            }
            grouped.push(*c);
        }
        grouped
    }
}

#[pymethods]
impl MegaInt {
    #[new]
    #[pyo3(signature = (value=None))]
    fn new(py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        gil_hold(|| {
            let v = match value {
                None => BigInt::zero(),
                Some(v) => {
                    if let Ok(s) = v.extract::<String>() {
                        BigInt::parse(&s).map_err(map_err)?
                    } else {
                        coerce(py, v)?
                    }
                }
            };
            Ok(MegaInt::from_bigint(v))
        })
    }

    /// Parse a decimal/hex/octal/binary string (sign, prefixes, `_`).
    #[staticmethod]
    fn parse(s: &str) -> PyResult<Self> {
        gil_hold(|| BigInt::parse(s).map(Self::from_bigint).map_err(map_err))
    }

    #[staticmethod]
    fn from_hex(s: &str) -> PyResult<Self> {
        Self::from_base(s, 16)
    }

    #[staticmethod]
    fn from_bin(s: &str) -> PyResult<Self> {
        Self::from_base(s, 2)
    }

    #[staticmethod]
    fn from_oct(s: &str) -> PyResult<Self> {
        Self::from_base(s, 8)
    }

    /// Parse in radix 2..=62 (digits 0-9, a-z, then A-Z for 36-61).
    /// Delegates to the core parser so sign, digit-grouping `_` and
    /// matching prefixes follow the CPython rules exactly.
    #[staticmethod]
    fn from_base(s: &str, radix: u32) -> PyResult<Self> {
        gil_hold(|| {
            if !(2..=62).contains(&radix) {
                return Err(map_err(CoreErr::Parse(format!(
                    "radix {radix} out of range 2..=62"
                ))));
            }
            BigInt::parse_radix(s, radix)
                .map(Self::from_bigint)
                .map_err(map_err)
        })
    }

    #[staticmethod]
    #[pyo3(signature = (data, byteorder = "big", signed = false))]
    fn from_bytes(data: &[u8], byteorder: &str, signed: bool) -> PyResult<Self> {
        gil_hold(|| {
            let v = match byteorder {
                "little" => BigInt::from_bytes_le(data, signed),
                "big" => BigInt::from_bytes_be(data, signed),
                other => {
                    return Err(map_err(CoreErr::Parse(format!(
                        "byteorder must be 'little' or 'big', got {other:?}"
                    ))))
                }
            };
            Ok(Self::from_bigint(v))
        })
    }

    #[pyo3(signature = (length = None, byteorder = "big", signed = false))]
    fn to_bytes(
        &self,
        length: Option<usize>,
        byteorder: &str,
        signed: bool,
    ) -> PyResult<Py<PyAny>> {
        let inner = self.inner.clone();
        let bo = byteorder.to_string();
        let bytes = match bo.as_str() {
            "little" => inner.to_bytes_le(length, signed),
            "big" => inner.to_bytes_be(length, signed),
            other => Err(CoreErr::Parse(format!(
                "byteorder must be 'little' or 'big', got {other:?}"
            ))),
        }
        .map_err(map_err)?;
        Ok(Python::attach(|py| {
            pyo3::types::PyBytes::new(py, &bytes).into_any().unbind()
        }))
    }

    fn to_hex(&self) -> String {
        self.inner.to_string_radix(16).unwrap_or_default()
    }

    fn to_bin(&self) -> String {
        self.inner.to_string_radix(2).unwrap_or_default()
    }

    /// Render in radix 2..=62 (digits 0-9, a-z, then A-Z for 36-61).
    fn to_base(&self, radix: u32) -> PyResult<String> {
        gil_hold(|| {
            if !(2..=62).contains(&radix) {
                return Err(map_err(CoreErr::Parse(format!(
                    "radix {radix} out of range 2..=62"
                ))));
            }
            self.inner.to_string_radix(radix).map_err(map_err)
        })
    }

    fn bit_length(&self) -> usize {
        self.inner.bit_length()
    }

    fn bit_count(&self) -> u32 {
        self.inner.bit_count()
    }

    fn trailing_zeros(&self) -> Option<usize> {
        self.inner.trailing_zeros()
    }

    fn leading_zeros(&self) -> usize {
        self.inner.leading_zeros()
    }

    /// Exact integer square root (floor).
    fn isqrt(&self) -> PyResult<Self> {
        let inner = self.inner.clone();
        gil_hold(|| inner.isqrt().map(Self::from_bigint).map_err(map_err))
    }

    fn gcd(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other_v = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.gcd(&other_v))))
    }

    fn lcm(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other_v = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.lcm(&other_v))))
    }

    /// Modular exponentiation: `(self ** exp) mod modulus`.
    fn modpow(
        &self,
        py: Python<'_>,
        exp: &Bound<'_, PyAny>,
        modulus: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let e = coerce(py, exp)?;
        let m = coerce(py, modulus)?;
        let a = self.inner.clone();
        gil_release(py, move || {
            a.modpow(&e, &m).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn mod_inverse(&self, py: Python<'_>, modulus: &Bound<'_, PyAny>) -> PyResult<Self> {
        let m = coerce(py, modulus)?;
        let a = self.inner.clone();
        gil_release(py, move || {
            a.mod_inverse(&m).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn jacobi(&self, py: Python<'_>, n: &Bound<'_, PyAny>) -> PyResult<i8> {
        let m = coerce(py, n)?;
        let a = self.inner.clone();
        gil_release(py, move || a.jacobi(&m).map_err(map_err))
    }

    /// Miller-Rabin probable-prime test (probabilistic; documented).
    fn is_probable_prime(&self, py: Python<'_>, rounds: u32) -> PyResult<bool> {
        let a = self.inner.clone();
        gil_release(py, move || Ok(a.is_probable_prime(rounds)))
    }

    // ----- operators -------------------------------------------------
    // The a-side is always `self.inner`; only `other` needs coercion (GIL
    // held), then the arithmetic runs detached.

    fn __add__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.add(&b))))
    }

    fn __radd__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.add(&b))))
    }

    fn __sub__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.sub(&b))))
    }

    fn __rsub__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.sub(&b))))
    }

    fn __mul__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.mul(&b))))
    }

    fn __rmul__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.mul(&b))))
    }

    fn __floordiv__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || {
            if b.is_zero() {
                return Err(map_err(CoreErr::DivisionByZero));
            }
            a.floordiv(&b).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn __rfloordiv__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || {
            if b.is_zero() {
                return Err(map_err(CoreErr::DivisionByZero));
            }
            a.floordiv(&b).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn __mod__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || {
            if b.is_zero() {
                return Err(map_err(CoreErr::DivisionByZero));
            }
            a.rem_floor(&b).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn __rmod__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || {
            if b.is_zero() {
                return Err(map_err(CoreErr::DivisionByZero));
            }
            a.rem_floor(&b).map(Self::from_bigint).map_err(map_err)
        })
    }

    fn __divmod__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || {
            if b.is_zero() {
                return Err(map_err(CoreErr::DivisionByZero));
            }
            let (q, r) = a.divmod_floor(&b).map_err(map_err)?;
            Ok(Python::attach(|py| {
                let q = Self::from_bigint(q)
                    .into_pyobject(py)
                    .expect("pyclass")
                    .into_any();
                let r = Self::from_bigint(r)
                    .into_pyobject(py)
                    .expect("pyclass")
                    .into_any();
                pyo3::types::PyTuple::new(py, [q, r])
                    .expect("tuple of two")
                    .into_any()
                    .unbind()
            }))
        })
    }

    fn __truediv__(&self, _other: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyTypeError::new_err(
            "MegaInt does not support true division; use // for floor division or MegaFloat for arbitrary-precision quotients",
        ))
    }

    fn __pow__(
        &self,
        py: Python<'_>,
        exp: &Bound<'_, PyAny>,
        modulus: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let e = coerce(py, exp)?;
        let a = self.inner.clone();
        let m = modulus.map(|mv| coerce(py, mv)).transpose()?;
        gil_release(py, move || match m {
            Some(m) => {
                // With a modulus CPython computes the modular inverse for a
                // negative exponent; the result stays exact, so the
                // float-escape TypeError does not apply here.
                a.modpow(&e, &m).map(Self::from_bigint).map_err(map_err)
            }
            None => {
                if e.is_negative() {
                    return Err(PyErr::new::<PyTypeError, _>(
                        "MegaInt ** requires a non-negative exponent; use MegaFloat for real-valued powers",
                    ));
                }
                // The result has ~a.bit_length() * e bits, where e is the
                // VALUE of the exponent: the guard must multiply by e
                // itself, not by its bit length (a bit-length-based
                // estimate under-guards and lets ~536 MB results through).
                let e_u = u64::try_from(e.mag())
                    .map_err(|_| map_err(CoreErr::Precision("exponent too large".to_owned())))?;
                let result_bits = (a.bit_length() as u128) * (e_u as u128);
                if result_bits > 64_000_000 {
                    return Err(map_err(CoreErr::Precision(format!(
                        "power result would exceed ~8 MB ({result_bits} bits)"
                    ))));
                }
                Ok(Self::from_bigint(a.pow_u64(e_u)))
            }
        })
    }

    fn __rpow__(
        &self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
        modulus: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let e = self.inner.clone();
        let m = modulus.map(|mv| coerce(py, mv)).transpose()?;
        gil_release(py, move || match m {
            Some(m) => a.modpow(&e, &m).map(Self::from_bigint).map_err(map_err),
            None => {
                if e.is_negative() {
                    return Err(PyErr::new::<PyTypeError, _>(
                        "MegaInt ** requires a non-negative exponent",
                    ));
                }
                let e_u = u64::try_from(e.mag())
                    .map_err(|_| map_err(CoreErr::Precision("exponent too large".to_owned())))?;
                let result_bits = (a.bit_length() as u128) * (e_u as u128);
                if result_bits > 64_000_000 {
                    return Err(map_err(CoreErr::Precision(format!(
                        "power result would exceed ~8 MB ({result_bits} bits)"
                    ))));
                }
                Ok(Self::from_bigint(a.pow_u64(e_u)))
            }
        })
    }

    fn __lshift__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = self.inner.clone();
        let n = shift_amount(py, other)?;
        gil_release(py, move || Ok(Self::from_bigint(a.shl(n))))
    }

    fn __rshift__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = self.inner.clone();
        let n = shift_amount(py, other)?;
        gil_release(py, move || Ok(Self::from_bigint(a.shr(n))))
    }

    fn __and__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_and(&b))))
    }

    fn __rand__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_and(&b))))
    }

    fn __or__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_or(&b))))
    }

    fn __ror__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_or(&b))))
    }

    fn __xor__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let b = coerce(py, other)?;
        let a = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_xor(&b))))
    }

    fn __rxor__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let a = coerce(py, other)?;
        let b = self.inner.clone();
        gil_release(py, move || Ok(Self::from_bigint(a.bit_xor(&b))))
    }

    fn __invert__(&self) -> Self {
        Self::from_bigint(self.inner.bit_not())
    }

    fn __neg__(&self) -> Self {
        Self::from_bigint(self.inner.neg())
    }

    fn __pos__(&self) -> Self {
        self.clone()
    }

    fn __abs__(&self) -> Self {
        Self::from_bigint(self.inner.abs())
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_zero()
    }

    fn __int__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        // Exact limb path: int.from_bytes, never digit-limited (FR-5.3).
        // `signed` is keyword-only on int.from_bytes, so pass it as kwargs.
        let bytes = self.inner.to_bytes_be(None, true).map_err(map_err)?;
        let builtins = py.import("builtins")?;
        let from_bytes = builtins.getattr("int")?.getattr("from_bytes")?;
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs.set_item("signed", true)?;
        let obj = from_bytes.call(
            (pyo3::types::PyBytes::new(py, &bytes), "big"),
            Some(&kwargs),
        )?;
        Ok(obj.into_any().unbind())
    }

    fn __index__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__int__(py)
    }

    fn __float__(&self) -> PyResult<f64> {
        gil_hold(|| float_of(&self.inner).map_err(PyOverflowError::new_err))
    }

    fn __hash__(&self) -> i64 {
        hash_of(&self.inner)
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        // Exact integer operands first (MegaInt and int); the float path is
        // last because f64 extraction succeeds via __float__ for MegaInt
        // operands and would lose precision above 2^53 (FR-5.2).
        if other.cast::<MegaInt>().is_ok() || other.is_instance_of::<pyo3::types::PyInt>() {
            let bv = Python::attach(|py| coerce(py, other))?;
            let a = self.inner.clone();
            return Ok(compare_with(a, bv, op));
        }
        // NaN handling: every comparison false except !=.
        let f: f64 = other.extract().map_err(|_| {
            PyTypeError::new_err("comparison operand must be MegaInt, int or float")
        })?;
        if f.is_nan() {
            return Ok(matches!(op, CompareOp::Ne));
        }
        if f.is_infinite() {
            // Any finite integer is ordered against +/-inf; magnitude is
            // irrelevant (0 < inf is True in CPython).
            return Ok(match op {
                CompareOp::Lt => f > 0.0,
                CompareOp::Le => f > 0.0,
                CompareOp::Eq => false,
                CompareOp::Ne => true,
                CompareOp::Gt => f < 0.0,
                CompareOp::Ge => f < 0.0,
            });
        }
        let a = self.inner.clone();
        Ok(compare_with_float(&a, f, op))
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        let inner = self.inner.clone();
        gil_release(py, move || {
            Ok(inner.to_string_radix(10).unwrap_or_default())
        })
    }

    fn __repr__(&self) -> String {
        repr_of(&self.inner)
    }

    fn __format__(&self, format_spec: &str) -> PyResult<String> {
        let inner = self.inner.clone();
        let spec = format_spec.to_string();
        gil_hold(|| format_spec::format(&inner, &spec).map_err(map_err))
    }

    #[pyo3(signature = (ndigits = None))]
    fn __round__(&self, _py: Python<'_>, ndigits: Option<i64>) -> PyResult<Self> {
        let nd = ndigits.unwrap_or(0);
        let inner = self.inner.clone();
        gil_hold(|| {
            if nd >= 0 {
                return Ok(self.clone());
            }
            let mag = inner.mag().clone();
            let places = (-nd) as usize;
            let ten = BigUInt::from(10u64);
            let mut div = BigUInt::one();
            for _ in 0..places {
                div = div.mul(&ten);
            }
            let (q, r) = mag.divmod(&div).map_err(map_err)?;
            let two_r = r.add(&r);
            let mut q_adj = q.clone();
            match two_r.cmp(&div) {
                std::cmp::Ordering::Greater => q_adj = q.add(&BigUInt::one()),
                std::cmp::Ordering::Equal => {
                    if q.limbs().first().copied().unwrap_or(0) & 1 == 1 {
                        q_adj = q.add(&BigUInt::one());
                    }
                }
                std::cmp::Ordering::Less => {}
            }
            let scaled = q_adj.mul(&div);
            Ok(Self::from_bigint(BigInt::new(inner.sign(), scaled)))
        })
    }

    fn __trunc__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__int__(py)
    }

    fn __floor__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__int__(py)
    }

    fn __ceil__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.__int__(py)
    }

    fn __reduce__(&self) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            let native = py.import("meganum._native")?;
            let unpickle = native.getattr("_unpickle_megaint")?;
            let payload = self.dump_payload();
            let tup = pyo3::types::PyTuple::new(
                py,
                [pyo3::types::PyBytes::new(py, &payload).into_any()],
            )?;
            Ok((unpickle, tup)
                .into_pyobject(py)
                .expect("tuple")
                .into_any()
                .unbind())
        })
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

impl MegaInt {
    fn dump_payload(&self) -> Vec<u8> {
        let mut out = vec![b'm', b'1'];
        out.push(self.inner.is_negative() as u8);
        out.extend_from_slice(&self.inner.mag().to_bytes_le());
        out
    }
}

/// Pickle loader (module-level function `_unpickle_megaint`).
pub(crate) fn unpickle_megaint(payload: &[u8]) -> PyResult<MegaInt> {
    if payload.len() < 3 || payload[0] != b'm' || payload[1] != b'1' {
        return Err(PyTypeError::new_err("invalid MegaInt pickle payload"));
    }
    let neg = payload[2] == 1;
    let mag = BigUInt::from_bytes_le(&payload[3..]);
    Ok(MegaInt::from_bigint(BigInt::new(
        if neg { Sign::Negative } else { Sign::Positive },
        mag,
    )))
}

fn compare_with(a: BigInt, b: BigInt, op: CompareOp) -> bool {
    let ord = a.cmp(&b);
    match op {
        CompareOp::Lt => ord == std::cmp::Ordering::Less,
        CompareOp::Le => ord != std::cmp::Ordering::Greater,
        CompareOp::Eq => ord == std::cmp::Ordering::Equal,
        CompareOp::Ne => ord != std::cmp::Ordering::Equal,
        CompareOp::Gt => ord == std::cmp::Ordering::Greater,
        CompareOp::Ge => ord != std::cmp::Ordering::Less,
    }
}

fn compare_with_float(a: &BigInt, f: f64, op: CompareOp) -> bool {
    let ord = cmp_bigint_f64(a, f);
    match op {
        CompareOp::Lt => ord == std::cmp::Ordering::Less,
        CompareOp::Le => ord != std::cmp::Ordering::Greater,
        CompareOp::Eq => ord == std::cmp::Ordering::Equal,
        CompareOp::Ne => ord != std::cmp::Ordering::Equal,
        CompareOp::Gt => ord == std::cmp::Ordering::Greater,
        CompareOp::Ge => ord != std::cmp::Ordering::Less,
    }
}

fn shift_amount(py: Python<'_>, ob: &Bound<'_, PyAny>) -> PyResult<usize> {
    let v = coerce(py, ob)?;
    if v.is_negative() {
        return Err(PyTypeError::new_err("shift count must be non-negative"));
    }
    let top =
        u64::try_from(v.mag()).map_err(|_| PyOverflowError::new_err("shift count too large"))?;
    usize::try_from(top).map_err(|_| PyOverflowError::new_err("shift count too large"))
}
