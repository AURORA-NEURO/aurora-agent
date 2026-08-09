//! Content-addressed storage, lookup by digest, and an append-only publication log.
//!
//! Blueprint 10.02 (four registry layers; "immutable artifact layer plus mutable indexed
//! projections"; "index updates are ... replayable from an append-only registry event stream"),
//! 10.05 (publication, supersession, withdrawal) and 27.16 ("historical records remain
//! immutable").
//!
//! # Nothing is ever edited
//!
//! There is no method on this type that mutates a stored artifact. A correction is a **new pack
//! with a new digest** that supersedes the old one; the old one stays readable forever, and the
//! supersession is a log entry. That is not a stylistic preference — 10.02's invariant is that
//! changes to scoring semantics "cannot retroactively rewrite already published results", and a
//! registry that lets a publisher fix a pack in place makes every past result uninterpretable.
//!
//! # Names bind to content, not to files
//!
//! `pack_id@version` binds permanently to one **core digest**, so republishing a version with
//! different benchmark content is [`RegistryError::VersionAlreadyBound`] — 10.02's "a mutable tag
//! silently changes the benchmark used by a historical run", caught at the door rather than
//! diagnosed afterwards.
//!
//! It binds to the core digest rather than the artifact digest because otherwise review evidence
//! could never be published at all. A reviewer approves content `C`; publishing that approval
//! produces a new artifact over the same `C`; if the name were bound to the artifact, the
//! publisher would have to bump the version, which changes `C`, which detaches the very review
//! they were trying to record. Binding to content dissolves that: several artifacts may share a
//! name precisely when they are the same benchmark differing only in accumulated provenance, and
//! [`RegistryIndex::resolve`] returns the most recent of them.
//!
//! Mutable aliases such as `stable` are *not* implemented; 10.02 wants them to resolve to explicit
//! revisions and to record resolution time, and there is no clock here.
//!
//! # What is missing
//!
//! No network, no federation, no replication, no signatures, no two-phase publication, no
//! eventual-consistency machinery — this index is a value, and publication is a function call, so
//! the failure modes those mechanisms exist to handle cannot arise. Deprecation, embargo, security
//! quarantine as a *state* and moderation are also absent; withdrawal is the only removal verb.

use crate::pack::BenchmarkPack;
use crate::promote::{promote_with, Promotion, PromotionError};
use crate::tier::{reassess, TierPolicy, TierVerdict, TrustTier, UnmetRequirement};
use bioprism_prism::Attestation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RegistryError {
    #[error("attestation failed for the submitted pack: {0}")]
    AttestationFailed(String),

    #[error("pack cannot be content-addressed: {0}")]
    Undigestible(String),

    #[error("no artifact stored under digest {0}")]
    UnknownDigest(String),

    #[error(
        "{name} is already bound to benchmark content {existing_core}; a correction is a new \
         version, never an edit to a published one"
    )]
    VersionAlreadyBound { name: String, existing_core: String },

    #[error("digest {digest} is already published at {published}; republishing at {requested} would rewrite history")]
    AlreadyPublished {
        digest: String,
        published: TrustTier,
        requested: TrustTier,
    },

    #[error("{target} is not above the published tier {published}")]
    NotAPromotion {
        published: TrustTier,
        target: TrustTier,
    },

    #[error("digest {digest} is not supported at {target}: {} requirement(s) unmet", unmet.len())]
    TierNotEarned {
        digest: String,
        target: TrustTier,
        earned: TrustTier,
        unmet: Vec<UnmetRequirement>,
    },

    #[error("digest {digest} is already superseded by {by}")]
    AlreadySuperseded { digest: String, by: String },

    #[error("digest {digest} was withdrawn: {reason}")]
    Withdrawn { digest: String, reason: String },

    #[error("a pack cannot supersede itself ({0})")]
    SelfSupersession(String),
}

/// The lifecycle state of a stored artifact, from 10.05's release channels.
///
/// `Active`, `Superseded` and `Withdrawn` only. Draft, candidate, deprecated and
/// security-quarantine are not modelled: they are process states, and this crate has no process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PackStatus {
    Active,
    Superseded { by: String, reason: String },
    Withdrawn { reason: String },
}

impl PackStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, PackStatus::Active)
    }
}

/// One entry in the append-only publication log.
///
/// The log is the source of truth; the maps beside it are the "mutable indexed projections" of
/// 10.02 and could be rebuilt from these events alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PublicationEvent {
    Published {
        sequence: u64,
        digest: String,
        core_digest: String,
        name: String,
        tier: TrustTier,
    },
    Promoted {
        sequence: u64,
        digest: String,
        from: TrustTier,
        to: TrustTier,
    },
    /// A published tier no longer holds. Reasons are the requirements that broke.
    Demoted {
        sequence: u64,
        digest: String,
        from: TrustTier,
        to: TrustTier,
        reasons: Vec<UnmetRequirement>,
    },
    Superseded {
        sequence: u64,
        digest: String,
        by: String,
        reason: String,
    },
    Withdrawn {
        sequence: u64,
        digest: String,
        reason: String,
    },
}

impl PublicationEvent {
    pub fn sequence(&self) -> u64 {
        match self {
            PublicationEvent::Published { sequence, .. }
            | PublicationEvent::Promoted { sequence, .. }
            | PublicationEvent::Demoted { sequence, .. }
            | PublicationEvent::Superseded { sequence, .. }
            | PublicationEvent::Withdrawn { sequence, .. } => *sequence,
        }
    }

    pub fn digest(&self) -> &str {
        match self {
            PublicationEvent::Published { digest, .. }
            | PublicationEvent::Promoted { digest, .. }
            | PublicationEvent::Demoted { digest, .. }
            | PublicationEvent::Superseded { digest, .. }
            | PublicationEvent::Withdrawn { digest, .. } => digest,
        }
    }
}

/// A local, content-addressed benchmark registry.
///
/// Serialisable in full, so `.prism/registry` can be a single JSON file — the zero-service local
/// registry 10.02 requires. Artifacts are stored as their *attested documents* rather than as
/// deserialised values, so a stored artifact can be re-verified byte-for-byte without trusting
/// this crate's structs to round-trip.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RegistryIndex {
    artifacts: BTreeMap<String, Value>,
    core_digests: BTreeMap<String, String>,
    tiers: BTreeMap<String, TrustTier>,
    statuses: BTreeMap<String, PackStatus>,
    /// `pack_id@version` to the core digest it is permanently bound to.
    names: BTreeMap<String, String>,
    /// `pack_id@version` to the most recently published artifact carrying that content.
    latest_artifact: BTreeMap<String, String>,
    log: Vec<PublicationEvent>,
}

impl RegistryIndex {
    pub fn new() -> Self {
        RegistryIndex::default()
    }

    fn next_sequence(&self) -> u64 {
        self.log.len() as u64
    }

    /// Stores a pack at a tier its evidence supports, returning its digest.
    ///
    /// Republishing byte-identical content at the same tier is idempotent (10.02: "artifact writes
    /// are immutable and idempotent") and appends nothing to the log — a log that grows on
    /// re-publication would make replay report events that never happened.
    pub fn publish(
        &mut self,
        pack: &BenchmarkPack,
        tier: TrustTier,
        policy: &TierPolicy,
    ) -> Result<String, RegistryError> {
        let document = pack
            .attest()
            .map_err(|error| RegistryError::Undigestible(error.to_string()))?;
        match BenchmarkPack::verify(&document) {
            Attestation::Valid => {}
            Attestation::Mismatch {
                claimed,
                recomputed,
            } => {
                return Err(RegistryError::AttestationFailed(format!(
                    "claims {claimed}, hashes to {recomputed}"
                )))
            }
            Attestation::Malformed(detail) => return Err(RegistryError::AttestationFailed(detail)),
        }

        let digest = pack
            .digest()
            .map_err(|error| RegistryError::Undigestible(error.to_string()))?
            .as_str()
            .to_string();
        let core = pack
            .core_digest()
            .map_err(|error| RegistryError::Undigestible(error.to_string()))?
            .as_str()
            .to_string();
        let name = pack.name();

        if let Some(published) = self.tiers.get(&digest) {
            if *published == tier {
                return Ok(digest);
            }
            return Err(RegistryError::AlreadyPublished {
                digest,
                published: *published,
                requested: tier,
            });
        }

        if let Some(existing_core) = self.names.get(&name) {
            if existing_core != &core {
                return Err(RegistryError::VersionAlreadyBound {
                    name,
                    existing_core: existing_core.clone(),
                });
            }
        }

        if tier != TrustTier::Unranked {
            match promote_with(pack, tier, policy) {
                Ok(_) => {}
                Err(PromotionError::EvidenceInsufficient { earned, unmet, .. }) => {
                    return Err(RegistryError::TierNotEarned {
                        digest,
                        target: tier,
                        earned,
                        unmet,
                    })
                }
                Err(PromotionError::NotATier { .. }) => {}
                Err(PromotionError::Undigestible(detail)) => {
                    return Err(RegistryError::Undigestible(detail))
                }
            }
        }

        let sequence = self.next_sequence();
        self.artifacts.insert(digest.clone(), document);
        self.core_digests.insert(digest.clone(), core.clone());
        self.tiers.insert(digest.clone(), tier);
        self.statuses.insert(digest.clone(), PackStatus::Active);
        self.names.insert(name.clone(), core.clone());
        self.latest_artifact.insert(name.clone(), digest.clone());
        self.log.push(PublicationEvent::Published {
            sequence,
            digest: digest.clone(),
            core_digest: core,
            name,
            tier,
        });
        Ok(digest)
    }

    /// The attested document stored under a digest.
    pub fn get(&self, digest: &str) -> Option<&Value> {
        self.artifacts.get(digest)
    }

    /// Loads and re-verifies a stored artifact.
    pub fn load(&self, digest: &str) -> Result<BenchmarkPack, RegistryError> {
        let document = self
            .get(digest)
            .ok_or_else(|| RegistryError::UnknownDigest(digest.to_string()))?;
        BenchmarkPack::from_attested(document)
            .map_err(|error| RegistryError::AttestationFailed(error.to_string()))
    }

    /// Resolves the human address `pack_id@version` to the most recent artifact carrying it.
    ///
    /// Every artifact under one name has identical benchmark content, so this is not a mutable
    /// tag in the sense 10.02 warns about: what varies between them is provenance, never what the
    /// benchmark tests. Use [`RegistryIndex::content_of`] for the invariant part.
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.latest_artifact.get(name).map(String::as_str)
    }

    /// The core digest a `pack_id@version` is permanently bound to.
    pub fn content_of(&self, name: &str) -> Option<&str> {
        self.names.get(name).map(String::as_str)
    }

    pub fn tier_of(&self, digest: &str) -> Option<TrustTier> {
        self.tiers.get(digest).copied()
    }

    pub fn status(&self, digest: &str) -> Option<&PackStatus> {
        self.statuses.get(digest)
    }

    pub fn core_digest_of(&self, digest: &str) -> Option<&str> {
        self.core_digests.get(digest).map(String::as_str)
    }

    pub fn log(&self) -> &[PublicationEvent] {
        &self.log
    }

    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Every digest sharing a core digest: the same benchmark content under different provenance.
    pub fn revisions_of_content(&self, core_digest: &str) -> Vec<&str> {
        self.core_digests
            .iter()
            .filter(|(_, core)| core.as_str() == core_digest)
            .map(|(digest, _)| digest.as_str())
            .collect()
    }

    /// Raises a published pack's tier, once the evidence supports it.
    pub fn promote(
        &mut self,
        digest: &str,
        target: TrustTier,
        policy: &TierPolicy,
    ) -> Result<Promotion, RegistryError> {
        let published = self
            .tier_of(digest)
            .ok_or_else(|| RegistryError::UnknownDigest(digest.to_string()))?;
        if target <= published {
            return Err(RegistryError::NotAPromotion { published, target });
        }
        let pack = self.load(digest)?;
        let promotion = promote_with(&pack, target, policy).map_err(|error| match error {
            PromotionError::EvidenceInsufficient { earned, unmet, .. } => {
                RegistryError::TierNotEarned {
                    digest: digest.to_string(),
                    target,
                    earned,
                    unmet,
                }
            }
            PromotionError::NotATier { .. } => RegistryError::NotAPromotion { published, target },
            PromotionError::Undigestible(detail) => RegistryError::Undigestible(detail),
        })?;

        let sequence = self.next_sequence();
        self.tiers.insert(digest.to_string(), target);
        self.log.push(PublicationEvent::Promoted {
            sequence,
            digest: digest.to_string(),
            from: published,
            to: target,
        });
        Ok(promotion)
    }

    /// Re-evaluates a published pack against its recorded tier, demoting it if the evidence no
    /// longer supports the claim.
    ///
    /// The artifact is immutable, so this changes verdict when the *policy* tightens, or when a
    /// pack was published at a tier it never earned. Both are worth finding; only the second is
    /// somebody's fault.
    pub fn reassess(
        &mut self,
        digest: &str,
        policy: &TierPolicy,
    ) -> Result<TierVerdict, RegistryError> {
        let published = self
            .tier_of(digest)
            .ok_or_else(|| RegistryError::UnknownDigest(digest.to_string()))?;
        let pack = self.load(digest)?;
        let verdict = reassess(&pack, published, policy);
        if let TierVerdict::Demoted {
            claimed,
            earned,
            reasons,
        } = &verdict
        {
            let sequence = self.next_sequence();
            self.tiers.insert(digest.to_string(), *earned);
            self.log.push(PublicationEvent::Demoted {
                sequence,
                digest: digest.to_string(),
                from: *claimed,
                to: *earned,
                reasons: reasons.clone(),
            });
        }
        Ok(verdict)
    }

    /// Publishes `replacement` and records that it supersedes `superseded`.
    ///
    /// The superseded artifact remains stored and readable. 10.05: "a new release can supersede a
    /// prior release while preserving historical results" — a result that cited the old digest
    /// still resolves, and a reader who follows it now learns that it was replaced and why.
    pub fn supersede(
        &mut self,
        superseded: &str,
        replacement: &BenchmarkPack,
        tier: TrustTier,
        reason: impl Into<String>,
        policy: &TierPolicy,
    ) -> Result<String, RegistryError> {
        match self.statuses.get(superseded) {
            None => return Err(RegistryError::UnknownDigest(superseded.to_string())),
            Some(PackStatus::Superseded { by, .. }) => {
                return Err(RegistryError::AlreadySuperseded {
                    digest: superseded.to_string(),
                    by: by.clone(),
                })
            }
            Some(PackStatus::Withdrawn { reason }) => {
                return Err(RegistryError::Withdrawn {
                    digest: superseded.to_string(),
                    reason: reason.clone(),
                })
            }
            Some(PackStatus::Active) => {}
        }

        let replacement_digest = replacement
            .digest()
            .map_err(|error| RegistryError::Undigestible(error.to_string()))?
            .as_str()
            .to_string();
        if replacement_digest == superseded {
            return Err(RegistryError::SelfSupersession(replacement_digest));
        }

        let digest = self.publish(replacement, tier, policy)?;
        let reason = reason.into();
        let sequence = self.next_sequence();
        self.statuses.insert(
            superseded.to_string(),
            PackStatus::Superseded {
                by: digest.clone(),
                reason: reason.clone(),
            },
        );
        self.log.push(PublicationEvent::Superseded {
            sequence,
            digest: superseded.to_string(),
            by: digest.clone(),
            reason,
        });
        Ok(digest)
    }

    /// Marks an artifact withdrawn. The bytes stay; the recommendation does not.
    ///
    /// 10.05: "raw public downloads may be disabled while metadata and reason remain visible".
    /// Here nothing is disabled, because there is nothing to serve — the artifact remains
    /// retrievable so that a historical result citing it can still be interpreted.
    pub fn withdraw(
        &mut self,
        digest: &str,
        reason: impl Into<String>,
    ) -> Result<(), RegistryError> {
        if !self.statuses.contains_key(digest) {
            return Err(RegistryError::UnknownDigest(digest.to_string()));
        }
        let reason = reason.into();
        let sequence = self.next_sequence();
        self.statuses.insert(
            digest.to_string(),
            PackStatus::Withdrawn {
                reason: reason.clone(),
            },
        );
        self.log.push(PublicationEvent::Withdrawn {
            sequence,
            digest: digest.to_string(),
            reason,
        });
        Ok(())
    }

    /// Every log entry touching a digest, oldest first.
    pub fn history(&self, digest: &str) -> Vec<&PublicationEvent> {
        self.log
            .iter()
            .filter(|event| event.digest() == digest)
            .collect()
    }

    /// Recomputes every stored artifact's attestation and checks it is filed under its own digest.
    ///
    /// The check a local registry needs after being read back from disk: a JSON file anyone can
    /// edit is not an immutable artifact layer until somebody re-derives the addresses.
    pub fn verify_all(&self) -> Vec<(String, Attestation)> {
        let mut broken = Vec::new();
        for (digest, document) in &self.artifacts {
            match BenchmarkPack::verify(document) {
                Attestation::Valid => {
                    let recomputed = document
                        .get(crate::pack::PACK_DIGEST_FIELD)
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if recomputed != digest {
                        broken.push((
                            digest.clone(),
                            Attestation::Mismatch {
                                claimed: digest.clone(),
                                recomputed: recomputed.to_string(),
                            },
                        ));
                    }
                }
                other => broken.push((digest.clone(), other)),
            }
        }
        broken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_registry_has_an_empty_log() {
        let registry = RegistryIndex::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.log().is_empty());
        assert!(registry.verify_all().is_empty());
        assert!(registry.resolve("nothing@0.1.0").is_none());
    }
}
