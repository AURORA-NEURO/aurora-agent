//! The environment and toolchain facts a reproduction would need, all of them declared.
//!
//! Blueprint 34.14 lists "environment and hardware" among the things a result card must resolve to,
//! and 13.15 §Pinning requires container images by digest, language dependencies through lockfiles
//! and hashes, and no floating `latest`.
//!
//! # Declared, never measured
//!
//! This crate is a library of plain Rust types that reads no clock, no environment variable, no
//! `/proc`, no CPUID and no filesystem. Every fact here is a string the caller wrote down.
//! [`FactSource`] has exactly one variant, [`FactSource::DeclaredByCaller`], for the same reason
//! `bioprism-sdk`'s `Enforcement` has one: there must be no value anywhere in this crate that
//! records a fact as *measured*, so no reproduction verdict can quietly acquire more authority than
//! its inputs have.
//!
//! The one exception is [`ToolchainFacts::bundle_crate_version`], which comes from
//! `CARGO_PKG_VERSION` — a compile-time constant of this crate, not a probe of the host — and names
//! the code that built the bundle, which is the one fact this crate is in a position to know.
//!
//! # Why the fields are options
//!
//! An absent CPU model is not an empty CPU model. A reproduction that cannot compare a field is in a
//! different position from one that compared it and found it equal, and [`ToolchainFacts::compare`]
//! reports that difference rather than treating `None` as a match.
//!
//! # Deliberately not implemented
//!
//! No probing of any kind. No SBOM generation, no lockfile parsing, no container digest resolution,
//! no GPU or accelerator enumeration, no detection of whether the declared facts are true. A caller
//! who declares `os: "linux"` on Windows will get a bundle that says linux.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where a fact in this module came from. From the caller, always.
///
/// One variant, on purpose. See the module docs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    /// Written down by whoever built the bundle. Not observed, not corroborated.
    #[default]
    DeclaredByCaller,
}

/// The host a run happened on, as declared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentFacts {
    pub os: Option<String>,
    pub arch: Option<String>,
    pub cpu_model: Option<String>,
    pub accelerator: Option<String>,
    /// A container image pinned by digest, per 13.15 §Pinning. `None` means no container was
    /// declared, which is different from a container declared without a digest — the latter belongs
    /// in [`crate::provenance::RejectionReason::FloatingRevision`].
    pub container_image_digest: Option<String>,
    /// Free-form additional facts, sorted so the canonical bytes are stable.
    pub additional: BTreeMap<String, String>,
    pub source: FactSource,
}

impl EnvironmentFacts {
    /// An environment about which nothing was declared. Distinct from one declared to be empty only
    /// in that every field is `None`, which is the distinction [`Self::declared_field_count`] reads.
    pub fn undeclared() -> Self {
        EnvironmentFacts::default()
    }

    pub fn with_os(mut self, os: impl Into<String>) -> Self {
        self.os = Some(os.into());
        self
    }

    pub fn with_arch(mut self, arch: impl Into<String>) -> Self {
        self.arch = Some(arch.into());
        self
    }

    pub fn with_container_image_digest(mut self, digest: impl Into<String>) -> Self {
        self.container_image_digest = Some(digest.into());
        self
    }

    pub fn with_fact(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional.insert(key.into(), value.into());
        self
    }

    /// How many of the named fields were declared. Zero means a reproduction has nothing to compare.
    pub fn declared_field_count(&self) -> usize {
        [
            self.os.is_some(),
            self.arch.is_some(),
            self.cpu_model.is_some(),
            self.accelerator.is_some(),
            self.container_image_digest.is_some(),
        ]
        .iter()
        .filter(|declared| **declared)
        .count()
            + self.additional.len()
    }
}

/// The code that produced a bundle, as declared, plus the one version this crate knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainFacts {
    /// `rustc --version` output, as declared. `None` when the caller did not record it.
    pub rustc_version: Option<String>,
    /// Workspace crate versions, name to version. Sorted, so canonical bytes are stable.
    pub crate_versions: BTreeMap<String, String>,
    /// This crate's own version, from `CARGO_PKG_VERSION` at compile time.
    pub bundle_crate_version: String,
    pub source: FactSource,
}

impl ToolchainFacts {
    pub fn declared() -> Self {
        ToolchainFacts {
            rustc_version: None,
            crate_versions: BTreeMap::new(),
            bundle_crate_version: env!("CARGO_PKG_VERSION").to_string(),
            source: FactSource::DeclaredByCaller,
        }
    }

    pub fn with_rustc_version(mut self, version: impl Into<String>) -> Self {
        self.rustc_version = Some(version.into());
        self
    }

    pub fn with_crate_version(
        mut self,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.crate_versions.insert(name.into(), version.into());
        self
    }

    /// Compares two toolchains field by field, distinguishing "differs" from "not comparable".
    ///
    /// A field the bundle declares and the host does not — or the reverse — yields
    /// [`ToolchainDifference::NotComparable`], never a silent match and never a false divergence.
    pub fn compare(&self, other: &ToolchainFacts) -> Vec<ToolchainDifference> {
        let mut differences = Vec::new();
        compare_optional(
            "rustc_version",
            self.rustc_version.as_deref(),
            other.rustc_version.as_deref(),
            &mut differences,
        );
        if self.bundle_crate_version != other.bundle_crate_version {
            differences.push(ToolchainDifference::Differs {
                field: "bundle_crate_version".to_string(),
                bundle: self.bundle_crate_version.clone(),
                host: other.bundle_crate_version.clone(),
            });
        }
        let mut names: Vec<&String> = self.crate_versions.keys().collect();
        for name in other.crate_versions.keys() {
            if !self.crate_versions.contains_key(name) {
                names.push(name);
            }
        }
        names.sort_unstable();
        names.dedup();
        for name in names {
            compare_optional(
                &format!("crate:{name}"),
                self.crate_versions.get(name).map(String::as_str),
                other.crate_versions.get(name).map(String::as_str),
                &mut differences,
            );
        }
        differences
    }
}

impl Default for ToolchainFacts {
    fn default() -> Self {
        ToolchainFacts::declared()
    }
}

/// One field-level outcome of comparing two toolchains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolchainDifference {
    /// Both sides declared the field and they disagree.
    Differs {
        field: String,
        bundle: String,
        host: String,
    },
    /// One side declared the field and the other did not, so no comparison happened.
    NotComparable {
        field: String,
        declared_by_bundle: bool,
    },
}

impl ToolchainDifference {
    pub fn field(&self) -> &str {
        match self {
            ToolchainDifference::Differs { field, .. } => field,
            ToolchainDifference::NotComparable { field, .. } => field,
        }
    }

    /// True only for an actual disagreement, so a caller cannot count uncomparable fields as
    /// evidence that the toolchains matched or that they differed.
    pub fn is_disagreement(&self) -> bool {
        matches!(self, ToolchainDifference::Differs { .. })
    }
}

fn compare_optional(
    field: &str,
    bundle: Option<&str>,
    host: Option<&str>,
    out: &mut Vec<ToolchainDifference>,
) {
    match (bundle, host) {
        (Some(a), Some(b)) if a != b => out.push(ToolchainDifference::Differs {
            field: field.to_string(),
            bundle: a.to_string(),
            host: b.to_string(),
        }),
        (Some(_), Some(_)) | (None, None) => {}
        (declared, _) => out.push(ToolchainDifference::NotComparable {
            field: field.to_string(),
            declared_by_bundle: declared.is_some(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_value_can_record_an_environment_fact_as_measured() {
        let facts = EnvironmentFacts::undeclared().with_os("linux");
        assert_eq!(facts.source, FactSource::DeclaredByCaller);
        let json = serde_json::to_string(&facts).expect("serialises");
        assert!(json.contains("declared_by_caller"), "{json}");
    }

    #[test]
    fn an_undeclared_environment_reports_nothing_to_compare() {
        assert_eq!(EnvironmentFacts::undeclared().declared_field_count(), 0);
        assert_eq!(
            EnvironmentFacts::undeclared()
                .with_os("linux")
                .with_fact("locale", "C")
                .declared_field_count(),
            2
        );
    }

    #[test]
    fn a_field_declared_on_one_side_only_is_not_comparable_rather_than_equal() {
        let bundle = ToolchainFacts::declared().with_rustc_version("1.85.0");
        let host = ToolchainFacts::declared();
        let differences = bundle.compare(&host);
        assert_eq!(differences.len(), 1);
        assert_eq!(differences[0].field(), "rustc_version");
        assert!(!differences[0].is_disagreement());
        assert_eq!(
            differences[0],
            ToolchainDifference::NotComparable {
                field: "rustc_version".into(),
                declared_by_bundle: true
            }
        );
    }

    #[test]
    fn two_declared_toolchains_that_agree_produce_no_differences() {
        let a = ToolchainFacts::declared()
            .with_rustc_version("1.85.0")
            .with_crate_version("bioprism-section", "0.1.0");
        let b = a.clone();
        assert!(a.compare(&b).is_empty());
    }

    #[test]
    fn a_crate_present_in_only_one_toolchain_is_named_in_the_difference() {
        let bundle = ToolchainFacts::declared().with_crate_version("bioprism-fiber", "0.1.0");
        let host = ToolchainFacts::declared().with_crate_version("bioprism-section", "0.1.0");
        let differences = bundle.compare(&host);
        let fields: Vec<&str> = differences.iter().map(ToolchainDifference::field).collect();
        assert_eq!(
            fields,
            vec!["crate:bioprism-fiber", "crate:bioprism-section"]
        );
        assert!(differences.iter().all(|d| !d.is_disagreement()));
    }

    #[test]
    fn a_crate_version_disagreement_is_a_disagreement_and_names_both_sides() {
        let bundle = ToolchainFacts::declared().with_crate_version("bioprism-ids", "0.1.0");
        let host = ToolchainFacts::declared().with_crate_version("bioprism-ids", "0.2.0");
        let differences = bundle.compare(&host);
        assert_eq!(
            differences,
            vec![ToolchainDifference::Differs {
                field: "crate:bioprism-ids".into(),
                bundle: "0.1.0".into(),
                host: "0.2.0".into(),
            }]
        );
    }

    #[test]
    fn the_bundle_crate_version_comes_from_compile_time_not_from_the_caller() {
        assert_eq!(
            ToolchainFacts::declared().bundle_crate_version,
            env!("CARGO_PKG_VERSION")
        );
    }
}
