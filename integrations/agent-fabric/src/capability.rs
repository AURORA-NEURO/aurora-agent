//! Capabilities: the routing keys tasks declare and agents advertise.
//!
//! A capability is a small normalized ASCII token (`a-z`, digits, `.`, `-`, `_`), capped in
//! length. Normalization is the point: routing must not depend on whether a caller wrote
//! `"Genomics.Align"` or `"genomics.align"`. Matching is exact on the normalized form — there is
//! no prefix or wildcard matching, because a wildcard hit that routes a task to an agent that
//! only *resembles* the requested capability is worse than an honest miss.

use std::fmt;

const MAX_LEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityError {
    Empty,
    TooLong,
    IllegalCharacter(char),
    NotAscii,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapabilityError::Empty => write!(f, "capability is empty"),
            CapabilityError::TooLong => write!(f, "capability exceeds {MAX_LEN} characters"),
            CapabilityError::IllegalCharacter(c) => {
                write!(f, "character {c:?} is not allowed in a capability")
            }
            CapabilityError::NotAscii => write!(f, "capability must be ASCII"),
        }
    }
}

impl std::error::Error for CapabilityError {}

/// A single normalized capability token.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Capability(String);

impl Capability {
    pub fn parse(raw: &str) -> Result<Capability, CapabilityError> {
        if raw.is_empty() {
            return Err(CapabilityError::Empty);
        }
        let lowered = raw.to_ascii_lowercase();
        let mut owned = String::with_capacity(lowered.len());
        for c in lowered.chars() {
            match c {
                'a'..='z' | '0'..='9' | '.' | '-' | '_' => owned.push(c),
                c if !c.is_ascii() => return Err(CapabilityError::NotAscii),
                c => return Err(CapabilityError::IllegalCharacter(c)),
            }
        }
        if owned.len() > MAX_LEN {
            return Err(CapabilityError::TooLong);
        }
        Ok(Capability(owned))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The set of capabilities a task requires (all of them) or an agent advertises. Stored sorted
/// and deduplicated so set equality is order-independent and routing decisions are stable.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct CapabilitySet(Vec<Capability>);

impl CapabilitySet {
    pub fn from_caps(caps: Vec<Capability>) -> CapabilitySet {
        let mut v = caps;
        v.sort();
        v.dedup();
        CapabilitySet(v)
    }

    pub fn one(cap: Capability) -> CapabilitySet {
        CapabilitySet(vec![cap])
    }

    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// True when this set contains every capability of `required` — the AND semantics used both
    /// for agent advertisement vs task requirement.
    pub fn covers(&self, required: &CapabilitySet) -> bool {
        required.0.iter().all(|r| self.0.binary_search(r).is_ok())
    }

    /// The primary routing key: first in sorted order. Shard affinity and round-robin cursors
    /// key off this so multi-capability tasks still have exactly one fairness lane.
    pub fn primary(&self) -> Option<&Capability> {
        self.0.first()
    }
}

impl std::fmt::Display for CapabilitySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|c| c.to_string()).collect();
        write!(f, "[{}]", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_is_case_and_spacing_insensitive_within_the_legal_alphabet() {
        assert_eq!(
            Capability::parse("Genomics.Align-2").map(|c| c.as_str().to_string()),
            Ok("genomics.align-2".to_string())
        );
    }

    #[test]
    fn illegal_capability_shapes_are_errors_not_silent_normalizations() {
        assert_eq!(Capability::parse(""), Err(CapabilityError::Empty));
        assert_eq!(
            Capability::parse("a b"),
            Err(CapabilityError::IllegalCharacter(' '))
        );
        assert_eq!(Capability::parse("héllo"), Err(CapabilityError::NotAscii));
        assert_eq!(
            Capability::parse(&"x".repeat(129)),
            Err(CapabilityError::TooLong)
        );
    }

    #[test]
    fn covers_is_set_inclusion_with_all_semantics() {
        let a = CapabilitySet::from_caps(vec![
            Capability::parse("compute").expect("ok"),
            Capability::parse("gpu").expect("ok"),
        ]);
        let need_both = CapabilitySet::from_caps(vec![
            Capability::parse("GPU").expect("ok"),
            Capability::parse("COMPUTE").expect("ok"),
        ]);
        let need_more = CapabilitySet::one(Capability::parse("tpu").expect("ok"));
        assert!(a.covers(&need_both), "normalization makes GPU == gpu");
        assert!(!a.covers(&need_more));
    }

    #[test]
    fn duplicate_capabilities_collapse_so_fairness_lanes_are_unique() {
        let s = CapabilitySet::from_caps(vec![
            Capability::parse("a").expect("ok"),
            Capability::parse("a").expect("ok"),
        ]);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn primary_is_stable_regardless_of_declaration_order() {
        let x = CapabilitySet::from_caps(vec![
            Capability::parse("b").expect("ok"),
            Capability::parse("a").expect("ok"),
        ]);
        let y = CapabilitySet::one(Capability::parse("A").expect("ok"));
        assert_eq!(x.primary(), y.primary());
    }
}
