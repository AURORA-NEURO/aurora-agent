//! The benchmark pack: immutable, content-addressed, self-attesting.
//!
//! Blueprint 10.02 ("content-addressed packs ... immutable artifact layer"), 27.16 (the count
//! ledger, generation manifest and effective sample-size report a release must carry) and 27.18
//! (the required artifacts of a submission). The self-verifying shape is taken directly from
//! [`bioprism_prism::ResultBundle`], and this module reuses its [`Attestation`] vocabulary so a
//! consumer learns one verification idiom rather than two.
//!
//! A pack carries its inputs **by digest**, never by value: parent worlds are named by their
//! semantic content digest (`bioprism_mutation::lineage::content_digest`), so a pack stays small
//! and a third party can check that the worlds they hold are the worlds the pack was built from.
//! The corresponding cost is that this crate cannot re-run an oracle; it can only check that the
//! pack recorded a postcondition outcome rather than assuming one. Hence
//! [`PostconditionEvidence::NotChecked`], which exists so "not validated" cannot be written down
//! as "validated" — the abuse 27.16 names as "unexecuted tasks counted as validated".

use bioprism_ids::ContentHash;
use bioprism_mutation::{measure, Diversity, Family, Instance};
use bioprism_prism::{Attestation, DecisionCell};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const PACK_SCHEMA_VERSION: &str = "bioprism-benchmark-pack/0.1";

/// The key an attested pack document carries its own digest under.
pub const PACK_DIGEST_FIELD: &str = "pack_sha256";

/// The provenance field, excluded from the core digest so evidence can accumulate.
const PROVENANCE_FIELD: &str = "provenance";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackError {
    #[error("pack is not canonically serialisable: {0}")]
    NotSerialisable(String),

    #[error("attested document is not a JSON object")]
    NotAnObject,

    #[error("attestation failed: {0}")]
    AttestationFailed(String),

    #[error("document does not deserialise as a benchmark pack: {0}")]
    Malformed(String),

    #[error(
        "instance {instance_id} descends from parent {parent_sha256}, which the pack does not \
         carry; 27.16 forbids orphan descendants"
    )]
    OrphanInstance {
        instance_id: String,
        parent_sha256: String,
    },
}

/// A parent world, named by the digest of its semantic content.
///
/// `provenance` is free text and is not verified by anything here. It is required to be present
/// because 27.16's release gate says every published object has lineage and intended use; an empty
/// string is a declaration that provenance is unknown, which is information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    pub world_id: String,
    pub sha256: String,
    pub provenance: String,
}

impl ParentRef {
    pub fn new(
        world_id: impl Into<String>,
        sha256: impl Into<String>,
        provenance: impl Into<String>,
    ) -> Self {
        ParentRef {
            world_id: world_id.into(),
            sha256: sha256.into(),
            provenance: provenance.into(),
        }
    }
}

/// What the generator observed when it checked an instance's declared metamorphic relation.
///
/// The three-way split is the point. `Held` and `Violated` are both *results*; `NotChecked` is the
/// absence of one, and a pack containing it may not be gated as validated however good its other
/// evidence is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "postcondition", rename_all = "snake_case")]
pub enum PostconditionEvidence {
    Held {
        relation: String,
        observed: String,
    },
    Violated {
        relation: String,
        expected: String,
        observed: String,
    },
    NotChecked {
        relation: String,
        reason: String,
    },
}

impl PostconditionEvidence {
    pub fn held(&self) -> bool {
        matches!(self, PostconditionEvidence::Held { .. })
    }

    pub fn reason(&self) -> String {
        match self {
            PostconditionEvidence::Held { relation, .. } => format!("{relation} held"),
            PostconditionEvidence::Violated {
                relation,
                expected,
                observed,
            } => format!("{relation} violated: expected {expected}, observed {observed}"),
            PostconditionEvidence::NotChecked { relation, reason } => {
                format!("{relation} was never checked: {reason}")
            }
        }
    }
}

/// One generated instance with the lineage that admits it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackInstance {
    pub instance: Instance,
    /// Digest of the parent this instance descends from. Must match a [`ParentRef`] in the pack.
    pub parent_sha256: String,
    pub postcondition: PostconditionEvidence,
}

impl PackInstance {
    pub fn id(&self) -> &str {
        &self.instance.id
    }
}

/// How a disagreement between two oracles over the same instance was settled.
///
/// 10.02's invariant — "nondeterministic judgments never silently override deterministic or
/// execution-grounded evidence" — is why a disagreement must be recorded and resolved explicitly
/// rather than dissolved by taking a majority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resolution", rename_all = "snake_case")]
pub enum Resolution {
    Unresolved,
    ResolvedInFavourOf { oracle: String, rationale: String },
    InstanceWithdrawn { rationale: String },
}

impl Resolution {
    pub fn is_unresolved(&self) -> bool {
        matches!(self, Resolution::Unresolved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleDisagreement {
    pub instance_id: String,
    /// Oracle kind to reported status, e.g. `{"fiber-split-integrity/0.1": "invalid"}`.
    pub statuses: BTreeMap<String, String>,
    pub resolution: Resolution,
}

/// The count ledger of 27.16.
///
/// Present so instance count can be read against what it cost: a family that accepted 8 of 400
/// attempts is a different artifact from one that accepted 8 of 9, and the headline number is
/// identical.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct YieldLedger {
    pub attempted: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub duplicates: usize,
}

impl YieldLedger {
    /// Whether the ledger adds up. A ledger that does not is evidence of dropped records.
    pub fn is_consistent(&self) -> bool {
        self.attempted == self.accepted + self.rejected + self.duplicates
    }

    pub fn yield_rate(&self) -> f64 {
        if self.attempted == 0 {
            return 0.0;
        }
        self.accepted as f64 / self.attempted as f64
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum ReviewFinding {
    Approved,
    ChangesRequested { detail: String },
    Rejected { detail: String },
}

/// A human review, bound to the exact benchmark content it looked at.
///
/// `reviewed_core_sha256` is the anti-staleness mechanism: a review names the core digest, so any
/// later edit to the pack's content silently detaches every review from it, and the pack falls
/// back to the tier its remaining evidence supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub reviewer: String,
    pub reviewed_core_sha256: String,
    pub finding: ReviewFinding,
    pub notes: String,
}

/// An independent rebuild: somebody other than the publisher reconstructed the pack content and
/// got the same core digest.
///
/// This is the strongest evidence available without a network or a signing key, because the claim
/// is self-checking — a rebuild attestation naming the wrong digest is worthless and says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildAttestation {
    pub rebuilt_by: String,
    pub rebuilt_core_sha256: String,
    pub command: String,
}

/// Publisher-side metadata and the accumulated review evidence.
///
/// Excluded from the core digest. Names here are claims, not identities: this crate has no key
/// material and cannot tell a real reviewer from an invented one. What it *can* check is that the
/// name differs from the publisher's, which is the reviewer-independence gate of 27.18.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub publisher: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default)]
    pub reviews: Vec<ReviewRecord>,
    #[serde(default)]
    pub rebuilds: Vec<RebuildAttestation>,
}

/// An immutable, content-addressed benchmark pack.
///
/// Fields are public so that an adversarial or hand-repaired pack can be constructed and then
/// *evaluated* — the checks in [`crate::tier`] are written against arbitrary values, not against
/// the honest path through [`PackBuilder`]. A pack whose diversity report was edited after the
/// fact is a valid Rust value and an untrustworthy artifact, and the requirement that catches it
/// has to be able to see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkPack {
    pub schema_version: String,
    pub pack_id: String,
    pub version: String,
    /// What this pack is for, and what a result on it would mean. Required by 27.16's release gate.
    pub intended_use: String,
    pub parents: Vec<ParentRef>,
    #[serde(default)]
    pub cells: Vec<DecisionCell>,
    pub instances: Vec<PackInstance>,
    /// Effective diversity, from `bioprism_mutation::measure`. Not recomputable here — the pack
    /// carries digests, not worlds — so the gate checks it against the instance list instead.
    pub diversity: Diversity,
    pub yield_ledger: YieldLedger,
    #[serde(default)]
    pub oracle_disagreements: Vec<OracleDisagreement>,
    /// What a result on this pack does *not* establish. An empty list is itself a finding.
    pub limitations: Vec<String>,
    pub provenance: Provenance,
}

impl BenchmarkPack {
    pub fn builder(pack_id: impl Into<String>, version: impl Into<String>) -> PackBuilder {
        PackBuilder::new(pack_id, version)
    }

    fn body(&self) -> Result<Map<String, Value>, PackError> {
        let value =
            serde_json::to_value(self).map_err(|e| PackError::NotSerialisable(e.to_string()))?;
        value.as_object().cloned().ok_or(PackError::NotAnObject)
    }

    /// Digest over the whole pack, provenance included. This is the artifact's address.
    pub fn digest(&self) -> Result<ContentHash, PackError> {
        let body = Value::Object(self.body()?);
        ContentHash::of_value(&body).map_err(|e| PackError::NotSerialisable(e.to_string()))
    }

    /// Digest over the benchmark content only, with the provenance block removed.
    ///
    /// This is what reviews and rebuilds name. Two packs with identical content and different
    /// review histories share a core digest and differ in artifact digest, which is exactly the
    /// distinction 10.05 needs to answer "is this the same benchmark?" separately from "is this
    /// the same file?".
    pub fn core_digest(&self) -> Result<ContentHash, PackError> {
        let mut body = self.body()?;
        body.remove(PROVENANCE_FIELD);
        ContentHash::of_value(&Value::Object(body))
            .map_err(|e| PackError::NotSerialisable(e.to_string()))
    }

    /// The core digest computed straight from an attested document, for a consumer that has the
    /// JSON and does not want to depend on this crate's structs.
    pub fn core_digest_of(document: &Value) -> Result<ContentHash, PackError> {
        let mut body = document
            .as_object()
            .cloned()
            .ok_or(PackError::NotAnObject)?;
        body.remove(PROVENANCE_FIELD);
        body.remove(PACK_DIGEST_FIELD);
        ContentHash::of_value(&Value::Object(body))
            .map_err(|e| PackError::NotSerialisable(e.to_string()))
    }

    /// The pack with its own digest attached, ready to be written to disk or published.
    pub fn attest(&self) -> Result<Value, PackError> {
        let body = self.body()?;
        let digest = ContentHash::of_value(&Value::Object(body.clone()))
            .map_err(|e| PackError::NotSerialisable(e.to_string()))?;
        let mut attested = body;
        attested.insert(PACK_DIGEST_FIELD.to_string(), json!(digest.as_str()));
        Ok(Value::Object(attested))
    }

    /// Recomputes an attested document's digest, the way a third party must before trusting a pack
    /// it did not build. Mirrors [`bioprism_prism::ResultBundle::verify`], including its
    /// separation of a shape defect in the claimed digest from a disagreement with the
    /// recomputation: a publisher whose digest field holds a typo has not been shown to have
    /// edited the pack, and a registry that said so would be accusing the wrong party.
    pub fn verify(document: &Value) -> Attestation {
        let Some(map) = document.as_object() else {
            return Attestation::Malformed("not an object".into());
        };
        let Some(claimed) = map.get(PACK_DIGEST_FIELD).and_then(Value::as_str) else {
            return Attestation::Malformed(format!("missing {PACK_DIGEST_FIELD}"));
        };
        if ContentHash::parse(claimed.to_string()).is_err() {
            return Attestation::Malformed(format!(
                "{PACK_DIGEST_FIELD} {claimed:?} is not a 64-character lowercase hex digest"
            ));
        }
        let mut body = map.clone();
        body.remove(PACK_DIGEST_FIELD);
        match ContentHash::of_value(&Value::Object(body)) {
            Ok(recomputed) if recomputed.as_str() == claimed => Attestation::Valid,
            Ok(recomputed) => Attestation::Mismatch {
                claimed: claimed.to_string(),
                recomputed: recomputed.as_str().to_string(),
            },
            Err(error) => Attestation::Malformed(error.to_string()),
        }
    }

    /// Verifies then deserialises. The only supported way to load a pack from an untrusted source.
    pub fn from_attested(document: &Value) -> Result<Self, PackError> {
        match Self::verify(document) {
            Attestation::Valid => {}
            Attestation::Mismatch {
                claimed,
                recomputed,
            } => {
                return Err(PackError::AttestationFailed(format!(
                    "document claims {claimed} but hashes to {recomputed}"
                )))
            }
            Attestation::Malformed(detail) => return Err(PackError::AttestationFailed(detail)),
        }
        let mut body = document
            .as_object()
            .cloned()
            .ok_or(PackError::NotAnObject)?;
        body.remove(PACK_DIGEST_FIELD);
        serde_json::from_value(Value::Object(body)).map_err(|e| PackError::Malformed(e.to_string()))
    }

    /// Attests the pack and then reads it straight back, comparing the result.
    ///
    /// The digest check alone is not enough here. The document *is* the artifact — it is what gets
    /// published, cited and re-verified — so a value that cannot survive its own canonical
    /// serialisation is unpublishable however well it hashes. The case that matters in practice is
    /// a non-finite number, such as a NaN inflation ratio from a diversity report over zero
    /// equivalence classes: JSON has no NaN, it serialises to null, and the null does not read
    /// back. Without this round trip such a pack would attest happily and then fail for everyone
    /// who downloaded it.
    pub fn self_attestation(&self) -> Attestation {
        let document = match self.attest() {
            Ok(document) => document,
            Err(error) => return Attestation::Malformed(error.to_string()),
        };
        match Self::verify(&document) {
            Attestation::Valid => {}
            other => return other,
        }
        match Self::from_attested(&document) {
            Ok(round_tripped) if round_tripped == *self => Attestation::Valid,
            Ok(_) => Attestation::Malformed(
                "pack does not survive its own canonical serialisation".into(),
            ),
            Err(error) => Attestation::Malformed(format!(
                "pack does not survive its own canonical serialisation: {error}"
            )),
        }
    }

    /// `pack_id@version` — the human address of 10.02's namespace model. Digests stay canonical.
    pub fn name(&self) -> String {
        format!("{}@{}", self.pack_id, self.version)
    }

    /// Instances whose declared relation did not hold, or was never checked.
    pub fn unvalidated(&self) -> Vec<&PackInstance> {
        self.instances
            .iter()
            .filter(|entry| !entry.postcondition.held())
            .collect()
    }

    /// Instances naming a parent the pack does not carry.
    pub fn orphans(&self) -> Vec<&PackInstance> {
        let known: BTreeSet<&str> = self.parents.iter().map(|p| p.sha256.as_str()).collect();
        self.instances
            .iter()
            .filter(|entry| !known.contains(entry.parent_sha256.as_str()))
            .collect()
    }
}

/// Builds a pack from validated mutation families.
///
/// The builder is the honest path: it derives the diversity report and the count ledger *from* the
/// families it was given, so a pack built this way cannot claim an instance count its lineage does
/// not support. Nothing forces a publisher through it — [`BenchmarkPack`]'s fields are public —
/// which is why the same properties are re-checked as tier requirements rather than assumed.
#[derive(Debug, Clone, Default)]
pub struct PackBuilder {
    pack_id: String,
    version: String,
    intended_use: String,
    publisher: String,
    license: Option<String>,
    parents: Vec<ParentRef>,
    cells: Vec<DecisionCell>,
    instances: Vec<PackInstance>,
    families: Vec<Family>,
    disagreements: Vec<OracleDisagreement>,
    reviews: Vec<ReviewRecord>,
    rebuilds: Vec<RebuildAttestation>,
    limitations: Vec<String>,
}

impl PackBuilder {
    pub fn new(pack_id: impl Into<String>, version: impl Into<String>) -> Self {
        PackBuilder {
            pack_id: pack_id.into(),
            version: version.into(),
            ..Default::default()
        }
    }

    pub fn intended_use(mut self, intended_use: impl Into<String>) -> Self {
        self.intended_use = intended_use.into();
        self
    }

    pub fn publisher(mut self, publisher: impl Into<String>) -> Self {
        self.publisher = publisher.into();
        self
    }

    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    pub fn limited_by(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    pub fn cell(mut self, cell: DecisionCell) -> Self {
        self.cells.push(cell);
        self
    }

    pub fn parent(mut self, parent: ParentRef) -> Self {
        self.parents.push(parent);
        self
    }

    pub fn instance(mut self, instance: PackInstance) -> Self {
        self.instances.push(instance);
        self
    }

    pub fn disagreement(mut self, disagreement: OracleDisagreement) -> Self {
        self.disagreements.push(disagreement);
        self
    }

    pub fn review(mut self, review: ReviewRecord) -> Self {
        self.reviews.push(review);
        self
    }

    pub fn rebuild(mut self, rebuild: RebuildAttestation) -> Self {
        self.rebuilds.push(rebuild);
        self
    }

    /// Ingests a validated mutation family: its parent, its accepted instances, and its counts.
    ///
    /// Only accepted instances become pack instances — a rejected mutation is not a benchmark —
    /// but the rejections and duplicates survive in the count ledger, because a yield rate that
    /// disappears is a yield rate nobody can question.
    pub fn family(mut self, family: &Family, parent_provenance: impl Into<String>) -> Self {
        self.parents.push(ParentRef::new(
            family.parent_id.clone(),
            family.parent_sha256.clone(),
            parent_provenance,
        ));
        for instance in &family.accepted {
            self.instances.push(PackInstance {
                parent_sha256: family.parent_sha256.clone(),
                postcondition: PostconditionEvidence::Held {
                    relation: instance.family.clone(),
                    observed: instance.signature(),
                },
                instance: instance.clone(),
            });
        }
        let mut lean = family.clone();
        lean.worlds.clear();
        self.families.push(lean);
        self
    }

    pub fn build(self) -> Result<BenchmarkPack, PackError> {
        let mut seen = BTreeSet::new();
        let mut parents = Vec::new();
        for parent in self.parents {
            if seen.insert(parent.sha256.clone()) {
                parents.push(parent);
            }
        }

        let rejected: usize = self
            .families
            .iter()
            .map(|family| family.rejected.len())
            .sum();
        let duplicates: usize = self
            .families
            .iter()
            .map(|family| family.duplicates.len())
            .sum();
        let accepted = self.instances.len();
        let ledger = YieldLedger {
            attempted: accepted + rejected + duplicates,
            accepted,
            rejected,
            duplicates,
        };

        let pack = BenchmarkPack {
            schema_version: PACK_SCHEMA_VERSION.to_string(),
            pack_id: self.pack_id,
            version: self.version,
            intended_use: self.intended_use,
            parents,
            cells: self.cells,
            instances: self.instances,
            diversity: measure(&self.families),
            yield_ledger: ledger,
            oracle_disagreements: self.disagreements,
            limitations: self.limitations,
            provenance: Provenance {
                publisher: self.publisher,
                license: self.license,
                reviews: self.reviews,
                rebuilds: self.rebuilds,
            },
        };

        if let Some(orphan) = pack.orphans().first() {
            return Err(PackError::OrphanInstance {
                instance_id: orphan.id().to_string(),
                parent_sha256: orphan.parent_sha256.clone(),
            });
        }
        Ok(pack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> BenchmarkPack {
        BenchmarkPack::builder("demo", "0.1.0")
            .intended_use("unit test")
            .build()
            .expect("no instances, no orphans")
    }

    #[test]
    fn a_yield_ledger_that_does_not_add_up_is_reported_as_inconsistent() {
        let honest = YieldLedger {
            attempted: 10,
            accepted: 4,
            rejected: 5,
            duplicates: 1,
        };
        assert!(honest.is_consistent());
        assert!((honest.yield_rate() - 0.4).abs() < f64::EPSILON);

        let dropped = YieldLedger {
            attempted: 4,
            accepted: 4,
            rejected: 5,
            duplicates: 1,
        };
        assert!(!dropped.is_consistent());
    }

    #[test]
    fn the_core_digest_ignores_provenance_so_a_review_does_not_invalidate_itself() {
        let pack = minimal();
        let core_before = pack.core_digest().expect("digestible");
        let artifact_before = pack.digest().expect("digestible");

        let mut reviewed = pack;
        reviewed.provenance.reviews.push(ReviewRecord {
            reviewer: "independent".into(),
            reviewed_core_sha256: core_before.as_str().to_string(),
            finding: ReviewFinding::Approved,
            notes: String::new(),
        });

        assert_eq!(reviewed.core_digest().expect("digestible"), core_before);
        assert_ne!(reviewed.digest().expect("digestible"), artifact_before);
    }

    #[test]
    fn the_core_digest_of_a_document_matches_the_core_digest_of_the_value() {
        let pack = minimal();
        let document = pack.attest().expect("digestible");
        assert_eq!(
            BenchmarkPack::core_digest_of(&document).expect("digestible"),
            pack.core_digest().expect("digestible")
        );
    }
}
