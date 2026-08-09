//! Typed projection failures.
//!
//! Blueprint 40.28 requires a projection to fail with a named class rather than degrade quietly,
//! and 43.01 makes two of those classes non-negotiable: a view may not be bound to provenance it
//! does not actually come from, and a view may not hide an obstruction. Both are errors here, not
//! warnings, because a caller that ignores a warning ships a view that lies.

use crate::view::ProjectionKind;
use bioprism_ids::CanonicalError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectionError {
    /// The section or certificate could not be canonicalised, so no digest can be quoted.
    #[error("canonical serialisation failed while digesting the projection source: {0}")]
    Canonicalisation(#[from] CanonicalError),

    /// The certificate names a different Decision Section than the one being projected.
    ///
    /// This is the forgery guard. Without it a caller could pair a favourable certificate with an
    /// unfavourable section and publish a view that appears attested.
    #[error(
        "certificate attests decision section {attested} but the section supplied digests to {actual}"
    )]
    CertificateAttestsAnotherSection { attested: String, actual: String },

    /// The certificate and the section disagree about which world or query they belong to.
    #[error("certificate {field} is {certificate:?} but the decision section {field} is {section:?}")]
    IdentityMismatch {
        field: &'static str,
        certificate: String,
        section: String,
    },

    /// The section changed between binding provenance and rendering the view.
    #[error("the decision section changed after provenance was bound: bound {bound}, now {actual}")]
    SectionMutatedAfterBinding { bound: String, actual: String },

    /// A projection finished without carrying every unresolved obligation into its body.
    ///
    /// 43.25 puts obligations ahead of narrative; a view that drops one is the exact failure the
    /// projection layer exists to prevent.
    #[error(
        "{kind} projection carried {carried} of {expected} unresolved obligations; \
         a view may not hide an obligation"
    )]
    ObligationDropped {
        kind: ProjectionKind,
        expected: usize,
        carried: usize,
    },

    /// A projection finished without carrying every oracle witness into its body.
    #[error(
        "{kind} projection carried {carried} of {expected} oracle conflict witnesses; \
         a view may not hide a conflict"
    )]
    ConflictDropped {
        kind: ProjectionKind,
        expected: usize,
        carried: usize,
    },

    /// A selected factor document lacked the fields a factor view needs.
    ///
    /// The factor is echoed verbatim into the Decision Section, so a malformed one means the
    /// compiler emitted something unprojectable; guessing a shape here would invent structure.
    #[error("selected factor at index {index} is not projectable: {detail}")]
    MalformedFactor { index: usize, detail: String },

    /// The decision cut could not be read, so no entry can be classified as available or withheld.
    #[error("decision time {value:?} is not an RFC 3339 instant: {detail}")]
    UnreadableDecisionTime { value: String, detail: String },

    /// 41.03: unknown edge types fail validation rather than passing through as free text.
    #[error("{value:?} is not a member of the normative edge vocabulary (41.03)")]
    UnknownEdgeType { value: String },
}
