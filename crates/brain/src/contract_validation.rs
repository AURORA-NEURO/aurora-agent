//! Shared low-level contract invariants used by brain integration boundaries.
//!
//! Domain modules own their schemas, digests, and policy decisions. This module only centralizes
//! the byte/text and identity invariants that must be identical across adapters.

use std::collections::BTreeSet;

pub(crate) const MAX_TEXT_BYTES: usize = 512;

pub(crate) fn validate_text(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        ));
    }
    Ok(())
}

pub(crate) fn validate_unique(values: &[String], field: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(format!(
                "{field} contains duplicate or case-colliding values"
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), String> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!("{field} is not in canonical order"));
    }
    Ok(())
}

pub(crate) fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

pub(crate) fn validate_partition(
    candidate: &[String],
    qualified: &[String],
    blocked: &[String],
    unknown: &[String],
    field: &str,
) -> Result<(), String> {
    let candidate_keys = identity_keys(candidate);
    let qualified_keys = identity_keys(qualified);
    let blocked_keys = identity_keys(blocked);
    let unknown_keys = identity_keys(unknown);
    let classified_keys = qualified_keys
        .union(&blocked_keys)
        .chain(unknown_keys.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    if !qualified_keys.is_disjoint(&blocked_keys)
        || !qualified_keys.is_disjoint(&unknown_keys)
        || !blocked_keys.is_disjoint(&unknown_keys)
        || classified_keys != candidate_keys
    {
        return Err(format!("{field} states do not partition candidates"));
    }
    Ok(())
}

pub(crate) fn validate_partition_with_unknown_subset(
    candidate: &[String],
    qualified: &[String],
    blocked: &[String],
    unknown: &[String],
    field: &str,
) -> Result<(), String> {
    let candidate_keys = identity_keys(candidate);
    let qualified_keys = identity_keys(qualified);
    let blocked_keys = identity_keys(blocked);
    let unknown_keys = identity_keys(unknown);
    if !qualified_keys.is_disjoint(&blocked_keys)
        || !qualified_keys.is_disjoint(&unknown_keys)
        || !unknown_keys.is_subset(&blocked_keys)
        || qualified_keys
            .union(&blocked_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            != candidate_keys
    {
        return Err(format!("{field} states do not partition candidates"));
    }
    Ok(())
}
