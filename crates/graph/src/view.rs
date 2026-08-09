//! The projection interface.
//!
//! Blueprint 43.01 asks for exactly one implementation artifact from this layer: a "projection
//! interface for graph, hypergraph, table, and timeline renderers". [`Projection`] is that
//! interface, and its shape encodes the normative decision rather than merely accompanying it.
//!
//! Two properties are structural, not documented conventions:
//!
//! 1. **A view cannot exist without provenance.** [`View`] has private fields and one
//!    constructor, `seal`, which is crate-private. The only route to a `View` from outside this
//!    crate is [`Projection::project`], which takes a [`ProjectionSource`] by value. An external
//!    implementor may override `project`, but cannot construct the type it would have to return.
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

    pub fn into_body(self) -> B {
        self.body
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
/// ledger. They never touch provenance and never seal the view: [`Projection::project`] threads
/// the source through, guards that the section has not drifted since binding, and refuses to
/// close a ledger that dropped an obstruction. Forgetting any of those three is therefore not
/// possible by omission — it would take deliberately reimplementing `project`, which cannot
/// produce a `View` anyway.
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

    /// Projects a compiled region into a view bound to the provenance it came from.
    ///
    /// Note the argument list: a section and its provenance. There is no relevance knob, because
    /// relevance was already decided by the compiler under protected closure.
    fn project(
        &self,
        section: &DecisionSection,
        source: ProjectionSource,
    ) -> Result<View<Self::Body>, ProjectionError> {
        source.guard_unchanged(section)?;
        let mut ledger = FidelityLedger::default();
        let body = self.render(section, &mut ledger)?;
        let fidelity = ledger.seal(Self::KIND, section)?;
        Ok(View::seal(Self::KIND, source, fidelity, body))
    }
}
