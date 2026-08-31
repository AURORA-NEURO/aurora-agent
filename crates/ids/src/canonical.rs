//! Canonical JSON serialization.
//!
//! The byte encoding produced here is defined to be identical to the Python reference
//! runtime's `json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`
//! followed by `.encode("utf-8")`. Content hashes in Context Certificates are taken over
//! these bytes, so any divergence from Python silently breaks cross-language replay.
//!
//! The two places where a naive implementation diverges are float formatting (Rust's
//! `{}`/`{:e}` and CPython's `repr` disagree on when to use exponential notation and on
//! exponent padding) and non-finite numbers (CPython emits bare `NaN`/`Infinity`, which is
//! not JSON). Both are handled explicitly below.

use crate::error::CanonicalError;
use serde_json::Value;

pub fn to_canonical_string(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write_value(value, &mut out)?;
    Ok(out)
}

pub fn to_canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    to_canonical_string(value).map(String::into_bytes)
}

fn write_value(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(n, out)?,
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(&map[*key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_number(n: &serde_json::Number, out: &mut String) -> Result<(), CanonicalError> {
    if let Some(u) = n.as_u64() {
        out.push_str(&u.to_string());
        return Ok(());
    }
    if let Some(i) = n.as_i64() {
        out.push_str(&i.to_string());
        return Ok(());
    }
    match n.as_f64() {
        Some(f) if f.is_finite() => {
            out.push_str(&python_repr_f64(f));
            Ok(())
        }
        Some(f) => Err(CanonicalError::NonFiniteNumber(f.to_string())),
        None => Err(CanonicalError::UnrepresentableNumber(n.to_string())),
    }
}

/// Formats a finite `f64` exactly as CPython's `repr` would.
///
/// CPython decides between fixed and exponential notation on `decpt`, the position of the
/// decimal point relative to the shortest round-trip digit string: exponential is used when
/// `decpt <= -4 || decpt > 16`. Exponents are always signed and zero-padded to two digits,
/// and a fixed-notation value with no fractional part gains a trailing `.0`.
pub fn python_repr_f64(f: f64) -> String {
    debug_assert!(f.is_finite());

    if f == 0.0 {
        return if f.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }

    let negative = f < 0.0;
    let exp_form = format!("{:e}", f.abs());
    let (mantissa, exponent) = exp_form
        .split_once('e')
        .expect("Rust LowerExp always emits an exponent");
    let exponent: i32 = exponent.parse().expect("Rust LowerExp emits a valid exponent");

    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };

    let decpt = exponent + 1;
    let body = if decpt <= -4 || decpt > 16 {
        format_exponential(digits, exponent)
    } else {
        format_fixed(digits, decpt)
    };

    if negative {
        format!("-{body}")
    } else {
        body
    }
}

fn format_exponential(digits: &str, exponent: i32) -> String {
    let mut s = String::new();
    s.push_str(&digits[..1]);
    if digits.len() > 1 {
        s.push('.');
        s.push_str(&digits[1..]);
    }
    s.push('e');
    if exponent < 0 {
        s.push('-');
    } else {
        s.push('+');
    }
    let magnitude = exponent.unsigned_abs();
    if magnitude < 10 {
        s.push('0');
    }
    s.push_str(&magnitude.to_string());
    s
}

fn format_fixed(digits: &str, decpt: i32) -> String {
    let len = digits.len() as i32;
    if decpt <= 0 {
        let mut s = String::from("0.");
        for _ in 0..(-decpt) {
            s.push('0');
        }
        s.push_str(digits);
        s
    } else if decpt >= len {
        let mut s = String::from(digits);
        for _ in 0..(decpt - len) {
            s.push('0');
        }
        s.push_str(".0");
        s
    } else {
        let split = decpt as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
