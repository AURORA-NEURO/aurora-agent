//! Supply chain and artifact security: pins, inventories, and the signature nobody checked.
//!
//! Implements blueprint 13.15 (supply chain and artifact security).
//!
//! # What is real here and what is not
//!
//! Exactly one thing in this module performs a check: [`DigestObservation::verify`] compares two
//! strings the caller supplied and reports [`SafetyError::ArtifactSubstitution`] when they differ.
//! That is a real comparison and a genuinely useful one — it is how substitution gets caught — but
//! notice what it requires. The caller must already have the observed digest, which means the
//! caller fetched the artifact and hashed it. This crate has no network, no filesystem, and no
//! archive reader. It cannot obtain an observed digest for anything.
//!
//! Everything else is inventory. [`Pin`] records what a manifest claims; [`Inventory`] holds the
//! SBOM and the benchmark bill of materials side by side; [`Inventory::audit_for_publication`]
//! refuses a floating reference, which is 13.15's "no floating `latest` in published runs" and is
//! a check on the *declaration*, catching an honest author who forgot rather than a dishonest one
//! who lied.
//!
//! # Signatures
//!
//! [`SignatureStatus`] has one variant, [`SignatureStatus::NotChecked`], for the reason
//! `bioprism_sdk::sandbox::Enforcement` has one variant. There is no cryptography in this
//! workspace: no key format, no curve, no trust root, no revocation list, no clock to check
//! validity against. A `Verified` variant would let a record say a signature was good, and the
//! only thing that could ever set it would be code that does not exist. 13.15's entire "Signing"
//! paragraph — publisher signature, builder attestation, hub publication receipt, purpose
//! separation — is therefore declared and not enforced, and [`crate::model`] records it that way.
//!
//! # What is deliberately not implemented
//!
//! * **No scanner.** 13.15 lists malware, vulnerability, secret, license and payload scanning.
//!   [`ScanCoverage`] records which categories a deployment *claims* to run and reports the ones it
//!   does not, so a gap is visible. It scans nothing.
//! * **No build isolation.** "Network-restricted reproducible builds" needs a builder.
//! * **No SBOM generation.** [`Inventory`] is a place to put one, not a tool that derives one from
//!   a lockfile.
//! * **No quarantine.** 13.15's "Response" paragraph is [`crate::incident`]'s subject, and even
//!   there it is a declared action.

use crate::error::SafetyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// How a component is referenced.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "pin", rename_all = "snake_case")]
pub enum Pin {
    /// A content digest. The only reference that identifies bytes.
    Digest { value: String },
    /// An immutable revision identifier: a dataset revision, a git commit, a model snapshot.
    ImmutableRevision { value: String },
    /// A tag, a range, `latest`. Identifies whatever the publisher points it at today.
    Floating { reference: String },
}

impl Pin {
    pub fn digest(value: impl Into<String>) -> Self {
        Pin::Digest {
            value: value.into(),
        }
    }

    pub fn revision(value: impl Into<String>) -> Self {
        Pin::ImmutableRevision {
            value: value.into(),
        }
    }

    pub fn floating(reference: impl Into<String>) -> Self {
        Pin::Floating {
            reference: reference.into(),
        }
    }

    /// Whether this reference identifies the same bytes on every resolution.
    pub fn is_immutable(&self) -> bool {
        !matches!(self, Pin::Floating { .. })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Pin::Digest { value } | Pin::ImmutableRevision { value } => value,
            Pin::Floating { reference } => reference,
        }
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What sort of thing a component is.
///
/// The split matters because 13.15 asks for two inventories: an SBOM for software and a "benchmark
/// bill of materials" for the evaluation material. Merging them loses the question "what data was
/// this result computed against", which is the one a reader of a leaderboard needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    ContainerImage,
    LanguageDependency,
    ModelWeights,
    Tokenizer,
    Dataset,
    BenchmarkAsset,
    OracleAsset,
    MutationGenerator,
    ExternalService,
}

impl ComponentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentKind::ContainerImage => "container_image",
            ComponentKind::LanguageDependency => "language_dependency",
            ComponentKind::ModelWeights => "model_weights",
            ComponentKind::Tokenizer => "tokenizer",
            ComponentKind::Dataset => "dataset",
            ComponentKind::BenchmarkAsset => "benchmark_asset",
            ComponentKind::OracleAsset => "oracle_asset",
            ComponentKind::MutationGenerator => "mutation_generator",
            ComponentKind::ExternalService => "external_service",
        }
    }

    /// Whether this component belongs in the benchmark bill of materials rather than the SBOM.
    pub fn is_benchmark_material(self) -> bool {
        matches!(
            self,
            ComponentKind::Dataset
                | ComponentKind::BenchmarkAsset
                | ComponentKind::OracleAsset
                | ComponentKind::MutationGenerator
                | ComponentKind::ExternalService
        )
    }
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a signature verified. It did not.
///
/// One variant. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    /// A signature may be recorded beside the component. Nothing in this process verified it, and
    /// no code here could.
    NotChecked,
}

impl fmt::Display for SignatureStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not-checked")
    }
}

/// One entry in an inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub kind: ComponentKind,
    pub pin: Pin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// A signature blob, if the manifest carried one. Recorded, never checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub signature_status: SignatureStatus,
}

impl Component {
    pub fn new(name: impl Into<String>, kind: ComponentKind, pin: Pin) -> Self {
        Component {
            name: name.into(),
            kind,
            pin,
            license: None,
            signature: None,
            signature_status: SignatureStatus::NotChecked,
        }
    }

    pub fn licensed(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    pub fn signed(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// The sentence that belongs beside any signature this component carries.
    pub fn honest_label(&self) -> String {
        match &self.signature {
            Some(_) => format!(
                "{} carries a signature; status {} — this process has no key material and verified nothing",
                self.name, self.signature_status
            ),
            None => format!("{} carries no signature", self.name),
        }
    }
}

/// A declared digest against an observed one.
///
/// Both values come from the caller. This crate fetched nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestObservation {
    pub component: String,
    pub declared: String,
    pub observed: String,
}

impl DigestObservation {
    pub fn new(
        component: impl Into<String>,
        declared: impl Into<String>,
        observed: impl Into<String>,
    ) -> Self {
        DigestObservation {
            component: component.into(),
            declared: declared.into(),
            observed: observed.into(),
        }
    }

    /// The one real check in this module.
    pub fn verify(&self) -> Result<(), SafetyError> {
        if self.declared == self.observed {
            Ok(())
        } else {
            Err(SafetyError::ArtifactSubstitution {
                component: self.component.clone(),
                declared: self.declared.clone(),
                observed: self.observed.clone(),
            })
        }
    }
}

/// The scan categories 13.15 lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanCategory {
    Malware,
    KnownVulnerabilities,
    Secrets,
    DangerousCapabilities,
    LicenseConflicts,
    SuspiciousScripts,
    DecompressionPayloads,
    PolicyViolations,
}

impl ScanCategory {
    /// Every category, so a gap list can be computed against the full set.
    pub const ALL: [ScanCategory; 8] = [
        ScanCategory::Malware,
        ScanCategory::KnownVulnerabilities,
        ScanCategory::Secrets,
        ScanCategory::DangerousCapabilities,
        ScanCategory::LicenseConflicts,
        ScanCategory::SuspiciousScripts,
        ScanCategory::DecompressionPayloads,
        ScanCategory::PolicyViolations,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ScanCategory::Malware => "malware",
            ScanCategory::KnownVulnerabilities => "known_vulnerabilities",
            ScanCategory::Secrets => "secrets",
            ScanCategory::DangerousCapabilities => "dangerous_capabilities",
            ScanCategory::LicenseConflicts => "license_conflicts",
            ScanCategory::SuspiciousScripts => "suspicious_scripts",
            ScanCategory::DecompressionPayloads => "decompression_payloads",
            ScanCategory::PolicyViolations => "policy_violations",
        }
    }
}

impl fmt::Display for ScanCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which scan categories a deployment says it runs.
///
/// A coverage declaration, not a scanner. Its value is [`ScanCoverage::gaps`]: the categories
/// nobody claimed, which is the list a reviewer should be looking at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanCoverage {
    declared: BTreeSet<ScanCategory>,
}

impl ScanCoverage {
    pub fn none() -> Self {
        ScanCoverage::default()
    }

    pub fn declaring(mut self, category: ScanCategory) -> Self {
        self.declared.insert(category);
        self
    }

    pub fn declares(&self, category: ScanCategory) -> bool {
        self.declared.contains(&category)
    }

    /// Categories nobody claimed to cover.
    pub fn gaps(&self) -> Vec<ScanCategory> {
        ScanCategory::ALL
            .into_iter()
            .filter(|category| !self.declared.contains(category))
            .collect()
    }
}

/// The software bill of materials and the benchmark bill of materials, together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub components: Vec<Component>,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory::default()
    }

    pub fn with(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    /// The SBOM half: software.
    pub fn software(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| !c.kind.is_benchmark_material())
            .collect()
    }

    /// The BBOM half: what the result was computed against.
    pub fn benchmark_material(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| c.kind.is_benchmark_material())
            .collect()
    }

    pub fn floating(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| !c.pin.is_immutable())
            .collect()
    }

    /// Components with no declared license. 13.15 asks the BBOM to carry licences and this is the
    /// gap list, not a compliance verdict.
    pub fn unlicensed(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| c.license.is_none())
            .collect()
    }

    /// 13.15's publication rule: nothing floating in a published run.
    pub fn audit_for_publication(&self) -> Result<(), SafetyError> {
        match self.floating().first() {
            Some(component) => Err(SafetyError::FloatingPin {
                component: component.name.clone(),
                reference: component.pin.as_str().to_string(),
            }),
            None => Ok(()),
        }
    }

    /// Every component carries a signature this process did not check.
    ///
    /// Returns the count, so a report can print "0 of 14 signatures verified" instead of a green
    /// tick beside a signed manifest.
    pub fn unchecked_signatures(&self) -> usize {
        self.components
            .iter()
            .filter(|c| c.signature.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_value_can_record_that_a_signature_verified() {
        let component = Component::new(
            "onco-pack",
            ComponentKind::BenchmarkAsset,
            Pin::digest("ab12"),
        )
        .signed("MEUCIQ...");
        assert_eq!(component.signature_status, SignatureStatus::NotChecked);
        assert!(component.honest_label().contains("verified nothing"));
    }

    #[test]
    fn a_floating_reference_blocks_publication_and_names_the_reference() {
        let inventory = Inventory::new()
            .with(Component::new(
                "runner",
                ComponentKind::ContainerImage,
                Pin::floating("latest"),
            ))
            .with(Component::new(
                "serde",
                ComponentKind::LanguageDependency,
                Pin::digest("cd34"),
            ));
        let error = inventory
            .audit_for_publication()
            .expect_err("13.15 forbids floating references in published runs");
        assert_eq!(
            error,
            SafetyError::FloatingPin {
                component: "runner".into(),
                reference: "latest".into(),
            }
        );
    }

    #[test]
    fn an_immutable_revision_is_an_acceptable_pin_and_a_tag_is_not() {
        assert!(Pin::digest("ab").is_immutable());
        assert!(Pin::revision("r-2026-01").is_immutable());
        assert!(!Pin::floating("v1").is_immutable());
    }

    #[test]
    fn a_declared_digest_that_differs_from_the_observed_one_is_substitution() {
        let observation = DigestObservation::new("image", "sha256:aaa", "sha256:bbb");
        assert!(matches!(
            observation.verify().expect_err("the bytes changed"),
            SafetyError::ArtifactSubstitution { .. }
        ));
        assert!(DigestObservation::new("image", "sha256:aaa", "sha256:aaa")
            .verify()
            .is_ok());
    }

    #[test]
    fn the_software_and_benchmark_inventories_do_not_share_components() {
        let inventory = Inventory::new()
            .with(Component::new(
                "tokio",
                ComponentKind::LanguageDependency,
                Pin::digest("a"),
            ))
            .with(Component::new(
                "holdout-v2",
                ComponentKind::OracleAsset,
                Pin::revision("r1"),
            ))
            .with(Component::new(
                "gencode",
                ComponentKind::Dataset,
                Pin::revision("v44"),
            ));
        assert_eq!(inventory.software().len(), 1);
        assert_eq!(inventory.benchmark_material().len(), 2);
        assert_eq!(
            inventory.software().len() + inventory.benchmark_material().len(),
            inventory.components.len()
        );
    }

    #[test]
    fn scan_coverage_reports_the_categories_nobody_claimed() {
        let coverage = ScanCoverage::none()
            .declaring(ScanCategory::Secrets)
            .declaring(ScanCategory::KnownVulnerabilities);
        assert!(coverage.declares(ScanCategory::Secrets));
        assert_eq!(coverage.gaps().len(), ScanCategory::ALL.len() - 2);
        assert!(coverage.gaps().contains(&ScanCategory::Malware));
    }

    #[test]
    fn a_deployment_that_declares_nothing_has_every_category_as_a_gap() {
        assert_eq!(ScanCoverage::none().gaps().len(), ScanCategory::ALL.len());
    }

    #[test]
    fn an_inventory_counts_signatures_it_did_not_check_rather_than_reporting_them_as_good() {
        let inventory = Inventory::new()
            .with(
                Component::new("a", ComponentKind::ContainerImage, Pin::digest("1")).signed("sig"),
            )
            .with(Component::new(
                "b",
                ComponentKind::Dataset,
                Pin::revision("2"),
            ));
        assert_eq!(inventory.unchecked_signatures(), 1);
    }

    #[test]
    fn unlicensed_components_are_listed_rather_than_assumed_permissive() {
        let inventory = Inventory::new()
            .with(
                Component::new("a", ComponentKind::Dataset, Pin::revision("1"))
                    .licensed("CC-BY-4.0"),
            )
            .with(Component::new(
                "b",
                ComponentKind::Dataset,
                Pin::revision("2"),
            ));
        assert_eq!(inventory.unlicensed().len(), 1);
        assert_eq!(inventory.unlicensed()[0].name, "b");
    }

    #[test]
    fn a_pin_survives_a_json_round_trip_with_its_kind_tagged() {
        let pin = Pin::floating("latest");
        let json = serde_json::to_string(&pin).expect("serialises");
        assert!(json.contains("\"pin\":\"floating\""), "{json}");
        assert_eq!(
            serde_json::from_str::<Pin>(&json).expect("deserialises"),
            pin
        );
    }
}
