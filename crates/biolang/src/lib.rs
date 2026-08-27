//! The biological IR family and BioQL.
//!
//! Implements the thirteen modules of blueprint section 25 that `bioprism-bioir` does not:
//! 25.01 BioWorld ([`world`]), 25.02 BioState ([`state`]), 25.06 Intervention and Action
//! ([`intervention`]), 25.07 Falsifiable Biological Contract ([`fbc`]), 25.09 BioWorldline
//! ([`worldline`]), 25.14 Model/Pipeline/Agent ([`system`]), 25.15 BioWeave Role and Act ([`act`]),
//! 25.16 BioContext Capsule ([`capsule`]), 25.17 BioCapability Molecule ([`molecule`]),
//! 25.18 BioOracle Mesh ([`oracle`]), 25.19 BioMutation ([`mutation`]), 25.20 BioResult Bundle
//! ([`bundle`]), and 25.21 BioQL ([`bioql`]).
//!
//! # What `bioprism-bioir` discharges
//!
//! `bioprism-bioir` owns 25.04 (specimen and aliquot lineage), 25.05 (AssayLens), 25.11 (biological
//! evidence object), 25.12 (uncertainty and reference standards) and 25.13 (cohort, eligibility and
//! split), plus the closure constraint of 39.05. Where this crate needs an evidence id, a specimen
//! id or a cohort, it uses that crate's types. It does not define a second `Quantity`, a second
//! measurement scale or a second uncertainty vocabulary.
//!
//! Five of §25's twenty-three modules are therefore covered elsewhere, thirteen are covered here,
//! and five are covered by nobody in this crate: 25.03 (entity and ontology binding, which
//! `bioprism-standards` covers as a standards layer rather than as an IR), 25.08 (BioDecision Cell,
//! which `bioprism-prism` implements), 25.10 (translation spine), 25.22 (schema versioning and
//! migration) and 25.23 (conformance suite).
//!
//! # Units, frames and builds come from `bioprism-standards`
//!
//! There is no unit system in this crate. `bioprism-bioir` refuses cross-unit arithmetic with a
//! `UnitMismatch`; `bioprism-standards` supplies the closed unit table, the dimensional analysis,
//! the coordinate frames, the genome builds, the ontology bindings, and
//! [`bioprism_standards::comparable`], which returns the first blocking dimension. BioQL's type
//! checker is a thin framing around that function: it decides *which two things* are being compared
//! and hands the decision to `comparable`. A third unit system would be a third thing to keep in
//! agreement with CPython, the eager Rust path and the indexed store.
//!
//! # What is deliberately not implemented
//!
//! A missing capability that is stated is a limitation; one that is implied to exist is a lie.
//!
//! - **No execution engine, no optimiser, no planner.** A BioQL query is lexed, parsed and typed
//!   here, and then it stops. Nothing reads a row, opens a store, estimates a cardinality or chooses
//!   an access path. The cost model is a declared base cost times a syntactic predicate count; it
//!   bounds what a query is *permitted* to cost and says nothing about what it would cost.
//! - **No permission filter.** 25.21 lists one. What is enforced is that a query *declared* its
//!   access labels, so something downstream has a label to filter on. A declaration is not an
//!   enforcement.
//! - **No ontology hierarchy.** [`bioql::ExpansionPolicy`] records which policy is in force. Without
//!   the ontology loaded, expanding `descendants` is not possible and inferring subsumption would be
//!   fabrication — the same position `bioprism-standards` takes.
//! - **No schema migration.** 25.22 owns versioning. Adding a field to an IR changes every digest
//!   taken from it and there is no migration path here.
//! - **No conformance fixtures.** 25.23 owns those. The digests computed here have not been
//!   reconciled against a reference implementation in another language.
//! - **No world immutability.** 25.01 requires a published world version to be immutable. That is a
//!   registry property; a struct cannot enforce it. What is provided is the digest that makes a
//!   mutation detectable.
//!
//! # Constructs §25 names but never specifies
//!
//! Four, recorded here rather than silently filled in:
//!
//! 1. **25.07 "scoring".** The module title is "claims, evidence obligations, allowed actions,
//!    falsifiers, scope, and scoring" and the body defines no scale, weighting or aggregation.
//!    [`fbc`] implements everything except scoring.
//! 2. **25.09 `decision_time`.** Required on a worldline; 25.02 gives a state only `event_time` and
//!    `record_time`, so a decision-ordered worldline has no field to order on.
//!    [`worldline::Worldline::orders_on_a_carried_clock`] reports this rather than hiding it.
//! 3. **25.14 "context policy" and "memory".** Both listed as required field groups with no shape.
//!    They are carried as strings.
//! 4. **25.20 "signatures".** The implementing bundle crate now offers an additive Ed25519
//!    public-key path as well as symmetric authentication. This blueprint IR still models the
//!    legacy HMAC-shaped [`bundle::Attestation`] only: it carries a MAC tag and a
//!    [`bundle::Repudiability`], and there is no field called `signature`. Projection tests refuse
//!    to coerce the stronger public-key value into that weaker shape.
//!
//! # How much of §25 is boilerplate, and how that was measured
//!
//! The section's 23 module files (24 with the index) share a large fixed skeleton: a six-step
//! Lifecycle, a "Security and provenance" paragraph, a five-step Implementation sequence and four
//! Release gates, all byte-identical everywhere, plus identical section headings, front-matter keys
//! and table scaffolding.
//!
//! One number would be misleading, so here are three, over the 23 spec modules, counting non-blank
//! lines after trimming:
//!
//! | Definition | Result |
//! |---|---|
//! | lines under the four byte-identical sections only | **31.3%** (460/1471) |
//! | lines whose exact text appears in ≥2 files | **55.2%** (812/1471) |
//! | the same, weighted by bytes rather than lines | **65.8%** (38498/58521) |
//!
//! Sensitivity, which is the part a single number hides:
//!
//! - **The repetition threshold barely matters.** Raising it from ≥2 files to ≥12 or to all 23 moves
//!   the line figure from 55.2% to 53.2% — 2.0 points. The shared text is near-universal, not
//!   pairwise; 33 distinct lines appear in all 23 modules.
//! - **The unit matters more.** Lines say 53–55%, bytes say 65–66%, an 11-point spread, because the
//!   repeated lines are long prose sentences while the module-specific lines are short table rows.
//! - **The definition matters most.** 31.3% against 53.2% is a 22-point gap between "the four
//!   identical sections" and "any line that repeats", and the gap is made of headings, front-matter
//!   keys and the `| --- | --- |` table scaffolding — real repetition that a stricter definition
//!   discards.
//! - **The corpus matters at the edge.** Adding the index file moves the ≥23-file figure by only
//!   0.9 points (53.2% to 52.3%), but the "present in *every* file" figure falls from 53.2% over the
//!   23 modules to 9.6% over all 24, because the index shares almost nothing with them. Any
//!   "identical everywhere" metric is one unlike file away from meaninglessness.
//!
//! So: **§25 is 31% to 66% boilerplate depending on definition and unit**, and the honest short form
//! is the middle one — about 53% of non-blank lines are text that appears in at least twelve of the
//! twenty-three modules. The consequence for this crate is that the *distinctive* content of a §25
//! module is roughly its Purpose, its Primary objects list, its Required-fields table and its three
//! or four Invariants — which is exactly what each module here implements, and why so much of the
//! prose in this crate is about what the blueprint did *not* say.

pub mod act;
pub mod bioql;
pub mod bundle;
pub mod canonical;
pub mod capsule;
pub mod clock;
pub mod error;
pub mod fbc;
pub mod ids;
pub mod intervention;
pub mod molecule;
pub mod mutation;
pub mod oracle;
pub mod projection;
pub mod publication_copilot;
pub mod quality_workbench;
pub mod contract_frontier_workbench;
pub mod retrieval_assurance;
pub mod span;
pub mod state;
pub mod system;
pub mod world;
pub mod worldline;

pub use canonical::{round_trips, Canonical};
pub use clock::{Clock, Stamped};
pub use projection::{Projection, ProjectionGap};
pub use publication_copilot::{
    prepare_publication_queue, publication_copilot_manifest, PublicationCopilotError,
    PublicationCopilotReceipt, PublicationCopilotRequest, PublicationDisposition, RunState,
    SignedResearchObject, ValidatedResearchRun,
    CONTRACT_VERSION as PUBLICATION_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as PUBLICATION_COPILOT_FEATURE_ID,
};
pub use span::Span;

pub use act::{
    BioRole, CommitmentType, EvidenceTransition, ScientificAct, ScientificActKind,
    SpecimenTransition,
};
pub use bioql::{compile, parse, BioType, CollectionDecl, FieldDecl, QuerySchema, TypedQuery};
pub use bundle::{Attestation, Repudiability, ResultBundle, RunManifest, Score};
pub use capsule::{Assumption, BioContextCapsule, Omission, Staleness, Stance};
pub use error::{
    ActError, BundleIrError, CapsuleError, FbcError, InterventionError, IrError, LexError,
    MoleculeError, MutationIrError, OracleIrError, ParseError, QueryError, StateError, SystemError,
    TypeError, WorldError, WorldlineError,
};
pub use fbc::{
    ClaimSchema, EvidenceObligation, Falsifier, Fbc, Intent, TerminalState, Termination,
};
pub use ids::{
    ActId, ActionId, AssetId, ComponentId, FbcId, MoleculeId, MutationId, ObligationId, StateId,
    SystemId, WorldlineId,
};
pub use intervention::{
    ActionClass, ActionDefinition, AuthorityRequirement, CostModel, Effect, Idempotence,
    Precondition, Reversibility,
};
pub use molecule::{
    CapabilityEvidence, Choreography, FailureSemantics, Guarantee, Molecule, NestedInterface,
    RoleBinding, Step,
};
pub use mutation::{MutationProgram, Risk, SeedDeclaration, SemanticRelation, Transformation};
pub use oracle::{DisagreementIr, EvidencePlane, EvidenceTier, OracleIr, Verdict};
pub use quality_workbench::{
    operate_quality_workbench, MetricDirection, QualityDisposition, QualityObservation,
    QualityState, QualityWorkbenchError, QualityWorkbenchReceipt, QualityWorkbenchRequest,
    QualityWorkbenchSummary, CONTRACT_VERSION as QUALITY_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as QUALITY_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY as QUALITY_WORKBENCH_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as QUALITY_WORKBENCH_SCHEMA_VERSION,
};
pub use contract_frontier_workbench::{
    build_contract_frontier_manifest, validate_contract_frontier,
    BiolangCapabilityManifest, BiolangContractInput, ContractFrontierError,
    CONTRACT_VERSION as CONTRACT_FRONTIER_CONTRACT_VERSION,
    FEATURE_ID as CONTRACT_FRONTIER_FEATURE_ID,
};
pub use retrieval_assurance::{
    assure_retrieval_synthesis, RetrievalAssuranceDisposition, RetrievalAssuranceError,
    RetrievalAssuranceReceipt, RetrievalAssuranceRequest, RetrievalAssuranceSummary,
    RetrievalCandidate, RetrievalEvidenceState,
    CONTRACT_VERSION as RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY as RETRIEVAL_ASSURANCE_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as RETRIEVAL_ASSURANCE_SCHEMA_VERSION,
};
pub use state::{BioState, Plane, ResourceLedger, Transition, UncertaintySummary};
pub use system::{Component, ComponentKind, Pin, PromptDisclosure, SystemManifest, Wire};
pub use world::{
    Asset, BioWorld, CatalogEntry, CounterfactualGrade, HiddenItem, LicensePolicy, SemanticVersion,
    VisibleState, WorldClass,
};
pub use worldline::{AlignmentConfidence, Branch, Censoring, RevealGate, Revision, Worldline};
