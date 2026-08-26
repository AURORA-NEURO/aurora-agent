//! Structure-aware document mutators, each with the effect it claims a verifier must have.
//!
//! Every generator returns a [`Mutation`] carrying an [`Expect`]. The two expectations are the
//! whole point of the battery: a *formatting-only* mutation must leave the verdict exactly as it
//! was, because canonicalisation is what makes a digest portable between implementations, and a
//! *semantic* mutation must be refused, because a digest that survives an edit is not a receipt. A
//! generator that could not decide which of the two it was producing would be testing nothing, so
//! no third expectation exists.
//!
//! A generator states only *that* a case must be refused, never *why*: which class of refusal is
//! correct depends on which field of which document the case landed on, and the generators do not
//! know which field seals the document they were handed. [`crate::refine`] adds that, turning a
//! bare `rejected` into `rejected (digest_mismatch)` wherever the correct answer is a single class.
//!
//! The generators are pure functions of the document and the seeded [`SplitMix64`] they are
//! handed. Where a family has more candidates than it emits — which sibling pair to swap, which
//! hex digit to substitute, how far to rotate an array — the choice comes from that generator,
//! never from iteration order alone, so the seed in a failure message reproduces the exact case.

use crate::rng::SplitMix64;
use crate::walk::{self, Patch};
use crate::{Expect, Refusal};
use bioprism_ids::{to_canonical_string, ContentHash};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;

/// One generated case: the edit to apply and the claim it makes about the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub mutator: &'static str,
    /// The position whose value the case changed. For a deletion this is the entry that went away,
    /// which is one level below the container the patch rewrites.
    pub pointer: String,
    pub description: String,
    pub expect: Expect,
    pub patch: Patch,
}

impl Mutation {
    /// The document a verifier sees for this case.
    ///
    /// This copies the document, which is what a caller wants for a single case and what a caller
    /// running thousands of them should avoid; [`crate::run_cases`] applies the patch in place
    /// instead.
    pub fn applied(&self, document: &Value) -> Value {
        walk::apply(document, &self.patch).expect("a patch names a position its document has")
    }
}

/// The mutator families, in the order [`generate`] runs them.
pub const MUTATORS: [&str; 14] = [
    "digest_byte_flip",
    "digest_length_change",
    "digest_case_change",
    "sibling_swap",
    "required_key_deletion",
    "array_element_deletion",
    "unexpected_key",
    "numeric_near_equal",
    "numeric_boundary",
    "object_key_reordering",
    "array_reordering",
    "unicode_confusable_string",
    "empty_or_null_substitution",
    "wire_duplicate_key",
];

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// The width of a workspace digest field, in hex characters.
pub const DIGEST_CHARS: usize = 64;

/// The first positive integer an `f64` cannot represent exactly.
pub const FIRST_INEXACT_INTEGER: u64 = 9_007_199_254_740_993;

fn push(
    out: &mut Vec<Mutation>,
    mutator: &'static str,
    pointer: &str,
    description: String,
    expect: Expect,
    patch: Option<Patch>,
) {
    if let Some(patch) = patch {
        out.push(Mutation {
            mutator,
            pointer: pointer.to_string(),
            description,
            expect,
            patch,
        });
    }
}

/// The claim every semantic family makes before [`crate::refine`] narrows it.
fn refused() -> Expect {
    Expect::Rejected(Refusal::Any)
}

/// Every position whose value is a 64-character lowercase hex digest.
///
/// Detection is by shape, through the same [`ContentHash::parse`] the verifiers use, so the list
/// is exactly the set of fields a reader would call a digest — no field-name allow-list that
/// could silently miss a digest a document gained later.
pub fn digest_pointers(document: &Value) -> Vec<String> {
    digest_pointers_among(document, &walk::pointers(document))
}

/// The digest fields among an already-enumerated pointer list.
///
/// A battery walks the document once and hands the result to every family; re-deriving the same
/// traversal inside each digest family was pure repeated work on a document of any size.
pub fn digest_pointers_among(document: &Value, positions: &[String]) -> Vec<String> {
    positions
        .iter()
        .filter(|pointer| {
            walk::get(document, pointer)
                .and_then(Value::as_str)
                .is_some_and(|text| ContentHash::parse(text.to_string()).is_ok())
        })
        .cloned()
        .collect()
}

fn digest_at(document: &Value, pointer: &str) -> Option<String> {
    walk::get(document, pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn alternative_hex(current: char, rng: &mut SplitMix64) -> char {
    let index = HEX
        .iter()
        .position(|candidate| *candidate == current)
        .expect("a digest position holds a lowercase hex character");
    HEX[(index + 1 + rng.below(HEX.len() - 1)) % HEX.len()]
}

/// One substituted hex character at **every** offset of **every** digest, exhaustively.
///
/// This family is never bounded. A digest that catches tampering at 63 of its 64 offsets is not a
/// digest, and a battery that sampled offsets could not tell the difference.
pub fn digest_byte_flips(
    document: &Value,
    digests: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in digests {
        let Some(digest) = digest_at(document, pointer) else {
            continue;
        };
        let characters: Vec<char> = digest.chars().collect();
        for offset in 0..characters.len() {
            let replacement = alternative_hex(characters[offset], rng);
            let mut mutated = characters.clone();
            mutated[offset] = replacement;
            push(
                &mut out,
                "digest_byte_flip",
                pointer,
                format!(
                    "offset {offset} of the digest at {pointer}: {} -> {replacement}",
                    characters[offset]
                ),
                refused(),
                walk::replace(
                    document,
                    pointer,
                    Value::String(mutated.into_iter().collect()),
                ),
            );
        }
    }
    out
}

/// Digests that are one character short, one long, or missing an end.
pub fn digest_length_changes(document: &Value, digests: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in digests {
        let Some(digest) = digest_at(document, pointer) else {
            continue;
        };
        let variants = [
            (
                "last character dropped",
                digest[..digest.len() - 1].to_string(),
            ),
            ("first character dropped", digest[1..].to_string()),
            ("one character appended", format!("{digest}0")),
            ("two characters appended", format!("{digest}00")),
            ("emptied", String::new()),
        ];
        for (label, replacement) in variants {
            push(
                &mut out,
                "digest_length_change",
                pointer,
                format!("the digest at {pointer} with its {label}"),
                refused(),
                walk::replace(document, pointer, Value::String(replacement)),
            );
        }
    }
    out
}

/// Digests whose hex is uppercased in whole or in part.
///
/// A digest differing only in case names the same bytes to a human and a different string to a
/// comparison, which is why the workspace pins lowercase in [`ContentHash::parse`] rather than
/// comparing case-insensitively.
pub fn digest_case_changes(document: &Value, digests: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in digests {
        let Some(digest) = digest_at(document, pointer) else {
            continue;
        };
        let mut variants = Vec::new();
        let upper = digest.to_ascii_uppercase();
        if upper != digest {
            variants.push(("fully uppercased", upper));
        }
        if let Some(offset) = digest.find(|c: char| c.is_ascii_alphabetic()) {
            let mut single = digest.clone();
            single.replace_range(
                offset..offset + 1,
                &digest[offset..offset + 1].to_ascii_uppercase(),
            );
            variants.push(("one character uppercased", single));
        }
        for (label, replacement) in variants {
            push(
                &mut out,
                "digest_case_change",
                pointer,
                format!("the digest at {pointer} {label}"),
                refused(),
                walk::replace(document, pointer, Value::String(replacement)),
            );
        }
    }
    out
}

fn type_tag(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn swappable_pair(values: &[&Value], rng: &mut SplitMix64) -> Option<(usize, usize)> {
    if values.len() < 2 {
        return None;
    }
    let start = rng.below(values.len());
    for step in 0..values.len() {
        let left = (start + step) % values.len();
        for offset in 1..values.len() {
            let right = (left + offset) % values.len();
            if right > left
                && type_tag(values[left]) == type_tag(values[right])
                && values[left] != values[right]
            {
                return Some((left, right));
            }
        }
    }
    None
}

/// Two same-typed siblings exchanged inside one container.
///
/// This is the mutation a field-by-field validator is most likely to miss: no key is added,
/// removed, or retyped, and every value in the container is one the producer really emitted. Only
/// the binding between name and value changed.
pub fn sibling_swaps(
    document: &Value,
    positions: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(container) = walk::get(document, pointer) else {
            continue;
        };
        match container {
            Value::Object(map) => {
                let keys: Vec<&String> = map.keys().collect();
                let values: Vec<&Value> = map.values().collect();
                let Some((left, right)) = swappable_pair(&values, rng) else {
                    continue;
                };
                let mut rebuilt = map.clone();
                rebuilt.insert(keys[left].clone(), values[right].clone());
                rebuilt.insert(keys[right].clone(), values[left].clone());
                push(
                    &mut out,
                    "sibling_swap",
                    pointer,
                    format!(
                        "the values of `{}` and `{}` exchanged at {pointer}",
                        keys[left], keys[right]
                    ),
                    refused(),
                    walk::replace(document, pointer, Value::Object(rebuilt)),
                );
            }
            Value::Array(items) => {
                let values: Vec<&Value> = items.iter().collect();
                let Some((left, right)) = swappable_pair(&values, rng) else {
                    continue;
                };
                let mut rebuilt = items.clone();
                rebuilt.swap(left, right);
                push(
                    &mut out,
                    "sibling_swap",
                    pointer,
                    format!("elements {left} and {right} exchanged at {pointer}"),
                    refused(),
                    walk::replace(document, pointer, Value::Array(rebuilt)),
                );
            }
            _ => {}
        }
    }
    out
}

/// Each key of each visited object removed in turn.
pub fn required_key_deletions(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Object(map)) = walk::get(document, pointer) else {
            continue;
        };
        for key in map.keys() {
            let child = format!("{pointer}/{}", walk::escape(key));
            push(
                &mut out,
                "required_key_deletion",
                &child,
                format!("`{key}` deleted from the object at {pointer}"),
                refused(),
                walk::remove(document, &child),
            );
        }
    }
    out
}

/// Each element of each visited array removed in turn.
pub fn array_element_deletions(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Array(items)) = walk::get(document, pointer) else {
            continue;
        };
        for index in 0..items.len() {
            let child = format!("{pointer}/{index}");
            push(
                &mut out,
                "array_element_deletion",
                &child,
                format!("element {index} deleted from the array at {pointer}"),
                refused(),
                walk::remove(document, &child),
            );
        }
    }
    out
}

/// A key the schema does not know, added at each visited object.
///
/// Two are added per object, one sorting before every existing key and one after, so the
/// canonical key ordering is exercised on both sides of the existing entries rather than only at
/// whichever end a naive probe would land.
pub fn unexpected_keys(
    document: &Value,
    positions: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Object(_)) = walk::get(document, pointer) else {
            continue;
        };
        let tag = rng.next_u64() & 0xffff;
        for (label, key) in [
            (
                "sorting before every existing key",
                format!("!probe{tag:04x}"),
            ),
            (
                "sorting after every existing key",
                format!("~probe{tag:04x}"),
            ),
        ] {
            push(
                &mut out,
                "unexpected_key",
                pointer,
                format!("an unexpected key `{key}` {label} at {pointer}"),
                refused(),
                walk::insert_key(
                    document,
                    pointer,
                    &key,
                    Value::String("receipts-audit probe".into()),
                ),
            );
        }
    }
    out
}

fn adjacent_float(value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let bits = value.to_bits();
    let adjacent = if value == 0.0 {
        1
    } else if value > 0.0 {
        bits.checked_add(1)?
    } else {
        bits.checked_sub(1)?
    };
    let adjacent = f64::from_bits(adjacent);
    adjacent.is_finite().then_some(adjacent)
}

/// Numbers replaced by the nearest thing that is not quite them.
///
/// An integer becomes the float of equal value, a float becomes its neighbour one unit in the last
/// place away, and zero picks up a sign. Each of these prints the same to a careless reader and
/// encodes differently, so each one has to land on a stable verdict — a verifier that accepted
/// `1` and `1.0` interchangeably would be a verifier whose digest depends on how a caller's JSON
/// parser happened to type a literal.
pub fn numeric_near_equal(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Number(number)) = walk::get(document, pointer) else {
            continue;
        };
        let mut variants: Vec<(String, Number)> = Vec::new();
        if let Some(integer) = number.as_i64() {
            if let Some(as_float) = Number::from_f64(integer as f64) {
                variants.push((
                    format!("the integer {integer} as the float {as_float}"),
                    as_float,
                ));
            }
        } else if let Some(unsigned) = number.as_u64() {
            if let Some(as_float) = Number::from_f64(unsigned as f64) {
                variants.push((
                    format!("the integer {unsigned} as the float {as_float}"),
                    as_float,
                ));
            }
        } else if let Some(float) = number.as_f64() {
            if let Some(neighbour) = adjacent_float(float).and_then(Number::from_f64) {
                variants.push((
                    format!("the float {float} moved one unit in the last place to {neighbour}"),
                    neighbour,
                ));
            }
        }
        if number.as_f64() == Some(0.0) {
            let signed = if number.to_string().starts_with('-') {
                Number::from_f64(0.0)
            } else {
                Number::from_f64(-0.0)
            };
            if let Some(signed) = signed {
                variants.push((
                    format!("the zero at {pointer} with its sign flipped"),
                    signed,
                ));
            }
        }
        for (description, replacement) in variants {
            push(
                &mut out,
                "numeric_near_equal",
                pointer,
                format!("{description} at {pointer}"),
                refused(),
                walk::replace(document, pointer, Value::Number(replacement)),
            );
        }
    }
    out
}

/// The substitutions this family makes at one numeric position.
///
/// Split out so the expectation for each one can be read next to the value that produces it, and
/// so a test can assert the list rather than infer it from a count.
fn numeric_boundary_variants(number: &Number) -> Vec<(String, Value)> {
    let mut variants: Vec<(String, Value)> = vec![
        (
            format!("i64::MAX ({})", i64::MAX),
            Value::Number(Number::from(i64::MAX)),
        ),
        (
            format!("i64::MIN ({})", i64::MIN),
            Value::Number(Number::from(i64::MIN)),
        ),
        ("zero".into(), Value::Number(Number::from(0))),
        ("minus one".into(), Value::Number(Number::from(-1))),
        (
            format!(
                "u64::MAX ({}), one step past every signed integer",
                u64::MAX
            ),
            Value::Number(Number::from(u64::MAX)),
        ),
        (
            format!(
                "{FIRST_INEXACT_INTEGER}, the first integer no f64 holds exactly — a verifier that \
                 rounds it through a float cannot tell it from its predecessor"
            ),
            Value::Number(Number::from(FIRST_INEXACT_INTEGER)),
        ),
    ];

    let is_integer = number.is_i64() || number.is_u64();
    if is_integer {
        // Above 2^52 an f64 has no fractional resolution left, so adding a half gives back an
        // integer and there is no float-where-an-integer-stood case to make at that magnitude.
        let shifted = number.as_f64().unwrap_or(0.0) + 0.5;
        if shifted.fract() != 0.0 {
            if let Some(fractional) = Number::from_f64(shifted) {
                variants.push((
                    format!("the non-integral float {fractional} where an integer stood"),
                    Value::Number(fractional),
                ));
            }
        }
    } else if let Some(float) = number.as_f64() {
        if let Some(truncated) = i64::try_from(float.trunc() as i128).ok().map(Number::from) {
            variants.push((
                format!("the integer {truncated} where a float stood"),
                Value::Number(truncated),
            ));
        }
    }

    for spelling in NON_FINITE_SPELLINGS {
        variants.push((
            format!("the string {spelling:?}, the only spelling of a non-finite number JSON has"),
            Value::String((*spelling).into()),
        ));
    }

    let mut seen = Vec::new();
    variants.retain(|(_, value)| {
        let written = to_canonical_string(value).ok();
        let fresh = !seen.contains(&written);
        seen.push(written);
        fresh
    });
    variants
}

/// The three non-finite values, as the strings a coercing parser would revive them from.
///
/// JSON has no spelling for a non-finite number and `serde_json::Number` refuses to hold one, so
/// the only NaN or infinity that can reach a verifier through this format is a string that says
/// so.
pub const NON_FINITE_SPELLINGS: &[&str] = &["NaN", "Infinity", "-Infinity"];

/// Numbers replaced by the values that break the arithmetic around them.
///
/// The near-equal family asks whether a verifier notices a change too small to see. This one asks
/// the opposite: whether it notices a change too large to be plausible. Every substitution here is
/// a value some consumer downstream would have to handle — a count at `i64::MAX`, a byte length of
/// `-1`, an identifier past the exact-integer range of the `f64` a JavaScript reader would parse it
/// into — and every one of them changes the canonical bytes, so every one has to be refused. A
/// verifier that accepted any of them would be a verifier whose digest does not cover the number it
/// is sitting on.
///
/// NaN and infinity appear only as strings, because the format permits them in no other form:
/// `serde_json::Number` cannot hold a non-finite value and `to_canonical_string` refuses one.
pub fn numeric_boundaries(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Number(number)) = walk::get(document, pointer) else {
            continue;
        };
        for (label, replacement) in numeric_boundary_variants(number) {
            push(
                &mut out,
                "numeric_boundary",
                pointer,
                format!("the number at {pointer} replaced by {label}"),
                refused(),
                walk::replace(document, pointer, replacement),
            );
        }
    }
    out
}

/// The same entries, written in a different order.
///
/// This is the only family that expects the verdict *not* to move. Canonicalisation sorts keys
/// before hashing, so two producers that emit the same receipt with different field order must
/// agree on its digest; if they did not, the digests in this workspace would be artefacts of one
/// serializer rather than names for content.
pub fn object_key_reorderings(
    document: &Value,
    positions: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Object(map)) = walk::get(document, pointer) else {
            continue;
        };
        if map.len() < 2 {
            continue;
        }
        let keys: Vec<String> = map.keys().cloned().collect();
        let mut orders: Vec<(String, Vec<String>)> = Vec::new();
        let mut reversed = keys.clone();
        reversed.reverse();
        orders.push(("reversed".into(), reversed));
        if map.len() > 2 {
            let shift = 1 + rng.below(map.len() - 1);
            let mut rotated = keys.clone();
            rotated.rotate_left(shift);
            orders.push((format!("rotated by {shift}"), rotated));
        }
        for (label, order) in orders {
            push(
                &mut out,
                "object_key_reordering",
                pointer,
                format!("the {} keys of the object at {pointer} {label}", map.len()),
                Expect::VerdictUnchanged,
                walk::reorder_keys(document, pointer, &order),
            );
        }
    }
    out
}

/// The same elements, written in a different order.
///
/// The mirror image of the key reordering family, and the reason both exist: JSON arrays are
/// ordered and JSON objects are not, so exactly one of these two mutations may change a digest.
pub fn array_reorderings(
    document: &Value,
    positions: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Array(items)) = walk::get(document, pointer) else {
            continue;
        };
        if items.len() < 2 {
            continue;
        }
        let mut orders: Vec<(String, Vec<Value>)> = Vec::new();
        let mut reversed = items.clone();
        reversed.reverse();
        orders.push(("reversed".into(), reversed));
        if items.len() > 2 {
            let shift = 1 + rng.below(items.len() - 1);
            let mut rotated = items.clone();
            rotated.rotate_left(shift);
            orders.push((format!("rotated by {shift}"), rotated));
        }
        for (label, order) in orders {
            if &order == items {
                continue;
            }
            push(
                &mut out,
                "array_reordering",
                pointer,
                format!(
                    "the {} elements of the array at {pointer} {label}",
                    items.len()
                ),
                refused(),
                walk::replace(document, pointer, Value::Array(order)),
            );
        }
    }
    out
}

fn confusable_variants(text: &str) -> Vec<(&'static str, String)> {
    let mut variants = Vec::new();
    if let Some(offset) = text.find('a') {
        let mut homoglyph = text.to_string();
        homoglyph.replace_range(offset..offset + 1, "\u{0430}");
        variants.push(("its first `a` replaced by Cyrillic U+0430", homoglyph));
    } else if let Some(offset) = text.find('o') {
        let mut homoglyph = text.to_string();
        homoglyph.replace_range(offset..offset + 1, "\u{043E}");
        variants.push(("its first `o` replaced by Cyrillic U+043E", homoglyph));
    }
    if let Some(offset) = text.find('e') {
        let mut precomposed = text.to_string();
        precomposed.replace_range(offset..offset + 1, "\u{00E9}");
        variants.push(("its first `e` replaced by precomposed U+00E9", precomposed));
        let mut decomposed = text.to_string();
        decomposed.replace_range(offset..offset + 1, "e\u{0301}");
        variants.push((
            "its first `e` replaced by decomposed U+0065 U+0301",
            decomposed,
        ));
    }
    variants.push(("a zero-width space appended", format!("{text}\u{200B}")));
    variants
}

/// Strings replaced by a form that reads the same and encodes differently.
///
/// Homoglyphs, a precomposed/decomposed accent pair, and an invisible character. The canonical
/// encoder applies no Unicode normalisation — deliberately, because the CPython reference it has
/// to agree with byte for byte applies none either — so every one of these is a different string
/// and has to be rejected. This family checks that no verifier has quietly introduced a
/// normalisation or trimming step that would erase the difference.
pub fn unicode_confusable_strings(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::String(text)) = walk::get(document, pointer) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        for (label, replacement) in confusable_variants(text) {
            push(
                &mut out,
                "unicode_confusable_string",
                pointer,
                format!("the string at {pointer} with {label}"),
                refused(),
                walk::replace(document, pointer, Value::String(replacement)),
            );
        }
    }
    out
}

/// Each visited value replaced by the empty string and by null.
pub fn empty_or_null_substitutions(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        if pointer.is_empty() {
            continue;
        }
        let Some(existing) = walk::get(document, pointer) else {
            continue;
        };
        for (label, replacement) in [
            ("the empty string", Value::String(String::new())),
            ("null", Value::Null),
        ] {
            if existing == &replacement {
                continue;
            }
            push(
                &mut out,
                "empty_or_null_substitution",
                pointer,
                format!("the value at {pointer} replaced by {label}"),
                refused(),
                walk::replace(document, pointer, replacement),
            );
        }
    }
    out
}

/// One object rewritten on the wire with a key that occurs twice.
///
/// JSON permits a duplicate key and says nothing about which occurrence wins; `serde_json` keeps
/// the last. The mutation writes the object out, appends a second copy of its first key with a
/// different value, and reads it back, so the case a verifier sees is the document a parser would
/// hand it after a producer emitted an ambiguous encoding.
///
/// The limit of this family is worth stating: a verifier is given a parsed `Value`, so it cannot
/// see that a key was ever duplicated. What is checked is that the resolved document — the one
/// with the injected value in place — is rejected, not that duplication itself is detectable.
pub fn wire_duplicate_keys(document: &Value, positions: &[String]) -> Vec<Mutation> {
    let mut out = Vec::new();
    for pointer in positions {
        let Some(Value::Object(map)) = walk::get(document, pointer) else {
            continue;
        };
        let Some((key, existing)) = map.iter().next() else {
            continue;
        };
        let injected = if existing == &Value::String("receipts-audit duplicate".into()) {
            Value::Number(Number::from(0))
        } else {
            Value::String("receipts-audit duplicate".into())
        };
        let Ok(text) = serde_json::to_string(&Value::Object(map.clone())) else {
            continue;
        };
        let Ok(injected_text) = serde_json::to_string(&injected) else {
            continue;
        };
        let Ok(key_text) = serde_json::to_string(key) else {
            continue;
        };
        let duplicated = format!("{},{key_text}:{injected_text}}}", &text[..text.len() - 1]);
        let Ok(reparsed) = serde_json::from_str::<Value>(&duplicated) else {
            continue;
        };
        push(
            &mut out,
            "wire_duplicate_key",
            pointer,
            format!("the object at {pointer} written with `{key}` twice, last occurrence winning"),
            refused(),
            walk::replace(document, pointer, reparsed),
        );
    }
    out
}

/// Every family, run in [`MUTATORS`] order.
///
/// `digests` is always the full exhaustive set of digest fields; `positions` is whatever the
/// caller's budget allows for the structural families. The cases come back unfiltered;
/// [`drop_degenerate`] is what removes the ones that cannot move a digest, and it is called by
/// [`crate::run_cases`] so that exactly one place decides which cases are executable.
pub fn generate(
    document: &Value,
    positions: &[String],
    digests: &[String],
    rng: &mut SplitMix64,
) -> Vec<Mutation> {
    let mut cases = Vec::new();
    cases.extend(digest_byte_flips(document, digests, rng));
    cases.extend(digest_length_changes(document, digests));
    cases.extend(digest_case_changes(document, digests));
    cases.extend(sibling_swaps(document, positions, rng));
    cases.extend(required_key_deletions(document, positions));
    cases.extend(array_element_deletions(document, positions));
    cases.extend(unexpected_keys(document, positions, rng));
    cases.extend(numeric_near_equal(document, positions));
    cases.extend(numeric_boundaries(document, positions));
    cases.extend(object_key_reorderings(document, positions, rng));
    cases.extend(array_reorderings(document, positions, rng));
    cases.extend(unicode_confusable_strings(document, positions));
    cases.extend(empty_or_null_substitutions(document, positions));
    cases.extend(wire_duplicate_keys(document, positions));
    cases
}

/// Removes the semantic cases that cannot move a digest, and returns how many went.
///
/// A digest cannot distinguish a document from itself, so claiming such a case is refused would be
/// claiming something untrue. The count comes back so the drop is reported rather than hidden.
pub fn drop_degenerate(document: &Value, cases: &mut Vec<Mutation>) -> usize {
    let before = cases.len();
    let mut original = SubtreeBytes::new(document);
    cases.retain(|case| case.expect == Expect::VerdictUnchanged || original.moved(case));
    before - cases.len()
}

/// Whether applying `case` would move `document`'s canonical bytes.
///
/// The comparison is between the patched subtree and the subtree it replaces, not between two
/// whole documents. The two answers are identical — canonical JSON writes a value the same way
/// wherever it sits, so the document's bytes move exactly when the replaced subtree's bytes move —
/// and the narrow one does not re-serialise the entire document once per case.
pub fn moves_canonical_bytes(document: &Value, case: &Mutation) -> bool {
    SubtreeBytes::new(document).moved(case)
}

/// The canonical bytes of each subtree a batch of patches targets, computed once per target.
struct SubtreeBytes<'a> {
    document: &'a Value,
    cache: HashMap<String, Option<String>>,
}

impl<'a> SubtreeBytes<'a> {
    fn new(document: &'a Value) -> Self {
        SubtreeBytes {
            document,
            cache: HashMap::new(),
        }
    }

    fn at(&mut self, target: &str) -> Option<String> {
        if let Some(cached) = self.cache.get(target) {
            return cached.clone();
        }
        let bytes =
            walk::get(self.document, target).and_then(|value| to_canonical_string(value).ok());
        self.cache.insert(target.to_string(), bytes.clone());
        bytes
    }

    fn moved(&mut self, case: &Mutation) -> bool {
        let before = self.at(&case.patch.target);
        to_canonical_string(&case.patch.value).ok() != before
    }
}

/// The object a mutator can rebuild in place, exposed so a caller can construct its own family.
pub fn rebuild_object(entries: Vec<(String, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key, value);
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sealed() -> Value {
        json!({
            "schema": "audit/test/0.1",
            "count": 2,
            "ratio": 0.5,
            "elapsed": 12.5,
            "zero": 0,
            "names": ["alpha", "beta"],
            "nested": { "left": 1, "right": 2, "note": "a message" },
            "digest": "a".repeat(64),
        })
    }

    fn all_positions(document: &Value) -> Vec<String> {
        walk::pointers(document)
    }

    /// One value from a seeded stream, drawn from the shapes the equivalence has to survive.
    ///
    /// Nulls and bools, the numeric spellings canonical JSON normalises, digest-shaped strings,
    /// arrays, and nested objects. `depth` bounds recursion by narrowing the draw, so a document
    /// terminates without a special case for the leaf level.
    fn arbitrary_value(rng: &mut SplitMix64, depth: usize) -> Value {
        match rng.below(if depth == 0 { 7 } else { 9 }) {
            0 => Value::Null,
            1 => Value::Bool(rng.below(2) == 1),
            2 => json!(rng.below(1_000) as i64),
            3 => json!(rng.below(1_000) as f64 / 8.0),
            4 => json!(0),
            5 => Value::String(HEX[rng.below(16)].to_string().repeat(DIGEST_CHARS)),
            6 => Value::String(format!("text-{}", rng.below(1_000))),
            7 => Value::Array(
                (0..rng.below(4))
                    .map(|_| arbitrary_value(rng, depth - 1))
                    .collect(),
            ),
            _ => arbitrary_document(rng, depth - 1),
        }
    }

    /// A pseudo-random object, with keys drawn so insertion order and sorted order often differ.
    fn arbitrary_document(rng: &mut SplitMix64, depth: usize) -> Value {
        let width = 1 + rng.below(4);
        let entries = (0..width)
            .map(|index| {
                let key = match rng.below(4) {
                    0 => format!("z{index}"),
                    1 => format!("a{index}"),
                    2 => format!("key_{}", rng.below(1_000)),
                    _ => format!("{}", rng.below(100)),
                };
                (key, arbitrary_value(rng, depth))
            })
            .collect();
        rebuild_object(entries)
    }

    #[test]
    fn a_digest_field_is_found_by_shape_rather_than_by_field_name() {
        let found = digest_pointers(&sealed());
        assert_eq!(found, vec!["/digest".to_string()]);

        let renamed = walk::with_replacement(&sealed(), "/schema", Value::String("b".repeat(64)))
            .expect("schema is replaceable");
        assert_eq!(digest_pointers(&renamed), vec!["/schema", "/digest"]);
    }

    #[test]
    fn the_digest_fields_found_among_a_walked_pointer_list_are_the_ones_a_fresh_walk_finds() {
        let document = sealed();
        assert_eq!(
            digest_pointers_among(&document, &all_positions(&document)),
            digest_pointers(&document)
        );
    }

    #[test]
    fn every_one_of_the_sixty_four_digest_offsets_gets_its_own_case() {
        let document = sealed();
        let mut rng = SplitMix64::new(1);
        let flips = digest_byte_flips(&document, &digest_pointers(&document), &mut rng);
        assert_eq!(flips.len(), DIGEST_CHARS);
        let mutated: Vec<String> = flips
            .iter()
            .map(|case| {
                case.applied(&document)["digest"]
                    .as_str()
                    .expect("string")
                    .into()
            })
            .collect();
        for (offset, digest) in mutated.iter().enumerate() {
            assert_eq!(digest.len(), DIGEST_CHARS);
            let differing: Vec<usize> = digest
                .chars()
                .zip("a".repeat(DIGEST_CHARS).chars())
                .enumerate()
                .filter(|(_, (left, right))| left != right)
                .map(|(index, _)| index)
                .collect();
            assert_eq!(
                differing,
                vec![offset],
                "case {offset} moved the wrong byte"
            );
        }
    }

    #[test]
    fn an_all_digit_digest_yields_no_degenerate_case_change() {
        let numeric = walk::with_replacement(&sealed(), "/digest", json!("1".repeat(64)))
            .expect("digest is replaceable");
        let cases = digest_case_changes(&numeric, &digest_pointers(&numeric));
        assert!(
            cases.is_empty(),
            "uppercasing digits changes nothing and must not be claimed as a mutation"
        );
    }

    fn key_sequence(value: &Value, pointer: &str) -> Vec<String> {
        walk::get(value, pointer)
            .and_then(Value::as_object)
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_key_reordering_leaves_the_canonical_bytes_identical() {
        let document = sealed();
        let mut rng = SplitMix64::new(2);
        let cases = object_key_reorderings(&document, &all_positions(&document), &mut rng);
        assert!(!cases.is_empty());
        let baseline = to_canonical_string(&document).expect("canonicalises");
        for case in cases {
            assert_eq!(case.expect, Expect::VerdictUnchanged);
            let applied = case.applied(&document);
            assert_ne!(
                key_sequence(&applied, &case.pointer),
                key_sequence(&document, &case.pointer),
                "{}: the reordering must move the keys",
                case.description
            );
            assert_eq!(
                to_canonical_string(&applied).expect("canonicalises"),
                baseline,
                "{}",
                case.description
            );
        }
    }

    /// `serde_json` keeps insertion order in the value and ignores it in `PartialEq`, so a
    /// verifier that compares two `Value`s directly cannot see a reordering at all. The battery
    /// therefore judges this family by the verifier's verdict and by canonical bytes, never by
    /// value equality, which would report a false pass.
    #[test]
    fn value_equality_does_not_see_a_key_reordering_but_the_key_sequence_does() {
        let document = sealed();
        let reordered = walk::with_key_order(
            &document,
            "/nested",
            &["note".into(), "right".into(), "left".into()],
        )
        .expect("reorders");
        assert_eq!(reordered, document);
        assert_ne!(
            key_sequence(&reordered, "/nested"),
            key_sequence(&document, "/nested")
        );
    }

    #[test]
    fn an_array_reordering_changes_the_canonical_bytes() {
        let document = sealed();
        let mut rng = SplitMix64::new(3);
        let cases = array_reorderings(&document, &all_positions(&document), &mut rng);
        assert!(!cases.is_empty());
        let baseline = to_canonical_string(&document).expect("canonicalises");
        for case in cases {
            assert_eq!(case.expect, refused());
            assert_ne!(
                to_canonical_string(&case.applied(&document)).expect("canonicalises"),
                baseline,
                "{}",
                case.description
            );
        }
    }

    #[test]
    fn an_integer_and_its_equal_float_encode_differently() {
        let document = sealed();
        let cases = numeric_near_equal(&document, &all_positions(&document));
        let counted: Vec<&Mutation> = cases
            .iter()
            .filter(|case| case.pointer == "/count")
            .collect();
        assert_eq!(counted.len(), 1);
        assert_eq!(
            to_canonical_string(&counted[0].patch.value).expect("canonicalises"),
            "2.0"
        );
        assert_eq!(
            to_canonical_string(&document["count"]).expect("canonicalises"),
            "2"
        );
    }

    #[test]
    fn a_signed_zero_is_a_distinct_encoding_from_an_unsigned_one() {
        let document = sealed();
        let cases = numeric_near_equal(&document, &all_positions(&document));
        let zeros: Vec<&Mutation> = cases
            .iter()
            .filter(|case| case.pointer == "/zero")
            .collect();
        assert_eq!(
            zeros.len(),
            2,
            "an integer zero yields both a float and a sign case"
        );
        assert!(zeros
            .iter()
            .any(|case| to_canonical_string(&case.patch.value).expect("canonicalises") == "-0.0"));
    }

    #[test]
    fn the_boundary_family_substitutes_every_value_a_count_is_not_allowed_to_be() {
        let document = sealed();
        let cases = numeric_boundaries(&document, &["/count".to_string()]);
        let written: Vec<String> = cases
            .iter()
            .map(|case| to_canonical_string(&case.patch.value).expect("canonicalises"))
            .collect();
        assert_eq!(
            written,
            vec![
                "9223372036854775807",
                "-9223372036854775808",
                "0",
                "-1",
                "18446744073709551615",
                "9007199254740993",
                "2.5",
                "\"NaN\"",
                "\"Infinity\"",
                "\"-Infinity\"",
            ]
        );
    }

    #[test]
    fn the_boundary_family_offers_an_integer_where_a_float_stood_and_a_float_where_an_integer_did()
    {
        let document = sealed();
        let at_float = numeric_boundaries(&document, &["/elapsed".to_string()]);
        assert!(
            at_float.iter().any(|case| case.patch.value == json!(12)
                && case
                    .description
                    .contains("the integer 12 where a float stood")),
            "a float position must be offered a bare integer"
        );
        let at_integer = numeric_boundaries(&document, &["/count".to_string()]);
        assert!(
            at_integer.iter().any(|case| case.patch.value == json!(2.5)),
            "an integer position must be offered a non-integral float"
        );
    }

    /// Past 2^52 an f64 carries no fractional part, so there is no float to substitute for an
    /// integer of that size and the family says nothing rather than labelling an integer a
    /// non-integral float.
    #[test]
    fn no_non_integral_float_is_offered_where_no_float_of_that_size_has_a_fraction() {
        let document = json!({ "small": 2, "enormous": 9007199254740992u64 });
        let at_small = numeric_boundaries(&document, &["/small".to_string()]);
        assert!(at_small
            .iter()
            .any(|case| case.description.contains("non-integral float 2.5")));

        let at_enormous = numeric_boundaries(&document, &["/enormous".to_string()]);
        assert!(
            !at_enormous
                .iter()
                .any(|case| case.description.contains("non-integral")),
            "9007199254740992.5 is not a value an f64 has"
        );
    }

    /// Two variants that would write the same bytes are one variant. Without the collapse a float
    /// position whose truncation is zero would carry `0` twice, and a battery that counted its own
    /// cases would be counting one of them for nothing.
    #[test]
    fn boundary_variants_that_write_the_same_bytes_collapse_to_one_case() {
        let document = json!({ "ratio": 0.5 });
        let cases = numeric_boundaries(&document, &["/ratio".to_string()]);
        let written: Vec<String> = cases
            .iter()
            .map(|case| to_canonical_string(&case.patch.value).expect("canonicalises"))
            .collect();
        let distinct: std::collections::BTreeSet<&String> = written.iter().collect();
        assert_eq!(distinct.len(), written.len(), "{written:?}");
        assert!(written.contains(&"0".to_string()));
    }

    /// The battery cannot put a NaN or an infinity into a document, because neither this format
    /// nor its parser has anywhere to put one. Asserting that here keeps the boundary family's
    /// string spellings honest: they are the closest the format gets, not a substitute that was
    /// chosen for convenience.
    #[test]
    fn json_has_no_number_for_nan_or_infinity_so_the_family_uses_the_strings_instead() {
        assert!(Number::from_f64(f64::NAN).is_none());
        assert!(Number::from_f64(f64::INFINITY).is_none());
        assert!(serde_json::from_str::<Value>("{\"x\":NaN}").is_err());
        assert!(serde_json::from_str::<Value>("{\"x\":Infinity}").is_err());
        assert!(to_canonical_string(&json!(1e308)).is_ok());

        let document = sealed();
        let spellings: Vec<String> = numeric_boundaries(&document, &["/count".to_string()])
            .iter()
            .filter_map(|case| case.patch.value.as_str().map(str::to_string))
            .collect();
        assert_eq!(spellings, NON_FINITE_SPELLINGS);
    }

    #[test]
    fn a_boundary_substitution_that_lands_on_the_value_already_there_is_dropped() {
        let document = json!({ "zero": 0, "digest": "a".repeat(64) });
        let positions = all_positions(&document);
        let digests = digest_pointers(&document);
        let mut rng = SplitMix64::new(6);
        let mut cases = generate(&document, &positions, &digests, &mut rng);
        assert!(cases
            .iter()
            .any(|case| case.mutator == "numeric_boundary" && case.patch.value == json!(0)));
        let dropped = drop_degenerate(&document, &mut cases);
        assert!(dropped > 0, "substituting 0 for 0 changes nothing");
        assert!(!cases
            .iter()
            .any(|case| case.mutator == "numeric_boundary" && case.patch.value == json!(0)));
    }

    #[test]
    fn the_precomposed_and_decomposed_forms_of_one_accent_hash_differently() {
        let document = sealed();
        let cases = unicode_confusable_strings(&document, &["/nested/note".to_string()]);
        let forms: Vec<String> = cases
            .iter()
            .map(|case| to_canonical_string(&case.patch.value).expect("canonicalises"))
            .collect();
        let distinct: std::collections::BTreeSet<&String> = forms.iter().collect();
        assert_eq!(
            distinct.len(),
            forms.len(),
            "each confusable form must encode to its own bytes: {forms:?}"
        );
    }

    #[test]
    fn a_duplicate_key_on_the_wire_resolves_to_the_last_occurrence() {
        let document = sealed();
        let cases = wire_duplicate_keys(&document, &["/nested".to_string()]);
        assert_eq!(cases.len(), 1);
        let applied = cases[0].applied(&document);
        assert_eq!(applied["nested"]["left"], json!("receipts-audit duplicate"));
        assert_eq!(applied["nested"]["right"], json!(2));
    }

    #[test]
    fn a_patch_names_the_container_a_deletion_rewrites_and_the_entry_it_removed() {
        let document = sealed();
        let cases = required_key_deletions(&document, &["/nested".to_string()]);
        let dropped = cases
            .iter()
            .find(|case| case.pointer == "/nested/left")
            .expect("the nested object has a `left`");
        assert_eq!(dropped.patch.target, "/nested");
        assert!(dropped.patch.value.get("left").is_none());
        assert!(dropped.applied(&document)["nested"].get("left").is_none());
        assert_eq!(dropped.applied(&document)["count"], json!(2));
    }

    #[test]
    fn dropping_the_degenerate_cases_leaves_only_edits_the_canonical_bytes_can_see() {
        let document = json!({ "only": "value", "empty": "", "flag": false, "zero": 0 });
        let positions = all_positions(&document);
        let digests = digest_pointers(&document);
        let mut rng = SplitMix64::new(4);
        let mut cases = generate(&document, &positions, &digests, &mut rng);
        assert!(drop_degenerate(&document, &mut cases) > 0);
        let baseline = to_canonical_string(&document).expect("canonicalises");
        for case in &cases {
            if case.expect != Expect::VerdictUnchanged {
                assert_ne!(
                    to_canonical_string(&case.applied(&document)).expect("canonicalises"),
                    baseline,
                    "{}",
                    case.description
                );
            }
        }
    }

    /// The narrow degeneracy test has to agree with the wide one it replaced, case for case, or
    /// the battery would be asserting on cases that cannot move a digest.
    #[test]
    fn judging_degeneracy_by_the_patched_subtree_agrees_with_judging_it_by_the_whole_document() {
        let mut documents = 0;
        let mut checked = 0;
        let mut disagreements = 0;
        for seed in 0..64u64 {
            let document = if seed == 0 {
                sealed()
            } else {
                arbitrary_document(&mut SplitMix64::new(seed ^ 0xD1B5_4A32_D192_ED03), 3)
            };
            let positions = all_positions(&document);
            let digests = digest_pointers(&document);
            let mut rng = SplitMix64::new(seed);
            let every_case = generate(&document, &positions, &digests, &mut rng);

            let baseline = to_canonical_string(&document).expect("canonicalises");
            let mut narrow = SubtreeBytes::new(&document);
            for case in &every_case {
                let wide = to_canonical_string(&case.applied(&document)).expect("canonicalises")
                    != baseline;
                if narrow.moved(case) != wide {
                    disagreements += 1;
                }
                assert_eq!(
                    narrow.moved(case),
                    wide,
                    "seed {seed}: {} — comparing the patched subtree said one thing and \
                     re-serialising the whole document said the other, so the optimisation the \
                     unbounded lane rests on is not an equivalence",
                    case.description
                );
                checked += 1;
            }
            documents += 1;
        }
        assert_eq!(disagreements, 0);
        assert_eq!(documents, 64);
        assert!(
            checked > 5_000,
            "the corpus produced only {checked} cases, which is an example set rather than a \
             population; the equivalence is only established by the cases actually compared"
        );
    }

    #[test]
    fn generation_is_a_pure_function_of_the_seed() {
        let document = sealed();
        let positions = all_positions(&document);
        let digests = digest_pointers(&document);
        let describe = |seed: u64| {
            let mut rng = SplitMix64::new(seed);
            generate(&document, &positions, &digests, &mut rng)
                .into_iter()
                .map(|case| (case.mutator, case.description, case.patch))
                .collect::<Vec<_>>()
        };
        assert_eq!(describe(9), describe(9));
        assert_ne!(describe(9), describe(10));
    }

    #[test]
    fn every_named_mutator_family_produces_at_least_one_case_on_a_representative_document() {
        let document = sealed();
        let positions = all_positions(&document);
        let digests = digest_pointers(&document);
        let mut rng = SplitMix64::new(5);
        let cases = generate(&document, &positions, &digests, &mut rng);
        for mutator in MUTATORS {
            assert!(
                cases.iter().any(|case| case.mutator == mutator),
                "{mutator} generated nothing"
            );
        }
    }

    #[test]
    fn rebuild_object_preserves_the_order_it_is_given() {
        let rebuilt = rebuild_object(vec![("z".into(), json!(1)), ("a".into(), json!(2))]);
        let keys: Vec<&str> = rebuilt
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, vec!["z", "a"]);
    }
}
