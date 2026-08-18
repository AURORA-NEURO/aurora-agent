//! What is left of the developer platform and the reference examples, honestly sorted.
//!
//! Twenty blueprint modules were still uncited when this crate started: fourteen in the developer
//! platform section and six in reference examples. `bioprism-sdk` had already taken plugin
//! registration and capability declaration, `bioprism-devx` diagnostics and the retryability
//! taxonomy and compile introspection, `bioprism-cookbook` the verifiable recipes,
//! `bioprism-examples` the vertical slices. This crate read what remained and classified it before
//! writing a line of it.
//!
//! **Four of the twenty are implementable here. Sixteen are not, and the reason differs.**
//!
//! ```text
//! process                 3  the design describes what a person does at an interface
//! foreign artifact        3  code-bearing, but not Rust and not in this repository
//! covered elsewhere       10 an existing crate already owns the substance
//! implemented here        4  predicates over an artifact this crate defines
//! ```
//!
//! The middle bucket is the finding. It was expected — `crates/ops` needed the same category for
//! its section — but not at this size: four of twenty remaining modules specify a real, precise,
//! testable artifact that simply is not a Rust crate. A Python distribution with nine importable
//! packages and composite GitHub Actions evaluated in somebody else's repository. Those are not
//! vague and they are not process; they are elsewhere. The TypeScript and Python authoring clients
//! are now in-tree integration artifacts over authoritative Rust contracts, so they are covered
//! rather than counted as foreign. The [`mission`] module now composes those contracts with every
//! other callable domain surface: it supplies deterministic DAG planning and a least-authority,
//! refusal-preserving execution boundary without inventing a second domain ontology or claiming
//! distributed scheduling.
//! Calling them process would be as wrong as implementing them, and the difference matters to a
//! contributor deciding what to work on.
//!
//! The full table, with the sentence that decided each row, is [`classify::classification`]. The
//! sixteen unimplemented modules are named there **by title and never by id**, for a reason given
//! below.
//!
//! # The one idea
//!
//! *A tutorial that names an API is a claim about the workspace.* `bioprism-cookbook` proved that
//! pattern: a recipe's crate names and entry points resolve against the working tree as text, so a
//! rename turns the recipe suite red.
//!
//! This crate extends it in the direction the developer-platform section forces. Every claim a
//! recipe can hold is an in-tree claim — the type cannot express anything else. But a quickstart
//! for the Python SDK names `prism.compiler.mine`, and nothing in this repository can look for it,
//! now or ever. A catalogue with two evidence states records that as "unresolved", which is what
//! it also records for a symbol somebody deleted yesterday, and the two need opposite responses.
//!
//! So [`claim::Evidence`] has three states and the third is terminal. [`claim::ApiClaim`] checks
//! evidence against [`surface::Surface`] at construction: a Python API cannot carry in-tree
//! evidence, an in-tree crate cannot be excused as foreign, and "cannot be checked here" without a
//! reason is refused. [`walkthrough::Walkthrough::standing`] is then *derived*, so a document
//! cannot advertise itself as verified, and a document all of whose claims are foreign says so
//! ([`walkthrough::Walkthrough::documents_absent_artifact`]).
//!
//! ```
//! use bioprism_devplat::{standard_walkthroughs, Standing};
//!
//! let book = standard_walkthroughs()?;
//! let python = book.iter().find(|w| w.id().as_str() == "python-sdk-quickstart").unwrap();
//!
//! assert!(python.documents_absent_artifact());
//! assert!(matches!(python.standing(), Standing::EntirelyOutside { claims: 4 }));
//! assert_eq!(python.standing().guarded_claims(), 0);
//! # Ok::<(), bioprism_devplat::WalkthroughError>(())
//! ```
//!
//! Run over the quickstarts this section assumes, that produces the second finding: **the Python
//! distribution and consumer-repository CI runner remain external artifacts.** A green test run
//! here tells a reader nothing about an external package install or hosted runner. The in-tree
//! [`workbench`] module now supplies the implementable contract layer around that gap: it validates
//! authoring/notebook sessions and can generate a review-only CI plan, but it does not publish a
//! package, contact GitHub, or execute a runner. See [`walkthrough::standard_walkthroughs`].
//!
//! # The citation rule, made executable
//!
//! `tools/coverage.sh` counts a module as covered when its `NN.MM` token appears anywhere under
//! `crates/`. So writing the id of a module while explaining that it is process moves the coverage
//! figure without moving the platform — the one thing this workspace must not do.
//!
//! Two mechanisms, because a convention is not a mechanism. First, only
//! [`classify::Verdict::ImplementedHere`] has a `module_id` field; the other three variants carry a
//! title and structurally cannot hold an id. Second, [`citations::audit`] reimplements the coverage
//! script's token rule and scans this crate's own source, and a test asserts the tokens found are
//! exactly the four declared. Both mechanisms are cheap; the second is the one that would have
//! caught the mistake this crate nearly made, which was writing a measured percentage to two
//! decimal places whose integer part fell in the section range. Every figure below is written to
//! one decimal place for that reason.
//!
//! # The four that survived
//!
//! | id | subject | why it is a predicate and not a description |
//! |---|---|---|
//! | 11.23 | Reporting and Export Formats | "audience renderers change explanation depth ... but not values, uncertainty, lineage, or status" is a statement about a function |
//! | 19.12 | Scientific figure reproduction | "an immediate final conclusion is prohibited because the evidence obligation is unresolved" is a construction precondition |
//! | 19.17 | Evaluator exploit and security cell | "platform containment is not credited as correct task behavior" is an invariance claim |
//! | 19.22 | Scientific reproduction molecule | "the molecule never collapses these into one status" is a claim about an enum and the functions it does not have |
//!
//! [`report`] is the one-evidence-state projection: four audiences, one digest, and a
//! `banner_precedes_headline` ordering check. [`repro`] is the obligation ledger and the eight
//! statuses, where sealing a report refuses a verification conclusion while an obligation is open.
//! [`exploit`] is four axes that are never summed, with `task_verdict` and `intent_verdict` given
//! narrow signatures so that containment *cannot* leak into either.
//!
//! # What this crate does not do
//!
//! **No second registry, catalogue or diagnostic code space.** `bioprism-sdk` owns registration;
//! nothing here is discovered or dispatched to. `bioprism-devx` owns diagnostics; [`audit::Finding`]
//! reuses its `Site`, `Certainty` and `Remedy` and deliberately mints no `DiagnosticCode`, because
//! devx validates codes against its own namespace and its catalogue is the registry for them.
//! `bioprism-cookbook` owns recipes; this crate defines no recipe type, no anti-recipe and no
//! second `Check`, and reuses its `CrateName` and `Workspace` rather than walking the tree again.
//! [`audit::catalogues_are_disjoint`] and [`audit::recipes_are_all_in_tree`] check the boundary
//! instead of asserting it.
//!
//! **The original predicate layer does not execute external systems.** No report is written in any
//! format; a [`report::Rendering`] is an ordered list of sections, which is the level at which both
//! of 11.23's checkable rules live, and HTML, Markdown, Parquet and PDF are all absent. No
//! reproduction is attempted — [`repro`] is bookkeeping over results that `bioprism-oracle` and
//! `bioprism-evalengine` produce. No sandbox is enforced — [`exploit`]'s `Containment` is an
//! observation recorded so it can be *excluded* from a verdict, not a mechanism. The separate
//! [`workbench`] module does execute structural validation and deterministic projections, while
//! keeping notebook kernels, filesystems, GitHub, and CI runners outside its trust boundary.
//!
//! **No clock and no randomness.** Every digest is a function of its input alone.
//!
//! # Both sections measured
//!
//! Same method for both, stated because the figure moves with the definition. Strip trailing
//! whitespace, drop blank lines, count each remaining line *instance* rather than each distinct
//! text. An instance is scaffolding when its exact trimmed text also appears in at least one other
//! module of the same section.
//!
//! **Developer platform, 25 modules, 1,670 non-blank instances, mean 66.8 per module.** 1,168
//! instances are shared with at least one other module: 69.9%. Raising the threshold to *all 25
//! modules* drops it only to 1,125, or **67.4%** — the shared frame is a block, not a gradient.
//!
//! **Reference examples, 22 modules, 1,264 non-blank instances, mean 57.5 per module.** Only 188
//! instances are shared with any other module at all (14.9%), 107 with fifteen or more (8.5%), and
//! **5.2%** appear in all 22.
//!
//! Both published figures reproduce exactly: 67.4% and 5.2%. The reference-examples section is the
//! least templated in the blueprint by a wide margin, and the ratio between the two is the reason
//! this crate is shaped the way it is — one section hands down a contract, the other hands down
//! almost nothing.
//!
//! ## Sensitivity, and one refinement
//!
//! Three other definitions, so the numbers can be compared against somebody else's:
//!
//! | | developer platform | reference examples |
//! |---|---|---|
//! | distinct trimmed texts | 560, compression 2.98 | 1,086, compression 1.16 |
//! | markdown furniture (front matter, headings, fences) | 34.6% | 21.6% |
//! | mean pairwise line-set Jaccard | 0.483 | 0.036 |
//! | per-module unique share | 22.2% to 38.2% | 67.9% to 90.8% |
//! | mean distinguishing lines per module | 21.8 | 54.5 |
//!
//! One refinement to the published developer-platform result. The universal block is **45 line
//! instances in every one of the 25 files** and **40 distinct texts**, because two of those lines
//! repeat within each module: the front-matter delimiter twice, and the sentence *"Detect the
//! condition explicitly, fail closed where integrity or safety is affected..."* five times, once
//! under each failure mode. Counting instances gives 45 and counting distinct texts gives 40; both
//! are right and they answer different questions. The instance figure is the one that belongs in a
//! percentage, since the denominator is also instances.
//!
//! The same refinement applied to reference examples is stark. Its universal core is **3 instances,
//! 2 distinct texts**: the front-matter delimiter, twice, and the line `last_updated:
//! "2026-08-07"`. That is the entire shared contract of the section — a horizontal rule and a date.
//! Every design decision in this crate about what a reference example *is* had to be made here, and
//! each one is argued in the module that makes it rather than attributed to the blueprint.
//!
//! # Where the sections assume something they never specify
//!
//! - **"Not empty code alone."** The environment-authoring module says its scaffolding commands
//!   generate tests and documentation stubs, "not empty code alone". That is the closest thing in
//!   the remainder to a template-conformance predicate, and it is one clause: no template
//!   manifest, no required file set, no way to state which stub belongs to which generated file.
//!   There is nothing to check a generated tree against.
//! - **"One evidence state."** 11.23 requires every audience view to agree, and never says what a
//!   view is allowed to change. [`report::Depth`] is this crate's answer — explanation prose, and
//!   nothing else — and it is a choice, not a transcription.
//! - **A "comparability banner" with no position.** 11.23 says limitations appear "before showing
//!   headline differences" and never models the document, so "before" has no referent.
//!   [`report::Section`] supplies the smallest one that gives the word meaning.
//! - **Eight statuses and no lattice.** 19.22 lists eight reproduction outcomes and forbids
//!   collapsing them, without saying how to describe several sub-claims at once. [`repro::summarise`]
//!   returns `Conflicted` rather than a majority, which is a decision the blueprint does not make.
//! - **A release gate with no scoring rule.** 19.17's gate — no release is stable if a known
//!   adversarial agent can obtain positive reward without the intended state transition — is
//!   precise about the conjunction and silent about what "intended state transition" is a predicate
//!   over. [`exploit::SecurityCell`] requires it as a string a reader can evaluate, which is weaker
//!   than an executable predicate and stronger than nothing.
//!
//! # A note on the remaining six of the reference-example section
//!
//! `bioprism-cookbook` recorded six recipes it could not write, each naming a missing capability.
//! Those are not these six. The overlap is empty: cookbook's blockers are about abstention, backend
//! portfolios, mutation families, decision loss, a CLI with no library and a federated exchange.
//! The six modules this crate faced are worked examples of artifacts — a decision cell, a routing
//! decision, a weave program, a molecule, a figure reproduction, a security cell — and four of the
//! six turned out to belong to a crate that already owns the artifact. That asymmetry is worth
//! stating: a section can look two-thirds unimplemented while the platform underneath it is not.

pub mod adapter_execution_evidence;
pub mod adapter_execution_evidence_query;
pub mod artifact_registry;
pub mod audit;
pub mod capability;
pub mod capability_dashboard;
pub mod ci_evidence;
pub mod ci_provider;
pub mod ci_provider_evidence;
pub mod citations;
pub mod claim;
pub mod classify;
pub mod cross_domain_audit;
pub mod delivery_receipt;
pub mod domain_acquisition;
pub mod domain_evidence;
pub mod domain_evidence_intake;
pub mod domain_evidence_provider;
pub mod domain_evidence_provider_external;
pub mod domain_evidence_provider_external_execution;
pub mod domain_evidence_provider_external_lineage;
pub mod domain_evidence_provider_external_normalization;
pub mod domain_evidence_provider_external_query;
pub mod domain_evidence_provider_handoff;
pub mod domain_evidence_source;
pub mod domain_evidence_source_execution;
pub mod domain_report;
pub mod engineering;
pub mod engineering_plan;
pub mod error;
pub mod evaluator;
pub mod evidence_bundle;
pub mod evidence_registry;
pub mod execution_provenance;
pub mod exploit;
pub mod mission;
pub mod operational_readiness;
pub mod release_pipeline;
pub mod report;
pub mod repro;
pub mod sandbox_admission;
pub mod sandbox_runtime;
pub mod security_privacy;
pub mod security_program;
pub mod surface;
pub mod walkthrough;
pub mod workbench;
pub mod workflow;
pub mod workflow_execution_evidence;
pub mod workflow_reconciliation;
pub mod workflow_reconciliation_registry;

pub use adapter_execution_evidence::{
    record_adapter_execution_evidence, AdapterExecutionEvidenceRequest, AdapterExecutionLoss,
    ADAPTER_EXECUTION_EVIDENCE_SCHEMA, ADAPTER_EXECUTION_EVIDENCE_WORKFLOW,
    MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES, MAX_ADAPTER_EXECUTION_EVIDENCE_DOMAINS,
    MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS, MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES,
    MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS, MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES,
};
pub use adapter_execution_evidence_query::{
    query_adapter_execution_evidence, AdapterExecutionEvidenceJoinProjection,
    AdapterExecutionEvidenceQueryReport, AdapterExecutionEvidenceQueryRequest,
    AdapterExecutionEvidenceQueryRow, AdapterExecutionEvidenceQuerySummary,
    ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA, ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW,
    MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS,
};
pub use artifact_registry::{
    ArtifactRecord, ArtifactRegistry, ArtifactRegistryError, ARTIFACT_REGISTRY_GET_SCHEMA_VERSION,
    ARTIFACT_REGISTRY_LINEAGE_SCHEMA_VERSION, ARTIFACT_REGISTRY_QUERY_SCHEMA_VERSION,
    ARTIFACT_REGISTRY_REGISTER_SCHEMA_VERSION, ARTIFACT_REGISTRY_SCHEMA_VERSION,
    MAX_ARTIFACT_REGISTRY_BYTES, MAX_ARTIFACT_REGISTRY_DOMAINS,
    MAX_ARTIFACT_REGISTRY_LINEAGE_NODES, MAX_ARTIFACT_REGISTRY_PARENTS,
    MAX_ARTIFACT_REGISTRY_QUERY_ITEMS, MAX_ARTIFACT_REGISTRY_RECORDS,
};
pub use audit::{
    catalogues_are_disjoint, findings, recipes_are_all_in_tree, unimplemented_titles,
    DevPlatReport, Finding, WalkthroughSummary,
};
pub use capability::{
    CapabilityCatalogue, CapabilityError, CapabilityGroup, CapabilityMatch, CapabilityQuery,
    CapabilityRouteNeed, CapabilityRouteRequest, CapabilitySearch, CAPABILITY_SCHEMA_VERSION,
};
pub use capability_dashboard::{
    build_dashboard, CapabilityDashboardAudit, CapabilityDashboardError, CapabilityDashboardGroup,
    CapabilityDashboardQuery, CapabilityDashboardSurfaces, CAPABILITY_DASHBOARD_SCHEMA,
    DEFAULT_DASHBOARD_GROUPS, MAX_DASHBOARD_GROUPS,
};
pub use ci_evidence::{
    audit_ci_execution_evidence, CiCheckEvidence, CiCheckStatus, CiEvidenceError,
    CiEvidenceFinding, CiEvidenceSource, CiExecutionEvidenceAudit, CiExecutionEvidenceRequest,
    CiRunConclusion, CiRunEvidence, CI_EXECUTION_EVIDENCE_SCHEMA,
};
pub use ci_provider::{
    normalize_ci_provider_payload, CiProviderNormalization, CiProviderNormalizationError,
    CiProviderNormalizationRequest, CI_PROVIDER_NORMALIZATION_SCHEMA,
};
pub use ci_provider_evidence::{
    audit_ci_provider_evidence, CiProviderArtifact, CiProviderAttestation, CiProviderEvidenceAudit,
    CiProviderEvidenceError, CiProviderEvidenceRequest, CiProviderLog, CI_PROVIDER_EVIDENCE_SCHEMA,
};
pub use citations::{audit as audit_citations, scan as scan_citations, CitationAudit};
pub use claim::{ApiClaim, ApiClaimDraft, ApiName, Evidence};
pub use classify::{
    classification, implemented_module_ids, not_implemented, verdict_counts, ModuleVerdict, Verdict,
};
pub use cross_domain_audit::{
    build_cross_domain_audit, CROSS_DOMAIN_AUDIT_SCHEMA_VERSION, CROSS_DOMAIN_AUDIT_WORKFLOW,
    MAX_CROSS_DOMAIN_AUDIT_FINDINGS,
};
pub use delivery_receipt::{
    build_delivery_receipt, verify_delivery_receipt, DeliveryReceiptAudit, DeliveryReceiptEvidence,
    DeliveryReceiptFinding, DeliveryReceiptRequest, DeliveryReceiptTarget,
    DeliveryReceiptVerification, DeliveryReceiptVerificationRequest, DELIVERY_RECEIPT_SCHEMA,
};
pub use domain_acquisition::{
    build_domain_acquisition_catalogue, DomainAcquisitionCatalogue, DomainAcquisitionError,
    DomainAcquisitionGroup, DomainAcquisitionQuery, DomainAcquisitionRoute, DomainAdapterRoute,
    DomainInterpretationRoute, DomainTransportRoute, DOMAIN_ACQUISITION_SCHEMA_VERSION,
    DOMAIN_ACQUISITION_WORKFLOW, MAX_DOMAIN_ACQUISITION_ADAPTERS, MAX_DOMAIN_ACQUISITION_DOMAINS,
    MAX_DOMAIN_ACQUISITION_GROUPS,
};
pub use domain_evidence::{
    harmonize_domain_evidence, validate_domain_evidence_harmonization, DomainEvidenceError,
    DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_WORKFLOW, DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW, MAX_DOMAIN_EVIDENCE_HARMONIZATION_BYTES,
    MAX_DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_ITEMS, MAX_DOMAIN_EVIDENCE_LINKS,
    MAX_DOMAIN_EVIDENCE_REPORTS, MAX_DOMAIN_EVIDENCE_REQUIREMENTS, MAX_DOMAIN_EVIDENCE_TEXT_BYTES,
};
pub use domain_evidence_intake::{
    intake_domain_evidence, validate_domain_evidence_intake, DomainEvidenceIntakeError,
    DOMAIN_EVIDENCE_INTAKE_COVERAGE_SCHEMA_VERSION, DOMAIN_EVIDENCE_INTAKE_COVERAGE_WORKFLOW,
    DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION, DOMAIN_EVIDENCE_INTAKE_WORKFLOW,
    MAX_DOMAIN_EVIDENCE_INTAKE_BYTES, MAX_DOMAIN_EVIDENCE_INTAKE_DOMAINS,
    MAX_DOMAIN_EVIDENCE_INTAKE_PARENTS, MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES,
};
pub use domain_evidence_provider::{
    normalize_domain_evidence_provider, verify_domain_evidence_provider_replay,
    DomainEvidenceProviderNormalization, DomainEvidenceProviderNormalizationError,
    DomainEvidenceProviderNormalizationRequest, DomainEvidenceProviderRecordIndex,
    DomainEvidenceProviderReplayRequest, DomainEvidenceProviderReplayVerification,
    DomainEvidenceProviderShapeAudit, DomainEvidenceProviderShapeCoverage,
    DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA, DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_SCHEMA, DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW, DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA,
    MAX_DOMAIN_EVIDENCE_PROVIDER_BYTES, MAX_DOMAIN_EVIDENCE_PROVIDER_DOMAINS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_PARENTS, MAX_DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_ITEMS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_TEXT_BYTES,
};
pub use domain_evidence_provider_external::{
    record_domain_evidence_provider_external_payload,
    verify_domain_evidence_provider_external_payload_replay,
    DomainEvidenceProviderExternalPayloadError, DomainEvidenceProviderExternalPayloadReceipt,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest,
    DomainEvidenceProviderExternalPayloadReplayVerification,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW,
    MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES,
    MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_DOMAINS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_PARENTS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_TEXT_BYTES,
};
pub use domain_evidence_provider_external_execution::{
    audit_domain_evidence_provider_external_payload_execution,
    DomainEvidenceProviderExternalPayloadExecutionEvidence,
    DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_WORKFLOW,
};
pub use domain_evidence_provider_external_lineage::{
    audit_domain_evidence_provider_external_payload_lineage,
    DomainEvidenceProviderExternalPayloadLineageAudit,
    DomainEvidenceProviderExternalPayloadLineageAuditRequest,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_WORKFLOW,
};
pub use domain_evidence_provider_external_normalization::{
    normalize_domain_evidence_provider_external_payload,
    DomainEvidenceProviderExternalPayloadNormalization,
    DomainEvidenceProviderExternalPayloadNormalizationError,
    DomainEvidenceProviderExternalPayloadNormalizationRequest,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_WORKFLOW,
};
pub use domain_evidence_provider_external_query::{
    query_domain_evidence_provider_external_payload_evidence,
    DomainEvidenceProviderExternalPayloadEvidenceQueryReport,
    DomainEvidenceProviderExternalPayloadEvidenceQueryRequest,
    DomainEvidenceProviderExternalPayloadEvidenceRow,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW,
    MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS,
};
pub use domain_evidence_provider_handoff::{
    handoff_domain_evidence_provider, DomainEvidenceProviderAuthPosture,
    DomainEvidenceProviderConnectorManifest, DomainEvidenceProviderHandoff,
    DomainEvidenceProviderHandoffError, DomainEvidenceProviderHandoffRequest,
    DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA, DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA, MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_BYTES,
    MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CAPABILITIES,
    MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_DOMAINS, MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_PARENTS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS,
    MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_TEXT_BYTES,
};
pub use domain_evidence_source::{
    plan_domain_evidence_source, validate_domain_evidence_source_plan,
    DomainEvidenceSourcePlanError, DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW, MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_ALLOWED_HOSTS,
    MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES, MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES_LIMIT,
    MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_DOMAINS, MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_NON_CLAIMS,
    MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_PARENTS, MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES,
    MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TIMEOUT_MS,
};
pub use domain_evidence_source_execution::{
    execute_domain_evidence_source, DomainEvidenceSourceExecutionError,
    DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA_VERSION, DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW,
    MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_HEADER_BYTES,
    MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_PREVIEW_BYTES,
};
pub use domain_report::{
    classify_domain_report_bridge, project_domain_report, validate_domain_report,
    DomainReportBridgeMetadata, DomainReportError, ADAPTER_DOMAIN_REPORT_SCHEMA_VERSION,
    ADAPTER_DOMAIN_REPORT_WORKFLOW, DOMAIN_REPORT_COVERAGE_SCHEMA_VERSION,
    DOMAIN_REPORT_COVERAGE_WORKFLOW, DOMAIN_REPORT_PROJECT_SCHEMA_VERSION,
    DOMAIN_REPORT_PROJECT_WORKFLOW, DOMAIN_REPORT_SCHEMA_VERSION, MAX_DOMAIN_REPORT_BYTES,
    MAX_DOMAIN_REPORT_DOMAINS, MAX_DOMAIN_REPORT_LIMITATIONS, MAX_DOMAIN_REPORT_NON_CLAIMS,
    MAX_DOMAIN_REPORT_PARENTS, MAX_DOMAIN_REPORT_TEXT_BYTES, PROVIDER_DOMAIN_REPORT_SCHEMA_VERSION,
    PROVIDER_DOMAIN_REPORT_WORKFLOW,
};
pub use engineering::{
    AdrSpec, AdrStatus, AdrSupersession, EngineeringAudit, EngineeringCounts, EngineeringError,
    EngineeringIssue, EngineeringManifest, EngineeringPolicies, IssueSeverity, OwnershipSpec,
    PackageSpec, ProjectIdentity, TechnologyBaseline, TicketReadiness, TicketSpec, TicketStatus,
    ENGINEERING_AUDIT_SCHEMA, ENGINEERING_MANIFEST_SCHEMA,
};
pub use engineering_plan::{
    EngineeringPlanAudit, EngineeringPlanError, EngineeringPlanGate, EngineeringPlanPolicies,
    EngineeringPlanRequest, EngineeringPlanWave, EngineeringTicketPlan,
    ENGINEERING_PLAN_AUDIT_SCHEMA, ENGINEERING_PLAN_REQUEST_SCHEMA, MAX_PLAN_PARALLELISM,
    MAX_PLAN_TICKETS,
};
pub use error::{
    CitationError, ClaimError, DevPlatError, ExploitError, ReportError, ReproError, SurfaceError,
    WalkthroughError,
};
pub use evaluator::{
    EvaluatorError, MissionEvaluatorAdapter, MissionEvaluatorCatalogue, MissionEvaluatorMatch,
    MissionEvaluatorQuery, MissionEvaluatorReplayCompareRequest, MissionEvaluatorReplayRequest,
    MissionEvaluatorReviewRequest, MissionEvaluatorSearch, MissionEvaluatorSelection,
    MISSION_EVALUATOR_CATALOGUE_SNAPSHOT_SCHEMA_VERSION,
    MISSION_EVALUATOR_REPLAY_COMPARE_SCHEMA_VERSION, MISSION_EVALUATOR_SCHEMA_VERSION,
};
pub use evidence_bundle::{
    verify_mission_evidence_bundle, EvidenceBundleError, MAX_EVIDENCE_BUNDLE_VERIFY_BYTES,
    MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION, MISSION_EVIDENCE_BUNDLE_VERIFY_SCHEMA_VERSION,
};
pub use evidence_registry::{
    EvidenceBundleRegistry, EvidenceRegistryError, EVIDENCE_REGISTRY_IMPORT_SCHEMA_VERSION,
    EVIDENCE_REGISTRY_QUERY_SCHEMA_VERSION, EVIDENCE_REGISTRY_SCHEMA_VERSION,
    MAX_EVIDENCE_REGISTRY_BUNDLES, MAX_EVIDENCE_REGISTRY_BYTES, MAX_EVIDENCE_REGISTRY_QUERY_ITEMS,
};
pub use execution_provenance::{
    audit_execution_provenance, DelegatedCheckEvidence, ExecutionProvenanceAudit,
    ExecutionProvenanceFinding, ExecutionProvenanceRequest, EXECUTION_PROVENANCE_SCHEMA,
    MAX_DELEGATED_CHECKS, MAX_FINDINGS,
};
pub use exploit::{
    intent_verdict, release_gate, standard_remediations, task_verdict, CellScore, Containment,
    GateOutcome, IntentVerdict, Remediation, Reward, SecurityCell, ServiceState, TamperAttempt,
    TaskVerdict,
};
pub use mission::{
    apply_binding, mission_claim_lineage, mission_claim_lineage_with_review, plan_mission,
    route_review_provenance, validate_route_review_provenance, MissionBinding,
    MissionClaimEvaluatorBinding, MissionClaimRequest, MissionError, MissionPlan, MissionPolicy,
    MissionReport, MissionRequest, MissionStep, MissionStepPlan, MissionStepResult,
    MissionTraceEvent, MissionTraceObserver, MAX_CLAIM_EVALUATORS, MAX_CLAIM_REFERENCES,
    MAX_CLAIM_REQUESTS, MAX_WORKFLOW_BINDING_BYTES, MISSION_SCHEMA_VERSION,
    MISSION_TRACE_SCHEMA_VERSION,
};
pub use operational_readiness::{
    DependencyCriticality, IncidentSeverity, IncidentState, IndicatorStatus, OperationalContract,
    OperationalContractKind, OperationalControls, OperationalCriticality, OperationalDependency,
    OperationalDependencyAudit, OperationalIncident, OperationalIncidentAudit,
    OperationalIndicator, OperationalIndicatorAudit, OperationalIssueSeverity,
    OperationalReadinessAudit, OperationalReadinessCounts, OperationalReadinessError,
    OperationalReadinessIssue, OperationalReadinessManifest, OperationalReadinessPolicies,
    OperationalRunbook, OperationalRunbookAudit, OperationalService, RunbookReviewStatus,
    OPERATIONAL_READINESS_AUDIT_SCHEMA, OPERATIONAL_READINESS_MANIFEST_SCHEMA,
};
pub use release_pipeline::{
    EnvironmentClass, PipelineArtifact, PipelineArtifactKind, PipelineAttestation,
    PipelineAttestationKind, PipelineEnvironment, PipelineIssueSeverity, PipelineProject,
    PipelinePromotion, PipelinePromotionAudit, PipelinePromotionKind, PipelineSource,
    PipelineStage, PipelineStageKind, PipelineStageReadiness, ReleasePipelineAudit,
    ReleasePipelineCounts, ReleasePipelineError, ReleasePipelineIssue, ReleasePipelineManifest,
    ReleasePipelinePolicies, RELEASE_PIPELINE_AUDIT_SCHEMA, RELEASE_PIPELINE_MANIFEST_SCHEMA,
};
pub use report::{
    drifted_figures, render, render_all, Audience, Depth, EvidenceState, Figure, FigureStatus,
    Limitation, RenderedFigure, Rendering, Section, SourcePointer, Uncertainty,
};
pub use repro::{
    figure_reproduction_case, forbidden_by_default, summarise, Effect, MoleculeCard, Obligation,
    ObligationLedger, ObligationStatus, ReproductionReport, ReproductionStatus,
};
pub use sandbox_admission::{
    SandboxArtifact, SandboxArtifactAudit, SandboxArtifactKind, SandboxAudit, SandboxBoundaryAudit,
    SandboxCapability, SandboxCapabilityAudit, SandboxCapabilityKind, SandboxDecision,
    SandboxError, SandboxExecutionProfile, SandboxIssue, SandboxIssueSeverity, SandboxManifest,
    SandboxMount, SandboxMountMode, SandboxNetworkMode, SandboxOutput, SandboxOutputAudit,
    SandboxPolicies, SandboxProfileAudit, SandboxResourceAudit, SandboxResourceLimits,
    SandboxSystem, SandboxTrust, SANDBOX_AUDIT_SCHEMA, SANDBOX_MANIFEST_SCHEMA,
};
pub use sandbox_runtime::{
    SandboxRuntimeAudit, SandboxRuntimeDecision, SandboxRuntimeError, SandboxRuntimeIssue,
    SandboxRuntimeManifest, SandboxRuntimePolicies, SandboxRuntimeRequest, SandboxRuntimeStepAudit,
    SandboxRuntimeUsage, SANDBOX_RUNTIME_AUDIT_SCHEMA, SANDBOX_RUNTIME_MANIFEST_SCHEMA,
};
pub use security_privacy::{
    SecurityPrivacyAsset, SecurityPrivacyAssetAudit, SecurityPrivacyAudit,
    SecurityPrivacyClassification, SecurityPrivacyControlAudit, SecurityPrivacyControls,
    SecurityPrivacyCounts, SecurityPrivacyError, SecurityPrivacyFlow, SecurityPrivacyFlowAudit,
    SecurityPrivacyFlowDecision, SecurityPrivacyIdentity, SecurityPrivacyIdentityAudit,
    SecurityPrivacyIssue, SecurityPrivacyIssueSeverity, SecurityPrivacyManifest,
    SecurityPrivacyPolicies, SecurityPrivacyReview, SecurityPrivacyReviewAudit,
    SecurityPrivacyReviewKind, SecurityPrivacyReviewStatus, SecurityPrivacySystem,
    SecurityPrivacyThreat, SecurityPrivacyThreatAudit, SecurityPrivacyThreatSeverity,
    SecurityPrivacyThreatStatus, SECURITY_PRIVACY_AUDIT_SCHEMA, SECURITY_PRIVACY_MANIFEST_SCHEMA,
};
pub use security_program::{
    SecurityProgramAudit, SecurityProgramCampaign, SecurityProgramCampaignAudit,
    SecurityProgramCampaignStatus, SecurityProgramControlAudit, SecurityProgramControls,
    SecurityProgramDisclosure, SecurityProgramDisclosureAudit, SecurityProgramDisclosureStage,
    SecurityProgramError, SecurityProgramFinding, SecurityProgramFindingAudit,
    SecurityProgramFindingSeverity, SecurityProgramFindingStatus, SecurityProgramIncident,
    SecurityProgramIncidentAudit, SecurityProgramIncidentStatus, SecurityProgramIssue,
    SecurityProgramIssueSeverity, SecurityProgramManifest, SecurityProgramPolicies,
    SecurityProgramRemediation, SecurityProgramRemediationAudit, SecurityProgramRemediationStatus,
    SecurityProgramScope, SecurityProgramScopeAudit, SecurityProgramScopeKind,
    SecurityProgramSystem, SecurityProgramTimelineEvent, SECURITY_PROGRAM_AUDIT_SCHEMA,
    SECURITY_PROGRAM_MANIFEST_SCHEMA,
};
pub use surface::{foreign_subjects, ForeignSubject, Locale, Surface, SurfaceKind};
pub use walkthrough::{
    recheck, standard_walkthroughs, Standing, Step, StepBody, Walkthrough, WalkthroughDraft,
    WalkthroughId,
};
pub use workbench::{
    audit_session, plan_ci, query_dashboard, run_workbench, ArtifactCard, ArtifactState, CellInput,
    CellKind, ChangeKind, CiCheck, CiPlan, CiRequest, DashboardQuery, DashboardReport,
    DashboardRow, EvidencePosture, NotebookPolicy, SessionAudit, StudioCell, StudioChange,
    StudioSession, WorkbenchError, WorkbenchFinding, WorkbenchReport, WorkbenchRequest,
    WORKBENCH_SCHEMA_VERSION,
};
pub use workflow::{
    build_domain_workflow_catalogue, instantiate_domain_workflow, scaffold_domain_workflow,
    verify_domain_workflow, DomainWorkflowError, DOMAIN_WORKFLOW_CATALOGUE_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION, DOMAIN_WORKFLOW_INSTANTIATE_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_SCAFFOLD_SCHEMA_VERSION, DOMAIN_WORKFLOW_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_VERIFY_SCHEMA_VERSION, MAX_DOMAIN_WORKFLOW_BYTES, MAX_DOMAIN_WORKFLOW_GROUPS,
    MAX_DOMAIN_WORKFLOW_STEPS, MAX_DOMAIN_WORKFLOW_TOOLS,
};
pub use workflow_execution_evidence::{
    build_workflow_execution_evidence, validate_workflow_execution_evidence,
    WorkflowExecutionEvidenceError, WorkflowExecutionEvidenceRegistry,
    MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES, MAX_WORKFLOW_EXECUTION_EVIDENCE_CAPABILITIES,
    MAX_WORKFLOW_EXECUTION_EVIDENCE_DOMAINS, MAX_WORKFLOW_EXECUTION_EVIDENCE_PARENTS,
    MAX_WORKFLOW_EXECUTION_EVIDENCE_QUERY_ITEMS, MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS,
    WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_REGISTRY_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION, WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW,
};
pub use workflow_reconciliation::{
    reconcile_domain_workflow, DomainWorkflowReconcileError,
    DOMAIN_WORKFLOW_RECONCILE_SCHEMA_VERSION, MAX_DOMAIN_WORKFLOW_RECONCILE_BYTES,
    MAX_DOMAIN_WORKFLOW_RECONCILE_FINDINGS, MAX_DOMAIN_WORKFLOW_RECONCILE_STEPS,
};
pub use workflow_reconciliation_registry::{
    DomainWorkflowReconciliationRegistry, DomainWorkflowReconciliationRegistryError,
    DOMAIN_WORKFLOW_RECONCILIATION_IMPORT_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_RECONCILIATION_QUERY_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION,
    DOMAIN_WORKFLOW_RECONCILIATION_SUMMARY_SCHEMA_VERSION, MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
    MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES, MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS,
};
