//! Semantic schema versions, in the exact spelling the wire formats already use.
//!
//! Blueprint 25.22 lists `SemanticVersion` as a primary object and 43.35's runtime contract says
//! *"semantic schema versions with negotiated compatibility"*. Neither states a grammar. The
//! grammar here is read off the three constants this workspace already ships:
//! `fiber-context-certificate/0.1`, `fiber-context-certificate/0.2-extended` and
//! `fiber-decision-section/0.1`.
//!
//! # Why the patch position is optional
//!
//! The shipped constant is the string `"fiber-context-certificate/0.1"`, and that string is
//! *inside the hashed certificate body*. Re-rendering it as `0.1.0` would change the canonical
//! bytes and therefore the certificate digest — the one thing schema evolution is not allowed to
//! do by accident. So [`SchemaVersion`] round-trips the written form rather than normalising it,
//! and an absent patch is [`None`], not zero.
//!
//! # Why 0.x shifts the ladder
//!
//! Under semver's 0.x rule the major position is not yet load-bearing: `0.1 -> 0.2` is the
//! breaking bump and there is no third position left to be additive. [`observed_bump`] encodes
//! that shift, which is what makes the shipped `0.1 -> 0.2-extended` transition a legitimate
//! carrier for a digest-changing addition.

use crate::error::VersionError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// A `major.minor[.patch][-label]` version.
///
/// `label` is not a semver pre-release. `0.2-extended` names a *variant* of the 0.2 wire format
/// carrying a different field set, and treating it as "less than 0.2" the way semver orders
/// pre-releases would claim a lineage that does not exist. See [`SchemaVersion::same_variant`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: Option<u64>,
    pub label: Option<String>,
}

impl SchemaVersion {
    pub fn new(major: u64, minor: u64) -> Self {
        SchemaVersion {
            major,
            minor,
            patch: None,
            label: None,
        }
    }

    pub fn with_patch(major: u64, minor: u64, patch: u64) -> Self {
        SchemaVersion {
            major,
            minor,
            patch: Some(patch),
            label: None,
        }
    }

    pub fn labelled(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let malformed = |detail: &str| VersionError::Malformed {
            value: text.to_string(),
            detail: detail.to_string(),
        };

        let (core, label) = match text.split_once('-') {
            Some((core, label)) if !label.is_empty() => (core, Some(label.to_string())),
            Some(_) => return Err(malformed("trailing '-' with an empty label")),
            None => (text, None),
        };

        let mut parts = core.split('.');
        let major = parts
            .next()
            .ok_or_else(|| malformed("no major position"))?
            .parse::<u64>()
            .map_err(|_| malformed("major position is not a non-negative integer"))?;
        let minor = parts
            .next()
            .ok_or_else(|| malformed("no minor position"))?
            .parse::<u64>()
            .map_err(|_| malformed("minor position is not a non-negative integer"))?;
        let patch = match parts.next() {
            None => None,
            Some(text) => Some(
                text.parse::<u64>()
                    .map_err(|_| malformed("patch position is not a non-negative integer"))?,
            ),
        };
        if parts.next().is_some() {
            return Err(malformed("more than three numeric positions"));
        }

        Ok(SchemaVersion {
            major,
            minor,
            patch,
            label,
        })
    }

    /// The numeric release, with an unwritten patch read as zero.
    pub fn release(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch.unwrap_or(0))
    }

    /// Whether two versions sit on the same variant line and can be compared as successors.
    ///
    /// `0.1` and `0.2` do. `0.2` and `0.2-extended` do not: they are two field sets published at
    /// the same release, and neither supersedes the other.
    pub fn same_variant(&self, other: &SchemaVersion) -> bool {
        self.label == other.label
    }

    /// True while the major position is still zero and the ladder is shifted up one place.
    pub fn is_zero_major(&self) -> bool {
        self.major == 0
    }
}

/// Storage order. Consistent with [`Eq`], and deliberately *not* the compatibility order.
///
/// An unwritten patch sorts before an explicit `.0` so the order is total; the two denote the same
/// release, which is why [`observed_bump`] compares [`SchemaVersion::release`] instead. Use this
/// for map keys and stable output, never to decide whether one version supersedes another.
impl Ord for SchemaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.release()
            .cmp(&other.release())
            .then_with(|| self.label.cmp(&other.label))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for SchemaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(f, ".{patch}")?;
        }
        if let Some(label) = &self.label {
            write!(f, "-{label}")?;
        }
        Ok(())
    }
}

impl TryFrom<String> for SchemaVersion {
    type Error = VersionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SchemaVersion::parse(&value)
    }
}

impl From<SchemaVersion> for String {
    fn from(value: SchemaVersion) -> Self {
        value.to_string()
    }
}

/// A named wire format at a version: `fiber-context-certificate/0.1`.
///
/// The name is the compatibility namespace. 14.16 requires that "breaking changes use new
/// namespace/version", so two ids with different names are not a lineage and
/// [`SchemaId::successor_of`] refuses to compare them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaId {
    pub name: String,
    pub version: SchemaVersion,
}

impl SchemaId {
    pub fn new(name: impl Into<String>, version: SchemaVersion) -> Self {
        SchemaId {
            name: name.into(),
            version,
        }
    }

    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let (name, version) = text
            .rsplit_once('/')
            .ok_or_else(|| VersionError::MissingSeparator(text.to_string()))?;
        if name.is_empty() {
            return Err(VersionError::EmptyName(text.to_string()));
        }
        Ok(SchemaId {
            name: name.to_string(),
            version: SchemaVersion::parse(version)?,
        })
    }

    /// Whether `self` is a later release of the same named format as `earlier`.
    ///
    /// A differing variant label does not block succession — the shipped
    /// `fiber-context-certificate/0.1 -> 0.2-extended` is a real evolution step — but an equal
    /// release with a differing label does, because `0.2` and `0.2-extended` are siblings and
    /// neither supersedes the other.
    pub fn successor_of(&self, earlier: &SchemaId) -> bool {
        self.name == earlier.name && self.version.release() > earlier.version.release()
    }

    /// Two ids published at the same release under different variant labels.
    pub fn sibling_variant_of(&self, other: &SchemaId) -> bool {
        self.name == other.name
            && self.version.release() == other.version.release()
            && !self.version.same_variant(&other.version)
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.name, self.version)
    }
}

impl TryFrom<String> for SchemaId {
    type Error = VersionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SchemaId::parse(&value)
    }
}

impl From<SchemaId> for String {
    fn from(value: SchemaId) -> Self {
        value.to_string()
    }
}

/// How far a version moved, or how far it is required to move.
///
/// Ordered so that a gate is a comparison: `observed >= required`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionBump {
    /// The version did not move. Only a no-op change may ship under this.
    None,
    Patch,
    Minor,
    Major,
}

impl VersionBump {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionBump::None => "none",
            VersionBump::Patch => "patch",
            VersionBump::Minor => "minor",
            VersionBump::Major => "major",
        }
    }
}

impl fmt::Display for VersionBump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The bump actually observed between two versions of the same format.
///
/// Under a zero major the ladder shifts up one position: `0.1 -> 0.2` is the breaking bump and
/// `0.1 -> 0.1.1` is the additive one, matching how Cargo reads a `0.x` requirement. This is why
/// the shipped `fiber-context-certificate/0.1 -> 0.2-extended` transition is large enough to carry
/// a change that moves the digest.
///
/// Returns [`VersionBump::None`] when the release did not advance, including the case where only
/// the variant label differs.
pub fn observed_bump(from: &SchemaVersion, to: &SchemaVersion) -> VersionBump {
    let (from_major, from_minor, from_patch) = from.release();
    let (to_major, to_minor, to_patch) = to.release();

    if to_major > from_major {
        VersionBump::Major
    } else if to_major < from_major {
        VersionBump::None
    } else if to_minor > from_minor {
        if from.is_zero_major() {
            VersionBump::Major
        } else {
            VersionBump::Minor
        }
    } else if to_minor < from_minor {
        VersionBump::None
    } else if to_patch > from_patch {
        if from.is_zero_major() {
            VersionBump::Minor
        } else {
            VersionBump::Patch
        }
    } else {
        VersionBump::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_section::{CERTIFICATE_SCHEMA_VERSION, CERTIFICATE_SCHEMA_VERSION_EXTENDED};

    #[test]
    fn the_shipped_schema_version_strings_round_trip_byte_for_byte() {
        for shipped in [
            CERTIFICATE_SCHEMA_VERSION,
            CERTIFICATE_SCHEMA_VERSION_EXTENDED,
            bioprism_section::SECTION_SCHEMA_VERSION,
        ] {
            let parsed = SchemaId::parse(shipped).expect("shipped constant parses");
            assert_eq!(
                parsed.to_string(),
                shipped,
                "re-rendering a schema_version string that sits inside a hashed body must not \
                 change it"
            );
        }
    }

    #[test]
    fn an_unwritten_patch_is_none_rather_than_zero() {
        let written = SchemaVersion::parse("0.1").expect("parses");
        assert_eq!(written.patch, None);
        assert_eq!(written.to_string(), "0.1");
        assert_eq!(SchemaVersion::with_patch(0, 1, 0).to_string(), "0.1.0");
        assert_eq!(written.release(), SchemaVersion::with_patch(0, 1, 0).release());
        assert_ne!(written, SchemaVersion::with_patch(0, 1, 0));
    }

    #[test]
    fn a_variant_label_is_not_a_successor_of_its_base() {
        let base = SchemaVersion::parse("0.2").expect("parses");
        let variant = SchemaVersion::parse("0.2-extended").expect("parses");
        assert!(!base.same_variant(&variant));
        assert_eq!(observed_bump(&base, &variant), VersionBump::None);

        let base_id = SchemaId::parse("fiber-context-certificate/0.2").expect("parses");
        let variant_id = SchemaId::parse("fiber-context-certificate/0.2-extended").expect("parses");
        assert!(!variant_id.successor_of(&base_id));
    }

    #[test]
    fn a_zero_major_minor_bump_counts_as_breaking_headroom() {
        let v0_1 = SchemaVersion::parse("0.1").expect("parses");
        let v0_2 = SchemaVersion::parse("0.2").expect("parses");
        assert_eq!(observed_bump(&v0_1, &v0_2), VersionBump::Major);

        let v1_1 = SchemaVersion::parse("1.1").expect("parses");
        let v1_2 = SchemaVersion::parse("1.2").expect("parses");
        assert_eq!(observed_bump(&v1_1, &v1_2), VersionBump::Minor);
    }

    #[test]
    fn a_version_that_moves_backwards_reports_no_bump() {
        let newer = SchemaVersion::parse("2.0").expect("parses");
        let older = SchemaVersion::parse("1.9").expect("parses");
        assert_eq!(observed_bump(&newer, &older), VersionBump::None);
        assert_eq!(observed_bump(&newer, &newer), VersionBump::None);
    }

    #[test]
    fn a_schema_id_without_a_separator_is_a_typed_error() {
        assert!(matches!(
            SchemaId::parse("fiber-context-certificate"),
            Err(VersionError::MissingSeparator(_))
        ));
        assert!(matches!(
            SchemaId::parse("/0.1"),
            Err(VersionError::EmptyName(_))
        ));
        assert!(matches!(
            SchemaId::parse("thing/0.1.2.3"),
            Err(VersionError::Malformed { .. })
        ));
        assert!(matches!(
            SchemaId::parse("thing/1"),
            Err(VersionError::Malformed { .. })
        ));
    }

    #[test]
    fn storage_order_is_total_and_agrees_with_equality() {
        let mut versions = [
            SchemaVersion::parse("0.2-extended").expect("parses"),
            SchemaVersion::parse("0.1").expect("parses"),
            SchemaVersion::parse("0.1.0").expect("parses"),
            SchemaVersion::parse("0.2").expect("parses"),
        ];
        versions.sort();
        let rendered: Vec<String> = versions.iter().map(SchemaVersion::to_string).collect();
        assert_eq!(rendered, ["0.1", "0.1.0", "0.2", "0.2-extended"]);
    }
}
