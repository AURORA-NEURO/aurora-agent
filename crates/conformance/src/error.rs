//! Typed conformance failures.
//!
//! Blueprint 40.31 and 40.32 each enumerate their failure modes — fixture drift, nondeterministic
//! result, partial unsupported requirement, test passes wrong version — and 40.36 requires those
//! modes to be a documented taxonomy rather than a string. Every variant here names one of them.
//!
//! One asymmetry keeps the suite meaningful. A *case* that fails is data: it lands in the report
//! as [`crate::CaseOutcome::Failed`] and the run continues, because a conformance report that
//! stops at the first failure tells a vendor almost nothing. A *suite* that cannot honestly run
//! is an error and aborts. Fixture drift is deliberately the second kind: if the inputs are not
//! the inputs the expectations were written against, every subsequent verdict is meaningless, so
//! it must not be reported as a tidy row of failures.

use std::fmt;
use thiserror::Error;

/// Why an override that adds a key could not be applied to a fixture.
///
/// A typed enum rather than a formatted string, because 40.36 asks for a documented failure
/// taxonomy and because these are four different authoring mistakes with four different fixes.
/// [`crate::case::OverrideOp::Insert`] is the operation that can hit them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertObstacle {
    /// The pointer is not of the form `/…/key`, so it names no parent and no key to add.
    NamesNoParentAndKey,
    /// Nothing exists at the parent location. An insert adds one key; it does not build a path,
    /// because a case that silently conjured `/budgets` into a query without one would be testing
    /// a document shape no fixture ever had.
    ParentAbsent,
    /// The parent location exists but is not an object, so there is nothing to add a key to.
    ParentIsNotAnObject,
    /// The key is already there. The case says the fixture lacks it and the fixture has grown it,
    /// so the variant no longer differs from the baseline in the way the case claims.
    KeyAlreadyPresent,
}

impl fmt::Display for InsertObstacle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertObstacle::NamesNoParentAndKey => {
                f.write_str("the pointer names no parent and no key; an insert needs /…/key")
            }
            InsertObstacle::ParentAbsent => {
                f.write_str("the document has nothing at the parent location")
            }
            InsertObstacle::ParentIsNotAnObject => {
                f.write_str("the parent location is not an object")
            }
            InsertObstacle::KeyAlreadyPresent => f.write_str(
                "the key is already there, so the variant no longer differs from the baseline",
            ),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConformanceError {
    /// A case names a fixture the manifest does not register.
    ///
    /// 40.33 requires fixtures to be declared with provenance and checksum, so an unregistered
    /// file is not a fixture: loading it anyway is how a suite starts testing against whatever
    /// happens to be on disk.
    #[error("fixture {id:?} is not registered in the fixture manifest")]
    UnknownFixture { id: String },

    #[error("fixture {id:?} could not be read from {path}: {message}")]
    FixtureUnreadable {
        id: String,
        path: String,
        message: String,
    },

    #[error("fixture {id:?} at {path} is not valid JSON: {message}")]
    FixtureNotJson {
        id: String,
        path: String,
        message: String,
    },

    /// A fixture no longer hashes to its declared digest.
    ///
    /// `detail` carries the declared and observed digests together with a structural summary of
    /// what moved, because "checksum mismatch" alone sends a reviewer to a 332 KB JSON file with
    /// no idea what to look for.
    #[error("golden fixture {id:?} drifted from its declared digest: {detail}")]
    FixtureDrifted { id: String, detail: String },

    /// A case's override targets a location that does not exist in the fixture.
    ///
    /// Silently creating the path would let a case mutate a document into a shape the fixture
    /// never had, which is the "fixture accidentally trivial" failure of 40.33 arriving by a
    /// different door.
    #[error("case {case:?} overrides JSON pointer {pointer:?}, which is absent from the {document} document")]
    PointerNotFound {
        case: String,
        pointer: String,
        document: String,
    },

    /// A case's override adds a key, and the fixture cannot take it.
    ///
    /// The mirror image of [`ConformanceError::PointerNotFound`], and kept apart from it for the
    /// same reason: a replace that finds nothing and an insert that finds something are both the
    /// case having lost track of the fixture, but they are opposite edits and a reviewer told the
    /// wrong one goes looking in the wrong direction.
    #[error("case {case:?} inserts JSON pointer {pointer:?} into the {document} document, and {obstacle}")]
    OverrideNotInsertable {
        case: String,
        pointer: String,
        document: String,
        obstacle: InsertObstacle,
    },

    #[error("suite document is malformed: {0}")]
    MalformedSuite(String),

    #[error("canonical encoding failed: {0}")]
    Canonical(String),

    /// Certification was requested for a report that did not fully pass.
    ///
    /// 40.32 invariant 1: conformance is not trust or accuracy. It is also not partial credit —
    /// a certificate that can be issued over failures certifies nothing.
    #[error("certification refused: {reason}")]
    CertificationRefused { reason: String },

    #[error(
        "conformance certificate digest mismatch: claims {claimed}, recomputes to {recomputed}"
    )]
    CertificateDigestMismatch { claimed: String, recomputed: String },

    /// The claimed digest is not a digest at all.
    ///
    /// Separate from [`ConformanceError::CertificateDigestMismatch`] because the two accuse
    /// different parties. A mismatch says the certified body has been edited since it was signed;
    /// this says the field beside the body does not hold a SHA-256 digest, which is a defect in
    /// the claim and evidence of nothing about the body.
    #[error(
        "conformance certificate digest is malformed: {claimed:?} is not a 64-character lowercase \
         hex digest"
    )]
    CertificateDigestMalformed { claimed: String },
}
