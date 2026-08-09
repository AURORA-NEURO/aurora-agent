//! The provenance a view cannot be built without.
//!
//! Blueprint 43.01, runtime contract: "Every graph or hypergraph view carries the source Decision
//! Section and Context Certificate IDs." This module makes that structural rather than
//! conventional. [`ProjectionSource`] has no public fields and exactly one constructor,
//! [`ProjectionSource::bind`], which computes both digests itself from the objects it is handed.
//! There is no way to assert a digest; you can only present the section and the certificate and
//! let this type hash them.
//!
//! `bind` additionally refuses a certificate that attests a *different* section. Pairing a clean
//! certificate with a dirty section would otherwise be the cheapest way to publish a view that
//! looks accountable and is not.
//!
//! Deliberately absent: `Deserialize`. A [`ProjectionSource`] is produced by hashing, never
//! parsed. Re-hydrating one from JSON would recreate exactly the "assert your own provenance"
//! path the type exists to close; a consumer reading a serialised view re-derives trust with
//! [`crate::View::verify`] against the section and certificate it holds.

use crate::error::ProjectionError;
use bioprism_ids::ContentHash;
use bioprism_section::{CertificateProfile, ContextCertificate, DecisionSection};
use serde::Serialize;

/// Immutable identity of the compiled region a view was projected from.
///
/// Carries both halves of 43.01's traceability requirement: the ids (world, query, decision cut)
/// that say *which* region, and the digests (world, query, section, certificate) that say
/// *which version* of it. A view holding this can always be walked back to the compiled region
/// that justifies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionSource {
    world_id: String,
    query_id: String,
    decision_time: String,
    world_sha256: String,
    query_sha256: String,
    section_sha256: String,
    certificate_sha256: String,
    certificate_profile: &'static str,
}

impl ProjectionSource {
    /// Binds a section to the certificate that attests it.
    ///
    /// Both digests are recomputed here rather than accepted from the caller. The three
    /// consistency checks — same world, same query, and a certificate whose
    /// `source_hashes.decision_section_sha256` equals the section's own digest — are what make a
    /// bound source evidence of anything at all.
    pub fn bind(
        section: &DecisionSection,
        certificate: &ContextCertificate,
        profile: CertificateProfile,
    ) -> Result<Self, ProjectionError> {
        if certificate.world_id != section.world_id {
            return Err(ProjectionError::IdentityMismatch {
                field: "world_id",
                certificate: certificate.world_id.clone(),
                section: section.world_id.clone(),
            });
        }
        if certificate.query_id != section.query_id {
            return Err(ProjectionError::IdentityMismatch {
                field: "query_id",
                certificate: certificate.query_id.clone(),
                section: section.query_id.clone(),
            });
        }

        let section_digest = section.content_hash()?;
        let attested = &certificate.source_hashes.decision_section_sha256;
        if attested != section_digest.as_str() {
            return Err(ProjectionError::CertificateAttestsAnotherSection {
                attested: attested.clone(),
                actual: section_digest.as_str().to_string(),
            });
        }

        Ok(ProjectionSource {
            world_id: section.world_id.clone(),
            query_id: section.query_id.clone(),
            decision_time: section.decision_time.clone(),
            world_sha256: certificate.source_hashes.world_sha256.clone(),
            query_sha256: certificate.source_hashes.query_sha256.clone(),
            section_sha256: section_digest.as_str().to_string(),
            certificate_sha256: certificate.digest(profile)?.as_str().to_string(),
            certificate_profile: profile_name(profile),
        })
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    /// The temporal cut the region was compiled at. A view inherits it; it never picks its own.
    pub fn decision_time(&self) -> &str {
        &self.decision_time
    }

    pub fn world_sha256(&self) -> &str {
        &self.world_sha256
    }

    pub fn query_sha256(&self) -> &str {
        &self.query_sha256
    }

    pub fn section_sha256(&self) -> &str {
        &self.section_sha256
    }

    pub fn certificate_sha256(&self) -> &str {
        &self.certificate_sha256
    }

    pub fn certificate_profile(&self) -> &'static str {
        self.certificate_profile
    }

    /// Re-derives both digests from the objects a consumer holds and reports whether they agree.
    ///
    /// The consumer-side twin of `bind`: the same recomputation a certificate reader performs in
    /// 43.26, applied to a view.
    pub fn recheck(
        &self,
        section: &DecisionSection,
        certificate: &ContextCertificate,
        profile: CertificateProfile,
    ) -> Result<ProvenanceCheck, ProjectionError> {
        let section_digest = section.content_hash()?;
        if section_digest.as_str() != self.section_sha256 {
            return Ok(ProvenanceCheck::SectionDigestMismatch {
                bound: self.section_sha256.clone(),
                recomputed: section_digest.as_str().to_string(),
            });
        }
        let certificate_digest = certificate.digest(profile)?;
        if certificate_digest.as_str() != self.certificate_sha256 {
            return Ok(ProvenanceCheck::CertificateDigestMismatch {
                bound: self.certificate_sha256.clone(),
                recomputed: certificate_digest.as_str().to_string(),
            });
        }
        Ok(ProvenanceCheck::Matches)
    }

    /// Confirms the section has not drifted since binding, quoting both digests when it has.
    pub(crate) fn guard_unchanged(&self, section: &DecisionSection) -> Result<(), ProjectionError> {
        let actual: ContentHash = section.content_hash()?;
        if actual.as_str() == self.section_sha256 {
            Ok(())
        } else {
            Err(ProjectionError::SectionMutatedAfterBinding {
                bound: self.section_sha256.clone(),
                actual: actual.as_str().to_string(),
            })
        }
    }
}

/// Outcome of re-deriving a view's provenance from the objects it claims to come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ProvenanceCheck {
    Matches,
    SectionDigestMismatch { bound: String, recomputed: String },
    CertificateDigestMismatch { bound: String, recomputed: String },
}

impl ProvenanceCheck {
    pub fn is_match(&self) -> bool {
        matches!(self, ProvenanceCheck::Matches)
    }
}

fn profile_name(profile: CertificateProfile) -> &'static str {
    match profile {
        CertificateProfile::Reference => "reference",
        CertificateProfile::Extended => "extended",
    }
}
