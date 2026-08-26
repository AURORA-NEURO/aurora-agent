//! The projection interface.
//!
//! Blueprint 43.01 asks for exactly one implementation artifact from this layer: a "projection
//! interface for graph, hypergraph, table, and timeline renderers". [`Projection`] is that
//! interface, and its shape encodes the normative decision rather than merely accompanying it.
//!
//! Two properties are structural, not documented conventions:
//!
//! 1. **A view cannot exist without provenance.** [`View`] has private fields and one
//!    constructor, `seal`, which is crate-private. The only routes to a `View` from outside this
//!    crate are [`ProjectRegion::project`], which takes a [`ProjectionSource`] by value, and
//!    [`crate::BoundSection::project`], which holds one. Neither can be overridden: `project`
//!    lives on an extension trait whose blanket implementation covers every [`Projection`], so a
//!    renderer supplies [`Projection::render`] and nothing else.
//! 2. **A projection takes no relevance parameter.** `project` receives a Decision Section and
//!    provenance — nothing else. 43.01 defines completeness "against query obligations and
//!    protected closure, not neighborhood radius", so there is no `k_hop`, `depth`, `radius` or
//!    `max_nodes` argument anywhere in this crate's public surface. What is relevant was decided
//!    by the compiler when it built the region; a view that could re-decide it would be a second,
//!    unaudited relevance policy.
//!
//! What this crate does *not* do: render. There is no layout, no coordinate assignment, no colour,
//! no HTML and no web UI here. These types are the data a renderer would draw, which is why every
//! status is a typed field rather than a visual cue — 40.29 requires that "color is never the only
//! encoding", and the cheapest way to guarantee that is never to emit colour at all.

use crate::error::ProjectionError;
use crate::fidelity::{FidelityLedger, FidelityReport};
use crate::provenance::{ProjectionSource, ProvenanceCheck};
use bioprism_section::{CertificateProfile, ContextCertificate, DecisionSection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The four projections 43.01 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    Graph,
    Hypergraph,
    Timeline,
    Table,
}

impl ProjectionKind {
    pub const ALL: [ProjectionKind; 4] = [
        ProjectionKind::Graph,
        ProjectionKind::Hypergraph,
        ProjectionKind::Timeline,
        ProjectionKind::Table,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProjectionKind::Graph => "graph",
            ProjectionKind::Hypergraph => "hypergraph",
            ProjectionKind::Timeline => "timeline",
            ProjectionKind::Table => "table",
        }
    }
}

impl fmt::Display for ProjectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A body that can name the source handles it makes recoverable.
///
/// 43.01's invariant — "every projection is reversible to stable source handles even when it is
/// not information-lossless" — is only checkable if a view can say which handles it exposes.
/// [`crate::roundtrip`] compares that set against the section.
pub trait ProjectedBody {
    fn stable_handles(&self) -> BTreeSet<String>;
}

/// A generated view, inseparable from the compiled region it came from.
///
/// `Serialize` only, deliberately. A view is an output: something a renderer or an accessibility
/// layer consumes. Deriving `Deserialize` would mint a Rust value carrying provenance that was
/// never computed, which is precisely the state [`ProjectionSource`] exists to make unreachable.
/// A consumer that receives a serialised view and wants to trust it re-derives the digests with
/// [`View::verify`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct View<B> {
    kind: ProjectionKind,
    source: ProjectionSource,
    fidelity: FidelityReport,
    body: B,
}

impl<B> View<B> {
    pub(crate) fn seal(
        kind: ProjectionKind,
        source: ProjectionSource,
        fidelity: FidelityReport,
        body: B,
    ) -> Self {
        View {
            kind,
            source,
            fidelity,
            body,
        }
    }

    pub fn kind(&self) -> ProjectionKind {
        self.kind
    }

    /// The ids and digests of the Decision Section and Context Certificate this view was
    /// projected from.
    pub fn source(&self) -> &ProjectionSource {
        &self.source
    }

    pub fn fidelity(&self) -> &FidelityReport {
        &self.fidelity
    }

    pub fn body(&self) -> &B {
        &self.body
    }

    /// Recomputes this view's provenance against the section and certificate a consumer holds.
    pub fn verify(
        &self,
        section: &DecisionSection,
        certificate: &ContextCertificate,
        profile: CertificateProfile,
    ) -> Result<ProvenanceCheck, ProjectionError> {
        self.source.recheck(section, certificate, profile)
    }
}

impl<B: ProjectedBody> View<B> {
    /// The source handles this view makes recoverable.
    pub fn stable_handles(&self) -> BTreeSet<String> {
        self.body.stable_handles()
    }
}

/// A functorial view over a compiled decision region.
///
/// Implementors supply [`Projection::render`], which builds the body and writes to the loss
/// ledger, and nothing else. They never touch provenance and never seal the view:
/// [`ProjectRegion::project`] threads the source through, guards that the section has not drifted
/// since binding, and refuses to close a ledger that dropped an obstruction. Forgetting any of
/// those three is not possible by omission *or* by intent — `project` is not a method of this
/// trait, so there is nothing here to override.
pub trait Projection {
    /// The rendered body. Serialisable because the whole point of a view is to leave the process.
    type Body: ProjectedBody + Serialize;

    /// Which of 43.01's four projections this is.
    const KIND: ProjectionKind;

    /// Builds the body, recording every flattening and every carried obstruction in `ledger`.
    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<Self::Body, ProjectionError>;
}

/// The guarded entry point, on a trait an implementor cannot reach.
///
/// [`Projection`] is the interface a renderer writes. This is the one a caller invokes, and the
/// separation is what makes the drift guard unskippable rather than merely provided: the single
/// blanket implementation below covers every `Projection` there will ever be, so a second
/// implementation of `project` for any concrete renderer is a coherence error rather than an
/// override. The guard is not a default an implementor may decline; it is the only body that
/// exists.
///
/// It was a provided method on `Projection` before. Nothing in the crate overrode it and nothing
/// outside could produce the [`View`] such an override would have to return, so the hazard was
/// latent rather than live — but "latent" is the state a guard is in right before it stops
/// running, and the fix costs one import at each call site.
///
/// Sealed by construction rather than by convention: `Projection` is this trait's supertrait and
/// the blanket impl is unconditional, so implementing `ProjectRegion` directly is impossible and
/// there is nothing to seal against.
pub trait ProjectRegion: Projection {
    /// Projects a compiled region into a view bound to the provenance it came from.
    ///
    /// Note the argument list: a section and its provenance. There is no relevance knob, because
    /// relevance was already decided by the compiler under protected closure.
    ///
    /// The section is re-hashed here and compared against the digest the source was bound with,
    /// because a detached [`ProjectionSource`] carries no evidence that the section still holds the
    /// bytes that produced it. A caller taking several views of one region pays that once by
    /// holding a [`crate::BoundSection`] instead.
    fn project(
        &self,
        section: &DecisionSection,
        source: ProjectionSource,
    ) -> Result<View<Self::Body>, ProjectionError>;
}

impl<P: Projection + ?Sized> ProjectRegion for P {
    fn project(
        &self,
        section: &DecisionSection,
        source: ProjectionSource,
    ) -> Result<View<Self::Body>, ProjectionError> {
        source.guard_unchanged(section)?;
        render_sealed(self, section, source)
    }
}

/// The half of `project` that runs once the binding is established: render, close, seal.
///
/// Crate-private, and it must stay that way. Publishing it would hand callers a route to a sealed
/// [`View`] whose source was never checked against the section it claims — the exact state
/// [`ProjectionSource`] exists to make unreachable. It has two callers, and each has already
/// discharged that obligation in its own way: [`ProjectRegion::project`] by running the guard, and
/// [`crate::BoundSection`] by holding a borrow that makes drift impossible.
pub(crate) fn render_sealed<P: Projection + ?Sized>(
    projection: &P,
    section: &DecisionSection,
    source: ProjectionSource,
) -> Result<View<P::Body>, ProjectionError> {
    let mut ledger = FidelityLedger::default();
    let body = projection.render(section, &mut ledger)?;
    let fidelity = ledger.seal(P::KIND, section)?;
    Ok(View::seal(P::KIND, source, fidelity, body))
}
