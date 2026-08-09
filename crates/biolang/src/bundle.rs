//! BioResult Bundle and Attestation IR — blueprint 25.20.
//!
//! What a third party needs to reconstruct what was run, scored, claimed and limited: a run
//! manifest, a decision trace, the evidence, the scores with their intervals, resource use,
//! violations, limitations and an attestation.
//!
//! # Where the IR and the implementing crate disagree, and it matters
//!
//! 25.20's required field group is called **signatures**, and its invariant is that "attestations
//! identify signer and evidence scope". `bioprism-bundle` cannot produce a signature. The workspace
//! builds offline against pinned dependencies whose only cryptographic primitive is `sha2`, from
//! which HMAC-SHA256 can be built and nothing asymmetric can. That crate says so in its own type
//! system: `AuthenticationScheme` has one variant, `SymmetricSharedSecret`; `Repudiability` has one
//! variant, `ForgeableByAnyVerifier`; a verified result deliberately has no accessor for the claimed
//! producer, because a symmetric tag cannot identify one.
//!
//! So this IR does not have a `signature` field. It has [`Attestation`], which carries a MAC tag, a
//! [`Repudiability`] that admits forgeability, and an evidence scope. Naming the field `signature`
//! would let a downstream reader conclude non-repudiation from an IR that cannot deliver it, and
//! that conclusion is exactly the harm 25.20's third-party-reconstruction purpose is meant to
//! prevent. **The blueprint's "signatures" requirement is not satisfiable on this platform, and the
//! IR says so rather than looking as though it is.**
//!
//! # What is deliberately not implemented
//!
//! No key management, no revocation, no transparency log, no timestamping authority, no
//! reconstruction. `bioprism-bundle` documents each of those absences and this IR inherits every
//! one; an IR cannot supply a capability its implementation lacks.

use crate::error::BundleIrError;
use crate::ids::{ActionId, SystemId};
use bioprism_ids::{RunId, WorldId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What was run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: RunId,
    pub system: SystemId,
    /// Component pins, copied from [`crate::system::SystemManifest`] at run time.
    pub component_versions: BTreeMap<String, String>,
    pub world: WorldId,
    pub world_version: String,
    /// Declared environment facts. Declared by the caller; nothing here measures an environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
}

/// One thing the system did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracedAction {
    pub action: ActionId,
    /// Sequence number within the run. Not a timestamp: this crate reads no clock.
    pub step: u32,
    /// Digests of the artifacts the action produced.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub produced: BTreeSet<String>,
    /// The oracle invoked by this step, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoked_oracle: Option<String>,
}

/// A score and the interval around it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub name: String,
    pub value: f64,
    /// A declared interval. `None` is "no interval was computed", not "the interval is zero".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<[f64; 2]>,
    /// The bundle entry this score resolves to. 25.20: "A published score resolves to a complete
    /// bundle."
    pub entry: String,
}

/// A recorded verdict from an oracle in the mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedVerdict {
    pub oracle: String,
    pub verdict: crate::oracle::Verdict,
    pub tier: crate::oracle::EvidenceTier,
}

/// Whether the attestation can be denied by its producer.
///
/// One variant, mirroring `bioprism_bundle::Repudiability`. A second variant would be a lie the
/// platform cannot back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repudiability {
    /// Anyone who can verify the tag could also have produced it.
    ForgeableByAnyVerifier,
}

/// A symmetric authentication over the manifest.
///
/// Called an attestation, not a signature, and the type documents why: see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// The MAC tag, rendered with its scheme prefix so it cannot be quoted as a signature.
    pub tag: String,
    /// Who claims to have produced it. A claim, not a verified identity: symmetric authentication
    /// cannot establish one, and [`Repudiability`] records that.
    pub claimed_producer: String,
    /// What the attestation covers. 25.20: "Attestations identify signer and evidence scope."
    pub evidence_scope: BTreeSet<String>,
    pub repudiability: Repudiability,
}

/// A published result bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultBundle {
    pub manifest: RunManifest,
    /// Monotone version. An amendment must increment it.
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amends: Option<u32>,
    pub trace: Vec<TracedAction>,
    /// Entry names to artifact digests.
    pub entries: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdicts: Vec<RecordedVerdict>,
    pub scores: Vec<Score>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resource_use: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<String>,
    /// 25.20: "limitations". Empty is a claim that there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<Attestation>,
}

impl ResultBundle {
    pub fn validate(&self) -> Result<(), BundleIrError> {
        for score in &self.scores {
            if !self.entries.contains_key(&score.entry) {
                return Err(BundleIrError::ScoreWithoutBundle {
                    score: score.name.clone(),
                });
            }
        }

        if let Some(amended) = self.amends {
            if amended == self.version {
                return Err(BundleIrError::AmendmentReusesVersion {
                    run: self.manifest.run_id.to_string(),
                    version: self.version.to_string(),
                });
            }
        }

        if let Some(attestation) = &self.attestation {
            if attestation.claimed_producer.trim().is_empty() {
                return Err(BundleIrError::AttestationWithout {
                    run: self.manifest.run_id.to_string(),
                    missing: "claimed producer".to_string(),
                });
            }
            if attestation.evidence_scope.is_empty() {
                return Err(BundleIrError::AttestationWithout {
                    run: self.manifest.run_id.to_string(),
                    missing: "evidence scope".to_string(),
                });
            }
        }

        let invoked: BTreeSet<&str> = self
            .trace
            .iter()
            .filter_map(|step| step.invoked_oracle.as_deref())
            .collect();
        for verdict in &self.verdicts {
            if !invoked.contains(verdict.oracle.as_str()) {
                return Err(BundleIrError::VerdictFromUninvokedOracle {
                    oracle: verdict.oracle.clone(),
                });
            }
        }

        Ok(())
    }

    /// Whether this bundle can support a claim of third-party verifiability.
    ///
    /// Always false. It is a method rather than a constant so a caller that asks the question in
    /// code gets the answer in code, and so the reason travels with it.
    pub fn supports_third_party_verification(&self) -> bool {
        false
    }
}
