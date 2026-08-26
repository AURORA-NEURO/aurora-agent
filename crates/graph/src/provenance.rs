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
//!
//! Two types live here, and the difference between them is the difference between an answer and a
//! guarantee. A [`ProjectionSource`] is a detached record of a binding that *was* true; checking
//! that it is still true costs a full canonicalisation and SHA-256 of the section, every time it is
//! asked. A [`BoundSection`] is a binding that is *still* live, because it holds the section
//! borrowed, and so answers the same question for free and for as many projections as the caller
//! takes from it.

use crate::error::ProjectionError;
use crate::view::{render_sealed, Projection, View};
use bioprism_ids::ContentHash;
use bioprism_section::{CertificateProfile, ContextCertificate, DecisionSection};
use serde::Serialize;

/// Counts calls to [`section_digest`] on the current thread.
///
/// Exists so that "how many times does this path canonicalise the section?" is a question a test
/// can answer by measurement instead of by reading the code and hoping. Thread-local rather than
/// global because cargo runs tests concurrently, and every digest a test provokes happens on that
/// test's own thread.
#[cfg(test)]
pub(crate) mod section_digest_calls {
    use std::cell::Cell;

    thread_local! {
        static CALLS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        CALLS.with(|calls| calls.set(calls.get() + 1));
    }

    pub(crate) fn reset() {
        CALLS.with(|calls| calls.set(0));
    }

    pub(crate) fn count() -> usize {
        CALLS.with(Cell::get)
    }
}

/// Canonicalises and hashes a Decision Section.
///
/// Every section digest this crate takes goes through this one function, which is what makes the
/// cost countable rather than merely asserted.
fn section_digest(section: &DecisionSection) -> Result<ContentHash, ProjectionError> {
    #[cfg(test)]
    section_digest_calls::record();
    Ok(section.content_hash()?)
}

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

        let section_digest = section_digest(section)?;
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
        let section_digest = section_digest(section)?;
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
    ///
    /// The full cost of a detached source: canonicalise the whole section again and hash it again,
    /// because an owned [`ProjectionSource`] carries no evidence about the object it came from.
    /// [`BoundSection`] is the same question asked once and then held.
    pub(crate) fn guard_unchanged(&self, section: &DecisionSection) -> Result<(), ProjectionError> {
        let actual: ContentHash = section_digest(section)?;
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

/// A section whose binding is still live, so drift is impossible rather than merely detected.
///
/// [`ProjectionSource`] owns its strings and carries no lifetime, which is what lets a
/// [`crate::View`] take it out of the process — and also what makes it *detached*. Nothing in its
/// type says which section produced it, or that that section still holds the bytes that were
/// hashed, so [`ProjectionSource::guard_unchanged`] must canonicalise and hash the section again
/// to answer both questions. A bundle that reads one compiled region four ways answers them four
/// times over, on bytes that provably cannot have changed in between.
///
/// This type answers them once, in the type system. It holds `&'a DecisionSection` next to the
/// source bound from it, so for as long as the binding lives:
///
/// - the source cannot have been paired with some other section — the reference *is* the section
///   it was bound to; and
/// - the section cannot have changed — a shared borrow excludes `&mut` for all of `'a`, and no
///   field reachable from a `DecisionSection` has interior mutability.
///
/// That is stronger than the runtime guard, not weaker.
/// [`ProjectionError::SectionMutatedAfterBinding`] is a state this path cannot reach, rather than
/// one it has stopped checking for — which is exactly why memoising the section's own
/// `content_hash` would be the wrong fix: a memo makes the *check* stop working while leaving the
/// mutation possible. Here the mutation is what becomes impossible.
///
/// The runtime guard therefore stays on [`crate::ProjectRegion::project`], where the caller hands
/// over a detached source and the question is genuinely open, and on [`BoundSection::rebind`],
/// which is that same check paid once to promote a detached source back into a live binding.
#[derive(Debug, Clone)]
pub struct BoundSection<'a> {
    section: &'a DecisionSection,
    source: ProjectionSource,
}

impl<'a> BoundSection<'a> {
    /// Binds a section to the certificate that attests it, keeping the section borrowed.
    ///
    /// The single section digest a projection run needs. It is spent on the check that carries the
    /// weight — that the certificate attests *this* section — and the borrow it takes is what
    /// makes every later re-derivation redundant rather than merely repeated.
    pub fn bind(
        section: &'a DecisionSection,
        certificate: &ContextCertificate,
        profile: CertificateProfile,
    ) -> Result<Self, ProjectionError> {
        let source = ProjectionSource::bind(section, certificate, profile)?;
        Ok(BoundSection { section, source })
    }

    /// Re-establishes a binding from a detached source, refusing if the section drifted.
    ///
    /// A caller holding an owned [`ProjectionSource`] has, by construction, let the borrow that
    /// guaranteed stillness expire, so the digest must be recomputed. Once. From then on the
    /// returned binding covers every projection taken from it.
    pub fn rebind(
        section: &'a DecisionSection,
        source: ProjectionSource,
    ) -> Result<Self, ProjectionError> {
        source.guard_unchanged(section)?;
        Ok(BoundSection { section, source })
    }

    /// The section this binding holds. Shared, because handing out `&mut` would defeat the type.
    pub fn section(&self) -> &'a DecisionSection {
        self.section
    }

    pub fn source(&self) -> &ProjectionSource {
        &self.source
    }

    /// Releases the provenance from the binding, for a caller that needs it past `'a`.
    ///
    /// The result is detached again, and a projection taken with it pays the guard again. That is
    /// correct rather than unfortunate: outside the borrow, drift is possible.
    pub fn into_source(self) -> ProjectionSource {
        self.source
    }

    /// Projects this region one way, without re-deriving what the binding already establishes.
    ///
    /// Runs the same tail as [`crate::ProjectRegion::project`] — render, close the loss ledger,
    /// seal — having skipped only the guard the borrow makes unnecessary. Skipping it is safe here
    /// and nowhere else: the borrow held by this type is what makes drift impossible, and a caller
    /// who cannot produce that borrow cannot reach this method.
    ///
    /// Nothing a renderer writes can intercept either path. `project` is not a method of
    /// [`Projection`]; it lives on [`crate::ProjectRegion`], whose blanket implementation covers
    /// every projection there will ever be, so the guarded route cannot be replaced and this
    /// unguarded one cannot be reached by replacing it.
    pub fn project<P: Projection>(&self, projection: &P) -> Result<View<P::Body>, ProjectionError> {
        render_sealed(projection, self.section, self.source.clone())
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
