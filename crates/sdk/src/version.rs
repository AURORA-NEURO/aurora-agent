//! Wire schema versions and the handshake that agrees on one.
//!
//! Blueprint 43.35 puts versioning first in its runtime-contract table — "semantic schema versions
//! with negotiated compatibility" — and 23.49 lists ABI and payload schema versions as the first
//! thing a handshake settles. 40.16 lists `dependency conflict` among the failure classes.
//!
//! # The version string is the one the workspace already emits
//!
//! [`SchemaVersion`] parses exactly the shape `bioprism-section` publishes:
//! `fiber-decision-section/0.1`, `fiber-context-certificate/0.2-extended`. It is not a general
//! semver parser and deliberately does not become one — a version this crate cannot round-trip
//! through the section crate's own constants would be a version no plugin could actually speak.
//!
//! # Why the qualifier is part of identity
//!
//! `fiber-context-certificate/0.2-extended` is a different profile, not a later patch, so a plugin
//! that speaks the base profile must not be handed extended bytes. The qualifier therefore
//! participates in equality and in compatibility: two versions are comparable only within one
//! family *and* one qualifier.

use crate::error::NegotiationError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A wire schema version: a family name, a major and minor number, and an optional profile
/// qualifier.
///
/// Ordering is by family, then qualifier, then major, then minor. That puts every version of one
/// profile in a contiguous run, which is what [`negotiate`] needs in order to take a maximum
/// without first grouping.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaVersion {
    family: String,
    qualifier: Option<String>,
    major: u32,
    minor: u32,
}

impl SchemaVersion {
    /// Parses `family/major.minor` or `family/major.minor-qualifier`.
    pub fn parse(text: impl AsRef<str>) -> Result<Self, NegotiationError> {
        let text = text.as_ref();
        let malformed = || NegotiationError::MalformedVersion {
            text: text.to_string(),
        };

        let (family, rest) = text.split_once('/').ok_or_else(malformed)?;
        if family.is_empty() {
            return Err(malformed());
        }
        let (numbers, qualifier) = match rest.split_once('-') {
            Some((numbers, qualifier)) if !qualifier.is_empty() => {
                (numbers, Some(qualifier.to_string()))
            }
            Some(_) => return Err(malformed()),
            None => (rest, None),
        };
        let (major, minor) = numbers.split_once('.').ok_or_else(malformed)?;
        Ok(SchemaVersion {
            family: family.to_string(),
            qualifier,
            major: major.parse().map_err(|_| malformed())?,
            minor: minor.parse().map_err(|_| malformed())?,
        })
    }

    pub fn new(family: impl Into<String>, major: u32, minor: u32) -> Self {
        SchemaVersion {
            family: family.into(),
            qualifier: None,
            major,
            minor,
        }
    }

    pub fn qualified(mut self, qualifier: impl Into<String>) -> Self {
        self.qualifier = Some(qualifier.into());
        self
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn qualifier(&self) -> Option<&str> {
        self.qualifier.as_deref()
    }

    pub fn major(&self) -> u32 {
        self.major
    }

    pub fn minor(&self) -> u32 {
        self.minor
    }

    /// Whether two versions describe the same profile line, ignoring the minor number.
    ///
    /// This is *not* the negotiation rule. It answers "are these even the same conversation",
    /// which is the question the error message needs to be useful: a plugin offering
    /// `weave-act/1.0` against a host speaking `fiber-decision-section/0.1` has a different
    /// problem from one offering `fiber-decision-section/9.0`.
    pub fn same_line(&self, other: &SchemaVersion) -> bool {
        self.family == other.family
            && self.qualifier == other.qualifier
            && self.major == other.major
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}.{}", self.family, self.major, self.minor)?;
        match &self.qualifier {
            Some(qualifier) => write!(f, "-{qualifier}"),
            None => Ok(()),
        }
    }
}

impl From<SchemaVersion> for String {
    fn from(value: SchemaVersion) -> Self {
        value.to_string()
    }
}

impl TryFrom<String> for SchemaVersion {
    type Error = NegotiationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SchemaVersion::parse(value)
    }
}

/// The set of versions one side of the handshake can speak.
///
/// A set rather than a range because the workspace's own versions are not a range: the base and
/// extended certificate profiles are siblings, not neighbours, and a host may well support
/// `0.1` and `0.2-extended` while never having supported `0.2`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionSet(BTreeSet<SchemaVersion>);

impl VersionSet {
    pub fn new() -> Self {
        VersionSet(BTreeSet::new())
    }

    pub fn with(mut self, version: SchemaVersion) -> Self {
        self.0.insert(version);
        self
    }

    pub fn insert(&mut self, version: SchemaVersion) -> bool {
        self.0.insert(version)
    }

    pub fn contains(&self, version: &SchemaVersion) -> bool {
        self.0.contains(version)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SchemaVersion> {
        self.0.iter()
    }

    /// Renders the set the way the negotiation errors name it: sorted, comma separated.
    pub fn describe(&self) -> String {
        if self.0.is_empty() {
            return "none".to_string();
        }
        self.0
            .iter()
            .map(SchemaVersion::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl FromIterator<SchemaVersion> for VersionSet {
    fn from_iter<I: IntoIterator<Item = SchemaVersion>>(iter: I) -> Self {
        VersionSet(iter.into_iter().collect())
    }
}

/// What the two sides settled on, and what each side gave up.
///
/// `host_only` and `plugin_only` are carried even on success because 23.49 requires adapters to
/// report semantic loss, and a handshake that quietly drops the extended certificate profile is
/// exactly the loss that would otherwise be invisible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Negotiated {
    pub agreed: SchemaVersion,
    pub host_only: Vec<SchemaVersion>,
    pub plugin_only: Vec<SchemaVersion>,
}

impl Negotiated {
    /// Whether either side supports a profile the other does not.
    pub fn is_lossy(&self) -> bool {
        !self.host_only.is_empty() || !self.plugin_only.is_empty()
    }
}

/// Agrees the highest version both sides speak, or fails naming both sets.
///
/// There is no "try it and see" fallback. 43.35's invariant is that a schema mismatch "negotiates
/// or rejects explicitly", and 40.16 requires a typed failure per class; a handshake that
/// optimistically proceeded on an unrecognised version would turn a version error into a decoding
/// error somewhere downstream, where the version numbers are no longer in scope to report.
///
/// The maximum is taken over the intersection rather than over either side alone, so the result
/// does not depend on which side is asked first.
pub fn negotiate(host: &VersionSet, plugin: &VersionSet) -> Result<Negotiated, NegotiationError> {
    if host.is_empty() {
        return Err(NegotiationError::HostSpeaksNothing);
    }
    if plugin.is_empty() {
        return Err(NegotiationError::PluginSpeaksNothing {
            host_supported: host.describe(),
        });
    }

    let agreed = plugin
        .iter()
        .filter(|version| host.contains(version))
        .max()
        .cloned();

    match agreed {
        Some(agreed) => Ok(Negotiated {
            host_only: host
                .iter()
                .filter(|version| !plugin.contains(version))
                .cloned()
                .collect(),
            plugin_only: plugin
                .iter()
                .filter(|version| !host.contains(version))
                .cloned()
                .collect(),
            agreed,
        }),
        None => Err(NegotiationError::NoCommonVersion {
            host_supported: host.describe(),
            plugin_supported: plugin.describe(),
            near_miss: near_miss(host, plugin),
        }),
    }
}

/// A version pair on the same profile line that differs only in minor number.
///
/// Reported as remediation, not as a fallback: knowing that the host wants `0.2` and the plugin
/// offers `0.1` of the same profile is the difference between "upgrade the plugin" and "this
/// plugin is for another system".
fn near_miss(host: &VersionSet, plugin: &VersionSet) -> Option<String> {
    for offered in plugin.iter() {
        for wanted in host.iter() {
            if offered.same_line(wanted) {
                return Some(format!("host wants {wanted}, plugin offers {offered}"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> SchemaVersion {
        SchemaVersion::parse(text).expect("test version parses")
    }

    #[test]
    fn the_section_crates_own_schema_strings_parse() {
        let section = v(bioprism_section::SECTION_SCHEMA_VERSION);
        assert_eq!(section.family(), "fiber-decision-section");
        assert_eq!((section.major(), section.minor()), (0, 1));
        assert_eq!(section.qualifier(), None);

        let extended = v(bioprism_section::CERTIFICATE_SCHEMA_VERSION_EXTENDED);
        assert_eq!(extended.qualifier(), Some("extended"));
        assert_eq!(
            extended.to_string(),
            bioprism_section::CERTIFICATE_SCHEMA_VERSION_EXTENDED
        );
    }

    #[test]
    fn a_qualified_profile_is_not_the_same_version_as_the_unqualified_one() {
        assert_ne!(v("p/0.2"), v("p/0.2-extended"));
        assert!(!v("p/0.2").same_line(&v("p/0.2-extended")));
    }

    #[test]
    fn negotiation_takes_the_highest_common_version_not_the_highest_offered() {
        let host = VersionSet::new().with(v("p/0.1")).with(v("p/0.2"));
        let plugin = VersionSet::new().with(v("p/0.2")).with(v("p/0.9"));
        let agreed = negotiate(&host, &plugin).expect("0.2 is common");
        assert_eq!(agreed.agreed, v("p/0.2"));
        assert_eq!(agreed.plugin_only, vec![v("p/0.9")]);
        assert_eq!(agreed.host_only, vec![v("p/0.1")]);
    }

    #[test]
    fn negotiation_is_symmetric_in_the_version_it_picks() {
        let a = VersionSet::new().with(v("p/0.1")).with(v("p/0.3"));
        let b = VersionSet::new().with(v("p/0.3")).with(v("p/0.4"));
        assert_eq!(
            negotiate(&a, &b).expect("common").agreed,
            negotiate(&b, &a).expect("common").agreed
        );
    }

    #[test]
    fn an_unknown_version_is_refused_with_both_sides_named() {
        let host = VersionSet::new().with(v("fiber-decision-section/0.1"));
        let plugin = VersionSet::new().with(v("weave-act/1.0"));
        let error = negotiate(&host, &plugin).expect_err("nothing in common");
        let rendered = error.to_string();
        assert!(
            rendered.contains("fiber-decision-section/0.1"),
            "{rendered}"
        );
        assert!(rendered.contains("weave-act/1.0"), "{rendered}");
    }

    #[test]
    fn a_minor_version_gap_on_the_same_line_is_reported_as_a_near_miss_not_accepted() {
        let host = VersionSet::new().with(v("p/0.2"));
        let plugin = VersionSet::new().with(v("p/0.1"));
        match negotiate(&host, &plugin) {
            Err(NegotiationError::NoCommonVersion {
                near_miss: Some(detail),
                ..
            }) => assert!(detail.contains("host wants p/0.2"), "{detail}"),
            other => panic!("expected a near-miss refusal, got {other:?}"),
        }
    }

    #[test]
    fn malformed_version_strings_are_rejected_rather_than_defaulted() {
        for text in ["no-slash", "p/1", "p/x.y", "/0.1", "p/0.1-"] {
            assert!(
                SchemaVersion::parse(text).is_err(),
                "{text} should not parse"
            );
        }
    }

    #[test]
    fn a_version_round_trips_through_json() {
        let version = v("fiber-context-certificate/0.2-extended");
        let json = serde_json::to_string(&version).expect("serialises");
        assert_eq!(json, "\"fiber-context-certificate/0.2-extended\"");
        let back: SchemaVersion = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, version);
    }
}
