//! Protocol conformance and the security properties of 11.11.

use bioprism_adapter::{TabularProfile, ValueType, VariableMapping};
use bioprism_adaptive::{AdaptivePanel, PanelConfig};
use bioprism_atlas::{
    Atlas, CapabilityDimension, CapabilityFamily, CapabilityId, CapabilityNode, CapabilityOntology,
    CausalChain, Detectability, EvidenceRecord, EvidenceStatus, EvidenceTier, FailureAxes,
    FailureLabel, FailureMechanism, FailureRecord, Inducement, LabelDistribution, OracleTier,
    Reversibility, Severity, TrialOutcome, UnmeasuredReason, WeightingPolicy,
};
use bioprism_bioethics::action::{ActionKind, ActionPlan, Authorisation, PlannedStep};
use bioprism_bioethics::dualuse::{CapabilityRelease, MisuseSurface, SurfaceAssessment};
use bioprism_bioethics::humansubject::{EngagementKind, ReturnOfResults, StudyDescription};
use bioprism_bioethics::representation::{
    ContextAxis, Stratum as BioethicsStratum, StratumCoverage, StratumObservation,
};
use bioprism_bioethics::validation::{
    EvidenceKind, EvidenceRecord as BioethicsEvidenceRecord, ValidationDossier,
};
use bioprism_bioeval::{Dispersion, ReferenceDistribution, ReferenceStandard};
use bioprism_bioevalx::repro::{Observed, OutputSpec, Reexecution};
use bioprism_bioevalx::trajectory::{PathProperty, Step, Trajectory};
use bioprism_bioevalx::worldline::{Decision, Observation as EvalObservation, Worldline};
use bioprism_biolang::{BioType, CollectionDecl, QuerySchema};
use bioprism_bundle::{
    AttestationPurpose, ClaimedProducer, EntryRole, KeyIdentity, KeyRegistry, KeyRole, KeyValidity,
    PubliclyAttestedBundle, RegisteredKey, ResultBundle, SigningKey, TrustPolicy,
};
use bioprism_devplat::{
    build_domain_workflow_catalogue, instantiate_domain_workflow, plan_mission,
    reconcile_domain_workflow, MissionRequest,
};
use bioprism_evalengine::{
    compose, Conclusion, Contribution, CoverageFloor, Observation, ReleaseGate, ScoreTier,
    UnknownPolicy,
};
use bioprism_fabric::synth::{Candidate as FabricCandidate, Goal as FabricGoal, RoleGraph};
use bioprism_factory::{Idempotency, Job, JobStore, ResourceClass, WorkerCapability};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_governance::SchemaVersion;
use bioprism_hub::{
    AccessTier, Board, BoardId, BudgetEnvelope, BuildProvenance, ComparabilityConditions,
    DeclaredScope, Entry, Epoch as HubEpoch, EvidenceScale, Licence, NonClaim,
    Provenance as HubProvenance, Score as HubScore, SubmissionDraft, SubmissionId, Submitter,
    SubmitterId, VerificationStatus,
};
use bioprism_hubapi::{
    Authority, Catalog, Facet, Federation, Namespace, PackName, PackRelease, Query, RegistryId,
    Request as HubRequest, Version, VersionReq,
};
use bioprism_ids::ContentHash;
use bioprism_ids::RunId;
use bioprism_infra::{Check as QualityCheck, Dataset as QualityDataset, Gate as QualityGate};
use bioprism_lab::{
    space::{CandidateArchitecture, ComponentKind, ComponentSpec},
    AcquisitionAction, AcquisitionCost, AcquisitionKind, PrivacyBoundary,
};
use bioprism_ledger::{
    Actor as LedgerActor, Event as LedgerEvent, EventClass as LedgerEventClass,
    EventKind as LedgerEventKind, EventTimes as LedgerEventTimes, IdempotencyKey,
    RecordTime as LedgerRecordTime, SubjectKey as LedgerSubjectKey, TemporalCut,
    ValidTime as LedgerValidTime,
};
use bioprism_mcp::rpc::code;
use bioprism_mcp::{
    serve, tool_definitions, Lifecycle, Request, Server, ADAPTIVE_QUERY_SCHEMA_URI,
    CAPABILITIES_URI, CERTIFICATE_SCHEMA_URI, PROTOCOL_VERSION,
};
use bioprism_megafactory::{
    AccessTier as PlacementAccessTier, Attestation, Locale, TrustDomain, WorkRequest, WorkerProfile,
};
use bioprism_metrics::{
    CapabilityGrid, GridCell, MeasurementConditions, NoIntervalReason, ScoringRule,
    Subject as MetricsSubject,
};
use bioprism_modalities::{
    ClaimKind as ModalityClaimKind, EvaluationHorizon, EvidenceTier as LiteratureEvidenceTier,
    LiteratureClaim, ModalMeasurement, Modality, Resolution, RetractionStatus, SourceProvenance,
};
use bioprism_obligation::{
    Action as ObligationAction, Obligation, ObligationGraph, ObligationPredicate, ObligationState,
    RegretClass, StateRecord,
};
use bioprism_onco::{
    AcquisitionTime, AvailabilityTime, BoundaryRequest, ClinicalObservation, ClinicalTrend, Clocks,
    Compartment, ConsentBasis, DirectionOfChange, EndpointKind, FollowUp, Histology,
    ImagingModality, ImagingObservation, Karnofsky, MarkerCall, MarkerPanel, MolecularMarker,
    Observation as OncoObservation, ObservationStatus, Observed as OncoObserved, OutputUse,
    Population, ProgressionEvidence, RequestContext, ResponseCriterion, SubjectRef, TargetLesion,
    TerminalFact, Timepoint, TreatmentContext, TreatmentModality, TumourWorldline,
};
use bioprism_oncoworlds::{
    Artifact as OncoArtifact, ArtifactLevel, Calibration, CellularFraction, ClaimTarget,
    ClassifierVersion, ClonalHistory, CohortSelection, DeclaredTransport, DetectionSensitivity,
    DiseaseEpoch, EstablishmentCohort, EvaluationDesign, FidelityAxis, FidelityEvidence,
    FractionDerivation, FractionEvidence, MethylationClass, MethylationOutcome, ModelIdentity,
    ModelResult, ModelSystem, Pseudonym, QcOutcome, RadiogenomicClaim, RawScore, RegionId,
    ReplicateStructure, SampleContext, ScoreValue, SpecimenObservation, SpecimenSampling,
    SplitUnit, Subclone, SubcloneId, TumourPopulation, VersionedResult,
};
use bioprism_ops::{
    ArtifactHandling, Assumption, Bound, CapacityModel, Concession, DegradationPlan, Demand,
    Operation, Workload,
};
use bioprism_ops::{
    Derivation as OpsDerivation, DomainEvent, Field as OpsField, MetricDefinition,
    Observations as OpsObservations, RedactionPolicy, Sample as OpsSample, SignalId,
    Treatment as OpsTreatment,
};
use bioprism_oracle::{
    Confidence, EvidenceTier as OracleEvidenceTier, Judgement, OracleId, OracleManifest, OracleRef,
    OracleVersion, Plane, Position, UtcTimestamp, ValidityWindow,
};
use bioprism_oraclex::missing::{AbsencePattern, Boundary, Field, MissingnessMechanism};
use bioprism_oraclex::panel::{Adjudication, Blinding, ConsensusRule, Read, ReaderPanel};
use bioprism_packs::{
    AgentCapability, DifficultyCalibration, Domain, InstanceSource, OracleTier as PackOracleTier,
    PackAxis, PackContent, PackId, PackIr, PackManifest, PackVersion, ParentEnvironment,
    SchemaRange, SystemObservation, WorldId,
};
use bioprism_policy::{Consent, Purpose, PurposeSet};
use bioprism_registry::{BenchmarkPack, TrustTier};
use bioprism_routing::{
    ApprovedSet, Architecture as RoutingArchitecture, EvidenceLedger, Fingerprint,
    Observation as RoutingObservation, RoutingPolicy,
};
use bioprism_runtime::{
    BudgetPlan, EffectKind, EffectPolicy, EffectRequest, Limit, RuntimeResource, WorldTape,
};
use bioprism_safety::release::{Rating, RiskAssessment, RiskDimension, SensitiveCategory};
use bioprism_scale::corpus::GeneratedItem;
use bioprism_scope::ScopeClass;
use bioprism_scope::{ScopeKey, Timestamp};
use bioprism_sdk::{
    AbiGrade, Capability, CapabilityKind, Determinism, PluginManifest, Priority, RegistryPolicy,
};
use bioprism_section::OracleStatus;
use bioprism_standards::{Measurement, OntologyId, Quantity, TermBinding, Unit};
use bioprism_stewardship::id::Actor;
use bioprism_stewardship::review::{
    full_corpus, EvaluatorRevision, Finding as StewardshipFinding, ReviewDimension, ReviewRecord,
};
use bioprism_stress::{Cohort, Knob, Magnitude, Procedure, Stress, Subject};
use bioprism_worldfactory::contradiction::{
    Discordance, DiscordanceClass, DiscriminatingAction, Hypothesis, Lens, ModalityId, Reading,
    ReadingValue, ReferenceDiscordance, Reported, SpatialExtent,
};
use bioprism_worldfactory::lineage::{Artifact, SpecimenNode, SpecimenRegistry};
use bioprism_worldfactory::observed::{Access, SourceRef, Stratum, StudyDesign};
use bioprism_worldfactory::preanalytic::{
    Edit, ExpectedResponse, FaultKind, Intensity, PreanalyticMutation, Specimen,
};
use bioprism_worldfactory::provenance::{Claim, ClaimKind, Provenance, Selection};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

#[path = "protocol/agent_workflows.rs"]
mod agent_workflows;
#[path = "protocol/context_repository.rs"]
mod context_repository;
#[path = "protocol/cross_crate_contracts.rs"]
mod cross_crate_contracts;
#[path = "protocol/developer_operations.rs"]
mod developer_operations;
#[path = "protocol/domain_evidence.rs"]
mod domain_evidence;
#[path = "protocol/evaluation.rs"]
mod evaluation;
#[path = "protocol/glioma.rs"]
mod glioma;
#[path = "protocol/governance_safety.rs"]
mod governance_safety;
#[path = "protocol/protocol_transport.rs"]
mod protocol_transport;
#[path = "protocol/research_modeling.rs"]
mod research_modeling;
#[path = "protocol/runtime_infrastructure.rs"]
mod runtime_infrastructure;

fn server() -> Server {
    Server::new(repo_root())
}

struct RejectingInstitutionWorker {
    calls: Arc<AtomicUsize>,
}

impl bioprism_research::GliomaActionExecutor for RejectingInstitutionWorker {
    fn execute_action(
        &mut self,
        _candidate: &bioprism_research::GliomaActionCandidate,
        _attempt: u8,
    ) -> Result<bioprism_research::ActionExecutionResult, bioprism_research::ActionExecutionFailure>
    {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(bioprism_research::ActionExecutionFailure {
            reason: "institution worker preflight refused this test action".into(),
            retryable: false,
        })
    }
}

fn ready(server: &mut Server) {
    if server.lifecycle() == Lifecycle::New {
        let initialize =
            Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
                .expect("initialize parses");
        server.handle(&initialize).expect("initialize is answered");
    }
    if server.lifecycle() == Lifecycle::Initialized {
        let notification =
            Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .expect("initialized notification parses");
        assert!(server.handle(&notification).is_none());
    }
    assert_eq!(server.lifecycle(), Lifecycle::Ready);
}

fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    ready(server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })
        .to_string(),
    )
    .expect("request parses");

    let response = server.handle(&request).expect("call is answered");
    let json = response.to_json();
    let text = json["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let mut parsed: Value = serde_json::from_str(text).expect("payload is JSON");
    if let Some(map) = parsed.as_object_mut() {
        map.insert("__isError".into(), json["result"]["isError"].clone());
    }
    parsed
}

/// Run a test body on a thread with the same explicit stack reservation the server's dispatch
/// threads use. Unoptimised builds give the widest workflow frames multi-megabyte activation
/// records; a body that walks every capability group through the devplat library directly —
/// without the server's guarded dispatch thread in between — overflows the default test stack,
/// which aborts the whole test process instead of failing one test.
///
fn on_a_dispatch_sized_stack<T: Send>(body: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, body)
            .expect("test worker thread should start")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    })
}

fn protocol_pack_fixture() -> PackIr {
    PackIr {
        manifest: PackManifest {
            id: PackId::parse("prism.verification-recovery").unwrap(),
            version: PackVersion::new(1, 0, 0),
            schema_range: SchemaRange::new(1, 1),
            title: "Verification, Recovery and Backtracking".into(),
            measures: "Whether agents detect silent failure and recover without compounding harm."
                .into(),
            blueprint_module: "15.05".into(),
            axis: PackAxis::Mechanism,
            capabilities: vec![bioprism_packs::CapabilityFamily::Agent(
                AgentCapability::VerificationAndRecovery,
            )],
            domains: vec![Domain::Coding],
            owners: vec!["prism-core".into()],
            license: "Apache-2.0".into(),
            dependencies: Vec::new(),
        },
        content: PackContent {
            parent_environments: vec![ParentEnvironment {
                world: WorldId::parse("world-fault-001").unwrap(),
                decision_parents: 30,
            }],
            decision_families: vec!["select the next verifier".into()],
            mutation_relations: vec!["fault-severity-ladder".into()],
            oracles: vec![PackOracleTier::Executable],
            instances: InstanceSource::Authored { validated: 900 },
            executed_trials: 900,
            independent_reproductions: 1,
            effective_sample_size: Some(40),
        },
    }
}

fn hub_review_fixture(id: &str, artifact: &[u8]) -> Value {
    let submitter =
        Submitter::unverified(SubmitterId::parse("lab-a").unwrap()).declaring_no_conflicts();
    let draft = SubmissionDraft {
        id: Some(SubmissionId::parse(id).unwrap()),
        submitter: Some(SubmitterId::parse("lab-a").unwrap()),
        content: Some(ContentHash::of_bytes(artifact)),
        scope: Some(DeclaredScope {
            disease: vec!["glioma".into()],
            modality: vec!["mri".into()],
            decision_family: vec!["evidence-acquisition".into()],
            intended_use: "compare synthetic context compilers".into(),
            out_of_scope: vec!["patient care".into()],
        }),
        licence: Some(Licence::permissive("CC0-1.0")),
        provenance: Some(HubProvenance {
            ancestors: Vec::new(),
            build: BuildProvenance {
                toolchain: "rustc".into(),
                source_digest: ContentHash::of_bytes(b"source"),
                reproducible: true,
            },
            attestations: vec!["local".into()],
        }),
        does_not_establish: vec![NonClaim::clinical_validity()],
        attributions: Vec::new(),
        evidence_scale: Some(EvidenceScale::new(10, 5)),
        claimed_verification: None,
        submitted_at: HubEpoch(1),
    };
    json!({
        "draft": serde_json::to_value(draft).unwrap(),
        "submitter": serde_json::to_value(submitter).unwrap(),
        "moderation": {
            "actor": "hub",
            "at": 1,
            "transitions": [
                { "to": "under_review", "decision": { "actor": "reviewer", "at": 2, "reason": null, "superseded_by": null } },
                { "to": "accepted", "decision": { "actor": "reviewer", "at": 3, "reason": null, "superseded_by": null } }
            ],
            "attestations": [
                { "to": "reproduced", "actor": "reviewer", "at": 4 }
            ]
        }
    })
}

const WORLD: &str = "fixtures/fiber-v0.1/radiogenomic_world.json";
const QUERY: &str = "fixtures/fiber-v0.1/leakage_query.json";
// Audited registry sizes: changes to either registry should update these contracts deliberately.
const CAPABILITY_GROUP_COUNT: usize = 58;
const TOOL_DEFINITION_COUNT: usize = 893;

fn ledger_event_fixture(kind: &str, subject: &str, instant: &str, key: &str) -> LedgerEvent {
    LedgerEvent::new(
        LedgerEventClass::Material,
        LedgerEventKind::parse(kind).unwrap(),
        LedgerActor::new("fixture-actor", "curator").unwrap(),
        LedgerSubjectKey::parse(subject).unwrap(),
        LedgerEventTimes::published_on_record(
            LedgerValidTime::parse(instant).unwrap(),
            LedgerRecordTime::parse(instant).unwrap(),
        ),
        json!({ "kind": kind, "subject": subject }),
    )
    .unwrap()
    .with_idempotency_key(IdempotencyKey::parse(key).unwrap())
}

fn metric_vector(system: &str, verify: f64, safety: f64, pack: &str) -> Value {
    json!({
        "system": system,
        "grid": {
            "label": "reference-grid",
            "conditions": {
                "subject": { "subject": "grid", "label": "reference-grid" },
                "scoring_rule": {
                    "name": "atlas pass rate",
                    "direction": "higher_is_better",
                    "unit": "fraction of evaluable trials"
                },
                "ontology_version": { "state": "recorded", "value": "test-ontology/1" },
                "pack_version": { "state": "recorded", "value": pack },
                "evidence_base": { "state": "recorded", "value": "public-observed/2026-01" },
                "oracle_floor": { "state": "recorded", "value": "executable" },
                "budget": { "state": "recorded", "value": { "label": "standard", "tokens": 100000 } },
                "stratum": {}
            },
            "cells": {
                "verify.oracle": {
                    "state": "measured",
                    "estimate": { "uncertainty": "point", "estimate": { "value": verify, "no_interval": "single_trial" } },
                    "effective_size": 3
                },
                "safety.boundary": {
                    "state": "measured",
                    "estimate": { "uncertainty": "point", "estimate": { "value": safety, "no_interval": "single_trial" } },
                    "effective_size": 3
                }
            }
        }
    })
}

fn attested_minimal_registry_pack() -> Value {
    BenchmarkPack::builder("mcp/demo", "0.1.0")
        .intended_use("MCP lifecycle protocol fixture")
        .publisher("mcp-test")
        .build()
        .expect("minimal pack builds")
        .attest()
        .expect("minimal pack attests")
}

fn risk_assessment(ratings: Value) -> Value {
    json!({
        "subject": "pack/biological-design@1",
        "category": "biological_design",
        "ratings": ratings
    })
}

/// A truncated mandatory set is indistinguishable at the point of use from a complete one, so a
/// budget that cannot hold it must surface as a refusal naming the shortfall — never as a
/// smaller bundle that still claims to be the route.
///
/// Both limits here are headroom, not measurements. The mandatory closure of `README.md` under
/// the normative policy grows whenever the repository's normative documents do — it passed
/// 30000 estimated tokens when the project-modeling documents landed — so a limit pinned just
/// above today's closure turns every documentation edit into a failure of this test. The two
/// refusals the old tight limits covered incidentally are each asserted on their own, above and
/// below, so the headroom costs no coverage.
///
/// The disclosure contract: L0 carries the decision and the omissions, never the evidence.
///
/// The 0.3 decision contract crosses the MCP boundary as an explicit, certificate-bound summary.
///
/// The 0.4 observed-evidence contract crosses MCP as a full, certificate-bound context audit.
///
/// The 0.5 adaptive contract crosses MCP as a certificate-bound plan, never as an execution
/// receipt or authorization claim.
///
const TRADE_WORLD: &str = "fixtures/domains/trade-surveillance/world.json";
const TRADE_QUERY: &str = "fixtures/domains/trade-surveillance/query.json";
const TRADE_DOMAIN: &str = "fixtures/domains/trade-surveillance/domain.json";

/// The domain parameter carries a non-biological world through the same pipeline: the pack's
/// oracle judges it and the witness is checkable at l2, exactly as a leakage witness would be.
///
/// A required variable withheld by the temporal cut abstains the verdict: the unjudged world
/// crosses MCP as underdetermined, never as valid.
///
/// Without the domain parameter nothing changed: the reference oracle judges, the pinned parity
/// digest holds, and no domain object appears anywhere in the response.
///
/// A domain compile's refinement handle carries the pack binding, so descending recompiles under
/// the same oracle and the certificate digest still verifies.
///
/// A pack cannot rewrite the query it judges — the certificate binds the query's bytes — so a
/// missing protected tag or goal is reported as an advisory instead of being injected silently.
///
const DEMO_PROJECT: &str = "fixtures/projects/demo-app";
const DEMO_PROJECT_ISSUES: &str = "fixtures/projects/demo-app/issues.json";

/// A whole software project crosses MCP as a world judged by a declared rule oracle, and the
/// reason it failed is a checkable object — the dependency's own declaration string — rather
/// than a readiness score. The pinned dependency must stay out of that set, or the witness
/// would be naming the tree instead of the defect.
///
/// An issue's evidence region comes from the components it *declares*, resolved syntactically:
/// the issue naming `src/lib.rs` gets the src inventory and not the unrelated assets one, and
/// the issue naming nothing gets the aggregates alone rather than a guessed region. There is no
/// semantic relevance step behind either result, so both must be visible on the wire.
///
/// The write is confined to the server root and reports every path it created, so a caller can
/// check the claim against the filesystem rather than trusting the summary counts.
///
/// A preview whose file list does not match what confirming actually writes is worse than no
/// preview, because the caller approves one effect and receives another. So the claim under test
/// is not "performed is false" but the equality itself: the unconfirmed call names exactly the
/// paths the confirmed call creates, and creates none of them. Issues are supplied because the
/// per-issue query documents are the part a preview built from a fixed list would silently omit
/// — the set of writes depends on the input, so it has to be computed, not assumed.
///
/// Every path parameter of both project tools is root-confined, on both separators, for
/// traversal and for absolute paths alike — a project tool must never become a scanner of, or a
/// writer into, arbitrary directories. The refused write is checked against the filesystem,
/// because a refusal that still created the directory would be a refusal in name only.
///
/// Determinism has to survive the whole server surface, not just the library: the same tree
/// ingested twice must produce the same world bytes, or a certificate over those bytes would
/// change for reasons no reader could name.
///
/// `decision_time` reaches the world's scan event and every generated query, so an ungated
/// malformed value comes back as the *assembled world* failing the reference validator — a
/// message that blames the emitter for a string only the caller can edit. The refusal has to
/// name the parameter instead.
///
/// An issue whose every declaration resolved to nothing compiles to the same region as an issue
/// that declared nothing at all. Without the declarations on the wire a reader takes the second
/// reading — the one that looks deliberate — so the two must be distinguishable in the response
/// itself, not only inside the world document the response does not carry.
///
/// Writes a `bioprism-repair-declarations/0.1` document under the server root and returns the
/// root-relative path `repair_plan` takes.
fn write_repair_declarations(directory: &str, document: Value) -> String {
    std::fs::create_dir_all(repo_root().join(directory)).unwrap();
    let relative = format!("{directory}/declared.json");
    std::fs::write(
        repo_root().join(&relative),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    relative
}

/// Plans ISSUE-1 of the demo app into `out`, confirming the write, and returns the payload.
fn plan_demo_issue_one(server: &mut Server, out: &str, extra: &[(&str, Value)]) -> Value {
    let mut arguments = json!({
        "root": DEMO_PROJECT,
        "issues": DEMO_PROJECT_ISSUES,
        "issue": "ISSUE-1",
        "out": out,
        "confirm": true,
    });
    for (key, value) in extra {
        arguments[*key] = value.clone();
    }
    let payload = call(server, "repair_plan", arguments);
    assert_ne!(
        payload["__isError"],
        json!(true),
        "planning failed: {payload}"
    );
    payload
}

/// The plan crosses MCP bound to the world it was planned from, and the acceptance report that
/// comes back for an unrepaired tree says so: `not_met`, with the release check that fired when
/// the plan was made still firing. A tool that congratulated the tree here would be the exact lie
/// this surface exists to refuse.
///
/// A plan is bound to the world it was planned from, and a world that is not that world gets no
/// verdict at all — not a verdict with a flag beside it, because a reader offered both takes the
/// verdict and skips the flag. So the claim is the absence: no outcome, no item list, nothing
/// evaluated. The two worlds are made to differ by scanning a genuinely different tree, so a
/// passing run cannot mean the comparison was vacuous.
///
/// A preview whose file list does not match what confirming actually writes is worse than no
/// preview, because the caller approves one effect and receives another. The claim under test is
/// the equality itself, and the bytes: the confirmed write must be the plan document the preview
/// already carried, not a second plan derived on a second pass.
///
/// Every path parameter of both repair tools is root-confined, on both separators, for traversal
/// and for absolute paths alike — planning a repair must never become a way to read a criteria or
/// plan document from, or write a plan into, an arbitrary directory. The refused write is checked
/// against the filesystem, because a refusal that still created the file would be a refusal in
/// name only.
///
/// A criterion the caller declared and a criterion the generator inferred carry different
/// authority, so the wire must keep them apart — and a criterion whose variable the world does not
/// carry must arrive as `not_evaluable` naming the obstruction, never as a failure. Both claims
/// are checked over one plan, because it is the mixture that a tool folding the third value into
/// the second would report as an ordinary `not_met`.
///
/// Three named refusals rather than one generic one, because the operator's next action differs.
/// A `issue` naming something the declarations do not carry is a parameter to fix, and the refusal
/// names the ids that are declared. An absent `issues` is a missing parameter, not an issue that
/// does not exist. A declarations document with a misspelled key is a document to fix, and
/// silently ignoring the key would produce a plan whose missing falsifier the author would then be
/// blamed for.
///
fn routing_fingerprint_fixture() -> Fingerprint {
    Fingerprint {
        facts: 10,
        factors: 3,
        protected_tag_count: 2,
        protected_fact_fraction: 0.2,
        distractor_density: 0.8,
        tag_informativeness: 1.0,
        mean_factor_arity: 4.0 / 3.0,
        max_factor_arity: 2,
        arity_histogram: BTreeMap::from([(1, 2), (2, 1)]),
        max_unary_chain: 0,
        hub_share: 0.5,
        hub_is_derived: false,
        target_producer_count: 1,
    }
}
