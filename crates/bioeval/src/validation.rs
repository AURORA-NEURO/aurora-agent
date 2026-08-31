//! Shared low-level validation for the biological evaluation contracts.
//!
//! These checks are intentionally limited to invariants shared by frame labels, outcome states,
//! and evaluator identifiers. Domain decisions—mass normalisation, bridge applicability, and
//! reference dispersion—remain in their owning modules.

pub(crate) const MAX_TEXT_BYTES: usize = 256;

pub(crate) fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}
