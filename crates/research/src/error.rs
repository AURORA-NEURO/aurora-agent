//! Every way a research run can refuse, in one typed vocabulary.
//!
//! The split the variants encode: a request defect is the caller's to fix
//! ([`ResearchError::InvalidRequest`]); a measurement the workspace cannot produce aborts the run
//! rather than being papered over with a partial dossier (`WorldRejected` through
//! `MinimizeFailed`); and a document handed back to this crate that is not what it claims to be
//! is refused at the door (`InvalidDossier`, `ArtifactMissing`, `ArtifactNotInlined`). There is
//! deliberately no variant for "step skipped": a step this runner cannot complete is an error,
//! never a silently thinner dossier.

/// Refusals and failures of the research runner.
#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    /// The request document failed validation. The reason names the rule, not just the field.
    #[error("invalid research request: {reason}")]
    InvalidRequest { reason: String },

    /// A value could not be canonicalised for hashing — a workspace invariant failure, not a
    /// request defect.
    #[error("canonicalisation failed: {reason}")]
    Canonicalisation { reason: String },

    /// The embedded reference fixture did not parse or compile. The fixtures are compiled into
    /// this crate, so this is a build-integrity failure, never a caller mistake.
    #[error("embedded reference fixture is unusable: {reason}")]
    ReferenceFixtureUnusable { reason: String },

    /// The embedded reference fixture compiled to a certificate whose digest is not the pinned
    /// cross-language parity value. The dossier's anchor would be a lie, so the run aborts.
    #[error(
        "the reference fixture no longer produces the pinned parity certificate digest: pinned \
         {pinned}, recomputed {recomputed}"
    )]
    ReferenceAnchorMismatch { pinned: String, recomputed: String },

    /// The generator emitted a world document `bioprism_world` rejects.
    #[error("generated world {world_id} does not load: {reason}")]
    WorldRejected { world_id: String, reason: String },

    /// The generator emitted a query document `bioprism_fiber` rejects.
    #[error("generated query for {world_id} does not load: {reason}")]
    QueryRejected { world_id: String, reason: String },

    /// The compiler refused a generated world/query pair.
    #[error("compile failed on {world_id}: {reason}")]
    CompileFailed { world_id: String, reason: String },

    /// A certificate this very run produced failed its own verification round-trip.
    #[error("certificate round-trip failed on {world_id}: {reason}")]
    CertificateRoundTrip { world_id: String, reason: String },

    /// The oracle refused the full-context reference, so the comparison has no completeness axis.
    /// Propagated rather than absorbed: `bioprism-baseline` aborts here instead of fabricating,
    /// and so does this runner.
    #[error("comparison on {world_id} has no reference verdict: {reason}")]
    NoReferenceVerdict { world_id: String, reason: String },

    /// The structural family sweep could not complete.
    #[error("sweep failed: {reason}")]
    SweepFailed { reason: String },

    /// The metamorphic suite could not run over the base world.
    #[error("mutation suite failed on {world_id}: {reason}")]
    MutationFailed { world_id: String, reason: String },

    /// The 1-minimal reduction could not run over the base world.
    #[error("minimization failed on {world_id}: {reason}")]
    MinimizeFailed { world_id: String, reason: String },

    /// A planned step referenced a distractor point no earlier step generated. The planner never
    /// produces such a protocol; hitting this means the executed steps and the plan diverged.
    #[error("protocol out of order: step {step} needs the generated world for {distractors} distractors")]
    ProtocolOutOfOrder { step: String, distractors: u32 },

    /// A document handed to [`crate::verify_dossier`] or [`crate::render_report`] is not a
    /// research dossier at all — wrong shape or wrong schema. Distinct from a dossier that *is*
    /// one and fails checks, which verification reports field by field.
    #[error("invalid research dossier: {reason}")]
    InvalidDossier { reason: String },

    /// The report renderer needed an artifact the dossier does not contain.
    #[error("artifact {name} is missing from the dossier")]
    ArtifactMissing { name: String },

    /// The report renderer needed an artifact's content, but the dossier holds only its digest.
    /// Never produced for dossiers this runner builds — every figure-source artifact is far below
    /// the inline cap — so this names a foreign or hand-edited dossier.
    #[error("artifact {name} (sha256 {digest}) is digest-only in the dossier; its figure cannot be rendered")]
    ArtifactNotInlined { name: String, digest: String },

    /// A figure renderer refused its input.
    #[error("figure for artifact {artifact} could not be rendered: {reason}")]
    FigureFailed { artifact: String, reason: String },
}
