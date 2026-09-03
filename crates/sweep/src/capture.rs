//! Environment and artifact capture — and the ledger of what was left out.
//!
//! Implements blueprint 04.04 (Environment and Artifact Capture). Its fourth responsibility is one
//! line long and is the whole reason this module is code rather than a design note: **"Declare
//! capture omissions."**
//!
//! # The undeclared dimension
//!
//! 04.04's first responsibility enumerates what a capture must inventory: "files, processes,
//! packages, environment variables, services, network dependencies, clocks, random seeds, and
//! hardware". Nine dimensions. A capture that includes seven of them and mentions nothing about
//! the other two is indistinguishable, at the record level, from a capture that considered all
//! nine and found two irrelevant — and the two produce different reruns.
//!
//! So [`Capture::validate`] partitions the nine into *included*, *declared omitted*, and
//! *undeclared*, and errors on the third with the dimensions named. There is no way to build a
//! valid capture that is silent about a dimension. The [`OmissionLedger`] is what makes the middle
//! category expressible at all.
//!
//! # Reproducibility is lowered, not asserted
//!
//! 04.04: "Unpinned dependencies reduce reproducibility level." That is [`crate::fidelity`]'s meet,
//! applied in [`Capture::reproducibility`]: each unpinned dependency contributes a `Degraded`
//! declaration naming itself, and the capture's headline level is the worst of them. A capture with
//! nine pinned dependencies and one unpinned one reports `Degraded`, because a rerun of it is not
//! nine-tenths reproducible; it is not reproducible.
//!
//! # Artifacts have nine required fields and no defaults
//!
//! 04.04: "Every artifact has media type, logical role, source event, digest, size, sensitivity,
//! license, retention, and encryption metadata." [`ArtifactBuilder::build`] returns
//! [`SweepError::MissingField`] listing every one that was not set, rather than filling in a
//! plausible value. Sensitivity and licence in particular have no safe default: `Public` and
//! `Unrestricted` are claims, and a builder that supplies them for you is making the claim on the
//! caller's behalf.
//!
//! # What is not implemented
//!
//! No capturing. Nothing here walks a filesystem, reads a process tree, or hashes a file — the
//! digests and sizes arrive from the caller. 04.04's capture profiles are an enumeration of what a
//! caller *declared it did*, and this module checks the declaration against itself. Process memory
//! capture, checkpoint handles and image pulls are named in the profile enum and implemented
//! nowhere.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};
use crate::fidelity::{meet_all, Declaration, Level};

/// What a capture attempted. 04.04's six profiles, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureProfile {
    MetadataOnly,
    ArtifactsAndManifest,
    EnvironmentImage,
    StateDelta,
    ProcessCheckpoint,
    ExternalResponseTape,
}

/// The nine things an environment capture must account for.
///
/// From 04.04's first responsibility, in its order. The set is closed on purpose: adding a tenth
/// dimension must be a deliberate edit that invalidates existing captures' completeness, which is
/// the correct behaviour — a capture audited against nine dimensions has not been audited against
/// ten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Files,
    Processes,
    Packages,
    EnvironmentVariables,
    Services,
    NetworkDependencies,
    Clocks,
    RandomSeeds,
    Hardware,
}

impl Dimension {
    pub const ALL: [Dimension; 9] = [
        Dimension::Files,
        Dimension::Processes,
        Dimension::Packages,
        Dimension::EnvironmentVariables,
        Dimension::Services,
        Dimension::NetworkDependencies,
        Dimension::Clocks,
        Dimension::RandomSeeds,
        Dimension::Hardware,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Files => "files",
            Dimension::Processes => "processes",
            Dimension::Packages => "packages",
            Dimension::EnvironmentVariables => "environment_variables",
            Dimension::Services => "services",
            Dimension::NetworkDependencies => "network_dependencies",
            Dimension::Clocks => "clocks",
            Dimension::RandomSeeds => "random_seeds",
            Dimension::Hardware => "hardware",
        }
    }
}

/// The record of what a capture deliberately left out, and why.
///
/// The reason is required. 04.04 excludes "secrets and caches" from the filesystem capture; that
/// exclusion is correct and must still be legible to whoever later fails to reproduce the run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmissionLedger {
    entries: BTreeMap<String, String>,
}

impl OmissionLedger {
    pub fn new() -> Self {
        OmissionLedger::default()
    }

    /// Declare a whole dimension omitted.
    pub fn omit_dimension(
        &mut self,
        dimension: Dimension,
        reason: impl Into<String>,
    ) -> Result<(), SweepError> {
        self.declare(dimension.as_str(), reason)
    }

    /// Declare a narrower omission: an excluded path, an unread field, a skipped service.
    pub fn declare(
        &mut self,
        subject: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), SweepError> {
        let (subject, reason) = (subject.into(), reason.into());
        require_nonempty(&subject, "OmissionLedger", "subject")?;
        require_nonempty(&reason, "OmissionLedger", "reason")?;
        self.entries.insert(subject, reason);
        Ok(())
    }

    pub fn reason(&self, subject: &str) -> Option<&str> {
        self.entries.get(subject).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn subjects(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// How sensitive an artifact's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Internal,
    Restricted,
    /// Credentials and keys. 04.04 requires these to be "excluded or separately encrypted", which
    /// [`ArtifactBuilder::build`] enforces.
    Secret,
}

/// How an artifact is protected at rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum Encryption {
    /// Stored as-is.
    None,
    /// Encrypted to the named key. This crate performs no cryptography; the field records a claim.
    ToKey { key_id: String },
}

/// One captured artifact and its nine required fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub media_type: String,
    pub logical_role: String,
    pub source_event: String,
    pub digest: ContentHash,
    pub size_bytes: u64,
    pub sensitivity: Sensitivity,
    pub license: String,
    pub retention: String,
    pub encryption: Encryption,
}

/// Builds an [`Artifact`], refusing to guess.
#[derive(Debug, Clone, Default)]
pub struct ArtifactBuilder {
    media_type: Option<String>,
    logical_role: Option<String>,
    source_event: Option<String>,
    digest: Option<ContentHash>,
    size_bytes: Option<u64>,
    sensitivity: Option<Sensitivity>,
    license: Option<String>,
    retention: Option<String>,
    encryption: Option<Encryption>,
}

impl ArtifactBuilder {
    pub fn new() -> Self {
        ArtifactBuilder::default()
    }

    pub fn media_type(mut self, value: impl Into<String>) -> Self {
        self.media_type = Some(value.into());
        self
    }

    pub fn logical_role(mut self, value: impl Into<String>) -> Self {
        self.logical_role = Some(value.into());
        self
    }

    pub fn source_event(mut self, value: impl Into<String>) -> Self {
        self.source_event = Some(value.into());
        self
    }

    pub fn digest(mut self, value: ContentHash) -> Self {
        self.digest = Some(value);
        self
    }

    pub fn size_bytes(mut self, value: u64) -> Self {
        self.size_bytes = Some(value);
        self
    }

    pub fn sensitivity(mut self, value: Sensitivity) -> Self {
        self.sensitivity = Some(value);
        self
    }

    pub fn license(mut self, value: impl Into<String>) -> Self {
        self.license = Some(value.into());
        self
    }

    pub fn retention(mut self, value: impl Into<String>) -> Self {
        self.retention = Some(value.into());
        self
    }

    pub fn encryption(mut self, value: Encryption) -> Self {
        self.encryption = Some(value);
        self
    }

    /// Build, or name every field that was not set.
    ///
    /// All missing fields are reported at once rather than one per attempt, because a caller
    /// filling in an artifact record needs the list, not a guessing game.
    pub fn build(self) -> Result<Artifact, SweepError> {
        let mut missing = Vec::new();
        macro_rules! need {
            ($field:ident) => {
                if self.$field.is_none() {
                    missing.push(stringify!($field).to_string());
                }
            };
        }
        need!(media_type);
        need!(logical_role);
        need!(source_event);
        need!(digest);
        need!(size_bytes);
        need!(sensitivity);
        need!(license);
        need!(retention);
        need!(encryption);
        if !missing.is_empty() {
            return Err(SweepError::MissingField {
                what: "Artifact",
                fields: missing,
            });
        }
        let artifact = match (
            self.media_type,
            self.logical_role,
            self.source_event,
            self.digest,
            self.size_bytes,
            self.sensitivity,
            self.license,
            self.retention,
            self.encryption,
        ) {
            (
                Some(media_type),
                Some(logical_role),
                Some(source_event),
                Some(digest),
                Some(size_bytes),
                Some(sensitivity),
                Some(license),
                Some(retention),
                Some(encryption),
            ) => Artifact {
                media_type,
                logical_role,
                source_event,
                digest,
                size_bytes,
                sensitivity,
                license,
                retention,
                encryption,
            },
            _ => {
                return Err(SweepError::MissingField {
                    what: "Artifact",
                    fields: missing,
                })
            }
        };
        if artifact.sensitivity == Sensitivity::Secret && artifact.encryption == Encryption::None {
            return Err(SweepError::malformed(
                "Artifact",
                "a secret artifact must be excluded or separately encrypted (04.04)",
            ));
        }
        Ok(artifact)
    }
}

/// A declared dependency and whether it is pinned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "pin")]
pub enum Pin {
    Pinned {
        version: String,
    },
    /// 04.04: "Unpinned dependencies reduce reproducibility level."
    Unpinned,
}

/// One entry in 04.04's dependency list: lockfiles, OS packages, runtime versions, model providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub pin: Pin,
}

impl Dependency {
    pub fn pinned(name: impl Into<String>, version: impl Into<String>) -> Self {
        Dependency {
            name: name.into(),
            pin: Pin::Pinned {
                version: version.into(),
            },
        }
    }

    pub fn unpinned(name: impl Into<String>) -> Self {
        Dependency {
            name: name.into(),
            pin: Pin::Unpinned,
        }
    }

    fn declaration(&self) -> Result<Declaration, SweepError> {
        match &self.pin {
            Pin::Pinned { .. } => Ok(Declaration::exact()),
            Pin::Unpinned => Declaration::degraded(format!("dependency {} is unpinned", self.name)),
        }
    }
}

/// A declared capture of one run's environment and artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    profile: CaptureProfile,
    included: BTreeSet<Dimension>,
    omissions: OmissionLedger,
    artifacts: Vec<Artifact>,
    dependencies: Vec<Dependency>,
}

impl Capture {
    pub fn new(profile: CaptureProfile) -> Self {
        Capture {
            profile,
            included: BTreeSet::new(),
            omissions: OmissionLedger::new(),
            artifacts: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn including(mut self, dimension: Dimension) -> Self {
        self.included.insert(dimension);
        self
    }

    /// Declare a dimension omitted, with a reason.
    pub fn omitting(
        mut self,
        dimension: Dimension,
        reason: impl Into<String>,
    ) -> Result<Self, SweepError> {
        self.omissions.omit_dimension(dimension, reason)?;
        Ok(self)
    }

    pub fn with_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    pub fn with_dependency(mut self, dependency: Dependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn profile(&self) -> CaptureProfile {
        self.profile
    }

    pub fn omissions(&self) -> &OmissionLedger {
        &self.omissions
    }

    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Dimensions neither included nor declared omitted.
    pub fn undeclared(&self) -> Vec<Dimension> {
        Dimension::ALL
            .into_iter()
            .filter(|d| !self.included.contains(d) && self.omissions.reason(d.as_str()).is_none())
            .collect()
    }

    /// Fails when any of the nine dimensions is unaccounted for.
    pub fn validate(&self) -> Result<(), SweepError> {
        let undeclared = self.undeclared();
        if undeclared.is_empty() {
            return Ok(());
        }
        Err(SweepError::Undeclared {
            what: "capture",
            items: undeclared
                .into_iter()
                .map(|d| d.as_str().to_string())
                .collect(),
        })
    }

    /// The capture's reproducibility declaration: the meet over its dependencies and its declared
    /// omissions.
    ///
    /// An omitted dimension contributes `Absent`, an unpinned dependency `Degraded`, everything
    /// else `Exact`. A capture that omits nothing and pins everything reports `Exact`; one that
    /// omits a single dimension reports `Absent`, which is severe on purpose — a rerun missing an
    /// entire inventory dimension is not a rerun.
    pub fn reproducibility(&self) -> Result<Declaration, SweepError> {
        self.validate()?;
        let mut declarations = Vec::new();
        for dependency in &self.dependencies {
            declarations.push(dependency.declaration()?);
        }
        for dimension in Dimension::ALL {
            if let Some(reason) = self.omissions.reason(dimension.as_str()) {
                declarations.push(Declaration::absent(format!(
                    "{}: {reason}",
                    dimension.as_str()
                ))?);
            }
        }
        Ok(meet_all(declarations.iter()))
    }

    /// Whether a rerun from this capture can be expected to restore identically.
    pub fn fully_reproducible(&self) -> Result<bool, SweepError> {
        Ok(self.reproducibility()?.level() == Level::Exact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete() -> Capture {
        Dimension::ALL.into_iter().fold(
            Capture::new(CaptureProfile::ArtifactsAndManifest),
            |c, d| c.including(d),
        )
    }

    fn artifact() -> ArtifactBuilder {
        ArtifactBuilder::new()
            .media_type("text/csv")
            .logical_role("input")
            .source_event("ev-1")
            .digest(ContentHash::of_bytes(b"x"))
            .size_bytes(12)
            .sensitivity(Sensitivity::Public)
            .license("CC-BY-4.0")
            .retention("90d")
            .encryption(Encryption::None)
    }

    #[test]
    fn the_dimension_list_is_the_blueprints_nine() {
        assert_eq!(Dimension::ALL.len(), 9);
        let unique: BTreeSet<_> = Dimension::ALL.iter().collect();
        assert_eq!(unique.len(), 9);
    }

    #[test]
    fn a_capture_silent_about_a_dimension_is_invalid_and_names_it() {
        let capture = Capture::new(CaptureProfile::MetadataOnly).including(Dimension::Files);
        let err = capture.validate().unwrap_err();
        match err {
            SweepError::Undeclared { items, .. } => {
                assert_eq!(items.len(), 8);
                assert!(items.contains(&"random_seeds".to_string()));
                assert!(!items.contains(&"files".to_string()));
            }
            other => panic!("expected Undeclared, got {other:?}"),
        }
    }

    #[test]
    fn declaring_a_dimension_omitted_satisfies_validation_without_pretending_it_was_captured() {
        let capture = Dimension::ALL
            .into_iter()
            .filter(|d| *d != Dimension::Hardware)
            .fold(Capture::new(CaptureProfile::MetadataOnly), |c, d| {
                c.including(d)
            })
            .omitting(
                Dimension::Hardware,
                "single-node CI, hardware is fixed by the image",
            )
            .unwrap();
        assert!(capture.validate().is_ok());
        assert_eq!(capture.reproducibility().unwrap().level(), Level::Absent);
    }

    #[test]
    fn an_omission_without_a_reason_is_refused() {
        assert!(Capture::new(CaptureProfile::MetadataOnly)
            .omitting(Dimension::Clocks, "")
            .is_err());
    }

    #[test]
    fn one_unpinned_dependency_degrades_the_whole_capture() {
        let capture = complete()
            .with_dependency(Dependency::pinned("numpy", "2.1.0"))
            .with_dependency(Dependency::pinned("scipy", "1.14.0"))
            .with_dependency(Dependency::unpinned("libblas"));
        assert_eq!(capture.reproducibility().unwrap().level(), Level::Degraded);
        assert!(!capture.fully_reproducible().unwrap());
        assert!(capture
            .reproducibility()
            .unwrap()
            .basis()
            .contains("libblas"));
    }

    #[test]
    fn a_fully_pinned_complete_capture_is_exact() {
        let capture = complete().with_dependency(Dependency::pinned("numpy", "2.1.0"));
        assert!(capture.fully_reproducible().unwrap());
    }

    #[test]
    fn reproducibility_refuses_to_report_at_all_while_a_dimension_is_undeclared() {
        let capture = Capture::new(CaptureProfile::EnvironmentImage);
        assert!(matches!(
            capture.reproducibility(),
            Err(SweepError::Undeclared { .. })
        ));
    }

    #[test]
    fn an_artifact_missing_fields_names_all_of_them_at_once() {
        let err = ArtifactBuilder::new()
            .media_type("text/csv")
            .build()
            .unwrap_err();
        match err {
            SweepError::MissingField { fields, .. } => {
                assert_eq!(fields.len(), 8);
                assert!(fields.contains(&"sensitivity".to_string()));
                assert!(fields.contains(&"license".to_string()));
            }
            other => panic!("expected MissingField, got {other:?}"),
        }
    }

    #[test]
    fn a_secret_artifact_cannot_be_stored_unencrypted() {
        let err = artifact()
            .sensitivity(Sensitivity::Secret)
            .encryption(Encryption::None)
            .build()
            .unwrap_err();
        assert!(matches!(err, SweepError::Malformed { .. }));
        assert!(artifact()
            .sensitivity(Sensitivity::Secret)
            .encryption(Encryption::ToKey {
                key_id: "org-1".into()
            })
            .build()
            .is_ok());
    }

    #[test]
    fn a_complete_artifact_round_trips_through_json() {
        let a = artifact().build().unwrap();
        let back: Artifact = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn the_omission_ledger_records_narrow_exclusions_as_well_as_whole_dimensions() {
        let mut ledger = OmissionLedger::new();
        ledger
            .declare("/etc/secrets", "credentials are excluded by policy")
            .unwrap();
        assert_eq!(ledger.len(), 1);
        assert!(ledger
            .reason("/etc/secrets")
            .unwrap()
            .contains("credentials"));
        assert!(ledger.reason("/etc/hosts").is_none());
    }
}
