//! Names, versions and version requirements — the vocabulary a resolution is stated in.
//!
//! Blueprint 10.04 (Packaging, Publishing and Resolution) says "aliases resolve to exact
//! manifests" and 10.08 (Versioning, License and Provenance) lists eight version axes without
//! saying what a version *is* as a value, or what syntax a dependency is written in. Both are
//! defined here, in the open, so a reader can disagree with the definition rather than discover it.
//!
//! # Why a requirement is an interval and never a union
//!
//! [`VersionReq`] denotes a contiguous range of versions. There is deliberately no `1.x || 3.x`.
//! That restriction is not a simplification, it is the property the whole dependency resolver is
//! built on: a family of intervals on a line has empty intersection **only if some pair of them
//! already has empty intersection** (Helly's theorem in one dimension). So when a constraint set
//! cannot be satisfied, a two-requirement witness always exists, and [`crate::deps`] can name it
//! without searching. Admit a disjunctive requirement and that guarantee is gone: the resolver
//! would have to backtrack, and a backtracking resolver that runs out of options can only say
//! "no solution", never "these two collide".
//!
//! # Not implemented
//!
//! No pre-release or build metadata. 10.08's version axes include a "generator version" and an
//! "environment image", which are properties of a pack's provenance rather than orderings on a
//! line; they belong in the pack document, which is `bioprism-registry`'s subject, not here.

use bioprism_ids::IdError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// The namespace half of a pack name — the unit an authority is granted over.
///
/// A registry is authoritative for namespaces, not for individual packs, because the question
/// "may this registry answer for this name?" has to be decidable before the name is known to
/// exist. See [`crate::registry::Authority`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Namespace(String);

impl Namespace {
    pub const KIND: &'static str = "namespace";

    /// Accepts lowercase alphanumerics, `-` and `_`. Case is rejected rather than folded: two
    /// names that differ only in case would resolve to the same pack on one filesystem and to two
    /// on another, and a resolution that depends on the host is not a resolution.
    pub fn parse(value: impl Into<String>) -> Result<Self, NameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(NameError::Id(IdError::Empty {
                kind: Namespace::KIND,
            }));
        }
        if let Some(bad) = value.chars().find(|c| !is_name_char(*c)) {
            return Err(NameError::IllegalCharacter {
                kind: Namespace::KIND,
                value,
                character: bad,
            });
        }
        Ok(Namespace(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Namespace> for String {
    fn from(value: Namespace) -> Self {
        value.0
    }
}

impl TryFrom<String> for Namespace {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Namespace::parse(value)
    }
}

/// A pack's name: a namespace and a local name, written `namespace/name`.
///
/// The name is not the identity of a pack — the digest is, and `bioprism-registry` owns that. A
/// name is a *lookup key that a registry has promised to keep pointing at the same content*, which
/// is why it belongs to the distribution layer and the digest belongs to the artifact layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackName {
    namespace: Namespace,
    local: String,
}

impl PackName {
    /// Parses `namespace/local`.
    pub fn parse(value: impl Into<String>) -> Result<Self, NameError> {
        let value = value.into();
        let mut parts = value.splitn(3, '/');
        let namespace = parts.next().unwrap_or_default();
        let local = parts.next().ok_or_else(|| NameError::MissingNamespace {
            value: value.clone(),
        })?;
        if parts.next().is_some() {
            return Err(NameError::IllegalCharacter {
                kind: "pack name",
                value: value.clone(),
                character: '/',
            });
        }
        PackName::new(namespace, local)
    }

    pub fn new(namespace: impl Into<String>, local: impl Into<String>) -> Result<Self, NameError> {
        let namespace = Namespace::parse(namespace)?;
        let local = local.into();
        if local.is_empty() {
            return Err(NameError::Id(IdError::Empty { kind: "pack name" }));
        }
        if let Some(bad) = local.chars().find(|c| !is_name_char(*c)) {
            return Err(NameError::IllegalCharacter {
                kind: "pack name",
                value: local,
                character: bad,
            });
        }
        Ok(PackName { namespace, local })
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn local(&self) -> &str {
        &self.local
    }
}

impl fmt::Display for PackName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.local)
    }
}

impl From<PackName> for String {
    fn from(value: PackName) -> Self {
        value.to_string()
    }
}

impl TryFrom<String> for PackName {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        PackName::parse(value)
    }
}

/// A content version, ordered by `(major, minor, patch)`.
///
/// This is 10.08's "pack content version" only. The other seven axes it lists are not orderings
/// and are not represented as one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(try_from = "String", into = "String")]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub const ZERO: Version = Version {
        major: 0,
        minor: 0,
        patch: 0,
    };

    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Version {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(text: &str) -> Result<Self, NameError> {
        let mut parts = text.split('.');
        let mut field = |name: &'static str| -> Result<u64, NameError> {
            let raw = parts.next().ok_or_else(|| NameError::MalformedVersion {
                value: text.to_string(),
                detail: format!("missing {name}"),
            })?;
            raw.parse::<u64>().map_err(|_| NameError::MalformedVersion {
                value: text.to_string(),
                detail: format!("{name} is not a number: {raw}"),
            })
        };
        let major = field("major")?;
        let minor = field("minor")?;
        let patch = field("patch")?;
        if parts.next().is_some() {
            return Err(NameError::MalformedVersion {
                value: text.to_string(),
                detail: "expected exactly three components".to_string(),
            });
        }
        Ok(Version::new(major, minor, patch))
    }

    /// The smallest version strictly greater than this one under the `(major, minor, patch)`
    /// ordering. Used to turn an inclusive upper bound into the exclusive one the interval algebra
    /// works in, so that inclusivity is normalised away exactly once rather than threaded through
    /// every comparison.
    fn successor(self) -> Option<Version> {
        if self.patch < u64::MAX {
            return Some(Version::new(self.major, self.minor, self.patch + 1));
        }
        if self.minor < u64::MAX {
            return Some(Version::new(self.major, self.minor + 1, 0));
        }
        if self.major < u64::MAX {
            return Some(Version::new(self.major + 1, 0, 0));
        }
        None
    }

    fn next_major(self) -> Option<Version> {
        self.major.checked_add(1).map(|m| Version::new(m, 0, 0))
    }

    fn next_minor(self) -> Option<Version> {
        self.minor
            .checked_add(1)
            .map(|m| Version::new(self.major, m, 0))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl From<Version> for String {
    fn from(value: Version) -> Self {
        value.to_string()
    }
}

impl TryFrom<String> for Version {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Version::parse(&value)
    }
}

/// A dependency requirement, written by a pack author.
///
/// Every variant denotes a contiguous interval; see the module docs for why that matters.
/// The representation is adjacently tagged rather than internally tagged, because a [`Version`]
/// serialises as a string and an internally tagged newtype variant cannot hold one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "req", content = "spec", rename_all = "snake_case")]
pub enum VersionReq {
    /// Exactly this version. The only requirement that survives a yank of its target as a
    /// *pin* rather than as a preference — see [`crate::lifecycle`].
    Exact(Version),
    /// This version or any later one. Unbounded above, which is a statement that the author has
    /// no idea what a future major release will do; it is legal and frequently wrong.
    AtLeast(Version),
    /// At least this version, below the next major. The usual meaning of `^`.
    Compatible(Version),
    /// At least this version, below the next minor. The usual meaning of `~`.
    Approximately(Version),
    /// `[low, high)`. Inclusive lower, exclusive upper, because a half-open interval is the only
    /// convention under which adjacent ranges tile the line without overlap or gap.
    Range { low: Version, high: Version },
    /// Any published version.
    Any,
}

impl VersionReq {
    pub fn matches(&self, version: &Version) -> bool {
        self.bounds().contains(version)
    }

    /// The interval this requirement denotes.
    pub fn bounds(&self) -> Bounds {
        match self {
            VersionReq::Exact(v) => Bounds {
                low: Some(*v),
                high: v.successor(),
            },
            VersionReq::AtLeast(v) => Bounds {
                low: Some(*v),
                high: None,
            },
            VersionReq::Compatible(v) => Bounds {
                low: Some(*v),
                high: v.next_major(),
            },
            VersionReq::Approximately(v) => Bounds {
                low: Some(*v),
                high: v.next_minor(),
            },
            VersionReq::Range { low, high } => Bounds {
                low: Some(*low),
                high: Some(*high),
            },
            VersionReq::Any => Bounds {
                low: None,
                high: None,
            },
        }
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionReq::Exact(v) => write!(f, "={v}"),
            VersionReq::AtLeast(v) => write!(f, ">={v}"),
            VersionReq::Compatible(v) => write!(f, "^{v}"),
            VersionReq::Approximately(v) => write!(f, "~{v}"),
            VersionReq::Range { low, high } => write!(f, ">={low}, <{high}"),
            VersionReq::Any => f.write_str("*"),
        }
    }
}

/// A half-open interval `[low, high)`, with `None` for unbounded.
///
/// This is the normal form every [`VersionReq`] is reduced to before anything is decided about it.
/// Keeping the algebra in one type is what makes "which two requirements collide" answerable: the
/// answer is a property of two intervals, not of a search that gave up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub low: Option<Version>,
    pub high: Option<Version>,
}

impl Bounds {
    pub const UNBOUNDED: Bounds = Bounds {
        low: None,
        high: None,
    };

    pub fn contains(&self, version: &Version) -> bool {
        if let Some(low) = &self.low {
            if version < low {
                return false;
            }
        }
        if let Some(high) = &self.high {
            if version >= high {
                return false;
            }
        }
        true
    }

    /// True when no version at all lies in the interval.
    ///
    /// Note that this is emptiness of the *interval*, not absence of a published version in it.
    /// The two are different outcomes and [`crate::deps`] reports them differently, because "you
    /// asked for something contradictory" and "nobody has published that yet" call for different
    /// actions from whoever reads the error.
    pub fn is_empty(&self) -> bool {
        match (&self.low, &self.high) {
            (Some(low), Some(high)) => low >= high,
            _ => false,
        }
    }

    pub fn intersect(&self, other: &Bounds) -> Bounds {
        Bounds {
            low: max_opt(self.low, other.low),
            high: min_opt(self.high, other.high),
        }
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds::UNBOUNDED
    }
}

impl fmt::Display for Bounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.low, &self.high) {
            (None, None) => f.write_str("any version"),
            (Some(low), None) => write!(f, ">={low}"),
            (None, Some(high)) => write!(f, "<{high}"),
            (Some(low), Some(high)) => write!(f, ">={low} and <{high}"),
        }
    }
}

fn max_opt(a: Option<Version>, b: Option<Version>) -> Option<Version> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.cmp(&b) == Ordering::Greater { a } else { b }),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

fn min_opt(a: Option<Version>, b: Option<Version>) -> Option<Version> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.cmp(&b) == Ordering::Less { a } else { b }),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

/// Everything that can go wrong reading a name or a version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error(transparent)]
    Id(#[from] IdError),

    #[error("{kind} {value:?} contains {character:?}, which is not one of a-z, 0-9, '-' or '_'")]
    IllegalCharacter {
        kind: &'static str,
        value: String,
        character: char,
    },

    #[error("{value:?} has no namespace; pack names are written namespace/name")]
    MissingNamespace { value: String },

    #[error("{value:?} is not a version: {detail}")]
    MalformedVersion { value: String, detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_without_a_namespace_is_refused_rather_than_defaulted() {
        let error =
            PackName::parse("onco-tp53").expect_err("a bare name has no authority attached");
        assert!(matches!(error, NameError::MissingNamespace { .. }));
    }

    #[test]
    fn names_differing_only_in_case_are_refused_rather_than_folded() {
        assert!(PackName::parse("bioprism/OncoTp53").is_err());
        PackName::parse("bioprism/onco-tp53").expect("lowercase is the only spelling");
    }

    #[test]
    fn a_name_round_trips_through_its_string_form() {
        let name = PackName::parse("bioprism/onco-tp53").expect("parses");
        assert_eq!(name.to_string(), "bioprism/onco-tp53");
        assert_eq!(name.namespace().as_str(), "bioprism");
        assert_eq!(name.local(), "onco-tp53");
        assert_eq!(PackName::parse(name.to_string()).expect("re-parses"), name);
    }

    #[test]
    fn a_version_orders_by_major_then_minor_then_patch() {
        let mut versions = [
            Version::new(1, 0, 0),
            Version::new(0, 9, 9),
            Version::new(1, 0, 1),
            Version::new(1, 1, 0),
            Version::new(10, 0, 0),
        ];
        versions.sort();
        assert_eq!(
            versions.map(|v| v.to_string()).join(" "),
            "0.9.9 1.0.0 1.0.1 1.1.0 10.0.0"
        );
    }

    #[test]
    fn a_version_with_four_components_is_not_a_version() {
        let error = Version::parse("1.2.3.4").expect_err("three components, exactly");
        assert!(matches!(error, NameError::MalformedVersion { .. }));
    }

    #[test]
    fn caret_stops_below_the_next_major_and_tilde_below_the_next_minor() {
        let caret = VersionReq::Compatible(Version::new(1, 2, 0));
        assert!(caret.matches(&Version::new(1, 9, 9)));
        assert!(!caret.matches(&Version::new(2, 0, 0)));
        assert!(!caret.matches(&Version::new(1, 1, 9)));

        let tilde = VersionReq::Approximately(Version::new(1, 2, 0));
        assert!(tilde.matches(&Version::new(1, 2, 9)));
        assert!(!tilde.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn a_range_is_half_open_so_adjacent_ranges_tile_without_overlap() {
        let lower = VersionReq::Range {
            low: Version::new(1, 0, 0),
            high: Version::new(2, 0, 0),
        };
        let upper = VersionReq::Range {
            low: Version::new(2, 0, 0),
            high: Version::new(3, 0, 0),
        };
        let boundary = Version::new(2, 0, 0);
        assert!(!lower.matches(&boundary));
        assert!(upper.matches(&boundary));
    }

    #[test]
    fn intersecting_two_disjoint_intervals_yields_an_interval_that_reports_itself_empty() {
        let below = VersionReq::Compatible(Version::new(1, 0, 0)).bounds();
        let above = VersionReq::AtLeast(Version::new(2, 0, 0)).bounds();
        let both = below.intersect(&above);
        assert!(both.is_empty());
        assert!(!below.is_empty());
        assert!(!above.is_empty());
    }

    #[test]
    fn an_interval_that_no_published_version_lands_in_is_still_not_an_empty_interval() {
        let narrow = VersionReq::Range {
            low: Version::new(1, 5, 0),
            high: Version::new(1, 6, 0),
        }
        .bounds();
        assert!(!narrow.is_empty());
        assert!(narrow.contains(&Version::new(1, 5, 3)));
    }

    #[test]
    fn intersection_is_order_independent() {
        let a = VersionReq::AtLeast(Version::new(1, 2, 0)).bounds();
        let b = VersionReq::Compatible(Version::new(1, 0, 0)).bounds();
        assert_eq!(a.intersect(&b), b.intersect(&a));
    }

    #[test]
    fn every_requirement_shape_survives_a_json_round_trip() {
        for req in [
            VersionReq::Any,
            VersionReq::Exact(Version::new(1, 2, 3)),
            VersionReq::AtLeast(Version::new(1, 0, 0)),
            VersionReq::Compatible(Version::new(1, 0, 0)),
            VersionReq::Approximately(Version::new(1, 4, 0)),
            VersionReq::Range {
                low: Version::new(1, 0, 0),
                high: Version::new(2, 0, 0),
            },
        ] {
            let text = serde_json::to_string(&req).expect("serialises");
            let back: VersionReq = serde_json::from_str(&text).expect("deserialises");
            assert_eq!(back, req);
        }
    }

    #[test]
    fn an_exact_requirement_admits_exactly_one_version() {
        let exact = VersionReq::Exact(Version::new(2, 3, 4));
        assert!(exact.matches(&Version::new(2, 3, 4)));
        assert!(!exact.matches(&Version::new(2, 3, 5)));
        assert!(!exact.matches(&Version::new(2, 3, 3)));
    }
}
