//! The plugin manifest: one document, self-checkable, content-addressed.
//!
//! Blueprint 40.16 takes a `plugin manifest` as its first input and emits a "registered immutable
//! plugin version"; 11.12 enumerates what the manifest carries — "identity and digest; API range;
//! entry points; language/runtime; dependencies; required effects; network domains; secret
//! references; supported schemas; deterministic claims; conformance evidence".
//!
//! # What is here and what is deliberately not
//!
//! Identity, digest, supported schemas, required effects, network domains (as
//! [`Effect::Network`](crate::Effect::Network)), secret references, determinism and conformance
//! evidence are all here. **Entry points, language/runtime and dependencies are not**, and their
//! absence is the honest consequence of this crate loading nothing: an entry point is a string only
//! a dynamic loader can act on, and this crate has no loader. A `entry_point: String` field would
//! be a field every author had to fill in and nothing ever read. See the crate docs for the full
//! list of what is not implemented.
//!
//! # Two digests, and why
//!
//! [`PluginManifest::digest`] covers the whole document and is the immutable version identity.
//! [`PluginManifest::core_digest`] covers everything except the conformance evidence, and exists
//! because evidence is attached *after* the thing it describes was built. A reviewer runs a suite
//! against a plugin, records the core digest, and hands back an attestation; folding that
//! attestation into the digest it names would be circular. `bioprism-registry` splits pack digests
//! the same way and for the same reason.

use crate::abi::AbiGrade;
use crate::capability::{Capability, CapabilityKind};
use crate::effect::{Determinism, Effect};
use crate::error::ManifestError;
use crate::evidence::{assess_trust, ConformanceEvidence, TrustAssessment};
use crate::loss::SemanticLossDeclaration;
use crate::sandbox::SandboxDeclaration;
use crate::version::VersionSet;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The key under which conformance evidence is serialised, excluded from the core digest.
const EVIDENCE_FIELD: &str = "evidence";

/// Name plus version. The registry's key, and what every error message names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluginId {
    pub name: String,
    pub version: String,
}

impl PluginId {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        PluginId {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

/// Everything a plugin declares about itself.
///
/// Every field is a *claim*. Nothing in this struct is observed behaviour, and the type deliberately
/// carries no handle to running code — the point of separating declaration from implementation is
/// that the declaration can be validated, digested, stored and compared without the plugin being
/// present, which is what makes registration checkable at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// Who published this. Compared against evidence attesters to decide independence.
    pub publisher: String,
    /// The wire schema versions this plugin can speak, for [`crate::negotiate`].
    pub speaks: VersionSet,
    pub capabilities: Vec<Capability>,
    pub effects: BTreeSet<Effect>,
    pub determinism: Determinism,
    pub abi_grade: AbiGrade,
    pub sandbox: SandboxDeclaration,
    /// Required of adapter plugins, meaningless for the rest.
    pub loss: Option<SemanticLossDeclaration>,
    pub evidence: Vec<ConformanceEvidence>,
}

impl PluginManifest {
    /// A manifest at the floor of every claim: no effects, nondeterministic, ABI-P0, in-process,
    /// no evidence.
    ///
    /// The defaults are the weakest claims rather than the most convenient ones. A plugin author
    /// who never touches `abi_grade` gets P0 and is refused a continuation, which is the failure
    /// 23.49 wants; a default of P4 would hand it one.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        publisher: impl Into<String>,
    ) -> Self {
        PluginManifest {
            name: name.into(),
            version: version.into(),
            publisher: publisher.into(),
            speaks: VersionSet::new(),
            capabilities: Vec::new(),
            effects: BTreeSet::new(),
            determinism: Determinism::Nondeterministic,
            abi_grade: AbiGrade::P0,
            sandbox: SandboxDeclaration::default(),
            loss: None,
            evidence: Vec::new(),
        }
    }

    pub fn speaking(mut self, versions: VersionSet) -> Self {
        self.speaks = versions;
        self
    }

    pub fn providing(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }

    pub fn needing(mut self, effect: Effect) -> Self {
        self.effects.insert(effect);
        self
    }

    pub fn claiming(mut self, determinism: Determinism) -> Self {
        self.determinism = determinism;
        self
    }

    pub fn at_grade(mut self, grade: AbiGrade) -> Self {
        self.abi_grade = grade;
        self
    }

    pub fn requesting(mut self, sandbox: SandboxDeclaration) -> Self {
        self.sandbox = sandbox;
        self
    }

    pub fn declaring_loss(mut self, loss: SemanticLossDeclaration) -> Self {
        self.loss = Some(loss);
        self
    }

    pub fn attesting(mut self, evidence: ConformanceEvidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn id(&self) -> PluginId {
        PluginId::new(self.name.clone(), self.version.clone())
    }

    /// The capability kinds declared, deduplicated. [`PluginManifest::validate`] rejects duplicates,
    /// so on a valid manifest this has the same length as `capabilities`.
    pub fn capability_kinds(&self) -> BTreeSet<CapabilityKind> {
        self.capabilities
            .iter()
            .map(|capability| capability.kind)
            .collect()
    }

    pub fn capability(&self, kind: CapabilityKind) -> Option<&Capability> {
        self.capabilities
            .iter()
            .find(|capability| capability.kind == kind)
    }

    /// Content hash of the whole manifest. The immutable version identity of 40.16.
    pub fn digest(&self) -> Result<ContentHash, ManifestError> {
        let value = serde_json::to_value(self).map_err(|error| self.not_digestible(error))?;
        ContentHash::of_value(&value).map_err(|error| self.not_digestible(error))
    }

    /// Content hash of the manifest excluding its conformance evidence.
    ///
    /// What an external reviewer records when attesting, so that attaching the attestation does
    /// not change the thing attested.
    pub fn core_digest(&self) -> Result<ContentHash, ManifestError> {
        let mut value: BTreeMap<String, serde_json::Value> = serde_json::from_value(
            serde_json::to_value(self).map_err(|error| self.not_digestible(error))?,
        )
        .map_err(|error| self.not_digestible(error))?;
        value.remove(EVIDENCE_FIELD);
        let value = serde_json::to_value(&value).map_err(|error| self.not_digestible(error))?;
        ContentHash::of_value(&value).map_err(|error| self.not_digestible(error))
    }

    /// The trust level this manifest's own evidence earns it (40.16: registration does not imply
    /// trust).
    pub fn trust(&self) -> Result<TrustAssessment, ManifestError> {
        let core = self.core_digest()?;
        Ok(assess_trust(
            &self.publisher,
            &self.capability_kinds(),
            &self.evidence,
            &core,
        ))
    }

    /// Every check that can be made against this manifest alone.
    ///
    /// Runnable by a plugin author with no host present, which is the point: 11.13 asks the SDK to
    /// provide scaffolding, and the most useful scaffolding is the one that gives the same answer
    /// the registry will give, before the registry is involved.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.trim().is_empty() {
            return Err(ManifestError::EmptyName);
        }
        if self.version.trim().is_empty() {
            return Err(ManifestError::EmptyVersion {
                plugin: self.name.clone(),
            });
        }
        if self.capabilities.is_empty() {
            return Err(ManifestError::NoCapabilities {
                plugin: self.name.clone(),
            });
        }

        let mut seen = BTreeSet::new();
        for capability in &self.capabilities {
            if !seen.insert(capability.kind) {
                return Err(ManifestError::DuplicateCapabilityKind {
                    plugin: self.name.clone(),
                    kind: capability.kind.to_string(),
                });
            }
        }

        for effect in &self.effects {
            if !self.determinism.tolerates(effect) {
                return Err(ManifestError::DeterminismContradictedByEffect {
                    plugin: self.name.clone(),
                    effect: effect.to_string(),
                });
            }
        }

        if seen.iter().any(|kind| kind.requires_loss_declaration()) {
            match &self.loss {
                None => {
                    return Err(ManifestError::AdapterWithoutLossDeclaration {
                        plugin: self.name.clone(),
                    })
                }
                Some(loss) if loss.is_silent() => {
                    return Err(ManifestError::AdapterWithoutLossDeclaration {
                        plugin: self.name.clone(),
                    })
                }
                Some(loss) if loss.is_contradictory() => {
                    return Err(ManifestError::ContradictoryLossDeclaration {
                        plugin: self.name.clone(),
                        kinds: loss.describe_kinds(),
                    })
                }
                Some(_) => {}
            }
        }

        for evidence in &self.evidence {
            if !seen.contains(&evidence.covers) {
                return Err(ManifestError::EvidenceForUndeclaredCapability {
                    plugin: self.name.clone(),
                    kind: evidence.covers.to_string(),
                });
            }
        }

        self.digest()?;
        Ok(())
    }

    fn not_digestible(&self, error: impl fmt::Display) -> ManifestError {
        ManifestError::NotDigestible {
            plugin: self.name.clone(),
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Priority;
    use crate::sandbox::IsolationClass;

    fn adapter_manifest() -> PluginManifest {
        PluginManifest::new("csv-adapter", "1.0.0", "acme")
            .providing(Capability::new(CapabilityKind::Adapter).handling(["text/csv"]))
            .declaring_loss(SemanticLossDeclaration::admitting(["unmapped_column"]))
    }

    #[test]
    fn an_adapter_plugin_without_a_loss_declaration_does_not_validate() {
        let manifest = PluginManifest::new("csv-adapter", "1.0.0", "acme")
            .providing(Capability::new(CapabilityKind::Adapter));
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::AdapterWithoutLossDeclaration { .. })
        ));
    }

    #[test]
    fn an_empty_loss_declaration_counts_as_no_declaration_at_all() {
        let manifest = PluginManifest::new("csv-adapter", "1.0.0", "acme")
            .providing(Capability::new(CapabilityKind::Adapter))
            .declaring_loss(SemanticLossDeclaration::default());
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::AdapterWithoutLossDeclaration { .. })
        ));
    }

    #[test]
    fn a_non_adapter_plugin_owes_no_loss_declaration() {
        let manifest = PluginManifest::new("judge", "0.1.0", "acme")
            .providing(Capability::new(CapabilityKind::Oracle));
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn a_determinism_claim_contradicted_by_a_declared_effect_does_not_validate() {
        let manifest = PluginManifest::new("judge", "0.1.0", "acme")
            .providing(Capability::new(CapabilityKind::Oracle))
            .claiming(Determinism::Deterministic)
            .needing(Effect::Network {
                domain: "api.example".into(),
            });
        match manifest.validate() {
            Err(ManifestError::DeterminismContradictedByEffect { effect, .. }) => {
                assert!(effect.contains("api.example"), "{effect}")
            }
            other => panic!("expected a determinism contradiction, got {other:?}"),
        }
    }

    #[test]
    fn evidence_for_a_capability_the_plugin_does_not_declare_does_not_validate() {
        let manifest = adapter_manifest().attesting(
            ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "acme").passing(1),
        );
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::EvidenceForUndeclaredCapability { .. })
        ));
    }

    #[test]
    fn declaring_the_same_capability_twice_does_not_validate() {
        let manifest = adapter_manifest()
            .providing(Capability::new(CapabilityKind::Adapter).at(Priority(200)));
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::DuplicateCapabilityKind { .. })
        ));
    }

    #[test]
    fn a_plugin_with_no_capabilities_has_nothing_to_be_registered_for() {
        let manifest = PluginManifest::new("empty", "1.0.0", "acme");
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::NoCapabilities { .. })
        ));
    }

    #[test]
    fn attaching_evidence_changes_the_digest_but_not_the_core_digest() {
        let bare = adapter_manifest();
        let core_before = bare.core_digest().expect("digestible");
        let attested = bare.clone().attesting(
            ConformanceEvidence::new("adapter/normalize", CapabilityKind::Adapter, "reviewer")
                .passing(4)
                .covering_core(core_before.clone()),
        );
        assert_eq!(attested.core_digest().expect("digestible"), core_before);
        assert_ne!(
            attested.digest().expect("digestible"),
            bare.digest().expect("digestible")
        );
    }

    #[test]
    fn the_digest_depends_only_on_content_not_on_field_order() {
        let manifest = adapter_manifest();
        let json = serde_json::to_string(&manifest).expect("serialises");
        let round_tripped: PluginManifest = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(
            round_tripped.digest().expect("digestible"),
            manifest.digest().expect("digestible")
        );
    }

    #[test]
    fn changing_the_declared_isolation_changes_the_identity_digest() {
        let a = adapter_manifest();
        let b = adapter_manifest()
            .requesting(SandboxDeclaration::requesting(IsolationClass::Sandboxed));
        assert_ne!(
            a.digest().expect("digestible"),
            b.digest().expect("digestible")
        );
    }

    #[test]
    fn a_default_manifest_claims_the_weakest_grade_available() {
        assert_eq!(
            PluginManifest::new("x", "1", "acme").abi_grade,
            AbiGrade::P0
        );
        assert_eq!(
            PluginManifest::new("x", "1", "acme").determinism,
            Determinism::Nondeterministic
        );
    }
}
