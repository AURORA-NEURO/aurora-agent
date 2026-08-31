//! Every position in a document, and the edits a mutator makes at one.
//!
//! A mutation applied only at the top level proves almost nothing: the receipts this workspace
//! emits nest attempt rows inside reports, artifacts inside step records, and retention blocks
//! inside bundles, and a verifier that re-hashes only the shallow keys would still pass such a
//! test. [`pointers`] enumerates *every* RFC 6901 JSON pointer in a document so a mutator can be
//! applied at each one in turn.
//!
//! Edits are expressed as a [`Patch`] — the pointer to the smallest subtree that differs, and the
//! value to put there — rather than as a whole mutated document. Every family reduces to that
//! shape, including the ones that add or remove a key, which patch the container instead of the
//! entry. The narrow form is what makes an exhaustive sweep affordable: a case costs the subtree
//! it changed rather than a copy of the document, [`swap_in`] applies one without copying anything
//! at all, and asking whether a case moved the canonical bytes becomes a question about the
//! subtree instead of about 42 kilobytes of dossier.
//!
//! When a document has more positions than a battery's budget, [`strided`] narrows the list by a
//! fixed step over the full traversal rather than by taking a prefix or by sampling: the reduced
//! set is reproducible, spread across the whole document, and reported with its step so the bound
//! is visible instead of implied.

use serde_json::{Map, Value};

/// Escapes one object key into a JSON pointer reference token.
pub fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Decodes one JSON pointer reference token back into an object key.
pub fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// Every JSON pointer in `document`, in document order, starting with the root pointer `""`.
pub fn pointers(document: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect(document, String::new(), &mut out);
    out
}

fn collect(value: &Value, prefix: String, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            out.push(prefix.clone());
            for (key, child) in map {
                collect(child, format!("{prefix}/{}", escape(key)), out);
            }
        }
        Value::Array(items) => {
            out.push(prefix.clone());
            for (index, child) in items.iter().enumerate() {
                collect(child, format!("{prefix}/{index}"), out);
            }
        }
        _ => out.push(prefix),
    }
}

/// The value at `pointer`, where `""` is the whole document.
pub fn get<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
    document.pointer(pointer)
}

/// The pointer to the parent container and the decoded final token, or `None` for the root.
pub fn split(pointer: &str) -> Option<(&str, String)> {
    let cut = pointer.rfind('/')?;
    Some((&pointer[..cut], unescape(&pointer[cut + 1..])))
}

/// One edit: the pointer to the subtree that differs, and what stands there instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub target: String,
    pub value: Value,
}

impl Patch {
    fn at(target: &str, value: Value) -> Self {
        Patch {
            target: target.to_string(),
            value,
        }
    }
}

/// A patch replacing the value at `pointer`, or `None` if nothing is there.
pub fn replace(document: &Value, pointer: &str, replacement: Value) -> Option<Patch> {
    get(document, pointer)?;
    Some(Patch::at(pointer, replacement))
}

/// A patch rewriting the container at `pointer`'s parent without the entry `pointer` names.
pub fn remove(document: &Value, pointer: &str) -> Option<Patch> {
    let (parent, token) = split(pointer)?;
    match get(document, parent)? {
        Value::Object(map) => {
            let mut rebuilt = map.clone();
            rebuilt.remove(&token)?;
            Some(Patch::at(parent, Value::Object(rebuilt)))
        }
        Value::Array(items) => {
            let index: usize = token.parse().ok()?;
            if index >= items.len() {
                return None;
            }
            let mut rebuilt = items.clone();
            rebuilt.remove(index);
            Some(Patch::at(parent, Value::Array(rebuilt)))
        }
        _ => None,
    }
}

/// A patch adding `key` to the object at `pointer`, or `None` if the key is already there or the
/// pointer does not name an object.
pub fn insert_key(document: &Value, pointer: &str, key: &str, value: Value) -> Option<Patch> {
    let map = get(document, pointer)?.as_object()?;
    if map.contains_key(key) {
        return None;
    }
    let mut rebuilt = map.clone();
    rebuilt.insert(key.to_string(), value);
    Some(Patch::at(pointer, Value::Object(rebuilt)))
}

/// A patch writing the object at `pointer` with the same entries in `order`.
pub fn reorder_keys(document: &Value, pointer: &str, order: &[String]) -> Option<Patch> {
    let existing = get(document, pointer)?.as_object()?;
    if order.len() != existing.len() {
        return None;
    }
    let mut rebuilt = Map::new();
    for key in order {
        rebuilt.insert(key.clone(), existing.get(key)?.clone());
    }
    Some(Patch::at(pointer, Value::Object(rebuilt)))
}

/// A copy of `document` with `patch` applied.
pub fn apply(document: &Value, patch: &Patch) -> Option<Value> {
    let mut copy = document.clone();
    let slot = copy.pointer_mut(&patch.target)?;
    *slot = patch.value.clone();
    Some(copy)
}

/// Exchanges `patch`'s value with the one standing at its target in `working`.
///
/// Nothing is copied: the subtree that was in the document ends up in the patch and vice versa, so
/// calling this a second time puts both back. That is how a battery feeds thousands of cases to a
/// verifier without ever cloning the document, and it is why the pair must always be called
/// together — a `swap_in` left unpaired leaves the working document mutated.
pub fn swap_in(working: &mut Value, patch: &mut Patch) -> bool {
    match working.pointer_mut(&patch.target) {
        Some(slot) => {
            std::mem::swap(slot, &mut patch.value);
            true
        }
        None => false,
    }
}

/// A copy of `document` with the value at `pointer` replaced.
pub fn with_replacement(document: &Value, pointer: &str, replacement: Value) -> Option<Value> {
    apply(document, &replace(document, pointer, replacement)?)
}

/// A copy of `document` with the object key or array element at `pointer` removed.
pub fn with_removal(document: &Value, pointer: &str) -> Option<Value> {
    apply(document, &remove(document, pointer)?)
}

/// A copy of `document` with `key` added to the object at `pointer`, or `None` if the key is
/// already there or the pointer does not name an object.
pub fn with_inserted_key(
    document: &Value,
    pointer: &str,
    key: &str,
    value: Value,
) -> Option<Value> {
    apply(document, &insert_key(document, pointer, key, value)?)
}

/// A copy of `document` in which the object at `pointer` carries the same entries in `order`.
pub fn with_key_order(document: &Value, pointer: &str, order: &[String]) -> Option<Value> {
    apply(document, &reorder_keys(document, pointer, order)?)
}

/// The subset of `positions` a battery will visit, plus the step it used.
///
/// Returns every position and a step of `1` when the budget covers the document. Otherwise it
/// returns every `step`-th position, which is a bound, not a sample: the same seed, the same
/// document, and the same cap always visit the same positions.
pub fn strided(positions: &[String], cap: usize) -> (Vec<String>, usize) {
    if cap == 0 || positions.len() <= cap {
        return (positions.to_vec(), 1);
    }
    let step = positions.len().div_ceil(cap);
    (positions.iter().step_by(step).cloned().collect(), step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document() -> Value {
        json!({
            "a": 1,
            "b": { "c": [10, { "d/e": "x" }], "~f": true },
            "g": []
        })
    }

    #[test]
    fn every_position_including_the_root_and_empty_containers_is_enumerated() {
        assert_eq!(
            pointers(&document()),
            vec![
                "",
                "/a",
                "/b",
                "/b/c",
                "/b/c/0",
                "/b/c/1",
                "/b/c/1/d~1e",
                "/b/~0f",
                "/g",
            ]
        );
    }

    #[test]
    fn an_escaped_pointer_round_trips_through_get() {
        let document = document();
        assert_eq!(get(&document, "/b/c/1/d~1e"), Some(&json!("x")));
        assert_eq!(get(&document, "/b/~0f"), Some(&json!(true)));
        assert_eq!(get(&document, ""), Some(&document));
    }

    #[test]
    fn replacement_removal_and_insertion_each_edit_exactly_one_position() {
        let document = document();
        let replaced = with_replacement(&document, "/b/c/0", json!(11)).expect("position exists");
        assert_eq!(replaced["b"]["c"][0], json!(11));
        assert_eq!(replaced["a"], json!(1));

        let removed = with_removal(&document, "/b/~0f").expect("key exists");
        assert!(removed["b"]
            .as_object()
            .expect("object")
            .get("~f")
            .is_none());
        assert_eq!(removed["b"]["c"], document["b"]["c"]);

        let dropped = with_removal(&document, "/b/c/0").expect("element exists");
        assert_eq!(dropped["b"]["c"].as_array().expect("array").len(), 1);

        let inserted =
            with_inserted_key(&document, "/b", "probe", json!(0)).expect("object accepts a key");
        assert_eq!(inserted["b"]["probe"], json!(0));
        assert!(with_inserted_key(&document, "/b", "c", json!(0)).is_none());
        assert!(with_inserted_key(&document, "/a", "probe", json!(0)).is_none());
    }

    #[test]
    fn a_patch_names_the_smallest_subtree_that_differs_rather_than_the_whole_document() {
        let document = document();
        assert_eq!(
            replace(&document, "/b/c/0", json!(11)).expect("position exists"),
            Patch {
                target: "/b/c/0".into(),
                value: json!(11)
            }
        );
        assert_eq!(
            remove(&document, "/b/~0f").expect("key exists"),
            Patch {
                target: "/b".into(),
                value: json!({ "c": [10, { "d/e": "x" }] })
            },
            "removing a key patches the container that held it"
        );
        assert_eq!(
            insert_key(&document, "/g", "probe", json!(0)),
            None,
            "an array is not an object and takes no key"
        );
        assert!(replace(&document, "/absent", json!(1)).is_none());
    }

    #[test]
    fn swapping_a_patch_in_and_out_again_restores_the_document_it_started_from() {
        let document = document();
        let mut working = document.clone();
        let mut patch = remove(&document, "/b/c/0").expect("element exists");
        let mutated = patch.value.clone();

        assert!(swap_in(&mut working, &mut patch));
        assert_eq!(working["b"]["c"].as_array().expect("array").len(), 1);
        assert_eq!(
            patch.value, document["b"]["c"],
            "the original moved into the patch"
        );

        assert!(swap_in(&mut working, &mut patch));
        assert_eq!(working, document);
        assert_eq!(
            patch.value, mutated,
            "the patch is reusable after the round trip"
        );
    }

    #[test]
    fn a_patch_at_the_root_replaces_and_restores_the_whole_document() {
        let document = document();
        let mut working = document.clone();
        let mut patch = replace(&document, "", json!({ "replaced": true })).expect("root exists");
        assert!(swap_in(&mut working, &mut patch));
        assert_eq!(working, json!({ "replaced": true }));
        assert!(swap_in(&mut working, &mut patch));
        assert_eq!(working, document);
    }

    #[test]
    fn reordering_keeps_every_entry_and_refuses_an_order_that_does_not_cover_the_object() {
        let document = document();
        let reordered =
            with_key_order(&document, "", &["g".into(), "b".into(), "a".into()]).expect("reorders");
        let keys: Vec<&str> = reordered
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, vec!["g", "b", "a"]);
        assert_eq!(reordered["b"], document["b"]);
        assert!(with_key_order(&document, "", &["g".into()]).is_none());
    }

    #[test]
    fn a_stride_bound_is_spread_over_the_document_and_never_a_prefix() {
        let positions: Vec<String> = (0..100).map(|index| format!("/{index}")).collect();
        let (all, step) = strided(&positions, 100);
        assert_eq!(step, 1);
        assert_eq!(all.len(), 100);

        let (bounded, step) = strided(&positions, 10);
        assert_eq!(step, 10);
        assert_eq!(bounded.len(), 10);
        assert_eq!(bounded.first().map(String::as_str), Some("/0"));
        assert_eq!(bounded.last().map(String::as_str), Some("/90"));
    }
}
