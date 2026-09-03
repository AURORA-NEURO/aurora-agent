//! Minimal JSON value model, parser and serializer (hand-rolled; builds are offline).
//!
//! Scope: exactly what the wire adapters need — envelopes, MCP frames, HTTP bodies, receipts.
//! Object key order is preserved as parsed/inserted, which keeps frame bytes stable across runs.
//! The parser has a depth limit because a hostile frame must fail with an error, not exhaust the
//! stack. Duplicate object keys are preserved in order; accessors return the first match, and
//! that choice is stated here rather than left implicit.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn obj(fields: Vec<(&str, Value)>) -> Value {
        Value::Obj(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    /// First match wins when keys repeat; see module docs for why this is a stated choice.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Uint(n) => Some(*n),
            Value::Int(n) if *n >= 0 => Some(*n as u64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Uint(n) if *n <= i64::MAX as u64 => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub msg: String,
    pub pos: usize,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "json error at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for JsonError {}

const MAX_DEPTH: usize = 64;

fn utf8_seq_len(lead: u8) -> usize {
    match lead {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Parses a complete JSON document. Trailing non-whitespace bytes are an error — a silently
/// truncated frame would otherwise be indistinguishable from a short one.
pub fn parse(input: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        b: input.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    let v = p.value(0)?;
    p.skip_ws();
    if p.i != p.b.len() {
        return Err(p.err("trailing characters after document"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> JsonError {
        JsonError {
            msg: msg.to_string(),
            pos: self.i,
        }
    }

    fn skip_ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected '{}'", c as char)))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting deeper than the documented limit"));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.err("expected a JSON value")),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, JsonError> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err(&format!("malformed literal, expected {word}")))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let val = self.value(depth + 1)?;
            fields.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Obj(fields));
                }
                _ => return Err(self.err("expected ',' or '}' in object")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Arr(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Arr(items));
                }
                _ => return Err(self.err("expected ',' or ']' in array")),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or_else(|| self.err("unterminated string"))?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&hi) {
                                // Surrogate pair: RFC 8259 requires the low half to follow.
                                if self.peek() != Some(b'\\') {
                                    return Err(self.err("lone high surrogate"));
                                }
                                self.i += 1;
                                if self.peek() != Some(b'u') {
                                    return Err(self.err("lone high surrogate"));
                                }
                                self.i += 1;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("invalid low surrogate"));
                                }
                                let cp = 0x10000 + ((hi - 0xD800) << 10).wrapping_add(lo - 0xDC00);
                                char::from_u32(cp)
                                    .ok_or_else(|| self.err("invalid surrogate pair"))?
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err("lone low surrogate"));
                            } else {
                                char::from_u32(hi).ok_or_else(|| self.err("invalid code point"))?
                            };
                            out.push(ch);
                        }
                        _ => return Err(self.err("unknown escape")),
                    }
                }
                c if c < 0x20 => return Err(self.err("control character in string")),
                c if c < 0x80 => out.push(c as char),
                c => {
                    // Multi-byte sequence: take the whole run of bytes and validate it as
                    // UTF-8 rather than pushing bytes individually (which would mojibake).
                    let len = utf8_seq_len(c);
                    let start = self.i - 1;
                    let end = start + len;
                    if end > self.b.len() {
                        return Err(self.err("truncated UTF-8 sequence"));
                    }
                    match std::str::from_utf8(&self.b[start..end]) {
                        Ok(s) => {
                            out.push_str(s);
                            self.i = end;
                        }
                        Err(_) => return Err(self.err("invalid UTF-8 in string")),
                    }
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.i + 4 > self.b.len() {
            return Err(self.err("truncated \\u escape"));
        }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4])
            .map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        // Integer part: no leading zeros unless the number IS zero (RFC 8259 grammar).
        match self.peek() {
            Some(b'0') => {
                self.i += 1;
                if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    return Err(self.err("leading zero in number"));
                }
            }
            Some(c) if c.is_ascii_digit() => {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            _ => return Err(self.err("expected digit")),
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            float = true;
            self.i += 1;
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err(self.err("digit required after '.'"));
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            float = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err(self.err("digit required in exponent"));
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).expect("ascii slice of &str");
        if !float {
            if text.starts_with('-') {
                if let Ok(n) = text.parse::<i64>() {
                    return Ok(Value::Int(n));
                }
            } else if let Ok(n) = text.parse::<u64>() {
                return Ok(Value::Uint(n));
            }
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.err("number out of range"))
    }
}

/// Serializes to compact JSON. Non-finite floats serialize as `null`, matching common JSON
/// encoders rather than emitting invalid JSON.
pub fn to_string(v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, v);
    s
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Uint(n) => out.push_str(&n.to_string()),
        Value::Float(f) => {
            if f.is_finite() {
                out.push_str(&format!("{f}"));
            } else {
                out.push_str("null");
            }
        }
        Value::Str(s) => write_escaped(out, s),
        Value::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Obj(fields) => {
            out.push('{');
            for (i, (k, val)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_escaped(out, k);
                out.push(':');
                write_value(out, val);
            }
            out.push('}');
        }
    }
}

fn write_escaped(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_of_a_wire_shaped_document_preserves_key_order() {
        let src = r#"{"b":1,"a":[true,null,-3.5],"s":"x\ny"}"#;
        let v = parse(src).expect("parses");
        assert_eq!(
            to_string(&v),
            "{\"b\":1,\"a\":[true,null,-3.5],\"s\":\"x\\ny\"}"
        );
    }

    #[test]
    fn multibyte_utf8_round_trips_through_raw_bytes() {
        let src = "{\"name\":\"héllo→世界\"}";
        let v = parse(src).expect("utf8 parses");
        assert_eq!(v.get("name").and_then(Value::as_str), Some("héllo→世界"));
        assert_eq!(to_string(&v), src);
    }

    #[test]
    fn surrogate_pairs_decode_to_the_combined_code_point() {
        let v = parse(r#""😀""#).expect("emoji");
        assert_eq!(v, Value::Str("😀".to_string()));
    }

    #[test]
    fn lone_surrogates_are_rejected_not_silently_replaced() {
        assert!(parse(r#""\ud83d""#).is_err());
        assert!(parse(r#""\udc00""#).is_err());
        assert!(parse(r#""\ud83dA""#).is_err());
    }

    #[test]
    fn nesting_beyond_the_limit_errors_instead_of_overflowing_the_stack() {
        let deep = format!("{}{}", "[".repeat(80), "]".repeat(80));
        let e = parse(&deep).expect_err("must reject");
        assert!(e.msg.contains("nesting"));
    }

    #[test]
    fn leading_zeros_trailing_garbage_and_bare_words_are_rejected() {
        assert!(parse("01").is_err());
        assert!(parse("1 2").is_err());
        assert!(parse("tru").is_err());
        assert!(parse("{\"a\":1,}").is_err());
        assert!(parse("[1,]").is_err());
    }

    #[test]
    fn integers_stay_integral_and_floats_parse() {
        assert_eq!(parse("42"), Ok(Value::Uint(42)));
        assert_eq!(parse("-42"), Ok(Value::Int(-42)));
        assert_eq!(parse("1e2"), Ok(Value::Float(100.0)));
        assert_eq!(
            parse("9007199254740993"),
            Ok(Value::Uint(9_007_199_254_740_993))
        );
    }

    #[test]
    fn non_finite_floats_serialize_as_null_rather_than_invalid_json() {
        assert_eq!(to_string(&Value::Float(f64::NAN)), "null");
    }

    #[test]
    fn control_characters_escape_so_frames_stay_one_line() {
        assert_eq!(to_string(&Value::str("\u{1}\u{1f}")), "\"\\u0001\\u001f\"");
    }

    #[test]
    fn duplicate_keys_are_preserved_and_first_wins_on_lookup() {
        let v = parse(r#"{"a":1,"a":2}"#).expect("duplicate keys allowed by this parser");
        assert_eq!(v.get("a").and_then(Value::as_u64), Some(1));
    }
}
