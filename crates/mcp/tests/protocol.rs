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
use bioprism_bundle::{EntryRole, ResultBundle};
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
use bioprism_mcp::{
    serve, tool_definitions, Lifecycle, Request, Server, CAPABILITIES_URI, CERTIFICATE_SCHEMA_URI,
    PROTOCOL_VERSION,
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
use std::sync::atomic::AtomicBool;

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn server() -> Server {
    Server::new(repo_root())
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

#[test]
fn initialize_reports_the_protocol_version_and_instructions() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();

    assert_eq!(
        response["result"]["protocolVersion"],
        json!(PROTOCOL_VERSION)
    );
    assert_eq!(response["result"]["serverInfo"]["name"], json!("bioprism"));
    let instructions = response["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains("not a medical device") || instructions.contains("not a medical")
    );
    assert!(instructions.contains("fiber_compile"));
}

#[test]
fn every_tool_declares_an_input_schema_with_required_fields() {
    let tools = tool_definitions();
    assert_eq!(tools.len(), 212);
    for tool in &tools {
        assert!(tool["name"].is_string());
        assert!(tool["description"].as_str().unwrap().len() > 40);
        assert_eq!(tool["inputSchema"]["type"], json!("object"));
        assert!(tool["inputSchema"]["required"].is_array());
    }
}

#[test]
fn pack_health_assessment_exposes_digest_bound_score_refusal() {
    let mut server = server();
    let observations = bioprism_packs::Observations {
        calibration: DifficultyCalibration::new(
            [
                ("system-a", 99, 100),
                ("system-b", 98, 100),
                ("system-c", 100, 100),
            ]
            .into_iter()
            .map(|(system, passes, trials)| SystemObservation::new(system, passes, trials).unwrap())
            .collect(),
        ),
        ..Default::default()
    };
    let payload = call(
        &mut server,
        "pack_health_assess",
        json!({
            "pack": serde_json::to_value(protocol_pack_fixture()).unwrap(),
            "observations": serde_json::to_value(observations).unwrap(),
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["verdict"], json!("unreportable"));
    assert_eq!(payload["score_gate"]["reportable"], json!(false));
    assert!(payload["pack_digest"].is_string());
    assert!(payload["health"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| { finding["finding"] == json!("saturated") }));
}

#[test]
fn sdk_registry_check_refuses_invalid_manifests_before_resolution() {
    let mut server = server();
    let manifest = PluginManifest::new("schema-oracle", "0.4.0", "fixture")
        .speaking(RegistryPolicy::workspace_versions())
        .providing(Capability::new(CapabilityKind::Oracle).at(Priority(10)))
        .claiming(Determinism::Deterministic)
        .at_grade(AbiGrade::P1);
    let mut invalid = serde_json::to_value(&manifest).unwrap();
    invalid["capabilities"] = json!([]);
    let payload = call(
        &mut server,
        "sdk_registry_check",
        json!({
            "manifests": [
                serde_json::to_value(manifest).unwrap(),
                invalid
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(false));
    assert_eq!(payload["stage"], json!("manifest_validation"));
    assert_eq!(payload["registry"], Value::Null);
    assert!(payload["manifests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| { row["valid"] == json!(false) }));
}

#[test]
fn repository_impact_reports_a_scanned_module_and_typed_closure() {
    let mut server = server();
    let catalogue = call(
        &mut server,
        "repository_catalog",
        json!({ "prefix": "docs/", "limit": 1 }),
    );
    let changed = catalogue["modules"][0]["id"]
        .as_str()
        .expect("repository catalog returns a module id")
        .to_string();
    let payload = call(
        &mut server,
        "repository_impact",
        json!({ "changed": changed }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["closure_count"].as_u64().unwrap() >= 1);
    assert!(payload["impact"]["affected"].is_array());
    assert!(payload["impact"]["stopped_at"].is_array());
    assert!(payload["guarantees"].as_array().unwrap().len() >= 4);
}

#[test]
fn world_generation_is_digest_stable_and_validates_both_documents() {
    let mut server = server();
    let spec = bioprism_worldgen::WorldSpec::discriminating(2);
    let arguments = json!({
        "spec": serde_json::to_value(spec).unwrap(),
        "include_world": true,
        "include_query": true,
    });
    let first = call(&mut server, "world_generate", arguments.clone());
    let second = call(&mut server, "world_generate", arguments);
    assert_eq!(first["ok"], json!(true));
    assert_eq!(first["validation"]["errors"], json!(0));
    assert_eq!(first["world_digest"], second["world_digest"]);
    assert_eq!(first["query_digest"], second["query_digest"]);
    assert!(first["world"]["facts"].is_array());
    assert!(first["query"]["targets"].is_array());
    assert!(first["guarantees"].as_array().unwrap().len() >= 4);
}

#[test]
fn hub_submission_review_replays_append_only_public_moderation() {
    let mut server = server();
    let submitter =
        Submitter::unverified(SubmitterId::parse("lab-a").unwrap()).declaring_no_conflicts();
    let draft = SubmissionDraft {
        id: Some(SubmissionId::parse("sub-mcp-1").unwrap()),
        submitter: Some(SubmitterId::parse("lab-a").unwrap()),
        content: Some(ContentHash::of_bytes(b"mcp-artifact")),
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
    let payload = call(
        &mut server,
        "hub_submission_review",
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
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["schema"], json!("bioprism-mcp/hub-submission/0.1"));
    assert_eq!(payload["state"], json!("accepted"));
    assert_eq!(payload["verification"], json!("reproduced"));
    assert_eq!(payload["event_count"], json!(4));
    assert_eq!(payload["ledger"]["events"].as_array().unwrap().len(), 4);
    assert!(payload["limitation_card"]
        .as_str()
        .unwrap()
        .contains("does not establish"));
}

#[test]
fn telemetry_projection_reports_loss_and_metric_observation_posture() {
    let mut server = server();
    let event = DomainEvent::new("evt-1", "job.completed", 7)
        .with_field(
            "subject",
            OpsField::new(json!("S001"), ScopeClass::Identity),
        )
        .with_field("count", OpsField::new(json!(3), ScopeClass::Unclassified));
    let policy = RedactionPolicy::new("telemetry-v1")
        .declare(
            ScopeClass::Identity,
            OpsTreatment::Coarsen {
                to: "subject".into(),
            },
        )
        .unwrap()
        .declare(ScopeClass::Unclassified, OpsTreatment::Drop)
        .unwrap();
    let spans = SignalId::parse("spans_emitted").unwrap();
    let total = SignalId::parse("operations_total").unwrap();
    let metric = MetricDefinition::new(
        "trace_coverage",
        OpsDerivation::Ratio {
            numerator: spans.clone(),
            denominator: total.clone(),
        },
        "ratio",
    )
    .unwrap();
    let observations = OpsObservations::new()
        .record(OpsSample::observed(spans, 98.0, "counter", 7))
        .record(OpsSample::observed(total, 100.0, "counter", 7));
    let payload = call(
        &mut server,
        "telemetry_project",
        json!({
            "event": serde_json::to_value(event).unwrap(),
            "policy": serde_json::to_value(policy).unwrap(),
            "trace": "trace-1",
            "metric": serde_json::to_value(metric).unwrap(),
            "observations": serde_json::to_value(observations).unwrap(),
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["schema"],
        json!("bioprism-mcp/telemetry-projection/0.1")
    );
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["loss"]["dropped"], json!(["count"]));
    assert_eq!(payload["metric"]["ok"], json!(true));
    assert_eq!(payload["metric"]["value"]["value"], json!(0.98));
    assert_eq!(payload["record"]["event_id"], json!("evt-1"));
}

#[test]
fn factory_lifecycle_replays_safe_retry_quarantine_compensation_and_commit_boundaries() {
    let mut server = server();
    let jobs = [
        Job::new(
            "a-idempotent",
            ResourceClass::Compile,
            Idempotency::Idempotent,
            json!({ "kind": "pure-build" }),
        ),
        Job::new(
            "b-nonidempotent",
            ResourceClass::Compile,
            Idempotency::NonIdempotent,
            json!({ "kind": "external-effect" }),
        ),
        Job::new(
            "c-compensable",
            ResourceClass::Compile,
            Idempotency::Compensable,
            json!({ "kind": "reversible-effect" }),
        ),
    ];
    let worker = WorkerCapability::new("worker-1", vec![ResourceClass::Compile])
        .with_lease_duration_nanos(30_000_000_000);
    let payload = call(
        &mut server,
        "factory_lifecycle_simulate",
        json!({
            "jobs": jobs.iter().map(serde_json::to_value).collect::<Result<Vec<_>, _>>().unwrap(),
            "workers": [serde_json::to_value(worker).unwrap()],
            "actions": [
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 0 },
                { "kind": "recover_expired", "now_nanos": 30_000_000_000i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 31_000_000_000i64 },
                { "kind": "stage", "job_id": "a-idempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 31_000_000_001i64, "output": { "digest": "a-out" } },
                { "kind": "commit", "job_id": "a-idempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 31_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 31_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 61_000_000_003i64 },
                { "kind": "release_quarantine", "job_id": "b-nonidempotent", "operator": "reviewer-1" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 62_000_000_000i64 },
                { "kind": "fail", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 62_000_000_001i64, "reason": "external service rejected the request" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_000i64 },
                { "kind": "stage", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 3, "now_nanos": 63_000_000_001i64, "output": { "effect_id": "effect-1" } },
                { "kind": "commit", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 3, "now_nanos": 63_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 93_000_000_003i64 },
                { "kind": "compensate", "job_id": "c-compensable" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 94_000_000_000i64 },
                { "kind": "stage", "job_id": "c-compensable", "worker_id": "worker-1", "attempt": 2, "now_nanos": 94_000_000_001i64, "output": { "compensated": true } },
                { "kind": "commit", "job_id": "c-compensable", "worker_id": "worker-1", "attempt": 2, "now_nanos": 94_000_000_002i64 }
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["action_count"], json!(19));
    assert_eq!(payload["action_failures"], json!(0));
    assert_eq!(payload["quarantined"], json!([]));
    assert_eq!(payload["dead_lettered"], json!([]));
    assert_eq!(
        payload["trace"][1]["result"][0]["outcome"],
        json!("requeued")
    );
    assert_eq!(
        payload["trace"][6]["result"][0]["outcome"],
        json!("quarantined")
    );
    assert_eq!(
        payload["trace"][14]["result"][0]["outcome"],
        json!("awaiting_compensation")
    );

    let jobs = payload["jobs"].as_array().unwrap();
    for id in ["a-idempotent", "b-nonidempotent", "c-compensable"] {
        let job = jobs
            .iter()
            .find(|row| row["id"] == json!(id))
            .expect("simulated job is included in final snapshot");
        assert_eq!(job["job"]["state"], json!("succeeded"));
        assert!(job["committed_result"].is_object());
    }
    assert!(payload["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("idempotency")));
}

#[test]
fn factory_authority_verify_audits_legacy_queue_checkpoint_without_dispatch() {
    let mut server = server();
    let checkpoint = JobStore::new().snapshot().unwrap();
    let payload = call(
        &mut server,
        "factory_authority_verify",
        json!({
            "checkpoint": serde_json::to_value(checkpoint).unwrap(),
            "include_events": true,
            "max_events": 8,
        }),
    );

    assert_eq!(payload["valid"], json!(true));
    assert_eq!(payload["revision"], json!(0));
    assert_eq!(payload["authority_epoch"], json!(1));
    assert_eq!(payload["event_count"], json!(0));
    assert_eq!(payload["events"], json!([]));
    assert_eq!(payload["job_count"], json!(0));
    assert_eq!(payload["active_lease_count"], json!(0));
    assert!(payload["does_not_claim"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "multi-host consensus or network-partition tolerance"));
}

#[test]
fn artifact_registry_audit_joins_cross_domain_records_without_inventing_provenance() {
    let mut server = server();
    let leaf = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "register",
            "registration": {
                "kind": "domain_report",
                "subject_id": "mission-leaf",
                "domains": ["oncology", "genomics"],
                "parent_digests": [],
                "artifact": {"status": "review_required"}
            }
        }),
    );
    assert_eq!(leaf["created"], json!(true));
    let leaf_digest = leaf["content_digest"].as_str().unwrap();
    let root = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "register",
            "registration": {
                "kind": "mission_report",
                "subject_id": "mission-root",
                "domains": ["oncology"],
                "parent_digests": [leaf_digest, "f".repeat(64)],
                "artifact": {"status": "partial"}
            }
        }),
    );
    assert_eq!(root["created"], json!(true));
    let lineage = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "lineage",
            "content_digest": root["content_digest"]
        }),
    );
    assert_eq!(lineage["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        lineage["missing_parent_digests"].as_array().unwrap().len(),
        1
    );
    assert_eq!(lineage["cycles"], json!([]));
    assert!(lineage["does_not_claim"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "parent presence proves causal provenance or scientific validity"));
    let cross_store = call(
        &mut server,
        "artifact_registry_audit",
        json!({"operation": "cross_store"}),
    );
    assert_eq!(
        cross_store["workflow"],
        json!("artifact_registry_cross_store_audit")
    );
    assert_eq!(cross_store["consistent"], json!(true));
    assert_eq!(
        cross_store["coverage"]["mission_evidence_bundle"]["complete"],
        json!(true)
    );
    assert_eq!(
        cross_store["stores"]["artifact_registry"]["record_count"],
        json!(2)
    );
    assert!(cross_store["does_not_claim"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "the three stores were read in one atomic transaction"));
}

#[test]
fn domain_report_projection_checks_catalogue_indexes_idempotently_and_reports_coverage() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "domain-report-subject",
        "source_tool": "modality_catalog",
        "report": {"observations": ["caller supplied"], "status": "review_required"},
        "claim_posture": {
            "status": "review_required",
            "does_not_claim": ["clinical validity", "execution completion"],
            "limitations": ["no external provenance was supplied"]
        }
    });
    let first = call(&mut server, "domain_report_project", arguments.clone());
    assert_eq!(first["workflow"], json!("domain_report_project"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["verification"]["method"],
        json!("domain_report_projection")
    );
    let second = call(&mut server, "domain_report_project", arguments);
    assert_eq!(
        first["artifact_registry"]["content_digest"],
        second["artifact_registry"]["content_digest"]
    );
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let coverage = call(
        &mut server,
        "domain_report_project",
        json!({"operation": "coverage", "include_report_digests": true}),
    );
    assert_eq!(coverage["workflow"], json!("domain_report_coverage"));
    assert_eq!(coverage["group_count"], json!(29));
    assert_eq!(coverage["reported_group_count"], json!(1));
    assert_eq!(coverage["missing_group_count"], json!(28));
    assert_eq!(coverage["complete"], json!(false));
    assert_eq!(coverage["readiness_claimed"], json!(false));
    assert_eq!(
        coverage["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap()["report_classes"]["ordinary"],
        json!(1)
    );
    assert_eq!(
        coverage["bridge_summary"]["lineage"]["reports_without_lineage_parents"],
        json!(1)
    );
    let filtered = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "coverage",
            "group_id": "biological_domains",
            "report_class": "ordinary",
            "bridge_mode": "inline"
        }),
    );
    assert_eq!(filtered["filters"]["report_class"], json!("ordinary"));
    assert_eq!(filtered["filters"]["bridge_mode"], json!("inline"));
    assert_eq!(filtered["reported_group_count"], json!(0));
    assert_eq!(filtered["missing_group_count"], json!(1));
    assert_eq!(coverage["coverage_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn domain_report_projection_refuses_unknown_source_and_domain_claims() {
    let mut server = server();
    let refused = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "domain-report-invalid",
            "source_tool": "not_a_declared_tool",
            "report": {"status": "review_required"},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert!(refused["error"].as_str().unwrap().contains("not declared"));

    let domain_refused = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["not_declared"],
            "subject_id": "domain-report-invalid-domain",
            "source_tool": "modality_catalog",
            "report": {"status": "review_required"},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(domain_refused["ok"], json!(false));
    assert!(domain_refused["error"]
        .as_str()
        .unwrap()
        .contains("not declared"));
}

#[test]
fn adapter_domain_report_operation_validates_and_joins_adapter_evidence() {
    let mut server = server();
    let result = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_adapter_execution",
            "evidence": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "adapter-domain-report-subject",
                "adapter_id": "bioprism.python.vcf_text",
                "adapter_version": "0.1.0",
                "source_id": "adapter-domain-report-source",
                "input_digest": "a".repeat(64),
                "output_digest": "b".repeat(64),
                "execution_status": "succeeded",
                "conformance_status": "verified",
                "semantic_loss_status": "unknown",
                "losses": [],
                "parent_digests": ["c".repeat(64)]
            },
            "conformance": {"status": "verified", "report_digest": "d".repeat(64)}
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-devplat-adapter-domain-report/0.1")
    );
    assert_eq!(result["workflow"], json!("adapter_domain_report"));
    assert_eq!(
        result["evidence"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(
        result["domain_report"]["workflow"],
        json!("domain_report_project")
    );
    assert_eq!(
        result["domain_report"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(
        result["domain_report"]["report"]["claim_posture"]["status"],
        json!("observed")
    );
    assert!(result["domain_report"]["report"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|parent| parent == &result["evidence"]["artifact_registry"]["content_digest"]));
    assert_eq!(result["readiness_claimed"], json!(false));
}

#[test]
fn provider_domain_report_operations_compose_inline_and_external_normalization() {
    let mut server = server();
    let payload = json!({"records": [{"id": "pmid:1", "title": "opaque"}]});
    let inline = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_provider_normalization",
            "normalization": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "provider-domain-report",
                "source_tool": "literature_bind_check",
                "connector_kind": "literature",
                "provider": "pubmed",
                "payload": payload,
                "outcome": "observed",
                "parent_digests": ["a".repeat(64)]
            }
        }),
    );
    assert_eq!(inline["ok"], json!(true));
    assert_eq!(inline["mode"], json!("inline"));
    assert_eq!(inline["workflow"], json!("provider_domain_report"));
    assert_eq!(
        inline["domain_report"]["report"]["source_tool"],
        json!("domain_evidence_provider_normalize")
    );
    assert_eq!(
        inline["domain_report"]["report"]["claim_posture"]["status"],
        json!("observed")
    );
    assert_eq!(
        inline["domain_report"]["report"]["report"]["kind"],
        json!("provider_normalization")
    );
    assert_eq!(
        inline["domain_report"]["report"]["report"]["payload_digest"],
        inline["normalization"]["payload_digest"]
    );
    assert!(inline["domain_report"]["report"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|parent| parent == &inline["normalization"]["artifact_registry"]["content_digest"]));
    assert_eq!(inline["readiness_claimed"], json!(false));

    let payload_digest = ContentHash::of_value(&payload).unwrap().to_string();
    let byte_length = serde_json::to_vec(&payload).unwrap().len() as u64;
    let external = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_external_provider_normalization",
            "normalization": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "external-provider-domain-report",
                "source_tool": "literature_bind_check",
                "provider": "pubmed",
                "connector_kind": "literature",
                "handoff_digest": "b".repeat(64),
                "transfer_id": "provider-domain-report-transfer",
                "payload_digest": payload_digest,
                "byte_length": byte_length,
                "storage_backend": "object_store",
                "locator_kind": "opaque",
                "locator": "store://caller/pubmed/provider-domain-report",
                "availability": "available",
                "retention": "durable",
                "payload": payload,
                "outcome": "observed"
            }
        }),
    );
    assert_eq!(external["ok"], json!(true));
    assert_eq!(external["mode"], json!("external_payload"));
    assert_eq!(
        external["domain_report"]["report"]["source_tool"],
        json!("domain_evidence_provider_external_payload_normalize")
    );
    assert_eq!(
        external["domain_report"]["report"]["report"]["materialization"]["locator_opened"],
        json!(false)
    );
    assert!(external["domain_report"]["report"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|parent| parent
            == &external["normalization"]["receipt_artifact_registry"]["content_digest"]));
    assert_eq!(external["readiness_claimed"], json!(false));
}

#[test]
fn domain_evidence_harmonization_indexes_traceability_idempotently() {
    let mut server = server();
    let first = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "harmonization-subject",
            "source_tool": "modality_catalog",
            "report": {"observations": ["modality contract retained"]},
            "claim_posture": {
                "status": "observed",
                "does_not_claim": ["clinical validity"]
            }
        }),
    );
    let second = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_ir_and_query",
            "domains": ["BioQL syntax"],
            "subject_id": "harmonization-subject",
            "source_tool": "bioql_compile",
            "report": {"observations": ["query syntax contract retained"]},
            "claim_posture": {
                "status": "review_required",
                "does_not_claim": ["query execution", "biological truth"],
                "limitations": ["no source dataset supplied"]
            }
        }),
    );
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(second["artifact_registry"]["indexed"], json!(true));

    let arguments = json!({
        "subject_id": "harmonization-subject",
        "claim": {"id": "claim-opaque-1", "statement": "caller-owned claim"},
        "reports": [first["report"].clone(), second["report"].clone()],
        "links": [
            {"report_index": 0, "role": "supports"},
            {"report_index": 1, "role": "qualifies", "note": "syntax coverage is not execution"}
        ],
        "required_group_ids": ["biological_domains", "biological_ir_and_query"],
        "required_domains": ["modalities", "BioQL syntax"]
    });
    let harmonized = call(&mut server, "domain_evidence_harmonize", arguments.clone());
    assert_eq!(harmonized["workflow"], json!("domain_evidence_harmonize"));
    assert_eq!(
        harmonized["harmonization"]["coverage"]["traceability_state"],
        json!("complete")
    );
    assert_eq!(
        harmonized["harmonization"]["coverage"]["all_reports_linked"],
        json!(true)
    );
    assert_eq!(
        harmonized["harmonization"]["posture"]["qualification_link_count"],
        json!(1)
    );
    assert_eq!(harmonized["readiness_claimed"], json!(false));
    assert_eq!(harmonized["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        harmonized["artifact_registry"]["verification"]["method"],
        json!("domain_evidence_harmonization")
    );

    let coverage = call(
        &mut server,
        "domain_evidence_harmonization_coverage",
        json!({
            "subject_id": "harmonization-subject",
            "domain": "modalities",
            "traceability_state": "complete",
            "include_report_digests": true
        }),
    );
    assert_eq!(
        coverage["workflow"],
        json!("domain_evidence_harmonization_coverage")
    );
    assert_eq!(coverage["matching_count"], json!(1));
    assert_eq!(coverage["returned_count"], json!(1));
    assert_eq!(coverage["rows"][0]["report_count"], json!(2));
    assert_eq!(coverage["rows"][0]["link_count"], json!(2));
    assert_eq!(
        coverage["rows"][0]["report_digests"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        coverage["summary"]["domain_summary"]["modalities"]["report_count"],
        json!(1)
    );
    let invalid_cursor = call(
        &mut server,
        "domain_evidence_harmonization_coverage",
        json!({"after": "not-a-digest"}),
    );
    assert_eq!(invalid_cursor["__isError"], json!(true));

    let replay = call(&mut server, "domain_evidence_harmonize", arguments);
    assert_eq!(
        harmonized["artifact_registry"]["content_digest"],
        replay["artifact_registry"]["content_digest"]
    );
    assert_eq!(replay["artifact_registry"]["already_present"], json!(true));
}

#[test]
fn domain_evidence_harmonization_refuses_subject_or_catalogue_mismatch() {
    let mut server = server();
    let report = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "subject-a",
            "source_tool": "modality_catalog",
            "report": {"observations": ["bounded"]},
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    )["report"]
        .clone();
    let mismatched = call(
        &mut server,
        "domain_evidence_harmonize",
        json!({
            "subject_id": "subject-b",
            "claim": {"id": "claim-mismatch"},
            "reports": [report],
            "links": [{"report_index": 0, "role": "context"}]
        }),
    );
    assert_eq!(mismatched["__isError"], json!(true));
    assert!(mismatched["error"].as_str().unwrap().contains("subject"));

    let invalid_catalogue = call(
        &mut server,
        "domain_evidence_harmonize",
        json!({
            "subject_id": "subject-a",
            "claim": {"id": "claim-catalogue"},
            "reports": [{
                "schema": "bioprism-devplat-domain-report/0.1",
                "workflow": "domain_report_project",
                "subject_id": "subject-a",
                "group_id": "biological_domains",
                "source_tool": "modality_catalog",
                "domains": ["not-declared"],
                "report": {},
                "claim_posture": {"status": "observed", "does_not_claim": ["truth"]},
                "parent_digests": [],
                "non_claims": ["truth"]
            }],
            "links": [{"report_index": 0, "role": "context"}]
        }),
    );
    assert_eq!(invalid_catalogue["__isError"], json!(true));
}

#[test]
fn domain_evidence_intake_accepts_one_declared_envelope_from_every_capability_group() {
    let mut server = server();
    let catalogue = call(&mut server, "workspace_capabilities", json!({}));
    let groups = catalogue
        .as_array()
        .expect("workspace catalogue is an array");
    assert_eq!(groups.len(), 29);
    for group in groups {
        let group_id = group["id"].as_str().expect("group id");
        let source_tool = group["mcp_tools"][0].as_str().expect("source tool");
        let domain = group["domains"][0].as_str().expect("domain");
        let result = call(
            &mut server,
            "domain_evidence_intake",
            json!({
                "group_id": group_id,
                "domains": [domain],
                "subject_id": format!("all-domain-{group_id}"),
                "source_tool": source_tool,
                "request": {"probe": "retained"},
                "response": {"group_id": group_id, "source_tool": source_tool, "status": "bounded"},
                "outcome": "observed",
                "claim_posture": {
                    "status": "observed",
                    "does_not_claim": ["scientific truth", "execution completion"]
                }
            }),
        );
        assert_eq!(result["__isError"], json!(false), "group={group_id}");
        assert_eq!(result["workflow"], json!("domain_evidence_intake"));
        assert_eq!(result["group_id"], json!(group_id));
        assert_eq!(result["artifact_registry"]["indexed"], json!(true));
        assert_eq!(result["report"]["group_id"], json!(group_id));
        assert_eq!(result["report"]["source_tool"], json!(source_tool));
        assert_eq!(result["report"]["domains"], json!([domain]));
    }
}

#[test]
fn domain_evidence_intake_replays_idempotently_and_refuses_catalogue_mismatch() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "intake-replay",
        "source_tool": "modality_catalog",
        "response": {"status": "refused", "reason": "caller withheld execution"},
        "outcome": "refused",
        "claim_posture": {"status": "refused", "does_not_claim": ["execution", "truth"]}
    });
    let first = call(&mut server, "domain_evidence_intake", arguments.clone());
    let second = call(&mut server, "domain_evidence_intake", arguments);
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["content_digest"],
        second["artifact_registry"]["content_digest"]
    );
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    assert_eq!(first["outcome"], json!("refused"));
    assert_eq!(first["request_supplied"], json!(false));

    let invalid = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["not-declared"],
            "subject_id": "intake-invalid",
            "source_tool": "modality_catalog",
            "response": {},
            "outcome": "unknown",
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].as_str().unwrap().contains("not declared"));
}

#[test]
fn domain_evidence_coverage_preserves_missing_groups_outcomes_and_digest_rows() {
    let mut server = server();
    let intake = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "coverage-subject",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "partial",
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(intake["artifact_registry"]["indexed"], json!(true));
    let coverage = call(
        &mut server,
        "domain_evidence_coverage",
        json!({
            "include_intake_digests": true
        }),
    );
    assert_eq!(
        coverage["workflow"],
        json!("domain_evidence_intake_coverage")
    );
    assert_eq!(coverage["group_count"], json!(29));
    assert_eq!(coverage["reported_group_count"], json!(1));
    assert_eq!(coverage["missing_group_count"], json!(28));
    assert_eq!(coverage["complete"], json!(false));
    assert_eq!(coverage["tool_coverage_complete"], json!(false));
    assert_eq!(coverage["domain_coverage_complete"], json!(false));
    assert_eq!(
        coverage["domain_summary"]["modalities"]["intake_count"],
        json!(1)
    );
    let group = coverage["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(group["outcomes"], json!(["partial"]));
    assert!(group["declared_tools"].as_array().unwrap().len() > 1);
    assert!(group["missing_source_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "bioworlds_catalog"));
    assert_eq!(group["tool_coverage_state"], json!("partial"));
    assert_eq!(group["domain_coverage_state"], json!("partial"));
    assert_eq!(
        group["source_tool_coverage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["tool"] == "modality_catalog")
            .unwrap()["intake_count"],
        json!(1)
    );
    assert_eq!(group["intake_digests"].as_array().unwrap().len(), 1);
    let filtered = call(
        &mut server,
        "domain_evidence_coverage",
        json!({"group_id": "biological_domains", "domain": "MODALITIES"}),
    );
    assert_eq!(filtered["group_count"], json!(1));
    assert_eq!(filtered["reported_group_count"], json!(1));
    assert_eq!(filtered["complete"], json!(true));
    assert_eq!(filtered["tool_coverage_complete"], json!(false));
    assert_eq!(filtered["domain_coverage_complete"], json!(false));
}

#[test]
fn domain_evidence_source_plan_is_catalogue_bound_digest_addressed_and_non_executing() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "source-plan-subject",
        "source_tool": "modality_catalog",
        "connector_kind": "literature",
        "locator_kind": "uri",
        "locator": "https://example.org/article/1",
        "retrieval_mode": "metadata_only",
        "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
        "does_not_claim": ["retrieval occurred", "source is true"]
    });
    let first = call(
        &mut server,
        "domain_evidence_source_plan",
        arguments.clone(),
    );
    assert_eq!(first["workflow"], json!("domain_evidence_source_plan"));
    assert_eq!(first["retrieval_status"], json!("not_started"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["verification"]["method"],
        json!("domain_evidence_source_plan")
    );
    assert_eq!(first["plan_digest"].as_str().unwrap().len(), 64);
    let bound_intake = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-subject",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "observed",
            "source_plan_digest": first["plan_digest"].clone(),
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(bound_intake["source_plan_digest"], first["plan_digest"]);
    assert!(bound_intake["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|digest| digest == &first["plan_digest"]));
    let mut expected_arguments = arguments.clone();
    expected_arguments["subject_id"] = json!("source-plan-expected");
    expected_arguments["expected_content_digest"] = json!("a".repeat(64));
    let expected_plan = call(
        &mut server,
        "domain_evidence_source_plan",
        expected_arguments,
    );
    let mismatch = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-expected",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "observed",
            "source_plan_digest": expected_plan["plan_digest"].clone(),
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(mismatch["__isError"], json!(true));
    assert!(mismatch["error"]
        .as_str()
        .unwrap()
        .contains("response digest differs"));
    let second = call(&mut server, "domain_evidence_source_plan", arguments);
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    let credential_refused = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-refused",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": "https://user:secret@example.org/evidence",
            "retrieval_mode": "content",
            "does_not_claim": ["retrieval occurred"]
        }),
    );
    assert_eq!(credential_refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_source_execute_reads_confined_file_and_retains_raw_and_json_digests() {
    let mut server = server();
    let planned = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-subject",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "fixtures/fiber-v0.1/leakage_query.json",
            "retrieval_mode": "content",
            "retrieval_policy": {"network": "disabled", "max_bytes": 65536, "cache": "content_addressed"},
            "does_not_claim": ["source truth", "scientific validity"]
        }),
    );
    let executed = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": planned["plan_digest"].clone()}),
    );
    assert_eq!(
        executed["workflow"],
        json!("domain_evidence_source_execute")
    );
    assert_eq!(executed["outcome"], json!("observed"));
    assert_eq!(
        executed["intake"]["workflow"],
        json!("domain_evidence_intake")
    );
    assert_eq!(
        executed["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(executed["raw_content_digest"].as_str().unwrap().len(), 64);
    assert_eq!(executed["response_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        executed["execution_result"]["response"]["retrieval"]["body_encoding"],
        json!("json")
    );
    assert!(executed["intake"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|digest| digest == &planned["artifact_registry"]["content_digest"]));
    let repeated = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": planned["plan_digest"].clone()}),
    );
    assert_eq!(repeated["outcome"], json!("observed"));
    assert_eq!(
        repeated["intake"]["artifact_registry"]["already_present"],
        json!(true)
    );

    let traversal_plan = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-refused",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "../outside.json",
            "retrieval_mode": "content",
            "does_not_claim": ["source truth"]
        }),
    );
    let refused = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": traversal_plan["plan_digest"].clone()}),
    );
    assert_eq!(refused["outcome"], json!("refused"));
    assert_eq!(refused["intake"]["outcome"], json!("refused"));
    assert_eq!(
        refused["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
}

#[test]
fn domain_evidence_provider_normalize_retains_caller_managed_payload_with_explicit_digests() {
    let mut server = server();
    let planned = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "locator_kind": "opaque",
            "locator": "caller://pubmed/query/oncology",
            "retrieval_mode": "reference_only",
            "does_not_claim": ["provider authenticity", "clinical truth"]
        }),
    );
    let normalized = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "pubmed",
            "payload": {"records": [{"id": "pmid:1", "title": "opaque"}]},
            "request": {"query": "oncology"},
            "outcome": "observed",
            "source_plan_digest": planned["plan_digest"].clone()
        }),
    );
    assert_eq!(
        normalized["workflow"],
        json!("domain_evidence_provider_normalize")
    );
    assert_eq!(normalized["connector_kind"], json!("literature"));
    assert_eq!(normalized["provider"], json!("pubmed"));
    assert_eq!(normalized["payload_digest"].as_str().unwrap().len(), 64);
    assert_eq!(normalized["shape_audit"]["status"], json!("structured"));
    assert_eq!(
        normalized["shape_audit"]["recognized_container"],
        json!("records")
    );
    assert_eq!(
        normalized["shape_audit"]["identifier_coverage"]["present_record_count"],
        json!(1)
    );
    assert_eq!(normalized["record_index"]["indexed_record_count"], json!(1));
    assert_eq!(normalized["record_index"]["omitted_record_count"], json!(0));
    assert_eq!(normalized["intake"]["outcome"], json!("observed"));
    assert_eq!(
        normalized["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert!(normalized["intake"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|digest| digest == &planned["artifact_registry"]["content_digest"]));

    let unknown = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-unknown",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "caller",
            "payload": {"records": []}
        }),
    );
    assert_eq!(unknown["outcome"], json!("unknown"));
    assert_eq!(unknown["intake"]["outcome"], json!("unknown"));
    assert_eq!(unknown["shape_audit"]["status"], json!("structured"));

    let fhir = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-fhir",
            "source_tool": "literature_bind_check",
            "connector_kind": "fhir",
            "provider": "caller",
            "payload": {"resourceType": "Bundle", "entry": [{"resource": {"resourceType": "Patient", "id": "opaque"}}]}
        }),
    );
    assert_eq!(fhir["shape_audit"]["recognized_container"], json!("entry"));
    assert_eq!(fhir["shape_audit"]["status"], json!("structured"));

    let object_store = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-object-store",
            "source_tool": "literature_bind_check",
            "connector_kind": "object_store",
            "provider": "caller",
            "payload": {"objects": [{"key": "opaque", "content_digest": "opaque"}]}
        }),
    );
    assert_eq!(object_store["shape_audit"]["status"], json!("structured"));
    assert_eq!(
        object_store["shape_audit"]["content_digest_coverage"]["present_record_count"],
        json!(1)
    );

    let replay = call(
        &mut server,
        "domain_evidence_provider_replay_verify",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "pubmed",
            "payload": {"records": [{"id": "pmid:1", "title": "opaque"}]},
            "request": {"query": "oncology"},
            "outcome": "observed",
            "parent_digests": normalized["intake"]["parent_digests"].clone(),
            "source_plan_digest": planned["plan_digest"].clone(),
            "expected_payload_digest": normalized["payload_digest"].clone(),
            "expected_request_digest": normalized["request_digest"].clone(),
            "expected_shape_digest": normalized["shape_audit"]["shape_digest"].clone(),
            "expected_normalization_digest": ContentHash::of_value(&normalized["normalization"])
                .unwrap()
                .to_string(),
            "expected_intake_digest": normalized["intake"]["intake_digest"].clone()
        }),
    );
    assert_eq!(replay["replay_status"], json!("matched"));
    assert_eq!(replay["matched"], json!(true));
    assert_eq!(replay["replay"]["differences"], json!([]));
    assert_eq!(replay["artifact_registry"]["created"], json!(true));
    assert_eq!(replay["replay_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn domain_evidence_provider_connector_handoff_is_scoped_secret_safe_and_idempotent() {
    let mut server = server();
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology", "genomics"],
        "subject_id": "connector-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "manifest": {
            "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
            "connector_id": "caller.pubmed",
            "version": "1.2.0",
            "provider": "pubmed",
            "connector_kind": "literature",
            "domains": ["genomics", "oncology"],
            "capabilities": ["query", "retain"],
            "transport": "caller_managed",
            "auth_posture": {
                "status": "caller_asserted",
                "secret_refs": ["secret://caller/pubmed"],
                "does_not_claim": ["provider authentication"]
            }
        },
        "status": "prepared",
        "request_digest": "a".repeat(64),
        "payload_digest": "b".repeat(64),
        "source_plan_digest": "c".repeat(64),
        "parent_digests": ["d".repeat(64)],
        "attempt_id": "attempt-1"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_connector_handoff")
    );
    assert_eq!(first["execution"], json!("not_started"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["handoff"]["status"], json!("prepared"));
    assert_eq!(
        first["handoff"]["manifest"]["auth_posture"]["secret_refs"][0],
        json!("secret://caller/pubmed")
    );
    assert_eq!(first["handoff_digest"].as_str().unwrap().len(), 64);
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    let second = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        request,
    );
    assert_eq!(second["handoff_digest"], first["handoff_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    assert!(!serde_json::to_string(&first["handoff"])
        .unwrap()
        .contains("credential_material"));

    let refused = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "connector-refused",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "manifest": {
                "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
                "connector_id": "caller.pubmed",
                "version": "1.2.0",
                "provider": "pubmed",
                "connector_kind": "literature",
                "domains": ["oncology"],
                "capabilities": ["query"],
                "transport": "caller_managed",
                "auth_posture": {"status": "unknown", "does_not_claim": ["auth"]}
            },
            "credential_material": "must-refuse"
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_receipt_is_out_of_line_and_restart_safe() {
    let mut server = server();
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-2026-08-17-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/1",
        "content_type": "application/json",
        "content_encoding": "gzip",
        "request_digest": "c".repeat(64),
        "parent_digests": ["d".repeat(64)],
        "availability": "available",
        "retention": "durable",
        "attempt_id": "attempt-1"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_external_payload_receipt")
    );
    assert_eq!(first["receipt"]["byte_length"], json!(4096));
    assert_eq!(first["receipt"]["retention"], json!("durable"));
    assert_eq!(first["receipt"]["readiness_claimed"], json!(false));
    assert_eq!(first["receipt"]["execution"], json!("not_started"));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["receipt_digest"].as_str().unwrap().len(), 64);
    assert_eq!(first["receipt"]["handoff_digest"], json!("a".repeat(64)));
    assert!(!serde_json::to_string(&first)
        .unwrap()
        .contains("credential_material"));

    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        request,
    );
    assert_eq!(second["receipt_digest"], first["receipt_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let refused = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "external-provider-refused",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "handoff_digest": "a".repeat(64),
            "transfer_id": "export-2",
            "payload_digest": "b".repeat(64),
            "byte_length": 1,
            "storage_backend": "object_store",
            "locator_kind": "uri",
            "locator": "https://user:pass@example.org/object",
            "credential_material": "never"
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_replay_is_metadata_only_and_idempotent() {
    let mut server = server();
    let receipt = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-2026-08-17-replay-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/1",
        "availability": "available",
        "retention": "durable"
    });
    let recorded = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        receipt.clone(),
    );
    let replay_request = json!({
        "expected_receipt_digest": recorded["receipt_digest"],
        "expected_handoff_digest": "a".repeat(64),
        "expected_payload_digest": "b".repeat(64),
        "expected_byte_length": 4096,
        "group_id": receipt["group_id"],
        "domains": receipt["domains"],
        "subject_id": receipt["subject_id"],
        "source_tool": receipt["source_tool"],
        "provider": receipt["provider"],
        "connector_kind": receipt["connector_kind"],
        "handoff_digest": receipt["handoff_digest"],
        "transfer_id": receipt["transfer_id"],
        "payload_digest": receipt["payload_digest"],
        "byte_length": receipt["byte_length"],
        "storage_backend": receipt["storage_backend"],
        "locator_kind": receipt["locator_kind"],
        "locator": receipt["locator"],
        "availability": receipt["availability"],
        "retention": receipt["retention"]
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        replay_request.clone(),
    );
    assert_eq!(first["replay_status"], json!("matched"));
    assert_eq!(first["matched"], json!(true));
    assert_eq!(first["replay"]["matches"]["receipt_digest"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert!(first["replay"]["receipt"].get("records").is_none());
    assert!(first["replay"]["receipt"]
        .get("credential_material")
        .is_none());
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        replay_request,
    );
    assert_eq!(second["replay_digest"], first["replay_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let mut mismatch_request = receipt;
    mismatch_request["byte_length"] = json!(8192);
    mismatch_request["expected_receipt_digest"] =
        first["replay"]["expected_receipt_digest"].clone();
    mismatch_request["expected_handoff_digest"] = json!("a".repeat(64));
    mismatch_request["expected_payload_digest"] = json!("b".repeat(64));
    mismatch_request["expected_byte_length"] = json!(4096);
    let mismatch = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        mismatch_request,
    );
    assert_eq!(mismatch["replay_status"], json!("mismatch"));
    assert_eq!(mismatch["matched"], json!(false));
    assert_eq!(
        mismatch["replay"]["differences"],
        json!(["byte_length", "receipt_digest"])
    );
}

#[test]
fn domain_evidence_provider_external_payload_normalize_requires_digest_verified_materialization() {
    let mut server = server();
    let payload = json!({"records": [{"id": "pmid:1", "title": "opaque"}]});
    let payload_digest = ContentHash::of_value(&payload).unwrap().to_string();
    let byte_length = serde_json::to_vec(&payload).unwrap().len() as u64;
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-materialized",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-materialized-1",
        "payload_digest": payload_digest,
        "byte_length": byte_length,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/materialized-1",
        "availability": "available",
        "retention": "durable",
        "payload": payload,
        "outcome": "observed"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_external_payload_normalize")
    );
    assert_eq!(first["materialization"]["matched"], json!(true));
    assert_eq!(first["materialization"]["locator_opened"], json!(false));
    assert_eq!(
        first["normalization"]["payload_digest"],
        first["payload_digest"]
    );
    assert_eq!(first["normalization"]["outcome"], json!("observed"));
    assert_eq!(first["receipt_artifact_registry"]["created"], json!(true));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));

    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        request.clone(),
    );
    assert_eq!(second["receipt_digest"], first["receipt_digest"]);
    assert_eq!(second["receipt_artifact_registry"]["created"], json!(false));

    let mut drift = request;
    drift["payload"] = json!({"records": [{"id": "pmid:drift"}]});
    let refused = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        drift,
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_lineage_audit_reconciles_handoff_scope_and_payload() {
    let mut server = server();
    let handoff = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "lineage-subject",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "manifest": {
                "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
                "connector_id": "caller.pubmed",
                "version": "1.2.0",
                "provider": "pubmed",
                "connector_kind": "literature",
                "domains": ["oncology"],
                "capabilities": ["query", "retain"],
                "transport": "caller_managed",
                "auth_posture": {
                    "status": "caller_asserted",
                    "secret_refs": ["secret://caller/pubmed"],
                    "does_not_claim": ["provider authentication"]
                }
            },
            "status": "prepared",
            "payload_digest": "b".repeat(64)
        }),
    );
    let receipt = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "lineage-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": handoff["handoff_digest"].clone(),
        "transfer_id": "transfer-lineage-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/lineage-1",
        "availability": "available",
        "retention": "durable"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        receipt.clone(),
    );
    assert_eq!(first["lineage_status"], json!("matched"));
    assert_eq!(first["payload_binding_status"], json!("matched"));
    assert_eq!(first["audit"]["matches"]["payload_digest"], json!(true));
    assert_eq!(first["receipt_registry"]["created"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        receipt,
    );
    assert_eq!(second["lineage_digest"], first["lineage_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let orphaned = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "orphaned-lineage-subject",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "handoff_digest": "c".repeat(64),
            "transfer_id": "transfer-lineage-orphan",
            "payload_digest": "d".repeat(64),
            "byte_length": 1,
            "storage_backend": "caller_managed",
            "locator_kind": "opaque",
            "locator": "caller://orphaned",
            "availability": "unknown",
            "retention": "unknown"
        }),
    );
    assert_eq!(orphaned["lineage_status"], json!("orphaned"));
    assert_eq!(orphaned["payload_binding_status"], json!("not_available"));
    assert_eq!(orphaned["readiness_claimed"], json!(false));
}

#[test]
fn domain_evidence_provider_external_payload_execution_evidence_is_observation_bound_and_idempotent(
) {
    let mut server = server();
    let base = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "execution-evidence-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "transfer-execution-evidence-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/execution-evidence-1",
        "availability": "available",
        "retention": "durable"
    });
    let receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        base.clone(),
    );
    let mut matched_request = base.clone();
    matched_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    matched_request["execution_status"] = json!("transferred");
    matched_request["executor_id"] = json!("caller-transfer-worker");
    matched_request["observed_payload_digest"] = json!("b".repeat(64));
    matched_request["observed_byte_length"] = json!(4096);
    matched_request["locator_opened"] = json!(true);
    matched_request["observation_digest"] = json!("c".repeat(64));
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        matched_request.clone(),
    );
    assert_eq!(first["evidence_status"], json!("matched"));
    assert_eq!(
        first["evidence"]["matches"]["observed_payload_digest"],
        json!(true)
    );
    assert_eq!(first["receipt_registry"]["already_present"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        matched_request,
    );
    assert_eq!(second["evidence_digest"], first["evidence_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let mut partial_request = base.clone();
    partial_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    partial_request["execution_status"] = json!("transferred");
    partial_request["executor_id"] = json!("caller-transfer-worker");
    partial_request["observed_payload_digest"] = json!("b".repeat(64));
    let partial = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        partial_request,
    );
    assert_eq!(partial["evidence_status"], json!("partial"));

    let mut mismatch_request = base.clone();
    mismatch_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    mismatch_request["execution_status"] = json!("transferred");
    mismatch_request["executor_id"] = json!("caller-transfer-worker");
    mismatch_request["observed_payload_digest"] = json!("d".repeat(64));
    mismatch_request["observed_byte_length"] = json!(4096);
    let mismatch = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        mismatch_request,
    );
    assert_eq!(mismatch["evidence_status"], json!("mismatch"));

    let mut orphaned_request = base;
    orphaned_request["expected_receipt_digest"] = json!("e".repeat(64));
    orphaned_request["execution_status"] = json!("unknown");
    orphaned_request["executor_id"] = json!("caller-transfer-worker");
    let orphaned = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        orphaned_request,
    );
    assert_eq!(orphaned["evidence_status"], json!("orphaned"));
}

#[test]
fn domain_evidence_provider_external_payload_evidence_query_joins_rows_and_paginates_deterministically(
) {
    let mut server = server();
    let first = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "query-subject-1",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "query-transfer-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/query/1",
        "availability": "available",
        "retention": "durable"
    });
    let second = json!({
        "group_id": "biological_domains",
        "domains": ["genomics"],
        "subject_id": "query-subject-2",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "c".repeat(64),
        "transfer_id": "query-transfer-2",
        "payload_digest": "d".repeat(64),
        "byte_length": 2048,
        "storage_backend": "caller_managed",
        "locator_kind": "opaque",
        "locator": "caller://query/2",
        "availability": "unknown",
        "retention": "unknown"
    });
    let first_receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        first,
    );
    let _second_receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        second,
    );
    let page = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"max_items": 1, "include_artifacts": true}),
    );
    assert_eq!(page["ok"], json!(true));
    assert_eq!(
        page["workflow"],
        json!("domain_evidence_provider_external_payload_evidence_query")
    );
    assert_eq!(page["rows"].as_array().unwrap().len(), 1);
    assert_eq!(page["rows"][0]["join_status"], json!("receipt_only"));
    assert!(page["rows"][0].get("receipt_artifact").is_some());
    assert_eq!(page["has_more"], json!(true));
    let next_after = page["next_after"].clone();
    assert_ne!(next_after, Value::Null);
    let next = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"after": next_after, "max_items": 2}),
    );
    assert_eq!(next["ok"], json!(true));
    assert_eq!(next["rows"].as_array().unwrap().len(), 1);
    assert_eq!(next["has_more"], json!(false));
    let filtered = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"subject_id": "query-subject-1"}),
    );
    assert_eq!(filtered["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        filtered["rows"][0]["receipt_digest"],
        first_receipt["receipt_digest"]
    );
    assert_eq!(filtered["readiness_claimed"], json!(false));
}

#[test]
fn hub_disclosure_replay_preserves_ratchet_and_refuses_unacknowledged_scores() {
    let mut server = server();
    let pack = ContentHash::of_bytes(b"mcp-disclosure-pack");
    let second_pack = ContentHash::of_bytes(b"mcp-split-integrity-pack");
    let payload = call(
        &mut server,
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() },
                { "kind": "disclose", "pack": serde_json::to_value(&pack).unwrap(), "at": 5 },
                { "kind": "headline_eligibility", "pack": serde_json::to_value(&pack).unwrap(), "computed_at": 6 },
                { "kind": "headline_eligibility", "pack": serde_json::to_value(&pack).unwrap(), "computed_at": 6, "acknowledges_disclosure": true },
                { "kind": "contaminate", "pack": serde_json::to_value(&pack).unwrap(), "witness": { "kind": "training_corpus_overlap", "detail": "training snapshot contains the public instances", "observed_at": 7, "reported_by": "audit-1" } },
                { "kind": "declare_held_out", "pack": serde_json::to_value(&second_pack).unwrap() },
                { "kind": "split_integrity", "pack": serde_json::to_value(&second_pack).unwrap(), "at": 8, "reported_by": "oracle-1", "verdict": { "status": "invalid", "witnesses": [{ "type": "identity_leakage", "alias": "ALT-1", "subjects": ["S1", "S2"], "splits": ["train", "test"] }], "oracle_kind": "deterministic_split_integrity_v1" } }
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["schema"], json!("bioprism-mcp/hub-disclosure/0.1"));
    assert_eq!(payload["action_failures"], json!(0));
    assert_eq!(payload["trace"][2]["result"]["eligible"], json!(false));
    assert_eq!(payload["trace"][2]["result"]["fail_closed"], json!(true));
    assert_eq!(payload["trace"][3]["result"]["eligible"], json!(true));
    assert_eq!(
        payload["trace"][4]["result"]["state"]["disclosure"],
        json!("contaminated")
    );
    assert_eq!(
        payload["trace"][6]["result"]["state"]["disclosure"],
        json!("contaminated")
    );
    assert_eq!(payload["entries"].as_array().unwrap().len(), 2);
    assert!(payload["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("ratchet")));
}

#[test]
fn hub_cards_and_leaderboards_compose_moderation_disclosure_and_comparability_gates() {
    let mut server = server();
    let review = call(
        &mut server,
        "hub_submission_review",
        hub_review_fixture("sub-mcp-card", b"mcp-card-artifact"),
    );
    assert_eq!(review["ok"], json!(true));

    let pack = ContentHash::of_bytes(b"mcp-board-pack");
    let disclosure = call(
        &mut server,
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() }
            ]
        }),
    );

    let withheld = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card"
        }),
    );
    assert_eq!(withheld["ok"], json!(true));
    assert_eq!(withheld["schema"], json!("bioprism-mcp/hub-card/0.1"));
    assert_eq!(withheld["card"]["state"], json!("available"));
    assert_eq!(withheld["card"]["score"]["display"], json!("withheld"));

    let refused = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card",
            "score": serde_json::to_value(HubScore::point(0.82)).unwrap(),
            "pack": serde_json::to_value(&pack).unwrap(),
            "computed_at": 4
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("card_disclosure_gate"));
    assert_eq!(refused["score"], Value::Null);
    assert_eq!(refused["fail_closed"], json!(true));

    let published = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card",
            "score": serde_json::to_value(HubScore::point(0.82)).unwrap(),
            "pack": serde_json::to_value(&pack).unwrap(),
            "computed_at": 4,
            "disclosure": disclosure["ledger"].clone()
        }),
    );
    assert_eq!(published["ok"], json!(true));
    assert_eq!(published["card"]["score"]["display"], json!("published"));
    assert_eq!(published["card"]["score"]["score"]["value"], json!(0.82));
    assert_eq!(published["score"]["attached"], json!(true));

    let conditions = ComparabilityConditions {
        pack,
        pack_version: "1.0.0".into(),
        split: "hidden-holdout".into(),
        metric: "first-divergence-rate".into(),
        higher_is_better: false,
        oracle_tier: "deterministic".into(),
        access_mode: AccessTier::Public,
        budget: BudgetEnvelope::unbounded(),
        protocol: ContentHash::of_bytes(b"mcp-scoring-protocol"),
    };
    let board = Board {
        id: BoardId::parse("glioma-first-divergence").unwrap(),
        conditions: conditions.clone(),
        min_verification: VerificationStatus::SelfReported,
    };
    let ranked_entry = Entry {
        submission: SubmissionId::parse("sub-mcp-card").unwrap(),
        conditions: conditions.clone(),
        score: HubScore::point(0.18),
        computed_at: HubEpoch(4),
        acknowledges_disclosure: false,
        scale: EvidenceScale::new(10, 5),
    };
    let unranked_entry = Entry {
        submission: SubmissionId::parse("sub-mcp-missing").unwrap(),
        conditions,
        score: HubScore::point(0.11),
        computed_at: HubEpoch(4),
        acknowledges_disclosure: false,
        scale: EvidenceScale::new(10, 5),
    };
    let leaderboard = call(
        &mut server,
        "hub_leaderboard_render",
        json!({
            "board": serde_json::to_value(board).unwrap(),
            "entries": [serde_json::to_value(ranked_entry).unwrap(), serde_json::to_value(unranked_entry).unwrap()],
            "moderation": review["ledger"].clone(),
            "disclosure": disclosure["ledger"].clone(),
            "include_details": true
        }),
    );
    assert_eq!(leaderboard["ok"], json!(true));
    assert_eq!(
        leaderboard["schema"],
        json!("bioprism-mcp/hub-leaderboard/0.1")
    );
    assert_eq!(leaderboard["ranked_count"], json!(1));
    assert_eq!(leaderboard["unranked_count"], json!(1));
    assert_eq!(leaderboard["leader_count"], json!(1));
    assert_eq!(leaderboard["rendered"]["ranked"][0]["rank"], json!(1));
    assert_eq!(
        leaderboard["rendered"]["unranked"][0]["reason"],
        json!({ "reason": "not_published", "state": null })
    );
    assert!(leaderboard["headline"].as_str().unwrap().contains("Rank 1"));
}

#[test]
fn bioatlas_publication_audit_binds_atlas_evidence_card_and_leaderboard_targets() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }

    let ontology = CapabilityOntology::from_nodes(
        "publication-atlas/1",
        [
            CapabilityNode::new(
                cap("agent"),
                "agent",
                CapabilityFamily::DomainReasoning,
                CapabilityDimension::Competence,
            ),
            CapabilityNode::new(
                cap("measured"),
                "measured",
                CapabilityFamily::Verification,
                CapabilityDimension::Reliability,
            )
            .with_parent(cap("agent")),
            CapabilityNode::new(
                cap("efficient"),
                "efficient",
                CapabilityFamily::ToolUse,
                CapabilityDimension::Efficiency,
            )
            .with_parent(cap("agent")),
        ],
    )
    .unwrap();
    let atlas = Atlas::builder(ontology)
        .evidence(EvidenceRecord::new(
            "agent-trial",
            cap("agent"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .evidence(EvidenceRecord::new(
            "measured-trial",
            cap("measured"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .evidence(EvidenceRecord::new(
            "efficient-trial",
            cap("efficient"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .build()
        .unwrap();

    let review = call(
        &mut server(),
        "hub_submission_review",
        hub_review_fixture("sub-bioatlas-publication", b"bioatlas-publication"),
    );
    assert_eq!(review["ok"], json!(true));
    let pack = ContentHash::of_bytes(b"bioatlas-publication-pack");
    let disclosure = call(
        &mut server(),
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() },
                { "kind": "disclose", "pack": serde_json::to_value(&pack).unwrap(), "at": 5 }
            ]
        }),
    );
    assert_eq!(disclosure["ok"], json!(true));

    let conditions = ComparabilityConditions {
        pack: pack.clone(),
        pack_version: "1.0.0".into(),
        split: "hidden-holdout".into(),
        metric: "atlas-publication-score".into(),
        higher_is_better: true,
        oracle_tier: "deterministic".into(),
        access_mode: AccessTier::Public,
        budget: BudgetEnvelope::unbounded(),
        protocol: ContentHash::of_bytes(b"bioatlas-publication-protocol"),
    };
    let board = Board {
        id: BoardId::parse("bioatlas-publication").unwrap(),
        conditions: conditions.clone(),
        min_verification: VerificationStatus::SelfReported,
    };
    let entry = Entry {
        submission: SubmissionId::parse("sub-bioatlas-publication").unwrap(),
        conditions,
        score: HubScore::point(0.91),
        computed_at: HubEpoch(5),
        acknowledges_disclosure: true,
        scale: EvidenceScale::new(10, 5),
    };

    let evidence_audit = json!({
        "vectors": [
            metric_vector("system-a", 0.91, 0.88, "pack/4"),
            metric_vector("system-b", 0.71, 0.69, "pack/4")
        ],
        "evidence": [{
            "id": "grounding-1",
            "dimension": "evidence_grounding",
            "status": "observed",
            "source": "publication-fixture",
            "scope": "public-atlas-card"
        }],
        "claim_requests": [{
            "id": "grounded-card",
            "claim": "the public atlas card is grounded in the supplied source",
            "requires": ["evidence_grounding"]
        }]
    });
    let atlas_json = serde_json::to_value(atlas).unwrap();
    let result = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({
            "atlas": atlas_json.clone(),
            "evidence_audit": evidence_audit,
            "card": {
                "moderation": review["ledger"].clone(),
                "submission": "sub-bioatlas-publication",
                "score": serde_json::to_value(HubScore::point(0.91)).unwrap(),
                "pack": serde_json::to_value(&pack).unwrap(),
                "computed_at": 5,
                "acknowledges_disclosure": true,
                "disclosure": disclosure["ledger"].clone()
            },
            "leaderboard": {
                "board": serde_json::to_value(board).unwrap(),
                "entries": [serde_json::to_value(entry).unwrap()],
                "moderation": review["ledger"].clone(),
                "disclosure": disclosure["ledger"].clone()
            },
            "release_request": {
                "id": "publication-fixture-release",
                "targets": [
                    "atlas_profile",
                    "atlas_aggregation",
                    "evidence_claims",
                    "card_render",
                    "numeric_card_score",
                    "ranked_leaderboard"
                ]
            }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioatlas-publication-audit/0.1")
    );
    assert_eq!(result["workflow"], json!("bioatlas_publication_audit"));
    assert_eq!(result["release_request"]["ready"], json!(true));
    assert_eq!(
        result["cross_layer"]["numeric_score_evidence_ready"],
        json!(true)
    );
    assert_eq!(
        result["cross_layer"]["leaderboard_unranked_count"],
        json!(0)
    );
    assert_eq!(result["card"]["score"]["attached"], json!(true));

    let no_request = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({ "atlas": atlas_json.clone() }),
    );
    assert_eq!(no_request["ok"], json!(true));
    assert_eq!(no_request["release_request"]["present"], json!(false));
    assert_eq!(no_request["release_request"]["ready"], json!(false));

    let numeric_without_evidence = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({
            "atlas": atlas_json,
            "card": {
                "moderation": review["ledger"].clone(),
                "submission": "sub-bioatlas-publication",
                "score": serde_json::to_value(HubScore::point(0.91)).unwrap(),
                "pack": serde_json::to_value(&pack).unwrap(),
                "computed_at": 5,
                "acknowledges_disclosure": true,
                "disclosure": disclosure["ledger"].clone()
            },
            "release_request": {
                "id": "numeric-without-evidence",
                "targets": ["numeric_card_score"]
            }
        }),
    );
    assert_eq!(
        numeric_without_evidence["release_request"]["ready"],
        json!(false)
    );
    assert!(
        numeric_without_evidence["release_request"]["targets"][0]["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| blocker == "evidence_audit_missing")
    );
}

#[test]
fn release_audit_composes_required_gates_and_keeps_advisory_impact_separate() {
    let mut server = server();
    let bundle = ResultBundle::builder("release-audit-bundle")
        .carrying("query", EntryRole::Query, json!({ "goal": "release" }))
        .unwrap()
        .build()
        .unwrap();
    let dataset = QualityDataset::new("release-dataset")
        .unwrap()
        .with_column("age", [json!(41), json!(42)])
        .unwrap();
    let gate = QualityGate::new("release-quality")
        .unwrap()
        .with(
            "age_range",
            QualityCheck::InRange {
                column: "age".into(),
                min: 0.0,
                max: 120.0,
            },
        )
        .unwrap();
    let catalogue = call(
        &mut server,
        "repository_catalog",
        json!({ "prefix": "docs/", "limit": 1 }),
    );
    let changed = catalogue["modules"][0]["id"]
        .as_str()
        .expect("repository catalog returns a module id")
        .to_string();
    let payload = call(
        &mut server,
        "release_audit",
        json!({
            "checks": [
                {
                    "kind": "bundle_verify",
                    "arguments": { "bundle": serde_json::to_value(&bundle).unwrap() }
                },
                {
                    "kind": "quality_gate_run",
                    "arguments": {
                        "dataset": serde_json::to_value(dataset).unwrap(),
                        "gate": serde_json::to_value(gate).unwrap()
                    }
                },
                {
                    "kind": "repository_impact",
                    "required": false,
                    "arguments": { "changed": changed }
                }
            ],
            "include_details": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["release_ready"], json!(true));
    assert_eq!(payload["required_check_count"], json!(2));
    assert_eq!(payload["blocking_count"], json!(0));
    assert_eq!(payload["checks"][0]["passed"], json!(true));
    assert_eq!(payload["checks"][1]["passed"], json!(true));
    assert_eq!(payload["checks"][2]["advisory"], json!(true));
    assert_eq!(payload["checks"][2]["gate"], Value::Null);
    assert!(payload["checks"][0]["result"].is_object());
    let mut tampered = serde_json::to_value(bundle).unwrap();
    tampered["contents"]["query"]["goal"] = json!("tampered");
    let blocked = call(
        &mut server,
        "release_audit",
        json!({
            "checks": [
                {
                    "kind": "bundle_verify",
                    "arguments": { "bundle": tampered }
                }
            ]
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(blocked["release_ready"], json!(false));
    assert_eq!(blocked["blocking_count"], json!(1));
    assert_eq!(blocked["checks"][0]["passed"], json!(false));
    assert_eq!(blocked["blockers"][0]["fail_closed"], json!(true));
}

#[test]
fn notifications_are_not_answered() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
    assert!(server.handle(&request).is_none());
}

#[test]
fn unknown_methods_and_malformed_input_produce_typed_errors() {
    let mut server = server();
    let request = Request::parse(r#"{"jsonrpc":"2.0","id":7,"method":"nope"}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();
    assert_eq!(response["error"]["code"], json!(-32601));

    let failure = Request::parse("not json at all").unwrap_err().to_json();
    assert_eq!(failure["error"]["code"], json!(-32700));
}

#[test]
fn malformed_json_rpc_envelopes_are_refused_before_dispatch() {
    let missing_version = Request::parse(r#"{"id":1,"method":"ping"}"#)
        .unwrap_err()
        .to_json();
    assert_eq!(missing_version["error"]["code"], json!(-32600));

    let scalar = Request::parse(r#"[1,2,3]"#).unwrap_err().to_json();
    assert_eq!(scalar["error"]["code"], json!(-32600));

    let bad_params = Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":true}"#)
        .unwrap_err()
        .to_json();
    assert_eq!(bad_params["error"]["code"], json!(-32600));
}

#[test]
fn tools_are_not_available_before_the_session_is_ready() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();
    assert_eq!(response["error"]["code"], json!(-32600));

    ready(&mut server);
    let response = server.handle(&request).unwrap().to_json();
    assert!(response["result"]["tools"].is_array());
}

#[test]
fn schemas_are_exposed_as_read_only_resources() {
    let mut server = server();
    ready(&mut server);

    let list = Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}}"#)
        .unwrap();
    let listed = server.handle(&list).unwrap().to_json();
    assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 4);

    let read = Request::parse(
        &json!({
            "jsonrpc":"2.0", "id":2, "method":"resources/read",
            "params": { "uri": CERTIFICATE_SCHEMA_URI }
        })
        .to_string(),
    )
    .unwrap();
    let document = server.handle(&read).unwrap().to_json();
    let text = document["result"]["contents"][0]["text"].as_str().unwrap();
    let schema: Value = serde_json::from_str(text).expect("resource is valid JSON");
    assert_eq!(schema["title"], json!("AURORA FIBER Context Certificate"));

    let capabilities = Request::parse(
        &json!({
            "jsonrpc":"2.0", "id":3, "method":"resources/read",
            "params": { "uri": CAPABILITIES_URI }
        })
        .to_string(),
    )
    .unwrap();
    let document = server.handle(&capabilities).unwrap().to_json();
    let text = document["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("world_and_ingestion"));
}

#[test]
fn workspace_capabilities_are_explicit_about_every_major_domain_surface() {
    let mut server = server();
    let payload = call(&mut server, "workspace_capabilities", json!({}));
    let capabilities = payload.as_array().expect("catalog is an array");
    assert!(capabilities.len() >= 10);
    for domain in [
        "world_and_ingestion",
        "decision_context",
        "trajectory_and_decision_cells",
        "benchmark_pack_portfolio",
        "evaluation_and_baselines",
        "mutation_and_causal_discovery",
        "bioevaluation_reference_contracts",
        "biological_domains",
        "biological_ir_and_query",
        "oncoworlds_identity_and_transport",
        "oncoworlds_models_and_assays",
        "oncoworlds_clonal_evolution",
        "safety_privacy_and_policy",
        "agent_orchestration",
        "registry_operations_and_infrastructure",
        "inference_lab",
        "oracle_mesh",
        "runtime_execution_and_replay",
        "release_and_reproduction",
        "documentation_and_knowledge",
        "developer_and_release_contracts",
    ] {
        assert!(
            capabilities.iter().any(|item| item["id"] == json!(domain)),
            "missing domain {domain}"
        );
    }
}

#[test]
fn domain_workflow_catalogue_covers_every_capability_group() {
    let mut server = server();
    let report = call(&mut server, "domain_workflow_catalogue", json!({}));
    assert_eq!(report["workflow"], json!("domain_workflow_catalogue"));
    assert_eq!(report["workflow_count"], json!(29));
    assert_eq!(report["coverage"]["group_count"], json!(29));
    assert_eq!(report["coverage"]["all_groups_have_workflow"], json!(true));
    assert_eq!(
        report["coverage"]["all_declared_tools_advertised"],
        json!(true)
    );
    assert_eq!(
        report["coverage"]["all_workflows_have_domain_contract"],
        json!(true)
    );
    assert_eq!(report["execution"], json!("not_started"));
    assert!(report["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|workflow| {
            workflow["workflow_id"].is_string()
                && workflow["workflow_digest"].is_string()
                && workflow["domain_contract"].is_object()
                && workflow["tool_contracts"].is_array()
                && workflow["recommended_stages"].is_array()
                && workflow["tool_contracts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|contract| contract["argument_contract"].is_object())
        }));
}

#[test]
fn domain_workflow_scaffolds_are_actionable_and_execution_disabled_for_every_group() {
    let mut server = server();
    let catalogue = call(&mut server, "domain_workflow_catalogue", json!({}));
    let workflows = catalogue["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 29);

    for workflow in workflows {
        let workflow_id = workflow["workflow_id"].as_str().unwrap();
        let report = call(
            &mut server,
            "domain_workflow_scaffold",
            json!({
                "workflow_id": workflow_id,
                "mission_id": format!("scaffold-{workflow_id}"),
                "goal": format!("prepare a reviewed starting plan for {workflow_id}")
            }),
        );
        assert_eq!(
            report["ok"], true,
            "scaffold failed for {workflow_id}: {report}"
        );
        assert_eq!(report["workflow"], "domain_workflow_scaffold");
        assert_eq!(report["execution"], "not_started");
        assert_eq!(report["mission"]["policy"]["execute"], false);
        assert_eq!(
            report["mission"]["workflow_binding"]["workflow_id"],
            workflow_id
        );
        assert!(!report["selection"]["selected_tools"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(matches!(
            report["preflight_status"].as_str(),
            Some("ready") | Some("blocked")
        ));
        assert_eq!(report["preflight_report"]["dispatch"], "not_started");
        assert_eq!(report["preflight_report"]["preflight"], true);
        assert_eq!(report["readiness_claimed"], false);
        assert!(report["next_actions"].as_array().unwrap().len() >= 2);
    }
}

#[test]
fn domain_workflow_bindings_cover_every_available_capability_group() {
    let capabilities = bioprism_mcp::workspace_capabilities();
    let definitions = Value::Array(tool_definitions());
    let catalogue = build_domain_workflow_catalogue(&capabilities, &definitions).unwrap();
    let workflows = catalogue["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 29);

    for workflow in workflows {
        let workflow_id = workflow["workflow_id"].as_str().unwrap();
        let tool = workflow["tools"]["available"]
            .as_array()
            .and_then(|tools| tools.first())
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("workflow {workflow_id} has no available tool"));
        let mission_id = format!("all-domain-binding-{workflow_id}");
        let report = instantiate_domain_workflow(
            &capabilities,
            &definitions,
            &json!({
                "workflow_id": workflow_id,
                "mission_id": mission_id,
                "goal": format!("exercise the {workflow_id} contract"),
                "steps": [{
                    "id": "contract-probe",
                    "tool": tool,
                    "arguments": {}
                }],
                "policy": {"execute": false}
            }),
        )
        .unwrap_or_else(|error| panic!("workflow {workflow_id} failed to instantiate: {error}"));
        let binding = &report["mission"]["workflow_binding"];
        assert_eq!(binding["workflow_id"], workflow["workflow_id"]);
        assert_eq!(binding["workflow_digest"], workflow["workflow_digest"]);
        assert_eq!(binding["catalog_digest"], workflow["catalog_digest"]);
        assert_eq!(
            binding["domain_contract_digest"],
            workflow["domain_contract_digest"]
        );
        assert_eq!(binding["domain_contract"], workflow["domain_contract"]);
        assert_eq!(binding["evidence_plan"], report["evidence_plan"]);
        assert_eq!(
            binding["evidence_plan_digest"],
            ContentHash::of_value(&report["evidence_plan"])
                .unwrap()
                .to_string()
        );
    }
}

#[test]
fn domain_workflow_reconciliation_preserves_outcomes_for_every_capability_group() {
    let capabilities = bioprism_mcp::workspace_capabilities();
    let definitions = Value::Array(tool_definitions());
    let catalogue = build_domain_workflow_catalogue(&capabilities, &definitions).unwrap();
    let workflows = catalogue["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 29);

    for workflow in workflows {
        let workflow_id = workflow["workflow_id"].as_str().unwrap();
        let tool = workflow["tools"]["available"]
            .as_array()
            .and_then(|tools| tools.first())
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("workflow {workflow_id} has no available tool"));
        let instantiation = instantiate_domain_workflow(
            &capabilities,
            &definitions,
            &json!({
                "workflow_id": workflow_id,
                "mission_id": format!("all-domain-reconcile-{workflow_id}"),
                "goal": format!("exercise retained evidence states for {workflow_id}"),
                "steps": [{"id": "outcome", "tool": tool, "arguments": {}}],
                "policy": {"execute": true}
            }),
        )
        .unwrap_or_else(|error| panic!("workflow {workflow_id} failed to instantiate: {error}"));
        let request: MissionRequest =
            serde_json::from_value(instantiation["mission"].clone()).unwrap();
        let plan = plan_mission(&request).unwrap();
        let step = &plan.steps[0];
        let report = |status: &str, wire: Option<Value>| {
            json!({
                "ok": true,
                "workflow": "agent_mission",
                "schema_version": "bioprism-devplat-mission/0.1",
                "plan": serde_json::to_value(&plan).unwrap(),
                "execution": "executed",
                "mission_status": if status == "succeeded" { "succeeded" } else { "failed" },
                "succeeded": usize::from(status == "succeeded"),
                "refused": usize::from(status == "refused"),
                "blocked": usize::from(status == "blocked"),
                "cancelled": usize::from(status == "cancelled"),
                "required_failures": usize::from(status != "succeeded"),
                "returned_bytes": if wire.is_some() { 12 } else { 0 },
                "results": [{
                    "id": step.id,
                    "tool": step.tool,
                    "status": status,
                    "required": step.required,
                    "arguments_digest": "a".repeat(64),
                    "bytes": if wire.is_some() { 12 } else { 0 },
                    "wire": wire,
                    "error": if status == "succeeded" { Value::Null } else { json!("explicit refusal") }
                }],
                "execution_trace_schema_version": "bioprism-devplat-mission-trace/0.1",
                "execution_trace": [
                    {"sequence": 0, "event": "mission.started", "wave": null, "step_id": null, "tool": null, "status": "running", "arguments_digest": null, "bytes": 0, "detail": null},
                    {"sequence": 1, "event": "mission.completed", "wave": null, "step_id": null, "tool": null, "status": if status == "succeeded" { "succeeded" } else { "failed" }, "arguments_digest": null, "bytes": if wire.is_some() { 12 } else { 0 }, "detail": null}
                ],
                "claim_requests": [],
                "claim_lineage": {},
                "guarantees": [],
                "limitations": []
            })
        };

        let success = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report("succeeded", Some(json!({"result": {"ok": true}})))
        }))
        .unwrap_or_else(|error| {
            panic!("workflow {workflow_id} success reconciliation failed: {error}")
        });
        assert_eq!(success["completion"]["status"], "complete");
        assert_eq!(success["completion"]["ready"], true);

        let refused = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report("refused", None)
        }))
        .unwrap_or_else(|error| {
            panic!("workflow {workflow_id} refusal reconciliation failed: {error}")
        });
        assert_eq!(refused["completion"]["status"], "failed");
        assert_eq!(refused["completion"]["ready"], false);
        assert_eq!(
            refused["evidence"]["rows"][0]["evidence_state"],
            "explicit_refusal"
        );

        let omitted = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report("succeeded", None)
        }))
        .unwrap_or_else(|error| {
            panic!("workflow {workflow_id} omission reconciliation failed: {error}")
        });
        assert_eq!(
            omitted["completion"]["status"],
            "complete_with_output_omissions"
        );
        assert_eq!(omitted["completion"]["ready"], false);
        assert_eq!(
            omitted["evidence"]["rows"][0]["evidence_state"],
            "completed_output_omitted"
        );

        let mut mismatched_report = report("succeeded", Some(json!({"result": {"ok": true}})));
        mismatched_report["plan"]["digest"] = json!("b".repeat(64));
        let mismatched = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": mismatched_report
        }))
        .unwrap_or_else(|error| {
            panic!("workflow {workflow_id} mismatch reconciliation failed: {error}")
        });
        assert_eq!(mismatched["integrity"]["valid"], false);
        assert_eq!(mismatched["completion"]["ready"], false);
        assert!(mismatched["integrity"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "mission_plan_digest_mismatch"));
    }
}

#[test]
fn domain_workflow_instantiation_is_scoped_and_preflighted_without_dispatch() {
    let mut server = server();
    let report = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-test",
            "goal": "discover the repository capability surface",
            "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}]
        }),
    );
    assert_eq!(report["workflow"], json!("domain_workflow_instantiate"));
    assert_eq!(
        report["mission"]["steps"][0]["tool"],
        json!("workspace_capabilities")
    );
    assert_eq!(
        report["selection"]["all_selected_tools_declared"],
        json!(true)
    );
    assert_eq!(
        report["selection"]["all_selected_tools_available"],
        json!(true)
    );
    assert_eq!(
        report["evidence_plan"]["steps"][0]["step_id"],
        json!("catalog")
    );
    assert_eq!(
        report["domain_contract"]["posture"],
        json!("advisory_review_gated")
    );
    assert_eq!(report["execution"], json!("not_started"));
    assert_eq!(
        report["preflight_report"]["workflow"],
        json!("agent_mission")
    );

    let refused = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-refused",
            "goal": "must refuse cross-group selection",
            "steps": [{"id": "compile", "tool": "bioql_compile"}]
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("outside workflow"));
}

#[test]
fn domain_workflow_reconciliation_binds_execution_results_to_the_instantiated_contract() {
    let mut server = server();
    let instantiation = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-reconcile",
            "goal": "reconcile repository capability evidence",
            "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
            "policy": {"execute": true}
        }),
    );
    let mission = call(
        &mut server,
        "agent_mission",
        instantiation["mission"].clone(),
    );
    assert_eq!(mission["mission_status"], json!("succeeded"));
    let reconciled = call(
        &mut server,
        "domain_workflow_reconcile",
        json!({"instantiation": instantiation, "mission_report": mission}),
    );
    assert_eq!(reconciled["workflow"], json!("domain_workflow_reconcile"));
    assert_eq!(reconciled["integrity"]["valid"], json!(true));
    assert_eq!(reconciled["completion"]["status"], json!("complete"));
    assert_eq!(reconciled["completion"]["ready"], json!(true));
    assert_eq!(reconciled["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        reconciled["artifact_registry"]["kind"],
        json!("workflow_reconciliation")
    );
    assert_eq!(
        reconciled["evidence"]["rows"][0]["result_retained"],
        json!(true)
    );

    let imported = call(
        &mut server,
        "domain_workflow_reconciliation_import",
        json!({"record": reconciled}),
    );
    assert_eq!(
        imported["workflow"],
        json!("domain_workflow_reconciliation_import")
    );
    // Executed workflow-bound missions are reconciled and indexed automatically; the explicit
    // import below must therefore exercise the registry's idempotent re-import path.
    assert_eq!(imported["created"], json!(false));
    assert_eq!(imported["already_present"], json!(true));
    assert_eq!(imported["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        imported["artifact_registry"]["content_digest"],
        reconciled["artifact_registry"]["content_digest"]
    );
    let digest = imported["reconciliation_digest"].as_str().unwrap();
    let queried = call(
        &mut server,
        "domain_workflow_reconciliation_query",
        json!({"mission_id": "workflow-reconcile", "completion_status": "complete"}),
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["reconciliation_digest"], json!(digest));
    let fetched = call(
        &mut server,
        "domain_workflow_reconciliation_get",
        json!({"reconciliation_digest": digest}),
    );
    assert_eq!(
        fetched["workflow"],
        json!("domain_workflow_reconciliation_get")
    );
    assert_eq!(fetched["record"]["reconciliation_digest"], json!(digest));
}

#[test]
fn mission_evaluator_discovery_covers_domains_without_executing_tools() {
    let mut server = server();
    let all = call(&mut server, "mission_evaluator_discover", json!({}));
    assert_eq!(all["ok"], json!(true));
    assert_eq!(all["workflow"], json!("mission_evaluator_discover"));
    assert_eq!(all["selection_posture"], json!("candidate_only"));
    assert_eq!(all["total_adapters"], json!(29));
    assert_eq!(all["result_count"], json!(29));
    assert_eq!(all["coverage"]["capability_group_count"], json!(29));
    assert_eq!(all["coverage"]["evaluator_group_count"], json!(29));
    assert_eq!(all["coverage"]["complete"], json!(true));
    assert_eq!(
        all["matches"][0]["adapter"]["status"],
        json!("candidate_only")
    );
    assert!(all["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "candidate tools are suggestions and were not executed"));

    let oncology = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "level": "evaluation", "max_items": 4}),
    );
    assert_eq!(oncology["result_count"], json!(1));
    assert_eq!(
        oncology["matches"][0]["adapter"]["id"],
        json!("oncoworlds.assay_fidelity")
    );
    assert_eq!(
        oncology["matches"][0]["adapter"]["group_id"],
        json!("oncoworlds_models_and_assays")
    );
}

#[test]
fn mission_evaluator_review_builds_claim_bindings_and_blocks_adversarial_rows() {
    let mut server = server();
    let discovery = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "level": "evaluation", "max_items": 4}),
    );
    let ready = call(
        &mut server,
        "mission_evaluator_review",
        json!({
            "discovery": discovery,
            "selections": [{
                "id": "assay-evaluator",
                "claim_id": "fidelity-claim",
                "adapter_id": "oncoworlds.assay_fidelity",
                "domain": "oncology",
                "step_id": "assay",
                "output_pointer": "/fidelity",
                "required": true
            }]
        }),
    );
    assert_eq!(ready["ok"], json!(true));
    assert_eq!(ready["workflow"], json!("mission_evaluator_review"));
    assert_eq!(ready["review_status"], json!("ready"));
    assert_eq!(
        ready["binding_posture"],
        json!("ready_for_mission_claim_bindings")
    );
    assert_eq!(ready["bindings"][0]["binding_posture"], json!("ready"));
    assert_eq!(
        ready["bindings"][0]["proposed_binding"]["step_id"],
        json!("assay")
    );
    assert_eq!(ready["execution"], json!("not_started"));

    let mission = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "reviewed-fidelity",
            "goal": "retain a reviewed evaluator output",
            "steps": [{
                "id": "assay",
                "domain": "oncology",
                "capability": "assay",
                "objective": "retain assay evidence",
                "tool": "workspace_capabilities"
            }],
            "claim_requests": [{
                "id": "fidelity-claim",
                "claim": "The assay output was retained for review.",
                "domains": ["oncology"],
                "requires_steps": ["assay"],
                "level": "evaluation",
                "evidence_mode": "successful_tool_result",
                "evaluator_bindings": [ready["bindings"][0]["proposed_binding"].clone()]
            }],
            "evaluator_review": ready
        }),
    );
    assert_eq!(mission["workflow"], json!("agent_mission"));
    assert_eq!(mission["execution"], json!("planned"));
    assert_eq!(
        mission["claim_lineage"]["evaluator_review"]["present"],
        json!(true)
    );
    assert_eq!(
        mission["claim_lineage"]["claims"][0]["evaluator_review"]["review_status"],
        json!("ready")
    );

    let replay = call(
        &mut server,
        "mission_evaluator_replay",
        json!({"mission": mission, "include_fixtures": true, "max_items": 64}),
    );
    assert_eq!(replay["workflow"], json!("mission_evaluator_replay"));
    assert_eq!(replay["execution"], json!("not_started"));
    assert_eq!(replay["coverage"]["catalogue_adapter_count"], json!(29));
    assert_eq!(replay["fixtures"].as_array().unwrap().len(), 29);
    assert_eq!(
        replay["fixtures"][0]["variants"].as_array().unwrap().len(),
        4
    );
    assert_eq!(replay["coverage"]["complete"], json!(false));

    let comparison = call(
        &mut server,
        "mission_evaluator_replay_compare",
        json!({"mission": mission.clone(), "include_fixtures": false, "max_items": 64}),
    );
    assert_eq!(
        comparison["workflow"],
        json!("mission_evaluator_replay_compare")
    );
    assert_eq!(comparison["catalog_drift"]["status"], json!("unchanged"));
    assert_eq!(comparison["catalog_drift"]["digest_match"], json!(true));
    let mut drifted_mission = mission.clone();
    drifted_mission["claim_lineage"]["evaluator_review"]["catalog_digest"] = json!("a".repeat(64));
    let drifted_comparison = call(
        &mut server,
        "mission_evaluator_replay_compare",
        json!({"mission": drifted_mission, "include_fixtures": false, "max_items": 64}),
    );
    assert_eq!(
        drifted_comparison["catalog_drift"]["status"],
        json!("drifted")
    );

    let mut bundle = json!({
        "schema": "bioprism-api/mission-evidence-bundle/0.1",
        "workflow": "mission_evidence_bundle_export",
        "mission_id": "mission-protocol",
        "retention": {"mode": "summary_only", "result_retained": false, "result_included": false, "summary_retained": true},
        "result": Value::Null,
        "result_digest": "d".repeat(64),
        "evaluator_replay": {"workflow": "mission_evaluator_replay_summary"},
        "catalog_drift": {"status": "not_recorded"},
        "trace": [{"sequence": 1, "event": "mission_succeeded"}],
        "export": {"format": "json", "include_result": false, "include_trace": true, "trace_included": true, "digest_algorithm": "sha256", "execution": "not_started"}
    });
    bundle["bundle_digest"] = json!(ContentHash::of_value(&bundle).unwrap().to_string());
    let verified = call(
        &mut server,
        "mission_evidence_bundle_verify",
        json!({"bundle": bundle.clone()}),
    );
    assert_eq!(
        verified["workflow"],
        json!("mission_evidence_bundle_verify")
    );
    assert_eq!(verified["valid"], json!(true));
    let imported_bundle = call(
        &mut server,
        "mission_evidence_bundle_import",
        json!({"bundle": bundle.clone()}),
    );
    assert_eq!(imported_bundle["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        imported_bundle["artifact_registry"]["kind"],
        json!("mission_evidence_bundle")
    );
    bundle["catalog_drift"]["status"] = json!("drifted");
    let tampered = call(
        &mut server,
        "mission_evidence_bundle_verify",
        json!({"bundle": bundle}),
    );
    assert_eq!(tampered["valid"], json!(false));

    let replayed_inconsistent = call(
        &mut server,
        "mission_evaluator_replay",
        json!({"mission": {"workflow": "agent_mission", "plan": {"mission_id": "replay-inconsistent"}, "mission_status": "planned", "claim_lineage": {"claims": [{"id": "fidelity-claim", "evaluator_bindings": [{"id": "assay-evaluator", "adapter_id": "oncoworlds.assay_fidelity", "domain": "oncology", "step_id": "assay", "output_pointer": "/fidelity", "required": true, "outcome_state": "retained", "output_digest": "x".repeat(64)}], "evaluator_coverage": {"outcome_counts": {}, "distinct_output_digests": 1, "disagreement_posture": "single_observation"}}]}}}),
    );
    assert_eq!(replayed_inconsistent["replay_status"], json!("blocked"));
    assert!(replayed_inconsistent["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| { finding["code"] == json!("outcome_count_mismatch") }));

    let mut mismatched_review = ready.clone();
    mismatched_review["bindings"][0]["domain"] = json!("unrelated");
    let rejected = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "reviewed-fidelity-rejected",
            "goal": "reject a stale binding",
            "steps": [{"id": "assay", "domain": "oncology", "capability": "assay", "objective": "retain", "tool": "workspace_capabilities"}],
            "claim_requests": [{
                "id": "fidelity-claim",
                "claim": "The assay output was retained for review.",
                "domains": ["oncology"],
                "requires_steps": ["assay"],
                "evaluator_bindings": [ready["bindings"][0]["proposed_binding"].clone()]
            }],
            "evaluator_review": mismatched_review
        }),
    );
    assert_eq!(rejected["__isError"], json!(true));

    let discovery_for_blocked = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "max_items": 4}),
    );
    let blocked = call(
        &mut server,
        "mission_evaluator_review",
        json!({
            "discovery": discovery_for_blocked,
            "selections": [
                {
                    "id": "duplicate",
                    "claim_id": "fidelity-claim",
                    "adapter_id": "oncoworlds.assay_fidelity",
                    "domain": "unrelated-domain",
                    "step_id": "assay",
                    "output_pointer": "/bad~2pointer"
                },
                {
                    "id": "duplicate",
                    "claim_id": "fidelity-claim",
                    "adapter_id": "not-in-discovery",
                    "domain": "oncology",
                    "step_id": "assay-2",
                    "output_pointer": ""
                }
            ]
        }),
    );
    assert_eq!(blocked["review_status"], json!("blocked"));
    assert_eq!(
        blocked["binding_posture"],
        json!("requires_caller_correction")
    );
    assert!(blocked["findings"].as_array().unwrap().len() >= 4);
    assert!(blocked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| {
            finding["message"] == json!("selection.id must be unique within the review")
        }));
    assert!(blocked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| {
            finding["message"] == json!("selection.output_pointer must be a valid RFC 6901 pointer")
        }));
}

#[test]
fn world_validation_is_available_before_context_compilation() {
    let mut server = server();
    let payload = call(&mut server, "world_validate", json!({ "world": WORLD }));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["counts"]["facts"], json!(761));
    assert!(payload["world_sha256"].as_str().unwrap().len() >= 32);
    assert!(payload["diagnostics"].is_array());
}

#[test]
fn context_comparison_exposes_the_equal_engineering_panel() {
    let mut server = server();
    let payload = call(
        &mut server,
        "context_compare",
        json!({ "world": WORLD, "query": QUERY }),
    );
    assert!(payload["results"].is_array());
    let fiber = payload["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == json!("fiber"))
        .expect("baseline panel includes fiber");
    assert!(fiber["facts_exposed"].is_number());
    assert!(fiber["admissible"].is_boolean());
}

#[test]
fn bioworlds_catalog_runs_reference_slices_and_keeps_unbuilt_worlds_explicit() {
    let mut server = server();
    let payload = call(&mut server, "bioworlds_catalog", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["mode"], json!("catalog"));
    assert_eq!(payload["slice_count"], json!(4));
    assert!(payload["report"]["digest"].is_string());
    assert!(payload["report"]["slices"].as_array().unwrap().len() >= 4);
    assert!(payload["unbuilt_worlds"].as_array().unwrap().len() >= 10);
}

#[test]
fn modality_catalog_exposes_resolution_and_failure_mode_boundaries() {
    let mut server = server();
    let payload = call(
        &mut server,
        "modality_catalog",
        json!({ "modality": "single-cell and multiome", "include_failure_modes": true }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["returned"], json!(1));
    assert_eq!(payload["modalities"][0]["blueprint_module"], json!("28.04"));
    assert!(payload["modalities"][0]["resolutions"].is_array());
    assert!(payload["modalities"][0]["failure_modes"].is_array());
    assert!(payload["unmechanised_failure_modes"].is_number());
}

#[test]
fn modality_support_check_separates_claim_eligibility_from_analysis_unit() {
    let refused = call(
        &mut server(),
        "modality_support_check",
        json!({
            "modality": "bulk_transcriptomics",
            "claim": "cell_intrinsic_change",
            "counted_unit": "population"
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(
        refused["support"]["root_refusal_kind"],
        json!("missing_resolution")
    );
    assert_eq!(refused["descriptor"]["complete"], json!(true));
    assert_eq!(refused["analysis_unit"]["admissible"], json!(false));
    assert_eq!(
        refused["analysis_unit"]["refusal_kind"],
        json!("named_failure_mode")
    );

    let admitted = call(
        &mut server(),
        "modality_support_check",
        json!({
            "modality": "single_cell",
            "claim": "cell_composition",
            "counted_unit": "subject"
        }),
    );
    assert_eq!(admitted["outcome_kind"], json!("supported"));
    assert_eq!(admitted["supported"], json!(true));
    assert_eq!(admitted["analysis_unit"]["admissible"], json!(true));
    assert!(admitted["claim_requirements"]["axes"].is_array());
    assert!(admitted["descriptor"]["supported_catalogue_claims"].is_array());
}

#[test]
fn modality_transport_check_preserves_loss_fidelity_and_support_changes() {
    let aggregated = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "single_cell",
            "to": "bulk_transcriptomics",
            "axis": "cell",
            "transport": {"kind": "aggregation", "operator": "mean"},
            "claims": ["cell_intrinsic_change", "cell_composition"]
        }),
    );
    assert_eq!(aggregated["ok"], json!(true));
    assert_eq!(aggregated["outcome_kind"], json!("constructed"));
    assert_eq!(aggregated["constructed"], json!(true));
    assert_eq!(aggregated["fidelity"]["fidelity"], json!("exact"));
    assert_eq!(aggregated["inverse"]["invertible"], json!(false));
    assert_eq!(aggregated["scope_mapping_check"], json!("sound"));
    assert!(aggregated["loss"]["discarded"].as_array().unwrap().len() >= 2);
    assert_eq!(aggregated["application"]["applied"], json!(true));
    assert_eq!(aggregated["claims"][0]["support_lost"], json!(true));

    let deconvolved = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "bulk_transcriptomics",
            "to": "single_cell",
            "axis": "cell",
            "transport": {"kind": "deconvolution", "reference": "signature-matrix-v1", "recomposition": "sum"},
            "claims": ["cell_composition", "cell_intrinsic_change"]
        }),
    );
    assert_eq!(deconvolved["fidelity"]["fidelity"], json!("estimated"));
    assert_eq!(deconvolved["inverse"]["invertible"], json!(true));
    assert_eq!(deconvolved["claims"][0]["after"]["supported"], json!(true));
    assert_eq!(deconvolved["claims"][1]["after"]["supported"], json!(false));

    let refused = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "bulk_transcriptomics",
            "to": "single_cell",
            "axis": "cell",
            "transport": {"kind": "deconvolution", "reference": "", "recomposition": "sum"}
        }),
    );
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["constructed"], json!(false));
    assert_eq!(
        refused["transport_evidence"]["refusal_kind"],
        json!("unstated_basis")
    );
}

#[test]
fn modality_comparability_check_blocks_category_errors_before_standards() {
    let term =
        TermBinding::exact("TP53", OntologyId::parse("HGNC:11998", "2026-01").unwrap()).unwrap();
    let rna = ModalMeasurement::new(
        bioprism_modalities::descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        Measurement::scalar("RNA abundance", Quantity::parse(1.0, "1").unwrap()).of(term.clone()),
    );
    let protein = ModalMeasurement::new(
        bioprism_modalities::descriptor(Modality::Proteomics),
        Resolution::Population,
        Measurement::scalar("protein abundance", Quantity::parse(1.0, "1").unwrap()).of(term),
    );
    let blocked = call(
        &mut server(),
        "modality_comparability_check",
        json!({
            "left": serde_json::to_value(&rna).unwrap(),
            "right": serde_json::to_value(&protein).unwrap()
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(blocked["outcome_kind"], json!("blocked"));
    assert_eq!(blocked["comparable"], json!(false));
    assert_eq!(
        blocked["report"]["verdict"]["reason"]["blocked_by"],
        json!("measurand_mismatch")
    );
    assert_eq!(blocked["report"]["standards"], Value::Null);
    assert_eq!(blocked["report_sha256"].as_str().unwrap().len(), 64);

    let comparable = call(
        &mut server(),
        "modality_comparability_check",
        json!({
            "left": serde_json::to_value(&rna).unwrap(),
            "right": serde_json::to_value(&rna).unwrap(),
            "policy": {"require_bound_terms": true}
        }),
    );
    assert_eq!(comparable["outcome_kind"], json!("comparable"));
    assert!(comparable["report"]["standards"].is_object());
}

#[test]
fn literature_bind_check_separates_source_binding_from_citation_support() {
    let published = Timestamp::parse("2026-01-01T00:00:00Z").unwrap();
    let population = ScopeKey::new().exact("disease", "diffuse_glioma");
    let claim = LiteratureClaim::new(
        "the source reports an observed cohort result",
        SourceProvenance::new(
            "doi:10.1000/example",
            LiteratureEvidenceTier::Primary,
            published,
        )
        .studying(population),
    );
    let bound = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&claim).unwrap(),
            "target": serde_json::to_value(ScopeKey::new().exact("disease", "diffuse_glioma").exact("site", "site-a")).unwrap(),
            "at_tier": "primary",
            "horizon": serde_json::to_value(EvaluationHorizon::open()).unwrap(),
            "claim_kind": serde_json::to_value(ModalityClaimKind::PublishedClaimSupport).unwrap()
        }),
    );
    assert_eq!(bound["ok"], json!(true));
    assert_eq!(bound["outcome_kind"], json!("citable"));
    assert_eq!(bound["bound"], json!(true));
    assert_eq!(bound["citable"], json!(true));
    assert_eq!(bound["evidence"]["citation"]["cited_as"], json!("primary"));
    assert_eq!(
        bound["evidence"]["citation"]["direct_evidence"],
        json!(true)
    );

    let review = LiteratureClaim::new(
        "a review summary",
        SourceProvenance::new(
            "doi:10.1000/review",
            LiteratureEvidenceTier::Review,
            published,
        )
        .studying(ScopeKey::new().exact("disease", "diffuse_glioma")),
    );
    let laundered = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&review).unwrap(),
            "target": { "disease": "diffuse_glioma" },
            "at_tier": "primary",
            "horizon": { "horizon": "open" }
        }),
    );
    assert_eq!(laundered["outcome_kind"], json!("refused"));
    assert_eq!(laundered["bound"], json!(false));
    assert_eq!(
        laundered["evidence"]["refusal_kind"],
        json!("citation_laundering")
    );

    let flagged = LiteratureClaim::new(
        "a flagged source",
        SourceProvenance::new(
            "doi:10.1000/flagged",
            LiteratureEvidenceTier::Primary,
            published,
        )
        .flagged(RetractionStatus::Retracted)
        .studying(ScopeKey::new().exact("disease", "diffuse_glioma")),
    );
    let warrant = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&flagged).unwrap(),
            "target": { "disease": "diffuse_glioma" },
            "at_tier": "review",
            "horizon": { "horizon": "open" },
            "flag_warrant": "citing the retraction history explicitly"
        }),
    );
    assert_eq!(warrant["outcome_kind"], json!("bound"));
    assert_eq!(warrant["bound"], json!(true));
    assert_eq!(warrant["evidence"]["flag_warrant_supplied"], json!(true));
}

#[test]
fn mutation_family_reports_validated_diversity_without_dumping_worlds_by_default() {
    let mut server = server();
    let payload = call(
        &mut server,
        "mutation_family",
        json!({ "world": "fixtures/generated/discriminating_world.json" }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["counts"]["attempted"], json!(8));
    assert!(payload["accepted"].is_array());
    assert!(payload["rejected"].is_array());
    assert!(payload["diversity"]["equivalence_classes"].is_number());
    assert!(payload.get("worlds").is_none());
}

#[test]
fn prism_minimize_preserves_the_oracle_signature_and_states_the_guarantee() {
    let mut server = server();
    let payload = call(
        &mut server,
        "prism_minimize",
        json!({
            "world": "fixtures/generated/discriminating_world.json",
            "facts": []
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["preserved"], json!(true));
    assert!(payload["minimization"]["guarantee"]
        .as_str()
        .unwrap()
        .contains("minimal"));
    assert!(payload["preservation"]["preservation"] == json!("preserved"));
}

#[test]
fn safety_posture_keeps_residual_and_unanalysed_threats_separate() {
    let mut server = server();
    let payload = call(&mut server, "safety_posture", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["coverage"]["mitigated"].is_number());
    assert!(payload["coverage"]["declared_only"].is_number());
    assert!(payload["coverage"]["unmitigated"].is_number());
    assert!(payload["residual_threat_ids"].is_array());
    assert!(payload["unanalysed_threat_ids"].is_array());
    assert_eq!(
        payload["perimeter_controls_are_not_claimed_as_enforced"],
        json!(true)
    );
}

#[test]
fn security_redteam_simulation_keeps_the_full_safety_loop_typed_and_honest() {
    let mut server = server();
    let payload = call(
        &mut server,
        "security_redteam_simulate",
        json!({
            "findings": [
                {
                    "id": "F-confirmed",
                    "campaign": "sandbox-escape",
                    "boundary": "agent_sandbox",
                    "class": "sandbox_bypass",
                    "status": "confirmed",
                    "reproduction": "probe-17",
                    "embargoed": true,
                    "minimised": true
                },
                {
                    "id": "F-reported",
                    "campaign": "privacy",
                    "boundary": "public_api",
                    "class": "privacy_leakage",
                    "status": "reported"
                }
            ],
            "vulnerabilities": [{
                "id": "V-holdout",
                "class": "hidden_test_exposure",
                "severity": "high",
                "epoch": 1,
                "impact": { "infrastructure": false, "data": true, "result_integrity": true },
                "advisory": {
                    "affected_versions": "0.1.0",
                    "impact": "holdout labels were exposed",
                    "mitigation": "rotate the holdout",
                    "fixed_versions": "0.1.1",
                    "result_implications": "runs before rotation require review",
                    "timeline": "reported e1; fixed e3",
                    "credit": "red-team",
                    "residual_risk": "old mirrors may retain copies"
                },
                "transitions": [
                    { "to": "triaged", "epoch": 2, "note": "reproduced" },
                    { "to": "fixed", "epoch": 3, "note": "holdout rotated" },
                    { "to": "disclosed", "epoch": 4, "note": "advisory published" }
                ]
            }],
            "deliveries": [
                {
                    "id": "sealed-output",
                    "kind": "agent_output",
                    "origin": "agent_sandbox",
                    "to": "artifact_service",
                    "via": "sealed_output_bundle"
                },
                {
                    "id": "artifact-fetch",
                    "kind": "agent_output",
                    "origin": "artifact_service",
                    "to": "evaluator_sandbox",
                    "via": "artifact_fetch"
                },
                {
                    "id": "hidden-oracle",
                    "kind": "hidden_oracle_asset",
                    "origin": "artifact_service",
                    "to": "agent_sandbox",
                    "via": "hidden_oracle_mount"
                }
            ],
            "incidents": [{
                "id": "I-holdout",
                "class": "hidden_holdout_leak",
                "opened_at": 5,
                "requests": [{
                    "action": "freeze_publication",
                    "requested_by": "operator:red-team",
                    "requested_at": 6
                }],
                "blast_radius": {
                    "completeness": "complete",
                    "dispositions": {
                        "run-1": "invalidated",
                        "run-2": "cleared"
                    }
                },
                "timeline": [
                    { "epoch": 5, "actor": "operator:red-team", "event": "incident opened" },
                    { "epoch": 6, "actor": "operator:red-team", "event": "publication freeze requested" }
                ]
            }],
            "audit_records": [
                {
                    "event": "security_quarantine",
                    "actor": "operator:red-team",
                    "subject": "hidden-oracle",
                    "epoch": 6,
                    "statement": {
                        "kind": "observed",
                        "observation": {
                            "kind": "boundary_crossing_refused",
                            "artifact": "hidden-oracle",
                            "from": "artifact_service",
                            "to": "agent_sandbox"
                        }
                    }
                },
                {
                    "event": "reviewer_decision",
                    "actor": "operator:red-team",
                    "subject": "V-holdout",
                    "epoch": 7,
                    "statement": {
                        "kind": "asserted",
                        "by": "operator:red-team",
                        "claim": "advisory reviewed"
                    }
                }
            ],
            "attestations": [
                { "kind": "digests_compared", "component": "holdout", "observed": true },
                { "kind": "built_from_manifest", "manifest": "m1", "runner": "builder", "observed": true }
            ],
            "boundary_universe": ["agent_sandbox", "evaluator_sandbox", "public_api"],
            "include_details": true
        }),
    );
    if payload["ok"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(payload["regression_corpus"]["sentinel_count"], json!(1));
    assert_eq!(
        payload["findings"][1]["regression_gate"]["eligible"],
        json!(false)
    );
    if payload["vulnerabilities"][0]["disclosed"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(payload["boundary"]["delivery_rows"][0]["ok"], json!(true));
    assert_eq!(
        payload["boundary"]["delivery_rows"][2]["fail_closed"],
        json!(true)
    );
    assert_eq!(
        payload["boundary"]["within_trial_evaluator_to_agent"],
        json!([])
    );
    assert!(!payload["boundary"]["feedback_loops"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["allowed"],
        json!(true)
    );
    assert_eq!(payload["audit"]["verified"], json!(true));
    assert_eq!(payload["audit"]["assertion_count"], json!(1));
    assert_eq!(payload["attestations"][0]["observed"], json!(true));
    assert_eq!(payload["attestations"][1]["ok"], json!(false));
}

#[test]
fn security_redteam_simulation_refuses_missing_advisories_and_partial_lineage() {
    let mut server = server();
    let payload = call(
        &mut server,
        "security_redteam_simulate",
        json!({
            "vulnerabilities": [{
                "id": "V-skip",
                "class": "sandbox_bypass",
                "severity": "critical",
                "epoch": 10,
                "transitions": [
                    { "to": "triaged", "epoch": 11 },
                    { "to": "fixed", "epoch": 12 },
                    { "to": "disclosed", "epoch": 13 }
                ]
            }],
            "incidents": [{
                "id": "I-partial",
                "class": "compromised_key",
                "opened_at": 1,
                "blast_radius": {
                    "completeness": "partial",
                    "unreachable_edges": 2,
                    "dispositions": { "run-1": "cleared" }
                }
            }],
            "max_items": 10
        }),
    );
    if payload["ok"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    if payload["vulnerabilities"][0]["disclosed"] != json!(false) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(
        payload["vulnerabilities"][0]["transitions"][2]["fail_closed"],
        json!(true)
    );
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["allowed"],
        json!(false)
    );
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["fail_closed"],
        json!(true)
    );
}

#[test]
fn registry_gate_fails_closed_on_an_unattested_document() {
    let mut server = server();
    let payload = call(
        &mut server,
        "registry_gate",
        json!({ "pack": WORLD, "policy": "experimental" }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["passed"], json!(false));
    assert_eq!(payload["blocked"], json!(true));
    assert_eq!(payload["outcome"]["outcome"], json!("block"));
}

#[test]
fn registry_lifecycle_keeps_invalid_packs_and_independent_actions_explicit() {
    let mut server = server();
    let payload = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "packs": [{ "not": "an attested benchmark pack" }],
            "actions": [
                { "op": "publish", "pack_index": 0, "tier": "exploratory" },
                { "op": "resolve", "name": "missing@0.1.0" },
                { "op": "verify_all" }
            ],
            "include_index": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["packs"][0]["valid"], json!(false));
    assert_eq!(payload["actions"][0]["ok"], json!(false));
    assert_eq!(payload["actions"][0]["fail_closed"], json!(true));
    assert_eq!(payload["actions"][1]["ok"], json!(true));
    assert_eq!(payload["actions"][1]["result"]["found"], json!(false));
    assert_eq!(payload["actions"][2]["result"]["clean"], json!(true));
    assert_eq!(payload["final"]["artifact_count"], json!(0));
    assert!(payload["registry"].is_object());
    assert!(payload["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("continu")));
}

#[test]
fn registry_lifecycle_returns_continuation_state_and_preserves_withdrawal_history() {
    let pack = attested_minimal_registry_pack();
    let mut server = server();
    let first = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "packs": [pack],
            "actions": [
                { "op": "publish", "pack_index": 0, "tier": "unranked" },
                { "op": "resolve", "name": "mcp/demo@0.1.0" }
            ]
        }),
    );
    assert_eq!(first["actions"][0]["ok"], json!(true));
    let digest = first["actions"][0]["result"]["digest"]
        .as_str()
        .expect("publish returns digest");
    assert_eq!(first["actions"][1]["result"]["digest"], json!(digest));
    assert_eq!(first["final"]["integrity_clean"], json!(true));

    let resumed = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "index": first["registry"],
            "actions": [
                { "op": "inspect", "digest": digest },
                { "op": "withdraw", "digest": digest, "reason": "fixture lifecycle complete" },
                { "op": "history", "digest": digest },
                { "op": "verify_all" }
            ]
        }),
    );
    assert_eq!(
        resumed["initial_integrity"]["operations_allowed"],
        json!(true)
    );
    assert_eq!(resumed["actions"][0]["result"]["found"], json!(true));
    assert_eq!(resumed["actions"][1]["ok"], json!(true));
    assert_eq!(resumed["actions"][2]["result"]["event_count"], json!(2));
    assert_eq!(resumed["actions"][3]["result"]["clean"], json!(true));
    assert_eq!(resumed["final"]["artifact_count"], json!(1));
    assert_eq!(resumed["final"]["log_count"], json!(2));
}

#[test]
fn cache_invalidation_simulation_keeps_partial_unknowns_and_replayable_misses_visible() {
    let mut server = server();
    let arguments = json!({
        "schema": {
            "name": "decision-cache",
            "components": ["input", "code"],
            "reuse": "same_build_only"
        },
        "entries": [
            {
                "components": { "input": "world@1", "code": "build-a" },
                "value": { "answer": "derived" },
                "produced_by": "build-a",
                "written_at": 1,
                "dependencies": { "kind": "declared", "resources": ["derived"] }
            },
            {
                "components": { "input": "world@2", "code": "build-a" },
                "value": { "answer": "legacy" },
                "produced_by": "build-a",
                "written_at": 1,
                "dependencies": "undeclared"
            }
        ],
        "graph": {
            "declared": [{ "resource": "derived", "depends_on": ["input"] }],
            "opaque": ["input"]
        },
        "changed": "input",
        "apply": true,
        "apply_at": 2,
        "lookups": [
            { "components": { "input": "world@2", "code": "build-a" }, "requested_by": "build-a" }
        ]
    });
    let payload = call(
        &mut server,
        "cache_invalidation_simulate",
        arguments.clone(),
    );
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["invalidation"]["plan"]["completeness"]["Partial"].is_object());
    assert_eq!(
        payload["invalidation"]["apply_report"]["invalidation_was_complete"],
        json!(false)
    );
    assert_eq!(
        payload["invalidation"]["apply_report"]["removed"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        payload["invalidation"]["apply_report"]["marked_unproven"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(payload["graph"]["opaque_resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "input"));
    assert_eq!(payload["lookups"]["pre_apply"][0]["hit"], json!(true));
    assert_eq!(payload["lookups"]["post_apply"][0]["hit"], json!(false));
    assert_eq!(payload["cache"]["unproven"].as_array().unwrap().len(), 1);

    let digest = payload["cache"]["unproven"][0]
        .as_str()
        .expect("unproven entry has a digest")
        .to_string();
    let resumed = call(
        &mut server,
        "cache_invalidation_simulate",
        json!({
            "schema": arguments["schema"].clone(),
            "entries": [arguments["entries"][1].clone()],
            "graph": { "declared": [], "opaque": [] },
            "reprove": [{ "digest": digest, "by": "build-b" }]
        }),
    );
    assert_eq!(resumed["reprove"][0]["ok"], json!(true));
    assert_eq!(resumed["cache"]["unproven"].as_array().unwrap().len(), 0);
}

#[test]
fn storage_lifecycle_simulation_plans_pins_reserve_and_non_copyable_allowance() {
    let mut server = server();
    let payload = call(
        &mut server,
        "storage_lifecycle_simulate",
        json!({
            "now": 20,
            "tiering_policy": {
                "demote_to_warm_after": 5,
                "demote_to_cold_after": 12,
                "promote_after_accesses": 3,
                "promote_within": 2
            },
            "records": [
                { "object": "stale-hot", "tier": "hot", "last_access": 0, "bytes": 100 },
                { "object": "pinned-hot", "tier": "hot", "last_access": 0, "bytes": 200, "pinned": true },
                { "object": "recent-cold", "tier": "cold", "last_access": 19, "recent_accesses": 3, "bytes": 50 }
            ],
            "apply_tiering": true,
            "quota": { "limit": 1000, "reserve": 100 },
            "charges": [
                { "class": "objects", "purpose": "ingest", "bytes": 850 },
                { "class": "events", "purpose": "ingest", "bytes": 100 },
                { "class": "events", "purpose": "cleanup", "bytes": 100 }
            ],
            "releases": [{ "class": "objects", "bytes": 50 }],
            "delegations": [{
                "bytes": 50,
                "charges": [{ "class": "cache", "purpose": "cleanup", "bytes": 30 }]
            }],
            "absorb_delegated": [0]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["tiering"]["transition_count"], json!(3));
    assert_eq!(payload["tiering"]["apply_report"]["applied"], json!(3));
    let records = payload["tiering"]["records"].as_array().unwrap();
    assert_eq!(records[1]["tier"], json!("Warm"));
    assert_eq!(records[2]["tier"], json!("Hot"));
    assert!(payload["quota"]["charges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["ok"] == json!(false) && row["fail_closed"] == json!(true)));
    assert_eq!(payload["quota"]["remaining"], json!(70));
    assert_eq!(payload["quota"]["reserve"], json!(100));
    assert_eq!(payload["quota"]["absorptions"][0]["ok"], json!(true));
    assert!(payload["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("allowance is not copied")));
}

#[test]
fn operations_catalog_executes_topology_parity_and_keeps_metric_debt_explicit() {
    let mut server = server();
    let payload = call(&mut server, "operations_catalog", json!({ "max_items": 2 }));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["topologies"]["promise_parity"]["holds"],
        json!(true)
    );
    assert_eq!(payload["data_classes"].as_array().unwrap().len(), 5);
    assert_eq!(payload["deployment_planes"].as_array().unwrap().len(), 9);
    assert_eq!(payload["service_contracts"]["summary"]["total"], json!(9));
    assert!(payload["metrics"]["named_in_scope"].as_u64().unwrap() >= 100);
    assert!(payload["metrics"]["named_but_undefined"].as_u64().unwrap() >= 100);
    assert_eq!(
        payload["metrics"]["undefined_metrics_returned"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        payload["metrics"]["omitted_undefined_metrics"]
            .as_u64()
            .unwrap()
            > 0
    );

    let detailed = call(
        &mut server,
        "operations_catalog",
        json!({ "include_details": true, "max_items": 1 }),
    );
    assert_eq!(detailed["detail_mode"], json!("full"));
    assert!(detailed["details"]["service_entries"].is_array());
    assert!(
        detailed["details"]["undefined_metrics"]
            .as_array()
            .unwrap()
            .len()
            >= 100
    );
}

#[test]
fn research_ci_check_keeps_failures_and_undetermined_checks_distinct() {
    let mut server = server();
    let digest = "a".repeat(64);
    let result = json!({
        "subject": "reference-result",
        "observations": [
            { "observation": "claim", "id": "claim-1", "resolves_to": digest },
            { "observation": "split", "name": "train", "members": ["p1"] },
            { "observation": "split", "name": "test", "members": ["p2"] },
            { "observation": "figure", "name": "roc", "declared": digest, "recomputed": digest },
            { "observation": "cell", "id": "cell-1", "previously_passed": true, "passes_now": true },
            { "observation": "dependency", "name": "rustc", "pinned": true },
            { "observation": "egress_event", "connector": "site-a", "permitted": "aggregate_only", "requested": "aggregate_only" },
            { "observation": "non_claim", "statement": "does not establish clinical validity" },
            { "observation": "world_reference", "world": "world-a", "rung": "observed" }
        ]
    });
    let passed = call(
        &mut server,
        "research_ci_check",
        json!({ "result": result.clone() }),
    );
    assert_eq!(passed["ok"], json!(true));
    assert_eq!(passed["publishable"], json!(true));
    assert!(passed["failed_checks"].as_array().unwrap().is_empty());
    assert!(passed["undetermined_checks"].as_array().unwrap().is_empty());
    assert_eq!(passed["check_count"], json!(8));

    let mut regressed = result;
    regressed["observations"][4]["passes_now"] = json!(false);
    let blocked = call(
        &mut server,
        "research_ci_check",
        json!({ "result": regressed }),
    );
    assert_eq!(blocked["publishable"], json!(false));
    assert!(blocked["failed_checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check == "decision cell regression"));
    assert!(blocked["undetermined_checks"]
        .as_array()
        .unwrap()
        .is_empty());
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

#[test]
fn capability_rank_preserves_dominance_tradeoffs_and_condition_refusals() {
    let mut server = server();
    let dominated = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "max_items": 10
        }),
    );
    assert_eq!(dominated["ok"], json!(true));
    assert_eq!(dominated["partial_order"]["is_total"], json!(true));
    assert_eq!(
        dominated["partial_order"]["relations"][0]["dominance"]["dominance"],
        json!("left_dominates")
    );
    assert_eq!(
        dominated["partial_order"]["maximal_systems"],
        json!(["system-a"])
    );

    let tradeoff = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.6, "pack/4"),
                metric_vector("system-b", 0.7, 0.8, "pack/4")
            ]
        }),
    );
    assert_eq!(tradeoff["partial_order"]["is_total"], json!(false));
    assert_eq!(tradeoff["partial_order"]["unresolved_count"], json!(1));
    assert_eq!(
        tradeoff["partial_order"]["unresolved"][0]["dominance"]["because"]["because"],
        json!("trade_off")
    );

    let mismatched = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.9, 0.8, "pack/5")
            ]
        }),
    );
    assert_eq!(mismatched["ok"], json!(true));
    assert_eq!(
        mismatched["partial_order"]["unresolved"][0]["dominance"]["because"]["because"],
        json!("conditions_differ")
    );

    let policy = json!({
        "intended_use": "reference comparison",
        "weights": { "verify.oracle": 1.0, "safety.boundary": 1.0 }
    });
    let weighting = json!({
        "policy": policy,
        "digest": ContentHash::of_value(&json!({
            "intended_use": "reference comparison",
            "weights": { "verify.oracle": 1.0, "safety.boundary": 1.0 }
        })).unwrap()
    });
    let weighted = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "weighting": weighting
        }),
    );
    assert_eq!(weighted["ok"], json!(true));
    assert_eq!(weighted["total_order"]["leaders"], json!(["system-a"]));
    assert_eq!(weighted["total_order"]["overwrote_a_refusal"], json!(false));
    assert!(weighted["rank_instability"]["instability"].is_object());
}

#[test]
fn metrics_profile_audit_keeps_unmeasured_capabilities_and_uncontested_leads_visible() {
    let mut server = server();
    let mut incomplete = metric_vector("system-b", 0.7, 0.6, "pack/4");
    incomplete["grid"]["cells"]
        .as_object_mut()
        .unwrap()
        .remove("safety.boundary");
    let payload = call(
        &mut server,
        "metrics_profile_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                incomplete
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["summary"]["capability_count"], json!(2));
    assert_eq!(payload["summary"]["uncontested_lead_count"], json!(1));
    let rows = payload["per_capability"]["rows"].as_array().unwrap();
    let safety = rows
        .iter()
        .find(|row| row["capability"] == json!("safety.boundary"))
        .expect("safety row is retained");
    assert_eq!(safety["best"], json!(["system-a"]));
    assert_eq!(safety["unmeasured_for"], json!(["system-b"]));
    assert_eq!(safety["lead_is_uncontested"], json!(true));
    let system_b = payload["per_system"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["system"] == json!("system-b"))
        .unwrap();
    assert_eq!(system_b["unmeasured_capability_count"], json!(1));
}

#[test]
fn metrics_analytics_audit_keeps_domains_missingness_and_paired_contrasts_typed() {
    let mut server = server();
    let payload = call(
        &mut server,
        "metrics_analytics_audit",
        json!({
            "observations": [
                {
                    "id": "verification-1",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.80,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "replicate_group": "world-1",
                    "cost": 4.0,
                    "latency_ms": 20.0,
                    "evidence": "observed"
                },
                {
                    "id": "verification-2",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.90,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "replicate_group": "world-2",
                    "cost": 5.0,
                    "latency_ms": 25.0,
                    "evidence": "reproduced"
                },
                {
                    "id": "verification-missing",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.0,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "evidence": "missing"
                }
            ],
            "pairs": [
                {
                    "id": "robustness-1",
                    "dimension": "robustness",
                    "domain": "oncology",
                    "baseline": 0.90,
                    "variant": 0.72,
                    "direction": "higher_is_better",
                    "tolerance": 0.20,
                    "evidence": "observed"
                },
                {
                    "id": "cross-modal-1",
                    "dimension": "cross_modal_consistency",
                    "domain": "oncology",
                    "baseline": 0.80,
                    "variant": 0.82,
                    "direction": "higher_is_better",
                    "tolerance": 0.05,
                    "evidence": "reproduced"
                }
            ],
            "calibration": [
                { "id": "forecast-1", "domain": "oncology", "predicted": 0.9, "observed": 1.0, "evidence": "observed" },
                { "id": "forecast-2", "domain": "oncology", "predicted": 0.1, "observed": 0.0, "evidence": "declared" }
            ],
            "calibration_bins": 5
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["workflow"], json!("metrics_descriptive_analytics"));
    assert_eq!(payload["coverage"]["measured_observations"], json!(2));
    assert_eq!(payload["coverage"]["excluded_observations"], json!(1));
    assert_eq!(payload["dimensions"][0]["values"]["count"], json!(2));
    assert_eq!(payload["paired"].as_array().unwrap().len(), 2);
    assert_eq!(payload["calibration"]["measured"], json!(1));
    assert!(payload["caveats"].as_array().unwrap().len() >= 4);
}

#[test]
fn metrics_analytics_audit_refuses_mixed_direction_with_a_structured_tool_error() {
    let mut server = server();
    let response = call(
        &mut server,
        "metrics_analytics_audit",
        json!({
            "observations": [
                {
                    "id": "one", "dimension": "latency", "domain": "runtime", "system": "a",
                    "value": 10.0, "direction": "lower_is_better", "unit": "ms", "condition": "v1", "evidence": "observed"
                },
                {
                    "id": "two", "dimension": "latency", "domain": "runtime", "system": "a",
                    "value": 0.5, "direction": "higher_is_better", "unit": "fraction", "condition": "v1", "evidence": "observed"
                }
            ]
        }),
    );
    assert_eq!(response["ok"], json!(false));
    assert!(response["error"]
        .as_str()
        .unwrap()
        .contains("metrics analytics refused"));
}

#[test]
fn biocapability_evidence_audit_composes_metrics_value_reference_and_claim_readiness() {
    let mut server = server();
    let payload = call(
        &mut server,
        "biocapability_evidence_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "evidence": [
                { "id": "grounding", "dimension": "evidence_grounding", "status": "observed", "domain": "oncology", "source": "ledger:run-1", "scope": "pack/4" },
                { "id": "acquisition", "dimension": "information_acquisition", "status": "observed", "domain": "bioir", "action_changed": true, "cost": 12.0 },
                { "id": "resource", "dimension": "resource_efficiency", "status": "observed", "domain": "operations", "denominator": "successful reproduced runs", "cost": 12.0 },
                { "id": "time", "dimension": "temporal_validity", "status": "observed", "domain": "evaluation", "decision_epoch": 10, "evidence_epoch": 9 },
                { "id": "modal", "dimension": "cross_modal_consistency", "status": "observed", "domain": "oncology", "modalities": ["mri", "pathology"], "agreement": true },
                { "id": "causal", "dimension": "causal_identification", "status": "observed", "domain": "inference", "identification": "identified", "estimand": "treatment effect" },
                { "id": "repro", "dimension": "reproducibility", "status": "reproduced", "domain": "research_ci", "replications": 3, "environment_pinned": true },
                { "id": "translation", "dimension": "translation_maturity", "status": "observed", "domain": "oncoworlds", "source_population": "xenograft", "target_population": "patient", "bridge": true },
                { "id": "coordination", "dimension": "multi_agent_coordination", "status": "observed", "domain": "orchestration", "agents": ["planner", "verifier"], "coordination_overhead": 1.2 }
            ],
            "claim_requests": [{
                "id": "public-profile",
                "claim": "publishable capability profile",
                "requires": [
                    "evidence_grounding",
                    "information_acquisition",
                    "resource_efficiency",
                    "temporal_validity",
                    "cross_modal_consistency",
                    "causal_identification",
                    "reproducibility",
                    "translation_maturity",
                    "multi_agent_coordination"
                ]
            }],
            "information": {
                "problem": {
                    "actions": ["treat", "abstain"],
                    "models": ["responsive", "resistant"],
                    "loss": [0.0, 10.0, 10.0, 0.0]
                },
                "belief": { "mass": [0.5, 0.5] },
                "acquisition": {
                    "id": "assay",
                    "cost": 0.1,
                    "outcomes": [
                        { "label": "positive", "likelihood": [0.9, 0.1] },
                        { "label": "negative", "likelihood": [0.1, 0.9] }
                    ]
                }
            },
            "reference": {
                "standard": "distribution",
                "mass": { "progression": 0.6, "stable": 0.4 },
                "dispersion": { "kind": "mixed", "aleatoric_fraction": 0.5 }
            },
            "reference_state": "progression",
            "max_items": 20
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["metrics_ok"], json!(true));
    assert_eq!(
        payload["claim_requests"]["rows"][0]["eligible"],
        json!(true)
    );
    assert_eq!(
        payload["release_posture"]["ready_for_requested_claims"],
        json!(true)
    );
    assert_eq!(payload["subaudits"]["information_value"]["ok"], json!(true));
    assert_eq!(
        payload["subaudits"]["reference_quality"]["can_certify_clean_pass"],
        json!(false)
    );
    assert_eq!(
        payload["evidence"]["dimensions"].as_array().unwrap().len(),
        9
    );
    assert_eq!(payload["evidence"]["invalid_item_count"], json!(0));
}

#[test]
fn biocapability_evidence_audit_blocks_temporal_leaks_and_declared_only_claims() {
    let mut server = server();
    let payload = call(
        &mut server,
        "biocapability_evidence_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "evidence": [
                { "id": "future", "dimension": "temporal_validity", "status": "observed", "decision_epoch": 10, "evidence_epoch": 11 },
                { "id": "declared-grounding", "dimension": "evidence_grounding", "status": "declared", "source": "operator assertion", "scope": "unknown" },
                { "id": "unknown", "dimension": "not-a-real-dimension", "status": "observed" }
            ],
            "claim_requests": [{
                "id": "temporal-profile",
                "claim": "temporally valid profile",
                "requires": ["temporal_validity", "evidence_grounding"]
            }]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["evidence"]["invalid_item_count"], json!(2));
    let temporal = payload["evidence"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["dimension"] == json!("temporal_validity"))
        .unwrap();
    assert_eq!(temporal["state"], json!("blocked"));
    assert_eq!(
        payload["claim_requests"]["rows"][0]["eligible"],
        json!(false)
    );
    assert_eq!(
        payload["release_posture"]["ready_for_requested_claims"],
        json!(false)
    );
}

fn risk_assessment(ratings: Value) -> Value {
    json!({
        "subject": "pack/biological-design@1",
        "category": "biological_design",
        "ratings": ratings
    })
}

#[test]
fn safety_release_gate_distinguishes_clear_conditioned_blocked_and_unrated() {
    let mut server = server();
    let complete_low = json!({
        "capability_uplift": "low",
        "actionability": "low",
        "scale": "low",
        "expertise_reduction": "low",
        "target_specificity": "low",
        "reversibility": "low",
        "detectability": "high",
        "available_safeguards": "high",
        "legitimate_scientific_value": "high"
    });
    let cleared = call(
        &mut server,
        "safety_release_gate",
        json!({ "assessment": risk_assessment(complete_low) }),
    );
    assert_eq!(cleared["ok"], json!(true));
    assert_eq!(cleared["decision"]["decision"], json!("cleared"));
    assert_eq!(cleared["cleared"], json!(true));
    assert!(cleared["unrated_dimensions"].as_array().unwrap().is_empty());

    let conditioned = call(
        &mut server,
        "safety_release_gate",
        json!({
            "assessment": risk_assessment(json!({
                "capability_uplift": "high",
                "actionability": "low",
                "scale": "low",
                "expertise_reduction": "low",
                "target_specificity": "low",
                "reversibility": "low",
                "detectability": "high",
                "available_safeguards": "high",
                "legitimate_scientific_value": "high"
            }))
        }),
    );
    assert_eq!(conditioned["decision"]["decision"], json!("conditioned"));
    assert_eq!(
        conditioned["decision"]["driven_by"],
        json!(["capability_uplift"])
    );
    assert_eq!(conditioned["cleared"], json!(false));

    let blocked = call(
        &mut server,
        "safety_release_gate",
        json!({
            "assessment": risk_assessment(json!({
                "capability_uplift": "high",
                "actionability": "high",
                "scale": "low",
                "expertise_reduction": "low",
                "target_specificity": "low",
                "reversibility": "low",
                "detectability": "high",
                "available_safeguards": "high",
                "legitimate_scientific_value": "high"
            }))
        }),
    );
    assert_eq!(blocked["decision"]["decision"], json!("blocked"));
    assert_eq!(
        blocked["decision"]["driven_by"].as_array().unwrap().len(),
        2
    );

    let incomplete = call(
        &mut server,
        "safety_release_gate",
        json!({ "assessment": risk_assessment(json!({
            "capability_uplift": "low"
        })) }),
    );
    assert_eq!(incomplete["__isError"], json!(true));
    assert!(incomplete["error"].as_str().unwrap().contains("unrated"));
}

#[test]
fn medical_boundary_admits_research_and_returns_structured_clinical_refusal() {
    let mut server = server();
    let admitted = call(
        &mut server,
        "medical_boundary_check",
        json!({
            "output": {
                "side": "research",
                "use_case": "evidence_synthesis",
                "label": "evidence synthesis for a research report"
            }
        }),
    );
    assert_eq!(admitted["ok"], json!(true));
    assert_eq!(admitted["admitted"], json!(true));
    assert_eq!(admitted["use_case"], json!("evidence_synthesis"));
    assert!(admitted["research_only_label"]
        .as_str()
        .unwrap()
        .contains("not a medical device"));

    let refused = call(
        &mut server,
        "medical_boundary_check",
        json!({
            "output": {
                "side": "clinical",
                "category": "treatment_selection",
                "label": "choose a treatment"
            }
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["admitted"], json!(false));
    assert_eq!(refused["clinical_output_is_never_admitted"], json!(true));
    assert!(refused["refusal"]
        .as_str()
        .unwrap()
        .contains("research-only"));
    assert_eq!(refused["__isError"], json!(false));
}

#[test]
fn hub_search_preserves_facet_reasons_authority_freshness_and_empty_query_refusal() {
    let mut server = server();
    let registry = RegistryId::parse("origin").unwrap();
    let namespace = Namespace::parse("bioprism").unwrap();
    let authority = Authority::new(registry.clone())
        .owning(namespace)
        .expect("authority builds");
    let mut federation = Federation::new();
    federation
        .admit(authority.clone())
        .expect("federation admits");
    let mut catalog = Catalog::origin(authority);
    catalog
        .record(
            PackRelease::new(
                PackName::parse("bioprism/onco").unwrap(),
                Version::new(1, 0, 0),
                "sha256:onco",
            )
            .described("oncology reference pack")
            .keyworded(["onco"])
            .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    catalog
        .record(
            PackRelease::new(
                PackName::parse("bioprism/other").unwrap(),
                Version::new(1, 0, 0),
                "sha256:other",
            )
            .described("other pack")
            .keyworded(["onco"])
            .at_tier(TrustTier::Unranked),
        )
        .unwrap();
    let payload = call(
        &mut server,
        "hub_search",
        json!({
            "federation": serde_json::to_value(&federation).unwrap(),
            "catalogs": [serde_json::to_value(&catalog).unwrap()],
            "query": serde_json::to_value(Query::new(vec![
                Facet::Keyword("onco".into()),
                Facet::TierAtLeast(TrustTier::Reviewed)
            ])).unwrap(),
            "max_items": 1
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["match_count"], json!(1));
    assert_eq!(payload["matches"][0]["name"], json!("bioprism/onco"));
    assert_eq!(
        payload["matches"][0]["authority"]["authority"],
        json!("authoritative")
    );
    assert_eq!(
        payload["matches"][0]["freshness"]["freshness"],
        json!("authoritative")
    );
    assert_eq!(payload["matches"][0]["why"].as_array().unwrap().len(), 2);
    assert_eq!(payload["excluded_count"], json!(1));
    assert_eq!(
        payload["excluded"][0]["failed"],
        json!("tier at least reviewed")
    );
    assert_eq!(payload["truncated"], json!(false));

    let refused = call(
        &mut server,
        "hub_search",
        json!({
            "federation": serde_json::to_value(&federation).unwrap(),
            "catalogs": [serde_json::to_value(&catalog).unwrap()],
            "query": serde_json::to_value(Query::new(vec![])).unwrap()
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("no facets"));
}

#[test]
fn measurement_compare_records_conversion_and_blocks_dimension_or_binding_mismatch() {
    let mut server = server();
    let left = serde_json::to_value(Measurement::scalar(
        "left",
        Quantity::parse(10.0, "mm").unwrap(),
    ))
    .unwrap();
    let right = serde_json::to_value(Measurement::scalar(
        "right",
        Quantity::parse(1.0, "cm").unwrap(),
    ))
    .unwrap();
    let converted = call(
        &mut server,
        "measurement_compare",
        json!({ "left": left, "right": right }),
    );
    assert_eq!(converted["ok"], json!(true));
    assert_eq!(converted["comparable"], json!(true));
    assert_eq!(
        converted["report"]["verdict"]["verdict"],
        json!("comparable")
    );
    assert_eq!(
        converted["report"]["conversions"].as_array().unwrap().len(),
        1
    );
    assert_eq!(converted["report"]["caveats"].as_array().unwrap().len(), 1);
    assert_eq!(converted["report_sha256"].as_str().unwrap().len(), 64);

    let blocked = call(
        &mut server,
        "measurement_compare",
        json!({
            "left": serde_json::to_value(Measurement::scalar(
                "length",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "right": serde_json::to_value(Measurement::scalar(
                "volume",
                Quantity::parse(1.0, "mL").unwrap()
            )).unwrap()
        }),
    );
    assert_eq!(blocked["comparable"], json!(false));
    assert_eq!(blocked["report"]["verdict"]["verdict"], json!("blocked"));
    assert_eq!(
        blocked["report"]["verdict"]["reason"]["blocked_by"],
        json!("dimension_mismatch")
    );

    let unbound = call(
        &mut server,
        "measurement_compare",
        json!({
            "left": serde_json::to_value(Measurement::scalar(
                "left",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "right": serde_json::to_value(Measurement::scalar(
                "right",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "require_bound_terms": true
        }),
    );
    assert_eq!(unbound["comparable"], json!(false));
    assert_eq!(unbound["report"]["verdict"]["verdict"], json!("blocked"));
}

#[test]
fn tabular_ingest_runs_independent_conformance_and_keeps_loss_visible() {
    let mut server = server();
    let profile = TabularProfile::new("RG-DEMO-001")
        .scope("subject", "subject")
        .variable(
            "age",
            VariableMapping::new("age_at_diagnosis").typed(ValueType::Integer),
        );
    let payload = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "cohort.csv",
            "format": "text/csv",
            "csv": "subject,age,comment\nS1,41,ok\n",
            "profile": serde_json::to_value(profile).unwrap(),
            "provenance": { "accession": "RG-DEMO-001", "version": "v1", "retrieved_at": "2026-08-14T00:00:00Z" },
            "include_facts": true,
            "max_items": 2
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["fact_count"], json!(1));
    assert_eq!(payload["facts"].as_array().unwrap().len(), 1);
    assert_eq!(payload["conformance"]["verified"], json!(true));
    assert!(payload["semantic_loss"].is_object());
    assert_eq!(payload["manifest"]["source_id"], json!("cohort.csv"));
    assert_eq!(
        payload["manifest"]["provenance"]["accession"],
        json!("RG-DEMO-001")
    );

    let refused = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "cohort.csv",
            "csv": "subject,age\nS1,41\n",
            "document": WORLD,
            "profile": serde_json::to_value(TabularProfile::new("RG-DEMO-001")).unwrap()
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("either csv or document"));
}

#[test]
fn observed_world_and_claim_tools_enforce_pinning_rungs_and_selection() {
    let mut server = server();
    let source = SourceRef::new("cohort", "v1").under(Access::Controlled {
        policy: "reviewer-only".into(),
    });
    let design = StudyDesign::new(
        2,
        Selection::Consecutive {
            criterion: "all eligible participants".into(),
        },
    )
    .with_stratum(Stratum::new("all", 2))
    .standing_for("RG-DEMO population");
    let declared = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "observed-demo",
            "sources": [serde_json::to_value(source).unwrap()],
            "design": serde_json::to_value(design).unwrap(),
            "outcome_labels": ["positive", "negative"]
        }),
    );
    assert_eq!(declared["ok"], json!(true));
    assert_eq!(declared["world_id"], json!("observed-demo"));
    assert_eq!(declared["controlled_sources"], json!(["cohort"]));
    assert_eq!(declared["provenance"]["top"], json!("observed"));

    let unpinned = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "bad-world",
            "sources": [serde_json::to_value(SourceRef::unpinned("cohort")).unwrap()],
            "design": serde_json::to_value(StudyDesign::new(
                1,
                Selection::Consecutive { criterion: "eligible".into() }
            )).unwrap(),
            "outcome_labels": ["positive"]
        }),
    );
    assert_eq!(unpinned["__isError"], json!(true));
    assert!(unpinned["error"].as_str().unwrap().contains("pinned"));

    let observed_provenance = Provenance::observed(Selection::Consecutive {
        criterion: "all eligible participants".into(),
    });
    let supported = call(
        &mut server,
        "world_claim_check",
        json!({
            "provenance": serde_json::to_value(&observed_provenance).unwrap(),
            "claim": serde_json::to_value(Claim::new(ClaimKind::Biology, "observed outcome")).unwrap()
        }),
    );
    assert_eq!(supported["ok"], json!(true));
    assert_eq!(supported["supported"], json!(true));
    assert!(supported["caveat"]
        .as_str()
        .unwrap()
        .contains("observed world"));

    let simulated = Provenance::mechanistic(["tumour growth rate"]);
    let refused = call(
        &mut server,
        "world_claim_check",
        json!({
            "provenance": serde_json::to_value(&simulated).unwrap(),
            "claim": serde_json::to_value(Claim::new(ClaimKind::Biology, "tumour growth rate")).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["supported"], json!(false));
    assert!(refused["refusal"]
        .as_str()
        .unwrap()
        .contains("construction"));
}

#[test]
fn hub_resolve_and_lock_preserve_federation_and_dependency_provenance() {
    let mut server = server();
    let registry = RegistryId::parse("origin").unwrap();
    let authority = Authority::new(registry)
        .owning(Namespace::parse("bioprism").unwrap())
        .unwrap();
    let mut federation = Federation::new();
    federation.admit(authority.clone()).unwrap();
    let child = PackName::parse("bioprism/child").unwrap();
    let root = PackName::parse("bioprism/root").unwrap();
    let mut catalog = Catalog::origin(authority);
    catalog
        .record(
            PackRelease::new(child.clone(), Version::new(1, 0, 0), "sha256:child")
                .described("child dependency")
                .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    catalog
        .record(
            PackRelease::new(root.clone(), Version::new(1, 0, 0), "sha256:root")
                .depending_on(child, VersionReq::Any)
                .described("root pack")
                .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    let request = HubRequest::new(root, VersionReq::Any);
    let args = json!({
        "federation": serde_json::to_value(&federation).unwrap(),
        "catalogs": [serde_json::to_value(&catalog).unwrap()],
        "request": serde_json::to_value(&request).unwrap()
    });
    let resolved = call(&mut server, "hub_resolve", args.clone());
    assert_eq!(resolved["ok"], json!(true));
    assert_eq!(
        resolved["resolution"]["subject"]["name"],
        json!("bioprism/root")
    );
    assert_eq!(resolved["authoritative"], json!(true));

    let locked = call(
        &mut server,
        "hub_lock",
        json!({
            "federation": args["federation"].clone(),
            "catalogs": args["catalogs"].clone(),
            "request": args["request"].clone(),
            "max_items": 10
        }),
    );
    assert_eq!(locked["ok"], json!(true));
    assert_eq!(locked["entry_count"], json!(2));
    assert_eq!(locked["fully_authoritative"], json!(true));
    assert_eq!(locked["entries"].as_array().unwrap().len(), 2);
    assert!(locked["entries"][0]["locked"]["required_by"].is_array());
}

#[test]
fn trace_analysis_ingests_segments_and_localizes_lossless_divergence() {
    let mut server = server();
    let failing = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"}}
{"step":1,"kind":"observation","payload":{"summary":"evidence"},"visible":["evidence-a"]}
{"step":2,"kind":"choice","payload":{"summary":"choose","chosen":"a","alternatives":["a","b"]},"visible":["evidence-a"]}
{"step":3,"kind":"action","payload":{"tool":"search"},"caused_by":2}
{"step":4,"kind":"result","payload":{"summary":"done"}}
{"step":5,"kind":"termination","payload":{"summary":"finished"}}
"#;
    let passing = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"}}
{"step":1,"kind":"observation","payload":{"summary":"evidence"},"visible":["evidence-a"]}
{"step":2,"kind":"choice","payload":{"summary":"choose","chosen":"b","alternatives":["a","b"]},"visible":["evidence-a","evidence-b"]}
{"step":3,"kind":"action","payload":{"tool":"search-alt"},"caused_by":2}
{"step":4,"kind":"result","payload":{"summary":"done"}}
{"step":5,"kind":"termination","payload":{"summary":"finished"}}
"#;

    let payload = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "failed-run",
            "jsonl": failing,
            "succeeded": false,
            "passing_trace_id": "passing-run",
            "passing_jsonl": passing,
            "passing_succeeded": true,
            "max_items": 10
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["valid"], json!(true));
    assert_eq!(payload["lossless"], json!(true));
    assert_eq!(payload["compilable"], json!(true));
    assert!(payload["trace_sha256"].as_str().unwrap().len() >= 32);
    assert!(payload["candidate_count"].as_u64().unwrap() >= 2);
    assert!(payload["excluded_count"].as_u64().unwrap() >= 4);
    assert!(payload["review_reduction"].as_f64().unwrap() > 0.0);
    assert_eq!(payload["divergence"]["kind"], json!("diverged"));
    assert_eq!(payload["divergence"]["failing_step"], json!(2));
    assert_eq!(payload["divergence_actionable"], json!(true));
    assert!(payload["divergence"]["visibility_gap"]
        .as_array()
        .unwrap()
        .contains(&json!("evidence-b")));
    assert!(payload["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|candidate| candidate["score"]["is_divergence"] == json!(true)));
    assert_eq!(
        payload["proposals"].as_array().unwrap().len(),
        payload["candidates"].as_array().unwrap().len()
    );
    assert_eq!(payload["approval_required"], json!(true));
}

#[test]
fn trace_analysis_preserves_import_loss_and_fails_closed_on_unsafe_inputs() {
    let mut server = server();
    let lossy = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"},"unknown":"retained-as-loss"}
not-json
{"step":2,"kind":"mystery","payload":{"summary":"cannot type"}}
{"step":3,"kind":"choice","payload":{"summary":"choose","alternatives":["a","b"]}}
"#;
    let payload = call(
        &mut server,
        "trace_analyze",
        json!({ "trace_id": "lossy", "jsonl": lossy }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["dropped_events"], json!(2));
    assert_eq!(payload["compilable"], json!(false));
    assert!(payload["loss"]["unparsed_lines"].as_array().unwrap().len() == 1);
    assert!(payload["loss"]["untyped_events"].as_array().unwrap().len() == 1);
    assert_eq!(payload["divergence"], Value::Null);

    let duplicate = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "duplicate",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}\n{\"step\":0,\"kind\":\"choice\",\"payload\":{}}"
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(duplicate["error"]
        .as_str()
        .unwrap()
        .contains("more than once"));

    let conflict = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "conflict",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}",
            "document": "fixtures/fiber-v0.1/radiogenomic_world.json"
        }),
    );
    assert_eq!(conflict["__isError"], json!(true));
    assert!(conflict["error"]
        .as_str()
        .unwrap()
        .contains("either jsonl or document"));

    let bounded = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "bounded",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}",
            "max_bytes": 0
        }),
    );
    assert_eq!(bounded["__isError"], json!(true));
    assert!(bounded["error"].as_str().unwrap().contains("max_bytes"));
}

#[test]
fn trace_otel_ingest_maps_spans_and_reports_compilation_readiness() {
    let mut server = server();
    let otlp = json!({
        "resourceSpans": [{
            "resource": {"attributes": [
                {"key": "service.name", "value": {"stringValue": "agent"}}
            ]},
            "scopeSpans": [{
                "scope": {"name": "fixture", "version": "1"},
                "spans": [
                    {
                        "traceId": "trace-a",
                        "spanId": "root",
                        "name": "agent.goal",
                        "startTimeUnixNano": "10",
                        "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "goal"}}]
                    },
                    {
                        "traceId": "trace-a",
                        "spanId": "child",
                        "parentSpanId": "root",
                        "name": "agent.tool.call",
                        "startTimeUnixNano": "20",
                        "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "action"}}],
                        "events": [{
                            "name": "tool.input",
                            "timeUnixNano": "21",
                            "attributes": [{"key": "arg.count", "value": {"intValue": "2"}}]
                        }]
                    }
                ]
            }]
        }]
    });
    let payload = call(
        &mut server,
        "trace_otel_ingest",
        json!({
            "trace_id": "otel-run",
            "otlp_json": otlp.to_string(),
            "include_events": true,
            "succeeded": false
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["schema"],
        json!("bioprism-mcp/trace-otel-ingest/0.1")
    );
    assert_eq!(payload["mapping"]["format"], json!("otlp_json"));
    assert_eq!(payload["mapping"]["accepted_span_count"], json!(2));
    assert_eq!(payload["mapping"]["span_event_count"], json!(1));
    assert_eq!(payload["lossless"], json!(true));
    assert_eq!(payload["compilable"], json!(true));
    assert_eq!(payload["events"][1]["caused_by"], json!(0));
    assert_eq!(payload["events"][1]["kind"], json!("action"));
    assert_eq!(
        payload["events"][1]["payload"]["events"][0]["name"],
        json!("tool.input")
    );
}

#[test]
fn trace_otel_ingest_keeps_ambiguous_exports_non_compilable_and_bounded() {
    let mut server = server();
    let otlp = json!({
        "resourceSpans": [{
            "scopeSpans": [{
                "spans": [{
                    "traceId": "trace-a",
                    "spanId": "span-a",
                    "name": "tool.call"
                }]
            }]
        }]
    })
    .to_string();
    let payload = call(
        &mut server,
        "trace_otel_ingest",
        json!({"trace_id": "ambiguous", "otlp_json": otlp}),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["compilable"], json!(false));
    assert_eq!(payload["events_included"], json!(false));

    let bounded = call(
        &mut server,
        "trace_otel_ingest",
        json!({
            "trace_id": "bounded",
            "otlp_json": otlp,
            "max_spans": 0
        }),
    );
    assert_eq!(bounded["__isError"], json!(true));
    assert!(bounded["error"].as_str().unwrap().contains("max_spans"));
}

#[test]
fn lineage_audit_separates_identity_gaps_from_material_and_ancestry_findings() {
    let mut server = server();
    let mut artifact = Artifact::new("slide-1", "s1", "pathology");
    artifact.observed_digest = Some("stale-digest".into());
    let registry = SpecimenRegistry::new()
        .with_specimen(
            SpecimenNode::new("s1", "donor-a", 10)
                .with_content("marker", json!("same-material"))
                .fingerprinted("donor-b"),
        )
        .with_specimen(
            SpecimenNode::new("s2", "donor-a", 10).with_content("marker", json!("same-material")),
        )
        .with_specimen(SpecimenNode::new("s3", "donor-a", 11).derived_from("s1"))
        .with_artifact(artifact);

    let payload = call(
        &mut server,
        "lineage_audit",
        json!({ "registry": serde_json::to_value(&registry).unwrap(), "max_items": 20 }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["clean"], json!(false));
    assert_eq!(payload["identity_complete"], json!(false));
    assert!(payload["finding_count"].as_u64().unwrap() >= 3);
    assert_eq!(payload["unchecked_identity_count"], json!(2));
    assert!(payload["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["finding"] == json!("mass_not_conserved")));
    assert!(payload["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["finding"] == json!("identity_mismatch")));
}

#[test]
fn preanalytic_apply_preserves_biology_and_reports_false_positive_and_response_contracts() {
    let mut server = server();
    let specimen = Specimen::new("sp-1")
        .with_biology("state", json!("stable"))
        .with_qc("drift", 0)
        .with_measurability("rna", 10_000);
    let mutation = PreanalyticMutation::new(
        "cold-30",
        "cold-family",
        FaultKind::ColdIschaemia { minutes: 30 },
        Intensity::FULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "drift".into(),
        delta: 5,
    })
    .editing(Edit::Handling {
        stage: bioprism_worldfactory::preanalytic::Stage::Collection,
        field: "minutes".into(),
        value: json!(30),
    });
    let null = PreanalyticMutation::new(
        "cold-null",
        "cold-family",
        FaultKind::ColdIschaemia { minutes: 30 },
        Intensity::NULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "drift".into(),
        delta: 5,
    });

    let payload = call(
        &mut server,
        "preanalytic_apply",
        json!({
            "specimen": serde_json::to_value(&specimen).unwrap(),
            "mutation": serde_json::to_value(&mutation).unwrap(),
            "family": [serde_json::to_value(&null).unwrap(), serde_json::to_value(&mutation).unwrap()],
            "available_actions": ["detect"],
            "qc_field": "drift",
            "alert_at": 3
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["applied"], json!(true));
    assert_eq!(payload["biology_unchanged"], json!(true));
    assert_eq!(payload["has_signature"], json!(true));
    assert_eq!(payload["response_check"]["ok"], json!(true));
    assert_eq!(payload["family_validation"]["ok"], json!(true));
    assert_eq!(payload["detectability"]["intensity"], json!(10_000));

    let damaging = mutation.editing(Edit::Biology {
        field: "state".into(),
        value: json!("changed"),
    });
    let refused = call(
        &mut server,
        "preanalytic_apply",
        json!({
            "specimen": serde_json::to_value(&specimen).unwrap(),
            "mutation": serde_json::to_value(&damaging).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["applied"], json!(false));
    assert!(refused["refusal"].as_str().unwrap().contains("biological"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn contradiction_review_keeps_hypotheses_and_resolution_states_explicit() {
    let mut server = server();
    let left = Reading::new(
        "imaging",
        "marker",
        Lens::new("mri", "macroscopic").over(SpatialExtent::Whole),
        ScopeKey::new().exact("specimen", "S1").exact("time", "T1"),
        Reported::Value(ReadingValue::interval(50, 60)),
    );
    let right = Reading::new(
        "pathology",
        "marker",
        Lens::new("slide", "sampled").over(SpatialExtent::Sampled {
            region: "core".into(),
        }),
        ScopeKey::new().exact("specimen", "S1").exact("time", "T1"),
        Reported::Value(ReadingValue::interval(0, 10)),
    );
    let hypotheses = vec![
        Hypothesis::new(
            "h-sampling",
            Discordance::SpatialSampling {
                modality: ModalityId::new("pathology"),
            },
        ),
        Hypothesis::new("h-assay", Discordance::AssayScope),
        Hypothesis::new("h-time", Discordance::DifferentTime),
    ];
    let actions = vec![
        DiscriminatingAction::new("pathology-review", 1)
            .refuting("h-sampling")
            .refuting("h-time"),
        DiscriminatingAction::new("assay-review", 2).refuting("h-assay"),
    ];
    let args = json!({
        "left": serde_json::to_value(&left).unwrap(),
        "right": serde_json::to_value(&right).unwrap(),
        "intent": serde_json::to_value(DiscordanceClass::Resolvable).unwrap(),
        "hypotheses": serde_json::to_value(&hypotheses).unwrap(),
        "actions": serde_json::to_value(&actions).unwrap(),
        "references": [serde_json::to_value(ReferenceDiscordance::new("imaging", "pathology", 100, 10)).unwrap()],
        "notable_below_per_ten_thousand": 2_000
    });
    let pending = call(&mut server, "contradiction_review", args.clone());
    assert_eq!(pending["ok"], json!(true));
    assert_eq!(pending["validated"], json!(true));
    assert_eq!(pending["declared_hypothesis_count"], json!(3));
    assert_eq!(pending["admissible_hypothesis_count"], json!(2));
    assert_eq!(pending["state_name"], json!("not_yet_examined"));
    assert_eq!(
        pending["next_actions"][0]["evidence"],
        json!("pathology-review")
    );
    assert_eq!(pending["expectedness"]["ok"], json!(true));
    assert_eq!(
        pending["expectedness"]["value"]["expectedness"],
        json!("notable")
    );

    let narrowed = call(
        &mut server,
        "contradiction_review",
        json!({
            "left": args["left"].clone(),
            "right": args["right"].clone(),
            "intent": args["intent"].clone(),
            "hypotheses": args["hypotheses"].clone(),
            "actions": args["actions"].clone(),
            "examine": ["pathology-review"]
        }),
    );
    assert_eq!(narrowed["ok"], json!(true));
    assert_eq!(narrowed["state_name"], json!("not_yet_examined"));
    assert_eq!(narrowed["live_hypothesis_count"], json!(1));

    let over_narrowed = call(
        &mut server,
        "contradiction_review",
        json!({
            "left": args["left"].clone(),
            "right": args["right"].clone(),
            "intent": args["intent"].clone(),
            "hypotheses": args["hypotheses"].clone(),
            "actions": args["actions"].clone(),
            "examine": ["pathology-review", "assay-review"]
        }),
    );
    assert_eq!(over_narrowed["ok"], json!(false));
    assert_eq!(over_narrowed["fail_closed"], json!(true));
    assert!(over_narrowed["refusal"]
        .as_str()
        .unwrap()
        .contains("every remaining hypothesis"));
}

#[test]
fn policy_screen_denies_unknown_policy_and_admits_only_under_a_typed_rule() {
    let mut server = server();
    let request = json!({
        "principal": {
            "id": "agent-1",
            "role": "researcher",
            "clearance": { "max_classification": "public_aggregate", "compartments": [] },
            "site": "us",
            "authorities": []
        },
        "purpose": "research_analysis",
        "channel": "local_compute",
        "at": "2026-08-14T00:00:00Z"
    });

    let denied = call(
        &mut server,
        "policy_screen",
        json!({ "world": WORLD, "request": request.clone(), "facts": ["fact.cohort"] }),
    );
    assert_eq!(denied["ok"], json!(true));
    assert_eq!(denied["admitted_count"], json!(0));
    assert_eq!(denied["refused_count"], json!(1));
    assert_eq!(
        denied["refused"][0]["constraint"],
        json!("unlabelled_evidence")
    );
    assert_eq!(denied["complete"], json!(false));
    assert_eq!(denied["trace"]["supports_sufficiency_claim"], json!(false));

    let label = json!({
        "classification": "public_aggregate",
        "compartments": [],
        "purposes": { "only": ["research_analysis"] },
        "residency": "anywhere",
        "export": "unrestricted",
        "retention": "indefinite",
        "min_cell_size": 0
    });
    let admitted = call(
        &mut server,
        "policy_screen",
        json!({
            "world": WORLD,
            "request": request,
            "facts": ["fact.cohort"],
            "rules": [{
                "id": "world-cohort-policy",
                "version": 1,
                "scope": { "cohort": "RG-DEMO-001" },
                "label": label
            }]
        }),
    );
    assert_eq!(admitted["ok"], json!(true));
    assert_eq!(admitted["admitted_count"], json!(1));
    assert_eq!(admitted["refused_count"], json!(0));
    assert_eq!(admitted["complete"], json!(true));
    assert_eq!(
        admitted["admitted"][0]["admission"]["mode"],
        json!({ "mode": "central" })
    );
    assert!(admitted["policy_version"].as_str().unwrap().len() >= 32);
}

#[test]
fn governance_schema_surface_lists_contracts_and_checks_a_reference_certificate() {
    let mut server = server();
    let catalog = call(&mut server, "governance_schema_check", json!({}));
    assert_eq!(catalog["ok"], json!(true));
    assert_eq!(catalog["schema_count"], json!(3));
    assert!(catalog["schemas"][0]["hashed_paths"].is_array());

    let checked = call(
        &mut server,
        "governance_schema_check",
        json!({ "document": "fixtures/fiber-v0.1/golden/reference_certificate.json" }),
    );
    assert_eq!(checked["ok"], json!(true));
    assert_eq!(checked["mode"], json!("document"));
    assert_eq!(
        checked["schema"]["id"],
        json!("fiber-context-certificate/0.1")
    );
    assert_eq!(checked["conforms"], json!(true));
    assert_eq!(checked["is_clean"], json!(true));
}

#[test]
fn developer_platform_status_verifies_local_contracts_and_marks_foreign_artifacts() {
    let mut server = server();
    let payload = call(&mut server, "developer_platform_status", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["devplat"]["digest"].is_string());
    assert!(payload["walkthroughs"].as_array().unwrap().len() >= 6);
    assert!(payload["walkthroughs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["documents_absent_artifact"] == json!(true)));
    assert_eq!(payload["cookbook"]["verification"]["clean"], json!(true));
    assert_eq!(payload["diagnostic_catalogue"]["clean"], json!(true));
    assert!(
        payload["developer_contract"]["surface_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(payload["limitations"].as_array().unwrap().len() >= 3);

    let bounded = call(
        &mut server,
        "developer_platform_status",
        json!({ "max_items": 1 }),
    );
    assert_eq!(bounded["detail_mode"], json!("summary"));
    assert_eq!(
        bounded["developer_contract"]["surfaces_returned"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        bounded["developer_contract"]["omitted_surfaces"]
            .as_u64()
            .unwrap()
            > 0
    );

    let detailed = call(
        &mut server,
        "developer_platform_status",
        json!({ "include_details": true, "max_items": 1 }),
    );
    assert_eq!(detailed["detail_mode"], json!("full"));
    assert!(detailed["details"]["developer_contract"].is_array());
}

#[test]
fn developer_delivery_audit_composes_local_health_and_blocks_missing_evidence() {
    let mut server = server();
    let payload = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {
                "id": "delivery-1",
                "targets": [
                    "local_delivery",
                    "developer_platform",
                    "developer_claims",
                    "repository_scope",
                    "sdk_admission",
                    "conformance",
                    "provider_capability",
                    "governance_schema",
                    "release"
                ]
            }
        }),
    );
    assert_eq!(payload["__isError"], json!(false));
    assert_eq!(payload["readiness"]["platform_checks_clean"], json!(true));
    assert_eq!(payload["readiness"]["repository_scope_clean"], json!(false));
    assert_eq!(payload["readiness"]["local_delivery_ready"], json!(false));
    assert_eq!(
        payload["external_surface_posture"]["foreign_artifacts_present"],
        json!(true)
    );
    assert_eq!(
        payload["external_surface_posture"]["local_integration_foundations"][0]["artifact"],
        json!("python/prism_sdk")
    );
    assert_eq!(payload["release_request"]["ready"], json!(false));
    let targets = payload["release_request"]["targets"].as_array().unwrap();
    let sdk = targets
        .iter()
        .find(|row| row["target"] == json!("sdk_admission"))
        .unwrap();
    assert_eq!(sdk["available"], json!(false));
    assert_eq!(sdk["eligible"], json!(false));
    let claims = targets
        .iter()
        .find(|row| row["target"] == json!("developer_claims"))
        .unwrap();
    assert_eq!(claims["eligible"], json!(false));
    assert_eq!(
        claims["blockers"][0],
        json!("unguarded_developer_claims_present")
    );

    let no_request = call(&mut server, "developer_delivery_audit", json!({}));
    assert_eq!(no_request["release_request"]["present"], json!(false));
    assert_eq!(no_request["release_request"]["ready"], json!(false));

    let duplicate = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {
                "id": "duplicate",
                "targets": ["local_delivery", "local_delivery"]
            }
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(duplicate["error"].as_str().unwrap().contains("duplicate"));
}

#[test]
fn engineering_manifest_audit_keeps_topology_ticket_readiness_and_raci_separate() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-engineering-manifest/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "baseline": {
            "language": "Rust 2021", "runtime": "cargo", "api": "MCP JSON-RPC",
            "storage": "in-memory", "observability": "structured stderr audit", "deployment": "local process"
        },
        "packages": [
            { "id": "core", "path": "crates/core", "language": "rust", "kind": "library", "owner": "platform", "depends_on": [], "public": true },
            { "id": "api", "path": "crates/api", "language": "rust", "kind": "service", "owner": "platform", "depends_on": ["core"], "public": true }
        ],
        "tickets": [
            { "id": "T-001", "title": "ship core", "package": "core", "contract": "core-contract", "status": "done", "depends_on": [], "acceptance": ["core tests pass"] },
            { "id": "T-002", "title": "ship api", "package": "api", "contract": "api-contract", "status": "planned", "depends_on": ["T-001"], "acceptance": ["protocol tests pass"] }
        ],
        "adrs": [{ "id": "ADR-001", "title": "use rust", "status": "accepted", "decision": "Rust owns canonical semantics", "affects": ["core", "api"] }],
        "ownership": [{ "surface": "api", "accountable": "platform-lead", "responsible": ["api-team"], "independent_reviewer": "review-board" }]
    });
    let result = call(
        &mut server,
        "engineering_manifest_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["audit"]["package_order"], json!(["core", "api"]));
    assert_eq!(
        result["audit"]["ticket_readiness"][1]["state"],
        json!("actionable")
    );
    assert_eq!(result["blocking_issue_count"], json!(0));
    assert_eq!(result["manifest_digest"].as_str().unwrap().len(), 64);

    let mut cyclic = manifest;
    cyclic["packages"][0]["depends_on"] = json!(["api", "missing"]);
    cyclic["ownership"][0]["independent_reviewer"] = json!("platform-lead");
    let refused = call(
        &mut server,
        "engineering_manifest_audit",
        json!({ "manifest": cyclic }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["valid"], json!(false));
    assert!(refused["audit"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "package_cycle"));
    assert!(refused["audit"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "reviewer_not_independent"));
}

#[test]
fn engineering_execution_plan_derives_waves_critical_path_and_fail_closed_manifest_gate() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-engineering-manifest/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "baseline": { "language": "Rust 2021", "runtime": "cargo", "api": "MCP JSON-RPC", "storage": "in-memory", "observability": "structured stderr audit", "deployment": "local process" },
        "packages": [
            { "id": "core", "path": "crates/core", "language": "rust", "kind": "library", "owner": "platform", "depends_on": [], "public": true },
            { "id": "api", "path": "crates/api", "language": "rust", "kind": "service", "owner": "platform", "depends_on": ["core"], "public": true }
        ],
        "tickets": [
            { "id": "T-001", "title": "ship core", "package": "core", "contract": "core-contract", "status": "done", "depends_on": [], "acceptance": ["core tests pass"] },
            { "id": "T-002", "title": "ship api", "package": "api", "contract": "api-contract", "status": "planned", "depends_on": ["T-001"], "acceptance": ["protocol tests pass"] },
            { "id": "T-003", "title": "publish api", "package": "api", "contract": "release-contract", "status": "planned", "depends_on": ["T-002"], "acceptance": ["release evidence exists"] }
        ],
        "adrs": [{ "id": "ADR-001", "title": "use rust", "status": "accepted", "decision": "Rust owns canonical semantics", "affects": ["core", "api"] }],
        "ownership": [{ "surface": "api", "accountable": "platform-lead", "responsible": ["api-team"], "independent_reviewer": "review-board" }]
    });
    let request =
        json!({ "schema": "bioprism-engineering-plan/0.1", "manifest": manifest.clone() });
    let result = call(
        &mut server,
        "engineering_execution_plan",
        json!({ "request": request.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["engineering_plan_ready"], json!(true));
    assert_eq!(result["audit"]["waves"].as_array().unwrap().len(), 2);
    assert_eq!(result["audit"]["waves"][0]["ticket_ids"], json!(["T-002"]));
    assert_eq!(result["audit"]["waves"][1]["ticket_ids"], json!(["T-003"]));
    assert_eq!(
        result["audit"]["critical_path"],
        json!(["T-001", "T-002", "T-003"])
    );
    assert_eq!(result["audit"]["planned_ticket_count"], json!(2));
    assert_eq!(result["plan_digest"].as_str().unwrap().len(), 64);

    let mut refused = request;
    refused["manifest"]["tickets"][1]["depends_on"] = json!(["missing"]);
    let refusal = call(
        &mut server,
        "engineering_execution_plan",
        json!({ "request": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["engineering_plan_ready"], json!(false));
    assert_eq!(refusal["audit"]["planning_started"], json!(false));
    assert!(refusal["audit"]["manifest_issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "missing_ticket_dependency"));
    assert!(refusal["audit"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "manifest_invalid"));
}

#[test]
fn release_pipeline_audit_preserves_promotion_provenance_and_rollback_boundaries() {
    let mut server = server();
    let digest = "a".repeat(64);
    let manifest = json!({
        "schema": "bioprism-release-pipeline/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "source": { "ref_name": "main", "commit_digest": digest, "workflow": "release.yml" },
        "environments": [
            { "id": "staging", "class": "staging", "protected": true, "required_approvals": 0, "secrets_allowed": true, "immutable_artifacts": true },
            { "id": "production", "class": "production", "protected": true, "required_approvals": 1, "secrets_allowed": true, "immutable_artifacts": true }
        ],
        "stages": [
            { "id": "build", "kind": "build", "environment": "staging", "depends_on": [], "command": "cargo build --locked", "produces": ["binary"], "required": true },
            { "id": "test", "kind": "test", "environment": "staging", "depends_on": ["build"], "command": "cargo test --locked", "produces": [], "required": true }
        ],
        "artifacts": [{ "id": "binary", "kind": "binary", "digest": digest, "produced_by": "build", "inputs": [], "attestations": ["prov", "sig"], "immutable": true }],
        "attestations": [
            { "id": "prov", "kind": "provenance", "artifact": "binary", "digest": digest, "issuer": "ci", "statement": "built from pinned source" },
            { "id": "sig", "kind": "signature", "artifact": "binary", "digest": digest, "issuer": "release-key", "statement": "signed artifact" },
            { "id": "approval", "kind": "approval", "artifact": "binary", "digest": digest, "issuer": "release-board", "statement": "approved" }
        ],
        "promotions": [
            { "id": "to-production", "kind": "advance", "from": "staging", "to": "production", "artifacts": ["binary"], "required_attestations": ["prov", "sig"], "approvals": ["approval"], "rollback_target": "rollback" },
            { "id": "rollback", "kind": "rollback", "from": "production", "to": "staging", "artifacts": ["binary"], "required_attestations": ["prov"], "approvals": [] }
        ]
    });
    let result = call(
        &mut server,
        "release_pipeline_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["release_ready"], json!(true));
    assert_eq!(result["audit"]["stage_order"], json!(["build", "test"]));
    assert_eq!(
        result["audit"]["promotion_audits"][0]["rollback_present"],
        json!(true)
    );
    assert_eq!(result["blocking_issue_count"], json!(0));

    let mut refused = manifest;
    refused["attestations"][1]["digest"] = json!("b".repeat(64));
    refused["promotions"][0]["rollback_target"] = json!(null);
    refused["stages"][0]["depends_on"] = json!(["test"]);
    let refusal = call(
        &mut server,
        "release_pipeline_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["release_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "attestation_digest_mismatch"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "production_rollback_missing"));
    assert!(issues.iter().any(|issue| issue["code"] == "stage_cycle"));
}

#[test]
fn operational_readiness_audit_keeps_observation_fallback_and_incident_closure_explicit() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-operational-readiness/0.1",
        "service": { "id": "prism-api", "version": "0.1.0", "owner": "platform-oncall", "criticality": "critical" },
        "contracts": [{ "id": "availability", "kind": "availability", "objective": "serve health checks", "target": "99.9%", "required": true }],
        "indicators": [{ "id": "availability-sli", "contract": "availability", "metric": "request_success_ratio", "source": "telemetry-digest", "status": "observed", "measurement": "0.999", "evidence_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }],
        "dependencies": [{ "id": "registry", "name": "artifact registry", "owner": "release-team", "criticality": "critical", "failure_mode": "artifact fetch unavailable", "fallback": "pinned offline mirror" }],
        "runbooks": [{ "id": "api-degraded", "trigger": "availability below target", "owner": "platform-oncall", "steps": ["freeze rollout", "restore last known good"], "review_status": "reviewed", "incident_classes": ["availability"] }],
        "incidents": [{ "id": "inc-1", "severity": "sev2", "state": "closed", "runbook": "api-degraded", "owner": "platform-oncall", "timeline": ["detected", "contained", "restored"], "postmortem": "postmortem-digest" }],
        "controls": { "on_call": true, "alerting": true, "tracing": true, "audit_logging": true, "backup": true, "restore_test": true, "access_review": true }
    });
    let result = call(
        &mut server,
        "operational_readiness_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["operationally_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["observed_indicators"], json!(1));
    assert_eq!(
        result["audit"]["dependency_audits"][0]["fallback_present"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["incident_audits"][0]["postmortem_present"],
        json!(true)
    );

    let mut refused = manifest;
    refused["indicators"][0]["status"] = json!("not_observed");
    refused["dependencies"][0]["fallback"] = json!(null);
    refused["controls"]["restore_test"] = json!(false);
    refused["incidents"][0]["postmortem"] = json!(null);
    let refusal = call(
        &mut server,
        "operational_readiness_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["operationally_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "indicator_not_observed"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "critical_dependency_fallback_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "required_control_disabled"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "closed_incident_postmortem_missing"));
}

#[test]
fn security_privacy_audit_keeps_asset_flow_identity_threat_and_review_layers_explicit() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-security-privacy/0.1",
        "system": { "id": "prism-api", "version": "0.1.0", "owner": "platform" },
        "assets": [{ "id": "patient-records", "name": "records", "classification": "regulated", "owner": "privacy", "purpose": "care research", "retention_days": 365, "residency": "us", "deletion_process": "erase workflow" }],
        "flows": [{ "id": "api-to-vendor", "asset": "patient-records", "source": "api", "destination": "approved-vendor", "purpose": "care research", "legal_basis": "consent", "decision": "allow", "authorization_evidence": digest }],
        "identities": [{ "id": "researcher", "principal": "team", "role": "research", "authentication": "oidc", "mfa": true, "least_privilege": true, "assets": ["patient-records"] }],
        "threats": [{ "id": "exfiltration", "category": "data-exfiltration", "severity": "high", "status": "mitigated", "control": "dlp", "evidence_digest": digest }],
        "reviews": [{ "id": "pia-1", "kind": "privacy_impact", "scope": "patient-records", "reviewer": "independent-reviewer", "status": "complete", "evidence_digest": digest, "expires_at": "2027-01-01", "findings": ["none"] }],
        "controls": { "access_control": true, "encryption_at_rest": true, "encryption_in_transit": true, "key_rotation": true, "audit_logging": true, "vulnerability_management": true, "backup_restore": true, "incident_response": true, "vendor_review": true, "data_subject_rights": true }
    });
    let result = call(
        &mut server,
        "security_privacy_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["security_privacy_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["sensitive_assets"], json!(1));
    assert_eq!(
        result["audit"]["flow_audits"][0]["authorization_present"],
        json!(true)
    );
    assert_eq!(result["audit"]["identity_audits"][0]["ready"], json!(true));
    assert_eq!(
        result["audit"]["threat_audits"][0]["evidence_valid"],
        json!(true)
    );
    assert_eq!(result["audit"]["review_audits"][0]["complete"], json!(true));

    let mut refused = manifest;
    refused["assets"][0]["retention_days"] = Value::Null;
    refused["flows"][0]["authorization_evidence"] = Value::Null;
    refused["identities"][0]["mfa"] = json!(false);
    refused["threats"][0]["evidence_digest"] = Value::Null;
    refused["reviews"][0]["status"] = json!("expired");
    refused["controls"]["encryption_at_rest"] = json!(false);
    let refusal = call(
        &mut server,
        "security_privacy_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["security_privacy_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "sensitive_retention_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "flow_authorization_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "sensitive_mfa_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "mitigation_evidence_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "review_evidence_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "required_control_disabled"));
}

#[test]
fn sandbox_admission_audit_keeps_artifact_isolation_capability_resource_and_output_layers_explicit()
{
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-sandbox/0.1",
        "system": { "id": "prism-sandbox", "version": "0.1.0", "owner": "platform" },
        "artifacts": [
            { "id": "source", "kind": "source_code", "digest": digest, "source": "repo/source.py", "producer": "ci", "trust": "reviewed" },
            { "id": "dataset", "kind": "dataset", "digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "source": "registry/dataset", "producer": "registry", "trust": "untrusted", "inputs": ["source"] }
        ],
        "profiles": [{
            "id": "profile", "artifact": "dataset", "runtime": "oci", "image_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "environment_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", "user": "runner", "rootless": true, "read_only_root": true, "no_privilege_escalation": true, "network": "allowlist", "network_allowlist": ["packages.example"], "mounts": [{ "id": "input", "source_artifact": "dataset", "target": "/inputs/data", "mode": "read_only" }], "capabilities": ["network"], "resources": { "cpu_millis": 1000, "memory_mb": 1024, "wall_time_seconds": 60, "processes": 8, "output_bytes": 1000000 }, "output_quarantine": true, "release_requires_review": true
        }],
        "capabilities": [{ "id": "network", "profile": "profile", "kind": "network_egress", "target": "packages.example", "decision": "allow", "evidence_digest": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" }],
        "outputs": [{ "id": "result", "profile": "profile", "artifact": "dataset", "digest": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "destination": "quarantine", "quarantined": true, "released": false, "reviewed": false, "parents": ["dataset"] }]
    });
    let result = call(
        &mut server,
        "sandbox_admission_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["sandbox_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["untrusted_artifacts"], json!(1));
    assert_eq!(
        result["audit"]["profile_audits"][0]["isolation_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["capability_audits"][0]["evidence_valid"],
        json!(true)
    );
    assert_eq!(result["audit"]["resource_audits"][0]["ready"], json!(true));
    assert_eq!(
        result["audit"]["output_audits"][0]["quarantined"],
        json!(true)
    );

    let mut refused = manifest;
    refused["profiles"][0]["rootless"] = json!(false);
    refused["profiles"][0]["network"] = json!("unrestricted");
    refused["profiles"][0]["resources"]["memory_mb"] = Value::Null;
    refused["capabilities"][0]["target"] = json!("*");
    refused["capabilities"][0]["evidence_digest"] = Value::Null;
    let refusal = call(
        &mut server,
        "sandbox_admission_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["sandbox_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "rootless_required"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "network_boundary_invalid"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "resource_limits_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "capability_target_broad"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "dangerous_capability_evidence_missing"));
}

#[test]
fn sandbox_runtime_simulate_preserves_admission_capability_resource_and_refusal_layers() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let runtime_manifest = json!({
        "schema": "bioprism-sandbox-runtime/0.1",
        "admission": {
            "schema": "bioprism-sandbox/0.1",
            "system": { "id": "runtime", "version": "0.1.0", "owner": "platform" },
            "artifacts": [
                { "id": "source", "kind": "source_code", "digest": digest, "source": "repo/source.py", "producer": "ci", "trust": "reviewed" },
                { "id": "dataset", "kind": "dataset", "digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "source": "registry/dataset", "producer": "registry", "trust": "untrusted", "inputs": ["source"] }
            ],
            "profiles": [{
                "id": "profile", "artifact": "dataset", "runtime": "oci", "image_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "environment_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", "user": "runner", "rootless": true, "read_only_root": true, "no_privilege_escalation": true, "network": "allowlist", "network_allowlist": ["packages.example"], "mounts": [{ "id": "input", "source_artifact": "dataset", "target": "/inputs/data", "mode": "read_only" }], "capabilities": ["read", "network"], "resources": { "cpu_millis": 1000, "memory_mb": 1024, "wall_time_seconds": 60, "processes": 8, "output_bytes": 1000000 }, "output_quarantine": true, "release_requires_review": true
            }],
            "capabilities": [
                { "id": "read", "profile": "profile", "kind": "filesystem_read", "target": "/inputs/data", "decision": "allow" },
                { "id": "network", "profile": "profile", "kind": "network_egress", "target": "packages.example", "decision": "allow", "evidence_digest": digest }
            ]
        },
        "profile": "profile",
        "requests": [
            { "id": "read-input", "kind": "filesystem_read", "target": "/inputs/data", "cpu_millis": 100, "memory_mb": 128, "wall_time_seconds": 5, "processes": 1, "output_bytes": 1000 },
            { "id": "fetch-package", "kind": "network_egress", "target": "packages.example", "cpu_millis": 100, "memory_mb": 128, "wall_time_seconds": 5, "processes": 1, "output_bytes": 1000 }
        ]
    });
    let result = call(
        &mut server,
        "sandbox_runtime_simulate",
        json!({ "manifest": runtime_manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["sandbox_runtime_ready"], json!(true));
    assert_eq!(result["audit"]["admission_valid"], json!(true));
    assert_eq!(result["audit"]["simulated_count"], json!(2));
    assert_eq!(result["audit"]["usage"]["cpu_millis"], json!(200));
    assert_eq!(result["audit"]["steps"][0]["decision"], json!("simulated"));
    assert!(result["trace_digest"].is_string());

    let mut refused = runtime_manifest;
    refused["requests"][0]["cpu_millis"] = json!(2000);
    let refusal = call(
        &mut server,
        "sandbox_runtime_simulate",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["audit"]["refused_count"], json!(1));
    assert_eq!(refusal["audit"]["not_run_count"], json!(1));
    assert_eq!(refusal["audit"]["stopped_on_refusal"], json!(true));
    assert_eq!(
        refusal["audit"]["steps"][0]["refusal"],
        json!("resource_budget_exceeded")
    );
    assert_eq!(refusal["audit"]["steps"][1]["decision"], json!("not_run"));
}

#[test]
fn security_program_audit_keeps_scope_campaign_finding_incident_and_disclosure_layers_explicit() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-security-program/0.1",
        "system": { "id": "aurora-security", "version": "0.1.0", "owner": "security-owner", "mission": "bounded adversarial assurance" },
        "scopes": [{ "id": "api-staging", "name": "staging API", "kind": "api", "target": "api-staging.internal", "owner": "service-owner", "authorization_digest": digest, "allowed_methods": ["authenticated-read", "rate-limited-input"], "forbidden_actions": ["production-write", "credential-exfiltration"], "environments": ["isolated-staging"], "data_handling": "synthetic fixtures only" }],
        "campaigns": [{ "id": "campaign-1", "scope": "api-staging", "operator": "red-team", "independent_reviewer": "independent-reviewer", "methodology": "bounded mutation and manual review", "hypothesis": "invalid input can cross a trust boundary", "status": "completed", "started_at": "2026-01-01", "completed_at": "2026-01-02", "evidence_digest": digest, "stop_conditions": ["stop on production boundary"], "finding_ids": ["finding-1"] }],
        "findings": [{ "id": "finding-1", "campaign": "campaign-1", "title": "boundary mismatch", "severity": "high", "status": "closed", "evidence_digest": digest, "reproduction_digest": digest, "regression_digest": digest, "discovered_at": "2026-01-02", "affected_targets": ["api-staging"], "remediation_ids": ["remediation-1"], "incident_id": "incident-1", "public_safe": true }],
        "remediations": [{ "id": "remediation-1", "finding": "finding-1", "owner": "service-owner", "action": "validate boundary before dispatch", "status": "complete", "due_at": "2026-01-10", "verification_digest": digest }],
        "incidents": [{ "id": "incident-1", "finding": "finding-1", "severity": "high", "owner": "incident-owner", "status": "closed", "opened_at": "2026-01-02", "contained_at": "2026-01-02", "closed_at": "2026-01-03", "containment_evidence": digest, "closure_evidence": digest, "notification_required": true, "timeline": [{ "epoch": 1, "actor": "incident-owner", "event": "incident opened", "evidence_digest": digest }, { "epoch": 2, "actor": "incident-owner", "event": "containment verified", "evidence_digest": digest }] }],
        "disclosures": [{ "id": "advisory-1", "finding": "finding-1", "stage": "advisory", "audience": "affected operators", "requested_at": "2026-01-04", "approver": "independent-reviewer", "approval_digest": digest, "advisory_digest": digest, "published_at": "2026-01-04" }],
        "controls": { "scope_authorization": true, "operator_separation": true, "independent_review": true, "evidence_retention": true, "remediation_tracking": true, "incident_response": true, "disclosure_review": true, "regression_testing": true }
    });
    let result = call(
        &mut server,
        "security_program_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["security_program_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["authorized_scopes"], json!(1));
    assert_eq!(
        result["audit"]["finding_audits"][0]["incident_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["remediation_audits"][0]["verification_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["incident_audits"][0]["closure_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["disclosure_audits"][0]["approval_valid"],
        json!(true)
    );

    let mut refused = manifest;
    refused["scopes"][0]["authorization_digest"] = Value::Null;
    refused["campaigns"][0]["independent_reviewer"] = Value::Null;
    refused["findings"][0]["evidence_digest"] = Value::Null;
    refused["remediations"][0]["verification_digest"] = Value::Null;
    refused["incidents"][0]["closure_evidence"] = Value::Null;
    refused["controls"]["disclosure_review"] = json!(false);
    let refusal = call(
        &mut server,
        "security_program_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["security_program_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "scope_authorization_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "campaign_independent_review_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "finding_evidence_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "remediation_verification_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "incident_closure_missing"));
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "required_control_disabled"));
}

#[test]
fn developer_workbench_audits_notebook_digests_queries_dashboard_and_plans_ci() {
    let mut server = server();
    let digest = "a".repeat(64);
    let output_digest = "b".repeat(64);
    let payload = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {
                "session_id": "studio-1",
                "owner": "agent-a",
                "goal": "author an oncology capability card",
                "environment_digest": digest,
                "artifacts": [{
                    "id": "artifact-1",
                    "title": "verification card",
                    "path": "artifacts/verification.json",
                    "domain": "oncology",
                    "capability": "verification",
                    "state": "validated",
                    "evidence": "reproduced",
                    "digest": digest,
                    "score": 0.8,
                    "tags": ["public-card"]
                }],
                "cells": [{
                    "id": "cell-1",
                    "kind": "query",
                    "source": "workspace.metrics_analytics_audit(...)" ,
                    "inputs": [{"artifact_id": "artifact-1", "digest": digest}],
                    "depends_on": [],
                    "executed": true,
                    "output_digest": output_digest
                }],
                "changes": [{
                    "id": "change-1",
                    "artifact_id": "artifact-1",
                    "kind": "create",
                    "actor": "agent-a",
                    "logical_time": 1,
                    "input_digest": null,
                    "output_digest": digest,
                    "reason": "initial artifact"
                }]
            },
            "dashboard": {"domains": ["oncology"], "include_holes": true, "limit": 10},
            "ci": {
                "workflow": "consumer contracts",
                "triggers": ["push", "pull_request"],
                "rust_toolchain": "1.85.0",
                "offline": true,
                "checks": [{"name": "tests", "run": "cargo test --workspace --offline", "required": true}]
            }
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["workflow"], json!("developer_workbench"));
    assert_eq!(payload["audit"]["stale_cells"].as_array().unwrap().len(), 0);
    assert_eq!(payload["dashboard"]["rows"][0]["score"], json!(0.8));
    assert_eq!(payload["ci"]["execution"], json!("not_executed"));
    assert_eq!(payload["ci"]["network_access"], json!("denied_by_plan"));
    assert!(payload["ci"]["workflow_yaml"]
        .as_str()
        .unwrap()
        .contains("cargo test --workspace --offline"));

    let mut stale = json!({
        "session": {
            "session_id": "studio-2", "owner": "agent-a", "goal": "stale check",
            "artifacts": [{
                "id": "artifact-1", "title": "card", "path": "card.json", "domain": "bioir",
                "capability": "retrieval", "state": "validated", "evidence": "observed", "digest": digest
            }],
            "cells": [{
                "id": "cell-1", "kind": "review", "source": "review", "inputs": [{"artifact_id": "artifact-1", "digest": digest}],
                "depends_on": [], "executed": true, "output_digest": output_digest
            }],
            "changes": []
        }
    });
    stale["session"]["artifacts"][0]["digest"] = "c".repeat(64).into();
    let stale_result = call(&mut server, "developer_workbench", stale);
    assert_eq!(stale_result["__isError"], json!(false));
    assert_eq!(stale_result["audit"]["stale_cells"], json!(["cell-1"]));
    assert_eq!(stale_result["audit"]["release_ready"], json!(false));
}

#[test]
fn developer_workbench_refuses_notebook_cycles_and_unsafe_ci() {
    let mut server = server();
    let cycle = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {
                "session_id": "cycle", "owner": "agent-a", "goal": "cycle",
                "artifacts": [],
                "cells": [
                    {"id": "a", "kind": "review", "source": "a", "depends_on": ["b"], "executed": true, "output_digest": "a".repeat(64)},
                    {"id": "b", "kind": "review", "source": "b", "depends_on": ["a"], "executed": true, "output_digest": "b".repeat(64)}
                ],
                "changes": []
            }
        }),
    );
    assert_eq!(cycle["__isError"], json!(true));
    assert!(cycle["error"].as_str().unwrap().contains("cycle"));

    let unsafe_ci = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "ci", "owner": "agent-a", "goal": "ci", "artifacts": [], "cells": [], "changes": []},
            "ci": {
                "workflow": "ci", "triggers": ["push"], "rust_toolchain": "stable",
                "checks": [{"name": "bad", "run": "cargo test", "working_directory": "../outside"}]
            }
        }),
    );
    assert_eq!(unsafe_ci["__isError"], json!(true));
    assert!(unsafe_ci["error"]
        .as_str()
        .unwrap()
        .contains("parent directory"));
}

#[test]
fn ci_execution_evidence_audit_reconciles_plan_and_run_without_execution() {
    let mut server = server();
    let ci = json!({
        "workflow": "consumer contracts",
        "triggers": ["push", "pull_request"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [
            {"name": "tests", "run": "cargo test --workspace --offline", "required": true},
            {"name": "lint", "run": "cargo clippy --workspace --offline", "required": false}
        ]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "ci-evidence", "owner": "agent-a", "goal": "reconcile CI", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let plan_digest = planned["ci"]["digest"].as_str().unwrap().to_owned();
    let result = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({
            "ci": ci.clone(),
            "evidence": {
                "run_id": "run-42",
                "provider": "github_actions",
                "source": "provider_observed",
                "plan_digest": plan_digest,
                "conclusion": "success",
                "checks": [
                    {"name": "tests", "status": "passed", "result_digest": "a".repeat(64)},
                    {"name": "lint", "status": "passed", "result_digest": "b".repeat(64)}
                ]
            }
        }),
    );
    assert_eq!(result["workflow"], json!("ci_execution_evidence_audit"));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["ci_evidence_ready"], json!(true));
    assert_eq!(result["audit"]["complete"], json!(true));
    assert_eq!(result["audit"]["passed_check_count"], json!(2));
    assert_eq!(result["audit"]["verification"], json!("structural_only"));
    assert_eq!(
        result["audit"]["execution"],
        json!("evidence_supplied_not_executed_here")
    );

    let incomplete = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({
            "ci": ci,
            "evidence": {
                "run_id": "run-43",
                "provider": "caller",
                "source": "caller_attested",
                "plan_digest": result["plan_digest"].clone(),
                "conclusion": "success",
                "checks": [
                    {"name": "tests", "status": "failed", "result_digest": "c".repeat(64)}
                ]
            }
        }),
    );
    assert_eq!(incomplete["valid"], json!(false));
    assert_eq!(incomplete["ci_evidence_ready"], json!(false));
    assert_eq!(incomplete["audit"]["required_failed"], json!(["tests"]));
    assert!(incomplete["audit"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "missing_check_evidence"));
}

#[test]
fn ci_provider_normalize_projects_github_payload_into_auditable_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "provider-normalizer",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [
            {"name": "tests", "run": "cargo test --workspace --offline", "required": true},
            {"name": "lint", "run": "cargo clippy --workspace --offline", "required": false}
        ]
    });
    let normalized = call(
        &mut server,
        "ci_provider_normalize",
        json!({
            "ci": ci.clone(),
            "provider": "github_actions",
            "payload": {
                "run": {"id": 9001, "conclusion": "success", "html_url": "https://example.test/runs/9001"},
                "jobs": [
                    {"name": "tests", "conclusion": "success"},
                    {"name": "lint", "conclusion": "success"}
                ]
            }
        }),
    );
    assert_eq!(normalized["workflow"], json!("ci_provider_normalize"));
    assert_eq!(normalized["provider"], json!("github_actions"));
    assert_eq!(normalized["source"], json!("provider_observed"));
    assert_eq!(normalized["run_id"], json!("9001"));
    assert_eq!(normalized["derived_result_digest_count"], json!(2));
    assert_eq!(
        normalized["evidence"]["checks"][0]["status"],
        json!("passed")
    );
    assert_eq!(
        normalized["evidence"]["run_url"],
        json!("https://example.test/runs/9001")
    );

    let gitlab = call(
        &mut server,
        "ci_provider_normalize",
        json!({
            "ci": ci.clone(),
            "provider": "gitlab_ci",
            "payload": {
                "pipeline": {"id": 9002, "status": "success", "web_url": "https://gitlab.example/pipelines/9002"},
                "jobs": [
                    {"name": "tests", "status": "success", "duration": 1.5},
                    {"name": "lint", "status": "skipped"}
                ]
            }
        }),
    );
    assert_eq!(gitlab["source"], json!("provider_observed"));
    assert_eq!(gitlab["run_id"], json!("9002"));
    assert_eq!(gitlab["evidence"]["checks"][0]["duration_ms"], json!(1500));

    let audited = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({"ci": ci, "evidence": normalized["evidence"].clone()}),
    );
    assert_eq!(audited["valid"], json!(true));
    assert_eq!(audited["ci_evidence_ready"], json!(true));
}

#[test]
fn ci_provider_evidence_audit_binds_rows_and_preserves_structural_limits() {
    let mut server = server();
    let ci = json!({
        "workflow": "provider-evidence",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let digest = |label: &str| ContentHash::of_bytes(label.as_bytes()).to_string();
    let audited = call(
        &mut server,
        "ci_provider_evidence_audit",
        json!({
            "ci": ci.clone(),
            "provider": "github_actions",
            "payload": {
                "run": {"id": 9020, "conclusion": "success"},
                "jobs": [{"name": "tests", "conclusion": "success"}]
            },
            "artifacts": [{
                "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                "check": "tests", "run_id": "9020", "provider": "github_actions",
                "uri": "https://example.test/artifact"
            }],
            "logs": [{
                "id": "log-tests", "digest": digest("log"), "check": "tests",
                "run_id": "9020", "provider": "github_actions", "truncated": false
            }],
            "attestations": [{
                "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                "statement_digest": digest("statement"), "method": "declared_provider_statement"
            }]
        }),
    );
    assert_eq!(audited["workflow"], json!("ci_provider_evidence_audit"));
    assert_eq!(audited["valid"], json!(true));
    assert_eq!(audited["conformance_ready"], json!(true));
    assert_eq!(audited["audit"]["artifact_count"], json!(1));
    assert_eq!(audited["audit"]["linked_log_count"], json!(1));
    assert_eq!(audited["audit"]["attestation_subject_count"], json!(1));
    assert_eq!(audited["audit"]["verification"], json!("structural_only"));

    let tampered = json!({
        "ci": ci,
        "provider": "github_actions",
        "payload": {
            "run": {"id": 9020, "conclusion": "success"},
            "jobs": [{"name": "tests", "conclusion": "success"}]
        },
        "artifacts": [{
            "id": "artifact-tests", "kind": "junit", "digest": "not-a-digest",
            "check": "unknown", "run_id": "wrong", "provider": "wrong"
        }],
        "attestations": [{
            "id": "attestation-tests", "subject": "missing", "issuer": "caller",
            "statement_digest": digest("statement"), "method": "declared"
        }]
    });
    let refused_rows = call(&mut server, "ci_provider_evidence_audit", tampered);
    assert_eq!(refused_rows["valid"], json!(false));
    assert_eq!(refused_rows["conformance_ready"], json!(false));
    for code in [
        "artifact_digest_invalid",
        "unknown_check_binding",
        "run_binding_mismatch",
        "provider_binding_mismatch",
        "attestation_subject_unknown",
    ] {
        assert!(refused_rows["audit"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == code));
    }
}

#[test]
fn developer_delivery_composes_provider_normalization_into_ci_release_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery-provider",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_provider": {
                "ci": ci,
                "provider": "github_actions",
                "payload": {
                    "run": {"id": 9010, "conclusion": "success"},
                    "jobs": [{"name": "tests", "conclusion": "success"}]
                }
            },
            "release_request": {"id": "delivery-provider-1", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(delivery["workflow"], json!("developer_delivery_audit"));
    assert_eq!(
        delivery["ci_provider_normalization"]["provider"],
        json!("github_actions")
    );
    assert_eq!(
        delivery["ci_provider_normalization"]["run_id"],
        json!("9010")
    );
    assert_eq!(
        delivery["readiness"]["ci_execution_evidence_ready"],
        json!(true)
    );
    assert_eq!(delivery["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(delivery["release_request"]["ready"], json!(true));

    let rejected = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_evidence": {"ci": {}, "evidence": {}},
            "ci_provider": {"ci": {}, "provider": "generic", "payload": {"run_id": "1", "checks": []}}
        }),
    );
    assert_eq!(rejected["__isError"], json!(true));
}

#[test]
fn provider_evidence_flows_into_delivery_receipts_and_tamper_verification() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery-provider-evidence",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let digest = |label: &str| ContentHash::of_bytes(label.as_bytes()).to_string();
    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_provider_evidence": {
                "ci": ci.clone(),
                "provider": "github_actions",
                "payload": {
                    "run": {"id": 9030, "conclusion": "success"},
                    "jobs": [{"name": "tests", "conclusion": "success"}]
                },
                "artifacts": [{
                    "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                    "check": "tests", "run_id": "9030", "provider": "github_actions"
                }],
                "logs": [{
                    "id": "log-tests", "digest": digest("log"), "check": "tests",
                    "run_id": "9030", "provider": "github_actions"
                }],
                "attestations": [{
                    "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                    "statement_digest": digest("statement"), "method": "declared"
                }]
            },
            "release_request": {"id": "delivery-provider-evidence-1", "targets": ["ci_provider_evidence"]}
        }),
    );
    assert_eq!(
        delivery["readiness"]["ci_provider_evidence_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["ci_provider_evidence"]["conformance_ready"],
        json!(true)
    );
    assert_eq!(delivery["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(
        delivery["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(delivery["release_request"]["ready"], json!(true));

    let receipt = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-provider-evidence-1",
            "delivery": {
                "ci_provider_evidence": {
                    "ci": ci,
                    "provider": "github_actions",
                    "payload": {
                        "run": {"id": 9030, "conclusion": "success"},
                        "jobs": [{"name": "tests", "conclusion": "success"}]
                    },
                    "artifacts": [{
                        "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                        "check": "tests", "run_id": "9030", "provider": "github_actions"
                    }],
                    "logs": [{
                        "id": "log-tests", "digest": digest("log"), "check": "tests",
                        "run_id": "9030", "provider": "github_actions"
                    }],
                    "attestations": [{
                        "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                        "statement_digest": digest("statement"), "method": "declared"
                    }]
                },
                "release_request": {"id": "delivery-provider-evidence-1", "targets": ["ci_provider_evidence"]}
            }
        }),
    );
    assert_eq!(receipt["valid"], json!(true));
    assert_eq!(receipt["receipt_ready"], json!(true));
    assert_eq!(
        receipt["evidence"][10]["name"],
        json!("ci_provider_evidence")
    );
    assert_eq!(receipt["evidence"][10]["ready"], json!(true));
    assert_eq!(
        receipt["evidence"][10]["digest"].as_str().unwrap().len(),
        64
    );

    let verified = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": receipt.clone(), "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(verified["verified"], json!(true));
    let mut tampered = receipt.clone();
    tampered["evidence"][10]["digest"] = json!("0".repeat(64));
    let rejected = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": tampered, "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(rejected["verified"], json!(false));
    assert!(rejected["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "evidence_mismatch"));
}

#[test]
fn developer_delivery_can_gate_ci_evidence_only_when_explicitly_requested() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery contracts",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "delivery-ci", "owner": "agent-a", "goal": "delivery evidence", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let evidence = json!({
        "run_id": "delivery-run",
        "provider": "github_actions",
        "source": "provider_observed",
        "plan_digest": planned["ci"]["digest"].clone(),
        "conclusion": "success",
        "checks": [{"name": "tests", "status": "passed", "result_digest": "a".repeat(64)}]
    });
    let payload = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_evidence": {"ci": ci.clone(), "evidence": evidence},
            "release_request": {"id": "delivery-ci-1", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(
        payload["readiness"]["ci_execution_evidence_ready"],
        json!(true)
    );
    assert_eq!(payload["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(
        payload["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(payload["release_request"]["ready"], json!(true));
    assert_eq!(
        payload["release_request"]["targets"][0]["eligible"],
        json!(true)
    );

    let missing = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {"id": "delivery-ci-2", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(
        missing["readiness"]["ci_execution_evidence_ready"],
        json!(false)
    );
    assert_eq!(missing["release_request"]["ready"], json!(false));
    assert_eq!(
        missing["release_request"]["targets"][0]["blockers"],
        json!(["ci_evidence_arguments_missing"])
    );
}

#[test]
fn developer_delivery_receipt_canonicalizes_explicit_targets_and_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "receipt contracts",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "receipt-ci", "owner": "agent-a", "goal": "receipt evidence", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let evidence = json!({
        "run_id": "receipt-run",
        "provider": "github_actions",
        "source": "provider_observed",
        "plan_digest": planned["ci"]["digest"].clone(),
        "conclusion": "success",
        "checks": [{"name": "tests", "status": "passed", "result_digest": "b".repeat(64)}]
    });
    let receipt = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-ci-1",
            "delivery": {
                "ci_evidence": {"ci": ci, "evidence": evidence},
                "release_request": {"id": "receipt-delivery-1", "targets": ["ci_execution_evidence"]}
            }
        }),
    );
    assert_eq!(receipt["workflow"], json!("developer_delivery_receipt"));
    assert_eq!(receipt["valid"], json!(true));
    assert_eq!(receipt["receipt_ready"], json!(true));
    assert_eq!(receipt["target_count"], json!(1));
    assert_eq!(receipt["ready_target_count"], json!(1));
    assert_eq!(receipt["evidence"][8]["name"], json!("ci_evidence"));
    assert_eq!(receipt["evidence"][8]["ready"], json!(true));
    assert_eq!(
        receipt["delivery"]["workflow"],
        json!("developer_delivery_audit")
    );
    assert_eq!(receipt["receipt_digest"].as_str().unwrap().len(), 64);

    let verified = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": receipt.clone(), "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(
        verified["workflow"],
        json!("developer_delivery_receipt_verify")
    );
    assert_eq!(verified["verified"], json!(true));
    assert_eq!(verified["receipt_digest_match"], json!(true));

    let mut tampered = receipt.clone();
    tampered["targets"][0]["ready"] = json!(false);
    let rejected = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": tampered, "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(rejected["verified"], json!(false));
    assert!(rejected["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "targets_mismatch"));

    let blocked = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-ci-2",
            "delivery": {"release_request": {"id": "receipt-delivery-2", "targets": ["ci_execution_evidence"]}}
        }),
    );
    assert_eq!(blocked["valid"], json!(true));
    assert_eq!(blocked["receipt_ready"], json!(false));
    assert_eq!(blocked["blocked_target_count"], json!(1));
}

#[test]
fn execution_provenance_reconciles_mission_trace_and_delegated_checks() {
    let mut server = server();
    let mission = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-provenance-1",
            "goal": "produce a bounded provenance trace",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
        }),
    );
    assert_eq!(mission["mission_status"], json!("succeeded"));
    let provenance = call(
        &mut server,
        "execution_provenance_audit",
        json!({
            "mission": mission.clone(),
            "delegated_checks": [{
                "name": "mission_trace_shape",
                "kind": "structural",
                "required": true,
                "status": "passed",
                "result_digest": "a".repeat(64),
                "source": "caller_attested"
            }]
        }),
    );
    assert_eq!(provenance["workflow"], json!("execution_provenance_audit"));
    assert_eq!(provenance["valid"], json!(true));
    assert_eq!(provenance["provenance_ready"], json!(true));
    assert_eq!(provenance["delegated_check_count"], json!(1));

    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "execution_provenance": {
                "mission": mission.clone(),
                "delegated_checks": [{
                    "name": "mission_trace_shape",
                    "kind": "structural",
                    "required": true,
                    "status": "passed",
                    "result_digest": "a".repeat(64),
                    "source": "caller_attested"
                }]
            },
            "release_request": {"id": "delivery-provenance-1", "targets": ["execution_provenance"]}
        }),
    );
    assert_eq!(
        delivery["readiness"]["execution_provenance_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["execution_provenance"]["provenance_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(delivery["release_request"]["ready"], json!(true));
    assert_eq!(
        delivery["release_request"]["targets"][0]["eligible"],
        json!(true)
    );

    let missing_provenance = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {"id": "delivery-provenance-2", "targets": ["execution_provenance"]}
        }),
    );
    assert_eq!(
        missing_provenance["readiness"]["execution_provenance_ready"],
        json!(false)
    );
    assert_eq!(missing_provenance["release_request"]["ready"], json!(false));
    assert_eq!(
        missing_provenance["release_request"]["targets"][0]["blockers"],
        json!(["execution_provenance_arguments_missing"])
    );

    let mut tampered = mission;
    tampered["execution_trace"][1]["sequence"] = json!(99);
    let rejected = call(
        &mut server,
        "execution_provenance_audit",
        json!({"mission": tampered}),
    );
    assert_eq!(rejected["valid"], json!(false));
    assert!(rejected["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "trace_identity_error"));
}

#[test]
fn agent_mission_plans_and_executes_allow_listed_cross_domain_steps() {
    let mut server = server();
    let planned = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-plan-1",
            "goal": "prepare a cross-domain evidence review",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "metrics", "domain": "metrics", "capability": "analytics", "objective": "prepare measurements", "tool": "metrics_analytics_audit", "arguments": {"observations": []}, "depends_on": ["catalog"]}
            ]
        }),
    );
    assert_eq!(planned["__isError"], json!(false));
    assert_eq!(planned["workflow"], json!("agent_mission"));
    assert_eq!(planned["execution"], json!("planned"));
    assert_eq!(planned["plan"]["critical_path_length"], json!(2));
    assert_eq!(
        planned["plan"]["ordered_steps"],
        json!(["catalog", "metrics"])
    );
    assert_eq!(planned["results"].as_array().unwrap().len(), 0);
    assert_eq!(planned["plan"]["digest"].as_str().unwrap().len(), 64);
    assert_eq!(planned["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        planned["artifact_registry"]["kind"],
        json!("mission_report")
    );
    assert_eq!(
        planned["artifact_registry"]["subject_id"],
        json!("mission-plan-1")
    );

    let executed = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-execute-1",
            "goal": "execute safe local discovery",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "protocol", "domain": "orchestration", "capability": "acts", "objective": "inspect protocol", "tool": "weave_protocol_catalog", "arguments": {"context": null}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "", "target_pointer": "/context"}]}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities", "weave_protocol_catalog"], "max_total_output_bytes": 2000000}
        }),
    );
    assert_eq!(executed["__isError"], json!(false));
    assert_eq!(executed["execution"], json!("executed"));
    assert_eq!(executed["mission_status"], json!("succeeded"));
    assert_eq!(executed["succeeded"], json!(2));
    assert_eq!(executed["refused"], json!(0));
    assert_eq!(executed["blocked"], json!(0));
    assert_eq!(executed["results"][0]["status"], json!("succeeded"));
    assert!(executed["results"][0]["wire"]["result"].is_object());
    assert_eq!(
        executed["execution_trace"][0]["event"],
        json!("mission.started")
    );
    assert_eq!(
        executed["execution_trace"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["event"],
        json!("mission.completed")
    );
    assert!(executed["execution_trace"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["event"] == "wave.completed"));
    let source_payload: Value = serde_json::from_str(
        executed["results"][0]["wire"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let expected_arguments_digest = ContentHash::of_value(&json!({"context": source_payload}))
        .unwrap()
        .to_string();
    assert_eq!(
        executed["results"][1]["arguments_digest"],
        json!(expected_arguments_digest)
    );
    assert_eq!(executed["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        executed["artifact_registry"]["content_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn agent_mission_schema_preflight_refuses_materialized_binding_before_dispatch() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-schema-serial",
            "goal": "prove authoritative argument validation",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "compile", "domain": "fiber", "capability": "compile", "objective": "must be refused before dispatch", "tool": "fiber_compile", "arguments": {"world": "fixture.json", "query": "fixture.query.json"}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "", "target_pointer": "/query"}]}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities", "fiber_compile"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["succeeded"], json!(1));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["results"][1]["status"], json!("refused"));
    assert!(result["results"][1]["error"]
        .as_str()
        .unwrap()
        .contains("authoritative schema validation refused"));
    assert!(result["results"][1]["error"]
        .as_str()
        .unwrap()
        .contains("schema_digest="));
    assert_eq!(
        result["execution_trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["event"] == "step.started" && event["step_id"] == "compile")
            .count(),
        0
    );
}

#[test]
fn agent_mission_parallel_schema_preflight_refuses_before_batch_launch() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-schema-parallel",
            "goal": "prove parallel schema refusal",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "compile", "domain": "fiber", "capability": "compile", "objective": "must be refused before dispatch", "tool": "fiber_compile", "arguments": {"world": "fixture.json", "query": "fixture.query.json"}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "", "target_pointer": "/query"}]}
            ],
            "policy": {"execute": true, "execution_mode": "parallel_waves", "max_parallelism": 2, "allowed_tools": ["workspace_capabilities", "fiber_compile"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["succeeded"], json!(1));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["results"][1]["status"], json!("refused"));
    assert!(result["results"][1]["error"]
        .as_str()
        .unwrap()
        .contains("authoritative schema validation refused"));
    assert_eq!(
        result["execution_trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["event"] == "step.started" && event["step_id"] == "compile")
            .count(),
        0
    );
}

#[test]
fn agent_mission_cancellation_preserves_a_closed_trace_and_unlaunched_steps() {
    let mut server = server();
    ready(&mut server);
    let cancellation = AtomicBool::new(true);
    let report = server
        .execute_agent_mission_with_cancellation(
            &json!({
                "mission_id": "mission-cancelled-1",
                "goal": "cancel before dispatch",
                "steps": [
                    {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
                ],
                "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
            }),
            &cancellation,
        )
        .expect("cancelled mission should still return a report");
    assert_eq!(report["mission_status"], json!("cancelled"));
    assert_eq!(report["cancelled"], json!(1));
    assert_eq!(report["results"][0]["status"], json!("cancelled"));
    assert_eq!(
        report["execution_trace"][0]["event"],
        json!("mission.started")
    );
    assert_eq!(
        report["execution_trace"][1]["event"],
        json!("step.cancelled")
    );
    assert_eq!(
        report["execution_trace"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["event"],
        json!("mission.completed")
    );
}

#[test]
fn agent_mission_executes_independent_parallel_waves_with_deterministic_reporting() {
    let mut server = server();
    let executed = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-parallel-1",
            "goal": "run independent discovery and protocol inspections",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "protocol", "domain": "orchestration", "capability": "catalogue", "objective": "inspect protocol", "tool": "weave_protocol_catalog", "arguments": {"context": null}},
                {"id": "protocol-extra", "domain": "orchestration", "capability": "catalogue", "objective": "inspect protocol again", "tool": "weave_protocol_catalog", "arguments": {"context": null}}
            ],
            "policy": {
                "execute": true,
                "execution_mode": "parallel_waves",
                "max_parallelism": 2,
                "allowed_tools": ["workspace_capabilities", "weave_protocol_catalog"],
                "max_step_output_bytes": 3000000,
                "max_total_output_bytes": 10000000
            }
        }),
    );
    assert_eq!(executed["__isError"], json!(false));
    assert_eq!(executed["execution"], json!("executed"));
    assert_eq!(executed["plan"]["execution_mode"], json!("parallel_waves"));
    assert_eq!(executed["plan"]["max_parallelism"], json!(2));
    assert_eq!(executed["plan"]["waves"].as_array().unwrap().len(), 1);
    assert_eq!(
        executed["plan"]["waves"][0],
        json!(["catalog", "protocol", "protocol-extra"])
    );
    assert_eq!(executed["mission_status"], json!("succeeded"));
    assert_eq!(executed["succeeded"], json!(3));
    assert_eq!(executed["refused"], json!(0));
    assert_eq!(executed["blocked"], json!(0));
    assert_eq!(executed["results"].as_array().unwrap().len(), 3);
    assert_eq!(executed["results"][0]["id"], json!("catalog"));
    assert_eq!(executed["results"][0]["status"], json!("succeeded"));
    assert_eq!(executed["results"][1]["id"], json!("protocol"));
    assert_eq!(executed["results"][1]["status"], json!("succeeded"));
    assert_eq!(executed["results"][2]["id"], json!("protocol-extra"));
    assert_eq!(executed["results"][2]["status"], json!("succeeded"));
    let trace = executed["execution_trace"].as_array().unwrap();
    assert_eq!(trace.len(), 10);
    for (sequence, event) in trace.iter().enumerate() {
        assert_eq!(event["sequence"], json!(sequence));
    }
    assert_eq!(trace[0]["event"], json!("mission.started"));
    assert_eq!(trace[1]["event"], json!("wave.started"));
    assert_eq!(trace[trace.len() - 2]["event"], json!("wave.completed"));
    assert_eq!(trace[trace.len() - 1]["event"], json!("mission.completed"));
    assert_eq!(
        trace
            .iter()
            .filter(|event| event["event"] == "step.started")
            .count(),
        3
    );
    assert_eq!(
        trace
            .iter()
            .filter(|event| event["event"] == "step.completed")
            .count(),
        3
    );
    assert_eq!(trace[trace.len() - 1]["bytes"], executed["returned_bytes"]);
}

#[test]
fn agent_mission_parallel_waves_preserve_refusals_and_block_dependents() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-parallel-refusal",
            "goal": "prove parallel refusal propagation",
            "steps": [
                {"id": "bad", "domain": "test", "capability": "refusal", "objective": "invoke an unknown tool", "tool": "not_a_real_tool"},
                {"id": "dependent", "domain": "test", "capability": "blocked", "objective": "must not run", "tool": "workspace_capabilities", "depends_on": ["bad"]}
            ],
            "policy": {
                "execute": true,
                "execution_mode": "parallel_waves",
                "allowed_tools": ["not_a_real_tool", "workspace_capabilities"]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["plan"]["execution_mode"], json!("parallel_waves"));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["blocked"], json!(1));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["results"][0]["status"], json!("refused"));
    assert_eq!(result["results"][1]["status"], json!("blocked"));
    let trace = result["execution_trace"].as_array().unwrap();
    assert!(trace.iter().any(|event| event["event"] == "step.refused"));
    assert!(trace.iter().any(|event| event["event"] == "step.blocked"));
    assert_eq!(trace.last().unwrap()["event"], json!("mission.completed"));
    assert_eq!(trace.last().unwrap()["status"], json!("failed"));
}

#[test]
fn capability_discovery_routes_across_domains_and_attaches_authoritative_schemas() {
    let mut server = server();
    let result = call(
        &mut server,
        "capability_discover",
        json!({
            "query": "oncology",
            "max_items": 2,
            "include_tools": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["workflow"], json!("capability_discover"));
    assert_eq!(result["result_count"], json!(1));
    assert_eq!(
        result["matches"][0]["group"]["id"],
        json!("biological_domains")
    );
    assert!(result["matches"][0]["matched_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "onco_response_assess"));
    assert!(result["matches"][0]["tool_schemas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|schema| schema["name"] == "onco_response_assess"));
    assert_eq!(result["schema_attachment"]["requested"], json!(true));
    assert_eq!(result["catalog_digest"].as_str().unwrap().len(), 64);

    let filtered = call(
        &mut server,
        "capability_discover",
        json!({"domain": "release", "tool": "bundle_verify"}),
    );
    assert_eq!(filtered["__isError"], json!(false));
    assert_eq!(filtered["result_count"], json!(1));
    assert_eq!(
        filtered["matches"][0]["matched_tools"],
        json!(["bundle_verify"])
    );
}

#[test]
fn adapter_plan_routes_biological_formats_without_sniffing_or_execution() {
    let mut server = server();
    let unknown = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "scan-1",
            "source_kind": "bytes",
            "declared_format": "application/dicom"
        }),
    );
    assert_eq!(unknown["workflow"], json!("adapter_plan"));
    assert_eq!(unknown["executable"], json!(false));
    assert_eq!(unknown["execution"], json!("not_started"));
    assert_eq!(unknown["plan_id"].as_str().unwrap().len(), 64);
    assert_eq!(
        unknown["plan"]["candidates"][0]["status"],
        json!("dependency_unknown")
    );
    assert_eq!(unknown["selected_adapter"], Value::Null);

    let ready = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "scan-1",
            "source_kind": "bytes",
            "declared_format": "application/dicom",
            "available_dependencies": ["pydicom"]
        }),
    );
    assert_eq!(ready["executable"], json!(true));
    assert_eq!(
        ready["selected_adapter"]["id"],
        json!("bioprism.python.dicom")
    );
    assert_eq!(
        ready["selected_adapter"]["execution"],
        json!("python_delegated")
    );

    let refused = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "opaque",
            "source_kind": "bytes",
            "declared_format": "application/octet-stream"
        }),
    );
    assert_eq!(refused["executable"], json!(false));
    assert_eq!(refused["selected_adapter"], Value::Null);
    assert!(refused["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "format matching is explicit and content sniffing is refused"));
}

#[test]
fn adapter_execution_evidence_binds_declared_adapter_scope_and_loss_posture() {
    let mut server = server();
    let evidence = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "output_digest": "b".repeat(64),
            "execution_status": "succeeded",
            "conformance_status": "verified",
            "semantic_loss_status": "lossless",
            "item_count": 4,
            "byte_length": 128,
            "parent_digests": ["c".repeat(64)]
        }),
    );
    assert_eq!(evidence["ok"], json!(true));
    assert_eq!(
        evidence["evidence"]["adapter_id"],
        json!("bioprism.python.vcf_text")
    );
    assert_eq!(
        evidence["evidence"]["attestation_posture"],
        json!("caller_asserted")
    );
    assert_eq!(evidence["adapter"]["execution"], json!("python_delegated"));
    assert_eq!(evidence["artifact_registry"]["indexed"], json!(true));
    assert_eq!(evidence["execution"], json!("not_started"));
    assert_eq!(evidence["readiness_claimed"], json!(false));

    let queried = call(
        &mut server,
        "adapter_execution_evidence_query",
        json!({"subject_id": "adapter-subject-1", "include_artifacts": true}),
    );
    assert_eq!(queried["ok"], json!(true));
    assert_eq!(
        queried["workflow"],
        json!("adapter_execution_evidence_query")
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        queried["rows"][0]["join_status"],
        json!("bound_with_missing_parents")
    );
    assert_eq!(
        queried["rows"][0]["evidence_artifact"]["evidence_digest"],
        evidence["evidence_digest"]
    );
    assert_eq!(queried["page_summary"]["page_row_count"], json!(1));
    assert_eq!(
        queried["page_summary"]["rows_with_missing_parents"],
        json!(1)
    );
    assert_eq!(queried["readiness_claimed"], json!(false));

    let inconsistent = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "execution_status": "refused",
            "conformance_status": "refused",
            "semantic_loss_status": "lossless"
        }),
    );
    assert_eq!(inconsistent["__isError"], json!(true));

    let out_of_scope = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["not-a-declared-domain"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "execution_status": "unknown",
            "conformance_status": "unknown",
            "semantic_loss_status": "unknown"
        }),
    );
    assert_eq!(out_of_scope["__isError"], json!(true));
}

#[test]
fn domain_acquisition_catalogue_covers_every_declared_domain_in_two_planes() {
    let mut server = server();
    let full = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"include_adapters": true}),
    );
    assert_eq!(full["ok"], json!(true));
    assert_eq!(full["workflow"], json!("domain_acquisition_catalogue"));
    let catalogue = &full["catalogue"];
    assert_eq!(catalogue["total_group_count"], json!(29));
    assert_eq!(catalogue["selected_group_count"], json!(29));
    assert_eq!(catalogue["complete"], json!(true));
    assert_eq!(catalogue["truncated"], json!(false));
    assert_eq!(
        catalogue["selected_domain_count"],
        catalogue["total_domain_count"]
    );
    assert_eq!(catalogue["groups"].as_array().unwrap().len(), 29);
    assert_eq!(
        catalogue["routes"].as_array().unwrap().len(),
        catalogue["total_domain_count"].as_u64().unwrap() as usize
    );
    assert!(catalogue["routes"].as_array().unwrap().iter().all(|route| {
        route["transport"]["status"] == "bounded_file_http"
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_normalize")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_replay_verify")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_connector_handoff")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_receipt")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_replay_verify")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_normalize")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_lineage_audit")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_execution_evidence")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_evidence_query")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "adapter_execution_evidence")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "adapter_execution_evidence_query")
            && route["interpretation"]["status"].is_string()
            && route["limitations"].as_array().is_some()
    }));
    assert!(catalogue["routes"].as_array().unwrap().iter().any(|route| {
        route["adapters"]
            .as_array()
            .is_some_and(|adapters| !adapters.is_empty())
    }));
    assert_eq!(catalogue["digest"].as_str().unwrap().len(), 64);

    let filtered = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"max_domains": 2}),
    );
    assert_eq!(filtered["ok"], json!(true));
    assert_eq!(filtered["catalogue"]["truncated"], json!(true));
    assert_eq!(filtered["catalogue"]["routes"].as_array().unwrap().len(), 2);

    let refused = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"max_domains": 0}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("max_domains"));
}

#[test]
fn capability_audit_proves_catalogue_and_transport_schema_parity() {
    let mut server = server();
    let result = call(&mut server, "capability_audit", json!({}));
    assert_eq!(result["workflow"], json!("capability_audit"));
    assert_eq!(result["healthy"], json!(true));
    assert_eq!(result["total_groups"], json!(29));
    assert_eq!(result["unique_catalog_tools"], json!(212));
    assert_eq!(result["advertised_tool_count"], json!(212));
    assert_eq!(result["catalog_only_tools"], json!([]));
    assert_eq!(result["advertised_only_tools"], json!([]));
    assert_eq!(result["schema_quality"]["checked"], json!(212));
    assert_eq!(result["schema_quality"]["valid"], json!(212));
    assert_eq!(result["schema_quality"]["findings"], json!([]));
    assert!(!result["duplicate_group_memberships"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        result["invariants"]["multi_group_membership_is_allowed"],
        json!(true)
    );
    assert_eq!(
        result["invariants"]["all_input_schemas_are_well_formed"],
        json!(true)
    );
    assert_eq!(result["groups"].as_array().unwrap().len(), 29);

    let compact = call(
        &mut server,
        "capability_audit",
        json!({"include_groups": false}),
    );
    assert!(compact.get("groups").is_none());
    assert_eq!(compact["healthy"], json!(true));

    let refused = call(
        &mut server,
        "capability_audit",
        json!({"include_groups": "yes"}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn capability_dashboard_separates_domain_surfaces_and_bounded_inventory() {
    let mut server = server();
    let oncology = call(
        &mut server,
        "capability_dashboard",
        json!({"domain": "oncology", "include_tools": true, "include_gaps": true}),
    );
    assert_eq!(oncology["workflow"], json!("capability_dashboard"));
    assert_eq!(
        oncology["schema"],
        json!("bioprism-devplat-capability-dashboard/0.1")
    );
    assert_eq!(oncology["audit"]["selected_group_count"], json!(1));
    assert_eq!(
        oncology["audit"]["groups"][0]["id"],
        json!("biological_domains")
    );
    assert_eq!(
        oncology["audit"]["groups"][0]["readiness"],
        json!("callable")
    );
    assert!(oncology["audit"]["groups"][0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "onco_response_assess"));
    assert_eq!(oncology["capability_dashboard_ready"], json!(true));
    assert_eq!(oncology["catalog_digest"].as_str().unwrap().len(), 64);
    assert_eq!(oncology["dashboard_digest"].as_str().unwrap().len(), 64);

    let bounded = call(
        &mut server,
        "capability_dashboard",
        json!({"max_groups": 1, "include_tools": false}),
    );
    assert_eq!(bounded["audit"]["selected_group_count"], json!(1));
    assert!(bounded["audit"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("bounded")));
    assert!(bounded["audit"]["groups"][0].get("tools").is_none());

    let refused = call(
        &mut server,
        "capability_dashboard",
        json!({"max_groups": 0}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn capability_route_batches_ranked_and_explicit_needs_without_execution() {
    let mut server = server();
    let result = call(
        &mut server,
        "capability_route",
        json!({
            "goal": "compose a cross-domain evidence route",
            "needs": [
                {"id": "oncology", "query": "oncology"},
                {"id": "release", "tool": "bundle_verify"}
            ],
            "max_candidates_per_need": 2,
            "max_tools": 4,
            "include_tools": true
        }),
    );
    assert_eq!(result["workflow"], json!("capability_route"));
    assert_eq!(result["execution"], json!("not_started"));
    assert_eq!(result["unresolved_needs"], json!([]));
    assert_eq!(result["needs"][0]["resolution"], json!("ranked_candidates"));
    assert_eq!(result["needs"][1]["resolution"], json!("explicit"));
    assert!(result["recommended_tools"]
        .as_array()
        .unwrap()
        .contains(&json!("bundle_verify")));
    assert_eq!(result["recommended_tools"].as_array().unwrap().len(), 4);
    assert_eq!(result["schema_attachment"]["requested"], json!(true));
    assert_eq!(result["schema_attachment"]["returned"], json!(4));
    assert_eq!(result["route_coverage"]["needs_total"], json!(2));
    assert_eq!(result["route_coverage"]["needs_resolved"], json!(2));
    assert_eq!(result["route_coverage"]["needs_unresolved"], json!(0));
    assert!(
        result["route_coverage"]["candidate_domain_count"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert!(result["needs"][0]["candidate_domains"]
        .as_array()
        .unwrap()
        .iter()
        .any(|domain| domain == "oncology"));
    assert_eq!(result["route_id"].as_str().unwrap().len(), 64);

    let refused = call(
        &mut server,
        "capability_route",
        json!({"goal": "bad", "needs": [{"id": "nested", "include_tools": true}]}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn capability_route_review_builds_non_executing_handoff_and_reports_bad_selection() {
    let mut server = server();
    let route = call(
        &mut server,
        "capability_route",
        json!({
            "goal": "compose a reviewed handoff",
            "needs": [
                {"id": "oncology", "query": "oncology"},
                {"id": "release", "tool": "bundle_verify"}
            ],
            "max_candidates_per_need": 2,
            "max_tools": 4
        }),
    );
    let oncology_tool = route["needs"][0]["candidate_tools"][0]
        .as_str()
        .unwrap()
        .to_string();
    let review = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "selections": [
                {
                    "need_id": "oncology",
                    "tool": oncology_tool,
                    "domain": "oncology",
                    "capability": "evidence",
                    "objective": "review oncology evidence",
                    "arguments": {}
                },
                {
                    "need_id": "release",
                    "tool": "bundle_verify",
                    "domain": "release",
                    "capability": "verification",
                    "objective": "verify the release bundle",
                    "arguments": {},
                    "depends_on": ["oncology"]
                }
            ]
        }),
    );
    assert_eq!(review["workflow"], json!("capability_route_review"));
    assert_eq!(review["review_id"].as_str().unwrap().len(), 64);
    assert_eq!(review["review_status"], json!("ready"));
    assert_eq!(
        review["handoff_status"],
        json!("mission_preflight_required")
    );
    assert_eq!(review["execution"], json!("not_started"));
    assert_eq!(
        review["dependency_waves"],
        json!([["oncology"], ["release"]])
    );
    assert_eq!(
        review["mission_draft"]["steps"].as_array().unwrap().len(),
        2
    );

    let blocked = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": review["route_coverage"].clone(),
            "selections": []
        }),
    );
    assert_eq!(blocked["__isError"], json!(true));

    let route = call(
        &mut server,
        "capability_route",
        json!({"goal": "review one", "needs": [{"id": "release", "tool": "bundle_verify"}]}),
    );
    let blocked = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "selections": [{
                "need_id": "release",
                "tool": "not_a_candidate",
                "domain": "release",
                "capability": "verification",
                "objective": "bad selection",
                "arguments": {}
            }]
        }),
    );
    assert_eq!(blocked["review_status"], json!("blocked"));
    assert!(blocked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "candidate_mismatch"));

    let route = call(
        &mut server,
        "capability_route",
        json!({"goal": "validate schemas", "needs": [{"id": "catalog", "tool": "workspace_capabilities"}]}),
    );
    let schema_review = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "validate_schemas": true,
            "selections": [{
                "need_id": "catalog",
                "tool": "workspace_capabilities",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "validate the catalogue schema",
                "arguments": {}
            }]
        }),
    );
    assert_eq!(schema_review["review_status"], json!("ready"));
    assert_eq!(schema_review["schema_review"]["requested"], json!(true));
    assert_eq!(schema_review["schema_review"]["valid"], json!(true));
    assert_eq!(schema_review["schema_review"]["checked"], json!(1));
}

#[test]
fn agent_mission_preserves_refusal_and_blocks_dependents() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-refusal-1",
            "goal": "prove refusal propagation",
            "steps": [
                {"id": "bad", "domain": "test", "capability": "refusal", "objective": "invoke an unknown tool", "tool": "not_a_real_tool"},
                {"id": "dependent", "domain": "test", "capability": "blocked", "objective": "must not run", "tool": "workspace_capabilities", "depends_on": ["bad"]}
            ],
            "policy": {"execute": true, "allowed_tools": ["not_a_real_tool", "workspace_capabilities"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["blocked"], json!(1));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["results"][0]["status"], json!("refused"));
    assert_eq!(result["results"][1]["status"], json!("blocked"));
    assert!(result["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

#[test]
fn new_surfaces_fail_closed_on_duplicate_facts_unknown_schemas_and_unbounded_requests() {
    let mut server = server();
    let request = json!({
        "principal": {
            "id": "agent-1",
            "role": "researcher",
            "clearance": { "max_classification": "public_aggregate", "compartments": [] },
            "site": "us",
            "authorities": []
        },
        "purpose": "research_analysis",
        "channel": "local_compute",
        "at": "2026-08-14T00:00:00Z"
    });
    let duplicate = call(
        &mut server,
        "policy_screen",
        json!({
            "world": WORLD,
            "request": request,
            "facts": ["fact.cohort", "fact.cohort"]
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(duplicate["error"].as_str().unwrap().contains("duplicate"));

    let unknown_schema = call(
        &mut server,
        "governance_schema_check",
        json!({ "schema": "invented/9.9" }),
    );
    assert_eq!(unknown_schema["__isError"], json!(true));
    assert!(unknown_schema["error"]
        .as_str()
        .unwrap()
        .contains("unknown schema"));

    let unbounded = call(
        &mut server,
        "developer_platform_status",
        json!({ "max_items": 0 }),
    );
    assert_eq!(unbounded["__isError"], json!(true));
    assert!(unbounded["error"].as_str().unwrap().contains("max_items"));

    let rank_too_small = call(
        &mut server,
        "capability_rank",
        json!({ "vectors": [metric_vector("only", 0.5, 0.5, "pack/4")] }),
    );
    assert_eq!(rank_too_small["__isError"], json!(true));
    assert!(rank_too_small["error"]
        .as_str()
        .unwrap()
        .contains("between 2 and 100"));

    let conflicting_ci_inputs = call(
        &mut server,
        "research_ci_check",
        json!({ "document": "missing.json", "result": { "subject": "inline" } }),
    );
    assert_eq!(conflicting_ci_inputs["__isError"], json!(true));
    assert!(conflicting_ci_inputs["error"]
        .as_str()
        .unwrap()
        .contains("either document or inline"));

    let hub_unbounded = call(&mut server, "hub_lock", json!({ "max_items": 0 }));
    assert_eq!(hub_unbounded["__isError"], json!(true));
    assert!(hub_unbounded["error"]
        .as_str()
        .unwrap()
        .contains("max_items"));

    let tabular_conflict = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "source.csv",
            "csv": "subject\nS1\n",
            "document": WORLD,
            "profile": serde_json::to_value(TabularProfile::new("dataset")).unwrap()
        }),
    );
    assert_eq!(tabular_conflict["__isError"], json!(true));
    assert!(tabular_conflict["error"]
        .as_str()
        .unwrap()
        .contains("either csv or document"));

    let adaptive_unbounded = call(
        &mut server,
        "adaptive_panel",
        json!({ "panel": serde_json::to_value(AdaptivePanel::new(PanelConfig::default())).unwrap(), "max_items": 0 }),
    );
    assert_eq!(adaptive_unbounded["__isError"], json!(true));
    assert!(adaptive_unbounded["error"]
        .as_str()
        .unwrap()
        .contains("max_items"));

    let oracle_empty = call(
        &mut server,
        "oracle_combine",
        json!({ "subject": "artifact-1", "at": "2026-08-14T00:00:00Z", "judgements": [] }),
    );
    assert_eq!(oracle_empty["__isError"], json!(true));
    assert!(oracle_empty["error"]
        .as_str()
        .unwrap()
        .contains("between 1 and 1000"));

    let bundle_conflict = call(
        &mut server,
        "bundle_verify",
        json!({ "bundle": {}, "document": "missing.json" }),
    );
    assert_eq!(bundle_conflict["__isError"], json!(true));
    assert!(bundle_conflict["error"]
        .as_str()
        .unwrap()
        .contains("either bundle or document"));

    let capacity_conflict = call(
        &mut server,
        "ops_capacity",
        json!({ "model": {}, "workload": {}, "degradation_plan": {} }),
    );
    assert_eq!(capacity_conflict["__isError"], json!(true));
    assert!(capacity_conflict["error"]
        .as_str()
        .unwrap()
        .contains("invalid capacity model"));

    let duplicate_labels = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "duplicate-labels",
            "sources": [],
            "design": serde_json::to_value(StudyDesign::new(
                0,
                Selection::Undeclared
            )).unwrap(),
            "outcome_labels": ["same", "same"]
        }),
    );
    assert_eq!(duplicate_labels["__isError"], json!(true));
    assert!(duplicate_labels["error"]
        .as_str()
        .unwrap()
        .contains("duplicate outcome"));

    let influence_duplicate_group = call(
        &mut server,
        "influence_analyze",
        json!({
            "label": "bounded-input",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"], "table": [1.0, 2.0] }],
            "free": ["a"],
            "factor_group": ["f.a", "f.a"],
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(influence_duplicate_group["__isError"], json!(true));
    assert!(influence_duplicate_group["error"]
        .as_str()
        .unwrap()
        .contains("duplicate factor"));

    let influence_foreign_assumption = call(
        &mut server,
        "influence_analyze",
        json!({
            "label": "bounded-input",
            "variables": { "a": 2 },
            "assumed_variables": ["missing"],
            "factors": [{ "id": "f.a", "scope": ["a"] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(influence_foreign_assumption["__isError"], json!(true));
    assert!(influence_foreign_assumption["error"]
        .as_str()
        .unwrap()
        .contains("undeclared"));
}

#[test]
fn weave_protocol_catalog_exposes_typed_antecedents() {
    let mut server = server();
    let payload = call(&mut server, "weave_protocol_catalog", json!({}));
    assert_eq!(payload["ok"], json!(true));
    let acts = payload["acts"].as_array().unwrap();
    let accept = acts
        .iter()
        .find(|act| act["kind"] == json!("accept"))
        .expect("accept act");
    assert_eq!(accept["requires_antecedent"], json!(["propose"]));
}

#[test]
fn repository_bundle_fails_instead_of_truncating_oversized_markdown() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_bundle",
        json!({
            "route": {
                "id": "orientation-too-small",
                "intent": "understand the repository",
                "must_read": ["README.md"]
            },
            "include_markdown": true,
            "max_markdown_chars": 1
        }),
    );
    assert_eq!(payload["__isError"], json!(true));
    assert!(payload["error"]
        .as_str()
        .unwrap()
        .contains("max_markdown_chars"));
}

#[test]
fn repository_catalog_is_bounded_and_reports_graph_health() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_catalog",
        json!({
            "prefix": "docs/",
            "limit": 3,
            "include_briefs": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["returned_modules"], json!(3));
    assert!(payload["matching_modules"].as_u64().unwrap() >= 3);
    assert_eq!(payload["truncated"], json!(true));
    assert!(payload["module_count"].as_u64().unwrap() > 0);
    assert!(payload["edge_count"].as_u64().unwrap() > 0);
    assert!(payload["modules"][0]["brief"].is_string());
    assert!(payload["lint"]["counts"].is_object());
}

#[test]
fn repository_bundle_compiles_a_route_with_progressive_disclosure() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_bundle",
        json!({
            "route": {
                "id": "orientation",
                "intent": "understand the repository before choosing a domain",
                "must_read": ["README.md"],
                "budget": 30000
            },
            "policy": "normative",
            "include_markdown": true,
            "max_markdown_chars": 120000
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["bundle"]["route"], json!("orientation"));
    assert!(!payload["bundle"]["entries"].as_array().unwrap().is_empty());
    assert!(payload["bundle"]["traversal"].is_object());
    assert!(payload["markdown"]
        .as_str()
        .unwrap()
        .contains("context bundle"));
}

/// The disclosure contract: L0 carries the decision and the omissions, never the evidence.
#[test]
fn compile_returns_the_contract_not_the_evidence() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({ "world": WORLD, "query": QUERY }),
    );

    assert_eq!(payload["layer"], json!("l0"));
    assert_eq!(payload["verdict"]["status"], json!("invalid"));
    assert_eq!(
        payload["certificate_sha256"],
        json!("c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4")
    );

    assert!(
        payload.get("evidence").is_none(),
        "L0 must not carry values"
    );
    assert!(payload.get("evidence_inventory").is_none());
    assert!(payload.get("factors").is_none());

    assert_eq!(payload["omissions"]["omitted_facts"], json!(750));
    assert_eq!(
        payload["omissions"]["supports_sufficiency_claim"],
        json!(true)
    );
    assert_eq!(payload["refine"]["next_layer"], json!("l1"));
    assert_eq!(payload["refine"]["handle"]["version"], json!(1));
    assert_eq!(
        payload["refine"]["handle"]["certificate_sha256"],
        payload["certificate_sha256"]
    );
}

/// The 0.3 decision contract crosses the MCP boundary as an explicit, certificate-bound summary.
#[test]
fn compile_projects_the_wire_decision_quotient_without_claiming_rate_distortion() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.3/decision_contract_query.json"
        }),
    );

    assert_eq!(payload["layer"], json!("l0"));
    let quotient = &payload["decision_quotient"];
    assert_eq!(
        quotient["schema"],
        json!("bioprism-mcp/epistemic-decision-quotient/0.1")
    );
    assert_eq!(
        quotient["permitted_actions"],
        json!(["accept", "defer", "reject"])
    );
    assert_eq!(quotient["original_model_count"], json!(3));
    assert_eq!(quotient["quotient_model_count"], json!(2));
    assert_eq!(quotient["merged_model_count"], json!(1));
    assert_eq!(
        quotient["certificate_binding"]["query_sha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        quotient["certificate_binding"]["certificate_sha256"],
        payload["certificate_sha256"]
    );
    assert!(quotient["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("rate-distortion")));

    let explained = call(
        &mut server,
        "fiber_explain",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.3/decision_contract_query.json"
        }),
    );
    assert_eq!(
        explained["decision_quotient"]["quotient_model_count"],
        json!(2)
    );
    assert!(!explained["passes_not_run"]
        .as_array()
        .unwrap()
        .iter()
        .any(|pass| pass["name"] == "decision_quotient"));
}

#[test]
fn a_refinement_handle_is_verified_and_stale_handles_are_refused() {
    let mut server = server();
    let compiled = call(
        &mut server,
        "fiber_compile",
        json!({ "world": WORLD, "query": QUERY }),
    );
    let handle = compiled["refine"]["handle"].clone();

    let refined = call(
        &mut server,
        "fiber_refine",
        json!({ "handle": handle, "layer": "l2" }),
    );
    assert_eq!(refined["layer"], json!("l2"));

    let mut stale = compiled["refine"]["handle"].clone();
    stale["certificate_sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    let refused = call(
        &mut server,
        "fiber_refine",
        json!({ "handle": stale, "layer": "l2" }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("stale"));
}

#[test]
fn omissions_are_reported_at_every_layer() {
    let mut server = server();
    for layer in ["l0", "l1", "l2", "l3", "l4"] {
        let payload = call(
            &mut server,
            "fiber_refine",
            json!({ "world": WORLD, "query": QUERY, "layer": layer }),
        );
        assert_eq!(
            payload["omissions"]["omitted_facts"],
            json!(750),
            "{layer} hid the omission count"
        );
        assert_eq!(
            payload["omissions"]["protected_closure_satisfied"],
            json!(true)
        );
    }
}

#[test]
fn layers_are_cumulative_and_grow_monotonically() {
    let mut server = server();
    let mut previous = 0usize;
    for layer in ["l0", "l1", "l2", "l3", "l4"] {
        let payload = call(
            &mut server,
            "fiber_refine",
            json!({ "world": WORLD, "query": QUERY, "layer": layer }),
        );
        let size = serde_json::to_string(&payload).unwrap().len();
        assert!(
            size > previous,
            "{layer} was not larger than the layer before it"
        );
        previous = size;
    }

    let l1 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l1" }),
    );
    assert!(l1["evidence_inventory"].is_array());
    assert!(l1.get("evidence").is_none(), "l1 lists names, not values");

    let l2 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l2" }),
    );
    assert_eq!(l2["evidence"].as_array().unwrap().len(), 11);
    assert_eq!(l2["witnesses"].as_array().unwrap().len(), 4);

    let l3 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l3" }),
    );
    assert_eq!(l3["factors"].as_array().unwrap().len(), 6);
}

#[test]
fn explain_reports_the_passes_that_did_not_run() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_explain",
        json!({ "world": WORLD, "query": QUERY }),
    );
    let deferred: Vec<&str> = payload["passes_not_run"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert!(deferred.contains(&"obstruction_tests"));
    assert!(deferred.contains(&"rate_distortion"));
    assert_eq!(payload["selection"]["facts"], json!(11));
}

#[test]
fn absolute_paths_and_traversal_are_refused() {
    let server = server();
    assert!(server
        .resolve("fixtures/fiber-v0.1/leakage_query.json")
        .is_ok());
    assert!(server
        .resolve("./fixtures/fiber-v0.1/leakage_query.json")
        .is_ok());

    for hostile in [
        "../../../etc/passwd",
        "fixtures/../../secrets.json",
        "/etc/passwd",
        "C:/Windows/System32/config/SAM",
        "C:\\Windows\\System32\\config\\SAM",
        "..\\outside.json",
    ] {
        assert!(
            server.resolve(hostile).is_err(),
            "{hostile} should have been refused"
        );
    }
}

#[test]
fn a_refused_path_surfaces_as_a_tool_error_not_a_crash() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({ "world": "../../../etc/passwd", "query": QUERY }),
    );
    assert_eq!(payload["__isError"], json!(true));
    assert!(payload["error"].as_str().unwrap().contains("refused"));
}

#[test]
fn writing_tools_preview_before_they_act() {
    let mut server = server();
    let store = "target/mcp-preview-store";
    let _ = std::fs::remove_dir_all(repo_root().join(store));

    let preview = call(
        &mut server,
        "world_index",
        json!({ "world": WORLD, "store": store }),
    );
    assert_eq!(preview["performed"], json!(false));
    assert!(preview["preview"]["effect"]
        .as_str()
        .unwrap()
        .contains("would write"));
    assert!(
        !repo_root().join(store).exists(),
        "preview must not create the store"
    );

    let performed = call(
        &mut server,
        "world_index",
        json!({ "world": WORLD, "store": store, "confirm": true }),
    );
    assert_eq!(performed["performed"], json!(true));
    assert_eq!(performed["facts"], json!(761));
    assert!(repo_root().join(store).join("manifest.json").exists());

    let _ = std::fs::remove_dir_all(repo_root().join(store));
}

#[test]
fn a_tampered_certificate_fails_verification() {
    let mut server = server();
    let good = call(
        &mut server,
        "fiber_verify",
        json!({ "certificate": "fixtures/fiber-v0.1/golden/reference_certificate.json" }),
    );
    assert_eq!(good["verified"], json!(true));

    let mut document: Value = serde_json::from_str(
        &std::fs::read_to_string(
            repo_root().join("fixtures/fiber-v0.1/golden/reference_certificate.json"),
        )
        .unwrap(),
    )
    .unwrap();
    document["selected_facts"]
        .as_array_mut()
        .unwrap()
        .push(json!("fact.smuggled"));
    let tampered = repo_root().join("target/mcp-tampered.json");
    std::fs::create_dir_all(tampered.parent().unwrap()).unwrap();
    std::fs::write(&tampered, serde_json::to_string_pretty(&document).unwrap()).unwrap();

    let bad = call(
        &mut server,
        "fiber_verify",
        json!({ "certificate": "target/mcp-tampered.json" }),
    );
    assert_eq!(bad["verified"], json!(false));
    assert!(bad["detail"].as_str().unwrap().contains("digest mismatch"));

    let _ = std::fs::remove_file(tampered);
}

#[test]
fn stdout_carries_only_json_rpc() {
    let mut server = server();
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "fiber_compile", "arguments": { "world": WORLD, "query": QUERY } }
        })
    );

    let mut output = Vec::new();
    serve(&mut server, input.as_bytes(), &mut output).expect("serves");
    let text = String::from_utf8(output).unwrap();

    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "the notification must not be answered");
    for line in lines {
        let parsed: Value = serde_json::from_str(line).expect("every stdout line is JSON-RPC");
        assert_eq!(parsed["jsonrpc"], json!("2.0"));
    }
}

#[test]
fn adaptive_panel_preserves_clustered_audit_and_selection_refusals() {
    let panel = AdaptivePanel::new(PanelConfig::default());
    let result = call(
        &mut server(),
        "adaptive_panel",
        json!({
            "panel": serde_json::to_value(panel).unwrap(),
            "candidates": [{
                "instance": "inst-1",
                "capability": "capability-a",
                "parent": "parent-1",
                "cost": 1.0
            }],
            "capability": "capability-a"
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/adaptive-panel/0.1"));
    assert_eq!(result["audit_summary"]["trials"], json!(0));
    assert_eq!(result["selection"]["ok"], json!(true));
    assert_eq!(
        result["selection"]["value"]["record"]["chosen"]["instance"],
        json!("inst-1")
    );
    assert_eq!(result["capability"]["estimate"], Value::Null);
    assert!(result["capability"]["estimate_refusal"]
        .as_str()
        .unwrap()
        .contains("no recorded trials"));
}

#[test]
fn ops_acceptance_keeps_unverifiable_criteria_out_of_release_passes() {
    let result = call(&mut server(), "ops_acceptance", json!({ "max_items": 20 }));

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["summary"]["total"], json!(14));
    assert_eq!(result["summary"]["met"], json!(0));
    assert_eq!(result["summary"]["refuted"], json!(2));
    assert_eq!(result["summary"]["unverifiable"], json!(12));
    assert_eq!(result["summary"]["is_release_ready"], json!(false));
    assert_eq!(result["findings"].as_array().unwrap().len(), 14);
}

#[test]
fn ops_capacity_requires_qualified_work_and_visible_saturation() {
    let work_units =
        Assumption::measured("supply", 10.0, "work/epoch", "protocol fixture").unwrap();
    let model = CapacityModel::new(work_units, 1024);
    let calls = Assumption::assumed("calls", 2.0, "calls/epoch", "protocol fixture").unwrap();
    let cost = Assumption::measured("scan-cost", 3.0, "work/step", "protocol fixture").unwrap();
    let operation = Operation::new(
        "scan",
        Bound::Bounded { steps: 2 },
        ArtifactHandling::Streamed,
        cost,
    )
    .unwrap();
    let workload = Workload::new("protocol-workload", calls)
        .unwrap()
        .with(operation);
    let plan = DegradationPlan::declare(
        "visible-backpressure",
        [Concession::Throughput],
        "queue_depth",
    )
    .unwrap();
    let result = call(
        &mut server(),
        "ops_capacity",
        json!({
            "model": serde_json::to_value(model).unwrap(),
            "workload": serde_json::to_value(workload).unwrap(),
            "demand": serde_json::to_value(Demand { calls_per_epoch: 10.0 }).unwrap(),
            "degradation_plan": serde_json::to_value(plan).unwrap()
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["fully_measured"], json!(false));
    assert_eq!(result["saturation"]["ok"], json!(true));
    assert_eq!(
        result["saturation"]["value"]["saturation"],
        json!("saturated")
    );
    assert_eq!(
        result["saturation"]["value"]["plan"]["visible_as"],
        json!("queue_depth")
    );
}

#[test]
fn bundle_verify_recomputes_carried_content_and_refuses_tampering() {
    let bundle = ResultBundle::builder("protocol-bundle")
        .carrying("query", EntryRole::Query, json!({ "goal": "verify" }))
        .unwrap()
        .build()
        .unwrap();
    let result = call(
        &mut server(),
        "bundle_verify",
        json!({ "bundle": serde_json::to_value(&bundle).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["not_recomputed"], json!([]));
    assert!(result["honest_label"].as_str().unwrap().contains("1 of 1"));

    let mut tampered = serde_json::to_value(bundle).unwrap();
    tampered["contents"]["query"]["goal"] = json!("tampered");
    let refused = call(
        &mut server(),
        "bundle_verify",
        json!({ "bundle": tampered }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert!(refused["refusal"].as_str().unwrap().contains("digest"));
}

#[test]
fn posterior_gate_keeps_vector_and_release_scalar_separate() {
    let scored = compose(
        "result-1",
        &[Contribution::new(
            ScoreTier::Execution,
            "deterministic-check",
            Conclusion::Pass,
        )],
        &UnknownPolicy::Block,
    )
    .unwrap();
    let observations = vec![
        Observation::new("capability-a", "parent-1", scored.clone()),
        Observation::new("capability-a", "parent-2", scored),
    ];
    let gate = ReleaseGate::new("release-a", "a named test gate for this protocol surface")
        .unwrap()
        .require("capability-a", CoverageFloor::requiring(2, 2.0).grounded());
    let result = call(
        &mut server(),
        "posterior_gate",
        json!({
            "observations": serde_json::to_value(observations).unwrap(),
            "gate": serde_json::to_value(gate).unwrap()
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/posterior-gate/0.1"));
    assert_eq!(result["schema_version"], json!("07.0.1"));
    assert_eq!(
        result["capabilities"]["capability-a"]["pass_rate"]["mean"],
        json!(1.0)
    );
    assert_eq!(result["unprovenanced_observations"], json!(2));
    assert_eq!(result["gate"]["ok"], json!(true));
    assert!(result["gate"]["value"]["sensitivity"].is_array());
}

#[test]
fn oracle_combine_keeps_grounded_decisions_and_suppressed_judgements_visible() {
    let at = UtcTimestamp::parse("2026-08-14T00:00:00Z").unwrap();
    let validity = ValidityWindow::new(at.clone(), None).unwrap();
    let deterministic = OracleManifest::new(
        OracleRef::new(
            OracleId::parse("reference:checksum").unwrap(),
            OracleVersion::new(1, 0, 0),
        ),
        OracleEvidenceTier::Deterministic,
        [Plane::Artifact],
        [],
        validity.clone(),
    )
    .unwrap()
    .disclaiming_the_rest();
    let judge = OracleManifest::new(
        OracleRef::new(
            OracleId::parse("review:human").unwrap(),
            OracleVersion::new(1, 0, 0),
        ),
        OracleEvidenceTier::Judge,
        [Plane::Artifact],
        [],
        validity,
    )
    .unwrap()
    .disclaiming_the_rest();
    let grounded = Judgement::from_manifest(
        &deterministic,
        &at,
        Position::Supported,
        Confidence::CERTAIN,
    );
    let opinion = Judgement::from_manifest(
        &judge,
        &at,
        Position::Contradicted,
        Confidence::new(0.99).unwrap(),
    );
    let result = call(
        &mut server(),
        "oracle_combine",
        json!({
            "subject": "artifact-1",
            "at": "2026-08-14T00:00:00Z",
            "judgements": [grounded, opinion],
            "minimum_deciding_tier": "judge"
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/oracle-combine/0.1"));
    assert_eq!(result["status"], json!("valid"));
    assert_eq!(result["deciding_tier"], json!("deterministic"));
    assert_eq!(result["suppressed_override"], json!(true));
    assert_eq!(result["contributing"].as_array().unwrap().len(), 1);
    assert_eq!(result["withheld"].as_array().unwrap().len(), 1);
}

#[test]
fn oracle_reference_panel_keeps_reader_splits_and_blinding_failures_explicit() {
    let panel = ReaderPanel::new([
        Read::independent("reader-a", "positive").citing(["feature-a"]),
        Read::independent("reader-b", "negative"),
        Read::post_discussion("reader-c", "positive"),
    ])
    .unwrap();
    let result = call(
        &mut server(),
        "oracle_reference_panel",
        json!({
            "panel": serde_json::to_value(panel).unwrap(),
            "rule": serde_json::to_value(ConsensusRule::Majority).unwrap(),
            "model_call": "positive"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["readers"], json!(2));
    assert_eq!(result["reads"].as_array().unwrap().len(), 3);
    assert_eq!(result["consensus"]["determination"], json!("unresolved"));
    assert_eq!(result["per_reader"]["reader-a"], json!(true));
    assert_eq!(result["per_reader"]["reader-b"], json!(false));

    let unblinded = ReaderPanel::new([Read::independent("reader-a", "positive")])
        .unwrap()
        .with_adjudication(Adjudication::new(
            "adjudicator",
            "positive",
            Blinding::unblinded(),
        ));
    let refused = call(
        &mut server(),
        "oracle_reference_panel",
        json!({
            "panel": serde_json::to_value(unblinded).unwrap(),
            "rule": serde_json::to_value(ConsensusRule::Adjudicated).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["consensus"]["determination"], json!("unresolved"));
}

#[test]
fn oracle_missingness_keeps_separation_complete_case_and_egress_distinct() {
    let pattern = AbsencePattern::new()
        .observe("site-a", 0, 10)
        .observe("site-b", 10, 0);
    let result = call(
        &mut server(),
        "oracle_missingness",
        json!({
            "pattern": serde_json::to_value(pattern).unwrap(),
            "field": serde_json::to_value(Field::individual("genomics")).unwrap(),
            "boundary": serde_json::to_value(Boundary::aggregate_only("federated-site")).unwrap(),
            "small_cell_floor": 5,
            "mechanism": serde_json::to_value(MissingnessMechanism::DependsOnUnobserved { suspected: "outcome".into() }).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["informativeness"]["determination"],
        json!("contradicted")
    );
    assert_eq!(result["egress"]["determination"], json!("contradicted"));
    assert_eq!(
        result["complete_case"]["determination"],
        json!("contradicted")
    );
    assert_eq!(result["small_cell_floor"], json!(5));
}

#[test]
fn lab_plan_orders_reachable_evidence_and_refuses_privacy_crossings() {
    let graph = json!({
        "goal": "choose a safe assay",
        "obligations": {
            "identity": {
                "id": "identity",
                "statement": "the specimen identity is established",
                "value": 3.0,
                "mandatory": true,
                "history": []
            },
            "assay": {
                "id": "assay",
                "statement": "the assay is validated",
                "depends_on": ["identity"],
                "value": 2.0,
                "mandatory": false,
                "history": []
            }
        }
    });
    let fast = AcquisitionAction {
        id: "inspect-metadata".into(),
        kind: AcquisitionKind::InspectMetadata,
        targets: vec!["identity".into()],
        value: 10.0,
        cost: AcquisitionCost::new(1, 0),
        privacy: PrivacyBoundary::Inside,
    };
    let blocked = AcquisitionAction {
        id: "private-query".into(),
        kind: AcquisitionKind::QueryDatabase,
        targets: vec!["identity".into()],
        value: 100.0,
        cost: AcquisitionCost::new(1, 0),
        privacy: PrivacyBoundary::Crosses {
            policy: "no-private-database".into(),
        },
    };
    let result = call(
        &mut server(),
        "lab_plan",
        json!({
            "graph": graph,
            "actions": [fast, blocked],
            "budget": AcquisitionCost::new(2, 0),
            "max_items": 10
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["ordered"][0]["action"], json!("inspect-metadata"));
    assert_eq!(result["excluded"][0][0], json!("private-query"));
    assert_eq!(
        result["excluded"][0][1]["excluded_because"],
        json!("crosses_boundary")
    );
    assert_eq!(result["should_escalate"], json!(true));
}

#[test]
fn lab_pareto_audit_preserves_tradeoffs_holes_archive_and_ambiguous_selection() {
    let result = call(
        &mut server(),
        "lab_pareto_audit",
        json!({
            "objectives": [
                { "axis": "admissible_rate", "direction": "higher_is_better" },
                { "axis": "cost_units", "direction": "lower_is_better" }
            ],
            "profiles": [
                {
                    "candidate": "cheap",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.80 },
                        "cost_units": { "state": "measured", "value": 10.0 }
                    }
                },
                {
                    "candidate": "accurate",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.95 },
                        "cost_units": { "state": "measured", "value": 40.0 }
                    }
                },
                {
                    "candidate": "dominated",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.70 },
                        "cost_units": { "state": "measured", "value": 50.0 }
                    }
                },
                {
                    "candidate": "hole",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.90 },
                        "cost_units": { "state": "unmeasured", "reason": "not_attempted" }
                    }
                }
            ],
            "relations": [{ "left": "cheap", "right": "accurate" }],
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-pareto-audit/0.1"));
    assert_eq!(result["front"]["count"], json!(3));
    assert_eq!(result["archived_count"], json!(1));
    assert_eq!(result["front"]["unresolved_count"], json!(1));
    assert_eq!(
        result["front"]["selection"]["selection"],
        json!("ambiguous")
    );
    assert_eq!(
        result["relations"][0]["relation"]["relation"],
        json!("incomparable")
    );
    assert_eq!(
        result["relations"][0]["relation"]["incomparable_because"],
        json!("trade_off")
    );
    assert_eq!(result["archived"][0]["dominated_by"], json!("accurate"));

    let refused = call(
        &mut server(),
        "lab_pareto_audit",
        json!({
            "objectives": [{ "axis": "cost_units", "direction": "lower_is_better" }],
            "profiles": [{
                "candidate": "missing-axis",
                "values": {}
            }]
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("profile_insertion"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn lab_branch_audit_keeps_undetermined_escalation_cost_catches_and_escapes_separate() {
    let result = call(
        &mut server(),
        "lab_branch_audit",
        json!({
            "policy": {
                "ceiling": { "max_branches": 4, "max_verifier_calls": 2 },
                "on_undetermined": "escalate",
                "rules": [
                    {
                        "id": "irreversible",
                        "trigger": { "trigger": "reversibility_at_least", "level": "irreversible" },
                        "action": "fork_suffixes",
                        "cost": { "branches": 2, "verifier_calls": 1 }
                    },
                    {
                        "id": "unmeasured-failure",
                        "trigger": { "trigger": "historical_failure_rate_at_least", "rate": 0.2 },
                        "action": "invoke_verifier",
                        "cost": { "branches": 1, "verifier_calls": 1 }
                    }
                ]
            },
            "decisions": [
                {
                    "decision": "external-write",
                    "features": {
                        "reversibility": "irreversible",
                        "permission": "external_effect",
                        "value_at_stake": "severe",
                        "unseparated_hypotheses": 2,
                        "unmet_mandatory_obligations": 1,
                        "historical_failure_rate": 0.8,
                        "verifier_available": false
                    },
                    "caught": { "what": "unsafe suffix", "would_have_been": "write would proceed" },
                    "escaped": "a secondary harm remained"
                },
                {
                    "decision": "benign-read",
                    "features": {
                        "reversibility": "reversible",
                        "permission": "read_only",
                        "value_at_stake": "negligible",
                        "unseparated_hypotheses": 0,
                        "unmet_mandatory_obligations": 0,
                        "historical_failure_rate": 0.0,
                        "verifier_available": true
                    }
                },
                {
                    "decision": "unmeasured-class",
                    "features": {
                        "reversibility": "reversible",
                        "permission": "write_scoped",
                        "value_at_stake": "moderate",
                        "unseparated_hypotheses": 1,
                        "unmet_mandatory_obligations": 0,
                        "historical_failure_rate": null,
                        "verifier_available": true
                    },
                    "escaped": "harm escaped without a measured failure rate"
                }
            ],
            "max_rows": 2
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-branch-audit/0.1"));
    assert_eq!(result["yield"]["decisions"], json!(3));
    assert_eq!(result["yield"]["escalations"], json!(2));
    assert_eq!(result["yield"]["escalations_on_undetermined"], json!(1));
    assert_eq!(result["yield"]["catches"], json!(1));
    assert_eq!(result["yield"]["wasted_escalations"], json!(1));
    assert_eq!(result["verdict"]["verdict"], json!("mixed"));
    assert_eq!(result["rows"].as_array().unwrap().len(), 2);
    assert_eq!(result["rows_omitted"], json!(1));

    let refused = call(
        &mut server(),
        "lab_branch_audit",
        json!({
            "policy": {
                "ceiling": { "max_branches": 1, "max_verifier_calls": 1 },
                "on_undetermined": "escalate",
                "rules": [{
                    "id": "over-budget",
                    "trigger": { "trigger": "no_verifier_available" },
                    "action": "invoke_verifier",
                    "cost": { "branches": 2, "verifier_calls": 0 }
                }]
            },
            "decisions": [{
                "decision": "one",
                "features": {
                    "reversibility": "reversible",
                    "permission": "read_only",
                    "value_at_stake": "negligible",
                    "unseparated_hypotheses": 0,
                    "unmet_mandatory_obligations": 0,
                    "historical_failure_rate": 0.0,
                    "verifier_available": true
                }
            }]
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("policy_validation"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn lab_holdout_audit_never_mints_clean_scores_after_selection_and_rollback() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let result = call(
        &mut server(),
        "lab_holdout_audit",
        json!({
            "cost_ceiling": 100,
            "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
            "holdouts": [{
                "id": "private-a",
                "partition": "rotating_private_certification",
                "query_budget": 4
            }],
            "current": "v1",
            "operations": [
                { "kind": "checkpoint", "label": "before-v2" },
                { "kind": "promote", "configuration": "v2", "selected_using": "private-a", "rationale": "won the development panel" },
                { "kind": "rollback", "checkpoint": "before-v2" },
                { "kind": "measure", "holdout": "private-a", "configuration": "v2", "metric": "admissible_rate", "value": 0.9 },
                { "kind": "measure", "holdout": "private-a", "configuration": "v1", "metric": "admissible_rate", "value": 0.8 },
                { "kind": "measure", "holdout": "private-a", "configuration": "v1", "metric": "admissible_rate", "value": 0.7 }
            ],
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/lab-holdout-audit/0.1")
    );
    assert_eq!(result["current"], json!("v1"));
    assert_eq!(result["measurement_count"], json!(1));
    assert_eq!(result["measurement_refusal_count"], json!(2));
    assert_eq!(result["rollback_count"], json!(1));
    assert_eq!(result["operations"][1]["result"], json!("accepted"));
    assert_eq!(
        result["operations"][2]["complete_restoration"],
        json!(false)
    );
    assert_eq!(
        result["operations"][3]["result"],
        json!("measurement_refused")
    );
    assert!(result["operations"][3]["refusal"]
        .as_str()
        .unwrap()
        .contains("used to select"));
    assert_eq!(
        result["operations"][4]["result"],
        json!("clean_measurement")
    );
    assert_eq!(
        result["operations"][5]["result"],
        json!("measurement_refused")
    );
    assert!(result["holdouts"][0]["exposure"].as_array().unwrap().len() >= 3);
    assert_eq!(result["holdouts"][0]["retired"], json!(false));
}

#[test]
fn lab_space_audit_preserves_lineage_diffs_and_fail_closed_candidate_validation() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .costing(2)
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let result = call(
        &mut server(),
        "lab_space_audit",
        json!({
            "cost_ceiling": 10,
            "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
            "inspect": ["v2"],
            "comparisons": [{"before": "v1", "after": "v2"}],
            "include_components": true,
            "max_rows": 1
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-space-audit/0.1"));
    assert_eq!(result["registered_count"], json!(2));
    assert_eq!(result["candidate_rows_omitted"], json!(1));
    assert_eq!(result["inspection_rows"][0]["lineage"], json!(["v2", "v1"]));
    assert_eq!(result["inspection_rows"][0]["root"], json!("v1"));
    assert_eq!(
        result["comparison_rows"][0]["derived_relation"],
        json!(true)
    );
    assert!(result["comparison_rows"][0]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change.as_str().unwrap().contains("cost_units 0 -> 2")));
    assert_eq!(result["comparison_rows"][0]["change_count"], json!(1));

    let invalid = call(
        &mut server(),
        "lab_space_audit",
        json!({
            "cost_ceiling": 10,
            "candidates": [{
                "id": "unsafe",
                "components": [
                    {"id": "select", "kind": "context_selector"},
                    {"id": "run", "kind": "executor"},
                    {"id": "stop", "kind": "terminator"}
                ],
                "cost_units": 0,
                "touches_protected": ["benchmark_splits"]
            }]
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["ok"], json!(false));
    assert_eq!(invalid["stage"], json!("candidate_validation"));
    assert_eq!(invalid["fail_closed"], json!(true));
    assert_eq!(invalid["space_committed"], json!(false));
    assert_eq!(
        invalid["candidate_rows"][0]["registration"],
        json!("not_attempted")
    );
}

#[test]
fn lab_evolution_audit_only_claims_clean_directional_improvement_and_retains_contamination() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let base = json!({
        "cost_ceiling": 100,
        "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
        "baseline": "v1",
        "candidate": "v2",
        "holdout": {
            "id": "private-a",
            "partition": "rotating_private_certification",
            "query_budget": 4
        },
        "card_id": "card-v2",
        "proposal": {
            "id": "proposal-v2",
            "rationale": "widen the protected closure",
            "target_failure_clusters": ["cluster:missing-closure"],
            "changed_artifacts": ["component select depth 3 -> 5"],
            "regression_cells": ["cell:closure"],
            "touches_protected": []
        },
        "rollback_handle": "v1",
        "direction": "higher_is_better",
        "would_have_to_be_true": ["the gain survives a second rotating private set"],
        "max_rows": 10
    });
    let claimed = call(
        &mut server(),
        "lab_evolution_audit",
        json!({
            "cost_ceiling": base["cost_ceiling"],
            "candidates": base["candidates"],
            "baseline": base["baseline"],
            "candidate": base["candidate"],
            "holdout": base["holdout"],
            "measurements": [
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.70 },
                { "configuration": "v2", "metric": "admissible_rate", "value": 0.83 }
            ],
            "card_id": base["card_id"],
            "proposal": base["proposal"],
            "rollback_handle": base["rollback_handle"],
            "direction": base["direction"],
            "would_have_to_be_true": base["would_have_to_be_true"],
            "max_rows": base["max_rows"]
        }),
    );
    assert_eq!(claimed["__isError"], json!(false));
    assert_eq!(claimed["ok"], json!(true));
    assert_eq!(
        claimed["schema"],
        json!("bioprism-mcp/lab-evolution-audit/0.1")
    );
    assert_eq!(claimed["status"], json!("improvement_claimed"));
    assert_eq!(claimed["claimable"], json!(true));
    assert!((claimed["claim"]["delta"].as_f64().unwrap() - 0.13).abs() < 1e-9);
    assert!(claimed["sentence"]
        .as_str()
        .unwrap()
        .contains("rotating_private_certification"));

    let contaminated = call(
        &mut server(),
        "lab_evolution_audit",
        json!({
            "cost_ceiling": base["cost_ceiling"],
            "candidates": base["candidates"],
            "baseline": base["baseline"],
            "candidate": base["candidate"],
            "holdout": base["holdout"],
            "measurements": [
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.70 },
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.71 },
                { "configuration": "v2", "metric": "admissible_rate", "value": 0.83 }
            ],
            "card_id": base["card_id"],
            "proposal": base["proposal"],
            "rollback_handle": base["rollback_handle"],
            "direction": base["direction"],
            "would_have_to_be_true": base["would_have_to_be_true"]
        }),
    );
    assert_eq!(contaminated["__isError"], json!(false));
    assert_eq!(contaminated["ok"], json!(true));
    assert_eq!(contaminated["status"], json!("contaminated"));
    assert_eq!(contaminated["claimable"], json!(false));
    assert_eq!(
        contaminated["card"]["surface"]["surface"],
        json!("contaminated")
    );
    assert!(contaminated["claim_refusal"]
        .as_str()
        .unwrap()
        .contains("contaminated"));
}

#[test]
fn obligation_gate_check_keeps_effective_states_and_mandatory_closure_visible() {
    let at = Timestamp::parse("2026-08-14T00:00:00Z").unwrap();
    let mut graph = ObligationGraph::new("publish a validation report");
    graph
        .insert(
            Obligation::new("identity", "the specimen identity is established")
                .mandatory()
                .with_value(3.0),
        )
        .unwrap();
    graph
        .insert(
            Obligation::new("validation", "the assay validation is complete")
                .depending_on(["identity"])
                .mandatory()
                .with_value(5.0),
        )
        .unwrap();
    graph
        .insert(
            Obligation::new("consent", "the release consent is recorded")
                .mandatory()
                .with_value(4.0),
        )
        .unwrap();
    graph
        .record(
            "identity",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.95)
                .with_evidence(["evidence://identity"]),
        )
        .unwrap();
    let action = ObligationAction::new("publish", RegretClass::Irreversible)
        .described("publish the validation result")
        .requiring(ObligationPredicate::satisfied("validation"));

    let blocked = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap(),
            "max_items": 1
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(
        blocked["schema"],
        json!("bioprism-mcp/obligation-gate-check/0.1")
    );
    assert_eq!(blocked["outcome_kind"], json!("blocked"));
    assert_eq!(
        blocked["gate"]["reason"]["reason"],
        json!("prerequisites_unmet")
    );
    assert_eq!(blocked["graph"]["valid"], json!(true));
    assert_eq!(blocked["graph"]["omitted_effective_states"], json!(2));
    assert_eq!(
        blocked["graph"]["frontier"][0]["obligation"],
        json!("validation")
    );

    graph
        .record(
            "validation",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.9)
                .with_evidence(["evidence://validation"]),
        )
        .unwrap();
    let blocked_closure = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap()
        }),
    );
    assert_eq!(blocked_closure["outcome_kind"], json!("blocked"));
    assert_eq!(
        blocked_closure["gate"]["reason"]["reason"],
        json!("mandatory_obligation_outstanding")
    );
    assert_eq!(
        blocked_closure["gate"]["reason"]["obligation"],
        json!("consent")
    );

    graph
        .record(
            "consent",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.9)
                .with_evidence(["evidence://consent"]),
        )
        .unwrap();
    let allowed = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap()
        }),
    );
    assert_eq!(allowed["outcome_kind"], json!("allowed"));
    assert_eq!(allowed["allowed"], json!(true));
    assert_eq!(allowed["refusal"], Value::Null);
    assert_eq!(allowed["gate"]["checked"], json!(["validation"]));
    assert_eq!(allowed["graph"]["undischarged"], json!([]));
    assert_eq!(allowed["graph"]["sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn atlas_report_preserves_holes_and_gates_composites() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }
    let ontology = CapabilityOntology::from_nodes(
        "atlas-test/1",
        [
            CapabilityNode::new(
                cap("agent"),
                "agent",
                CapabilityFamily::DomainReasoning,
                CapabilityDimension::Competence,
            ),
            CapabilityNode::new(
                cap("measured"),
                "measured",
                CapabilityFamily::Verification,
                CapabilityDimension::Reliability,
            )
            .with_parent(cap("agent")),
            CapabilityNode::new(
                cap("unmeasured"),
                "unmeasured",
                CapabilityFamily::ToolUse,
                CapabilityDimension::Efficiency,
            )
            .with_parent(cap("agent")),
        ],
    )
    .unwrap();
    let atlas = Atlas::builder(ontology)
        .evidence(EvidenceRecord::new(
            "trial-1",
            cap("measured"),
            "atlas-test/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .build()
        .unwrap();
    let weighting = WeightingPolicy::declare(
        "test composite",
        [(cap("measured"), 1.0), (cap("unmeasured"), 1.0)],
    )
    .unwrap();
    let result = call(
        &mut server(),
        "atlas_report",
        json!({
            "atlas": serde_json::to_value(atlas).unwrap(),
            "weighting": serde_json::to_value(weighting).unwrap(),
            "max_items": 10
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/atlas-report/0.1"));
    assert_eq!(result["summary"]["measured"], json!(1));
    assert_eq!(result["summary"]["holes"], json!(2));
    assert_eq!(result["composite"]["ok"], json!(false));
    assert!(result["composite"]["refusal"]
        .as_str()
        .unwrap()
        .contains("unmeasured"));
}

#[test]
fn atlas_surface_audit_preserves_debt_browse_visibility_and_rate_denominators() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }
    fn conditions(label: &str) -> MeasurementConditions {
        MeasurementConditions::new(MetricsSubject::grid(label), ScoringRule::atlas_pass_rate())
    }
    fn measured(value: f64, effective_size: usize) -> GridCell {
        GridCell::point(
            value,
            NoIntervalReason::EstimatorNotAvailable,
            effective_size,
        )
        .unwrap()
    }
    fn record(id: &str, inducement: Inducement) -> FailureRecord {
        let chain = CausalChain::new(
            id,
            FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
            FailureLabel::new(FailureMechanism::StaleEvidenceTrusted, 2),
            vec![FailureLabel::new(
                FailureMechanism::UncertaintyMisreportedToCaller,
                3,
            )],
            FailureLabel::new(FailureMechanism::SuccessfulCommandMistakenForTaskSuccess, 4),
        )
        .unwrap();
        FailureRecord::new(
            id,
            RunId::parse(format!("run-{id}")).unwrap(),
            cap("identity.lineage"),
            "atlasx-test/1",
            chain,
            FailureAxes::new(
                EvidenceStatus::Preserved,
                Reversibility::Reversible,
                Detectability::DetectedByReview,
                Severity::Degraded,
                inducement,
            ),
            LabelDistribution::certain(
                FailureMechanism::StaleEvidenceTrusted,
                "protocol fixture diagnosis",
            ),
        )
    }

    let grid = CapabilityGrid::new("surface-system", conditions("surface-system"))
        .with_cell(cap("identity.lineage"), measured(0.8, 4))
        .with_cell(
            cap("causal.interpretation"),
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        )
        .with_cell(
            cap("cohort.statistics"),
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        );
    let later_grid = CapabilityGrid::new("surface-system", conditions("surface-system"))
        .with_cell(cap("identity.lineage"), measured(0.9, 5))
        .with_cell(
            cap("causal.interpretation"),
            GridCell::unmeasured(UnmeasuredReason::OutOfScopeByDeclaredUse),
        )
        .with_cell(cap("cohort.statistics"), measured(0.7, 6));
    let result = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(grid).unwrap(),
            "later_grid": serde_json::to_value(later_grid).unwrap(),
            "failures": [
                serde_json::to_value(record("f-visible", Inducement::ModelInduced)).unwrap(),
                serde_json::to_value(record("f-withheld", Inducement::EvaluatorInduced)).unwrap()
            ],
            "facet": "mechanism",
            "visibility": [{ "failure_id": "f-withheld", "state": "under-review" }],
            "rate_capabilities": ["identity.lineage"],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/atlas-surface-audit/0.1")
    );
    assert_eq!(result["coverage"]["measured"], json!(1));
    assert_eq!(result["coverage"]["unmeasured"], json!(2));
    assert_eq!(
        result["debt_discharge"]["measured"]["rows"],
        json!(["cohort.statistics"])
    );
    assert_eq!(
        result["debt_discharge"]["declared_away"]["rows"],
        json!(["causal.interpretation"])
    );
    assert_eq!(result["failure_browse"]["records_browsed"], json!(2));
    assert_eq!(result["failure_browse"]["visible"], json!(1));
    assert_eq!(result["failure_browse"]["withheld"], json!(1));
    assert_eq!(result["surface_audits"]["sound"], json!(true));
    assert_eq!(
        result["rate_checks"]["rows"][0]["answer"]["outcome"],
        json!("answered")
    );
    assert_eq!(
        result["rate_checks"]["rows"][0]["answer"]["cell"]["kind"],
        json!("score")
    );
    assert!(
        (result["rate_checks"]["rows"][0]["answer"]["cell"]["value"]
            .as_f64()
            .unwrap()
            - 0.25)
            .abs()
            < 1e-9
    );

    let no_holes = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(CapabilityGrid::new(
                "surface-policy",
                conditions("surface-policy")
            ).with_cell(
                cap("causal.interpretation"),
                GridCell::unmeasured(UnmeasuredReason::NotAttempted)
            )).unwrap(),
            "require_no_holes": true
        }),
    );
    assert_eq!(no_holes["__isError"], json!(false));
    assert_eq!(no_holes["ok"], json!(false));
    assert_eq!(no_holes["stage"], json!("coverage_policy"));
    assert_eq!(no_holes["fail_closed"], json!(true));

    let mismatched = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(CapabilityGrid::new(
                "surface-left",
                conditions("surface-left")
            )).unwrap(),
            "later_grid": serde_json::to_value(CapabilityGrid::new(
                "surface-right",
                conditions("surface-right")
            )).unwrap()
        }),
    );
    assert_eq!(mismatched["__isError"], json!(false));
    assert_eq!(mismatched["ok"], json!(false));
    assert_eq!(mismatched["stage"], json!("debt_discharge"));
}

#[test]
fn bioeval_reference_audit_validates_mass_and_preserves_distributed_truth() {
    let distribution = ReferenceDistribution::new(
        [
            ("progression".to_string(), 0.6),
            ("stable".to_string(), 0.4),
        ],
        Dispersion::Mixed {
            aleatoric_fraction: 0.5,
        },
    )
    .unwrap();
    let result = call(
        &mut server(),
        "bioeval_reference_audit",
        json!({
            "reference": serde_json::to_value(ReferenceStandard::Distribution(distribution)).unwrap(),
            "state": "progression"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-reference-audit/0.1")
    );
    assert_eq!(result["reference_kind"], json!("distribution"));
    assert_eq!(result["can_certify_clean_pass"], json!(false));
    assert_eq!(result["modal_state"], json!("progression"));
    assert_eq!(result["modal_mass"], json!(0.6));
    assert_eq!(result["queried_state_mass"], json!(0.6));
    assert_eq!(result["dispersion"], json!("mixed"));
    assert!(result["entropy_bits"].as_f64().unwrap() > 0.9);

    let invalid = call(
        &mut server(),
        "bioeval_reference_audit",
        json!({
            "reference": {
                "standard": "distribution",
                "mass": {"progression": 0.8},
                "dispersion": {"kind": "aleatoric"}
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].is_string());
}

#[test]
fn bioeval_acquisition_audit_preserves_obligation_stopping_and_named_regret() {
    let result = call(
        &mut server(),
        "bioeval_acquisition_audit",
        json!({
            "obligations": [
                { "id": "subtype", "required": true },
                { "id": "context", "required": false }
            ],
            "actions": [
                { "id": "read-notes", "kind": "metadata", "cost": 2, "closes": ["context"] },
                { "id": "search", "kind": "retrieval", "cost": 5, "closes": [] },
                { "id": "panel", "kind": "assay", "cost": 40, "closes": ["subtype"] },
                { "id": "extra", "kind": "analysis", "cost": 1, "closes": [] }
            ],
            "stopped_after": true,
            "reference_policy": { "name": "random-acquisition", "cost": 30, "admissible": false }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-acquisition-audit/0.1")
    );
    assert_eq!(result["status"], json!("admissible"));
    assert_eq!(result["required_open_count"], json!(0));
    assert_eq!(result["cost"], json!(48));
    assert_eq!(result["findings"]["deferred_decisive_cost"], json!(7));
    assert_eq!(
        result["findings"]["redundant_action_ids"],
        json!(["extra", "search"])
    );
    assert_eq!(
        result["findings"]["unnecessary_action_ids"],
        json!(["extra"])
    );
    assert_eq!(result["regret"]["cost_difference"], json!(18));
    assert_eq!(result["regret"]["like_for_like"], json!(false));

    let missing_reference = call(
        &mut server(),
        "bioeval_acquisition_audit",
        json!({
            "obligations": [],
            "actions": [],
            "require_reference": true
        }),
    );
    assert_eq!(missing_reference["__isError"], json!(false));
    assert_eq!(missing_reference["ok"], json!(false));
    assert_eq!(missing_reference["stage"], json!("reference_policy"));
    assert_eq!(missing_reference["fail_closed"], json!(true));
}

#[test]
fn bioeval_grounding_audit_preserves_states_locators_staleness_and_lineage() {
    let result = call(
        &mut server(),
        "bioeval_grounding_audit",
        json!({
            "claims": [
                { "id": "supported" },
                { "id": "contested" },
                { "id": "unverified" },
                { "id": "contradicted" },
                { "id": "unsupported" }
            ],
            "evidence": [
                { "id": "shown", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-1"], "locator_status": { "locator": "resolved", "digest": "sha256:shown" } },
                { "id": "changed", "last_modified": "2026-06-01T00:00:00Z", "lineage": ["specimen-2"], "locator_status": { "locator": "resolved", "digest": "sha256:changed" } },
                { "id": "asserted", "last_modified": "2026-01-01T00:00:00Z", "locator_status": { "locator": "not_checked" } },
                { "id": "opposed", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-3"], "locator_status": { "locator": "unresolvable", "detail": "fixture missing" } },
                { "id": "adjacent", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-4"], "locator_status": { "locator": "resolved", "digest": "sha256:adjacent" } },
                { "id": "orphan", "last_modified": "2026-06-01T00:00:00Z", "locator_status": { "locator": "not_checked" } }
            ],
            "edges": [
                { "claim": "supported", "evidence": "shown", "kind": "supports" },
                { "claim": "contested", "evidence": "changed", "kind": "supports" },
                { "claim": "contested", "evidence": "opposed", "kind": "contradicts" },
                { "claim": "unverified", "evidence": "asserted", "kind": "supports" },
                { "claim": "contradicted", "evidence": "opposed", "kind": "contradicts" },
                { "claim": "unsupported", "evidence": "adjacent", "kind": "adjacent" }
            ],
            "stale_against": "2026-03-01T00:00:00Z",
            "max_items": 3
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-grounding-audit/0.1")
    );
    assert_eq!(result["census"]["supported"], json!(1));
    assert_eq!(result["census"]["contested"], json!(1));
    assert_eq!(result["census"]["support_unverified"], json!(1));
    assert_eq!(result["census"]["contradicted"], json!(1));
    assert_eq!(result["census"]["unsupported"], json!(1));
    assert_eq!(result["census"]["adjacent_citations"], json!(1));
    assert_eq!(result["census"]["fully_grounded"], json!(false));
    assert_eq!(result["staleness"]["stale_count"], json!(2));
    assert_eq!(
        result["findings"]["lineage_gap_evidence"]["ids"],
        json!(["asserted", "orphan"])
    );
    assert_eq!(
        result["findings"]["orphan_evidence"]["ids"],
        json!(["orphan"])
    );
    assert_eq!(result["claims"]["omitted"], json!(2));
    assert_eq!(result["graph"]["duplicate_edge_count"], json!(0));

    let invalid = call(
        &mut server(),
        "bioeval_grounding_audit",
        json!({
            "claims": [{ "id": "claim" }],
            "evidence": [],
            "edges": [{ "claim": "claim", "evidence": "missing", "kind": "supports" }]
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["ok"], json!(false));
    assert_eq!(invalid["stage"], json!("edge_validation"));
    assert_eq!(invalid["fail_closed"], json!(true));
}

#[test]
fn bioeval_estimand_audit_preserves_claim_language_identification_and_transport() {
    let result = call(
        &mut server(),
        "bioeval_estimand_audit",
        json!({
            "estimand": {
                "intervention": "knockdown",
                "comparator": "control",
                "unit": "cell line",
                "outcome": "viability",
                "horizon": "72h",
                "scope": "pdac-twin"
            },
            "kind": "intervention",
            "basis": { "evidentiary": "model_conditional", "model": "pdac-twin-v2" },
            "identification": {
                "identification": "probed",
                "strategy": "backdoor",
                "assumptions": ["no unmeasured confounding"],
                "checks": [
                    { "name": "negative-control", "passed": false, "detail": "signal remained" },
                    { "name": "sensitivity", "passed": true, "detail": "stable" }
                ]
            },
            "corroborations": [
                { "source": "GSE-14520", "kind": "intervention", "detail": "external replication" }
            ],
            "transport_requests": [
                { "target": "pdac-twin", "declared_scopes": ["pdac-twin"] },
                { "target": "patients", "declared_scopes": ["pdac-twin"] }
            ],
            "require_identification": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-estimand-audit/0.1")
    );
    assert_eq!(result["estimand"]["five_elements_complete"], json!(true));
    assert_eq!(result["claim"]["kind"], json!("intervention"));
    assert_eq!(result["claim"]["still_model_conditional"], json!(false));
    assert!(result["claim"]["claim_language"]
        .as_str()
        .unwrap()
        .contains("changes"));
    assert_eq!(
        result["claim"]["identification_summary"]["status"],
        json!("probed")
    );
    assert_eq!(
        result["claim"]["identification_summary"]["failed_check_count"],
        json!(1)
    );
    assert_eq!(result["transport"]["status"], json!("partially_declared"));
    assert_eq!(result["transport"]["accepted"], json!(1));
    assert_eq!(result["transport"]["refused"], json!(1));

    let same_model = call(
        &mut server(),
        "bioeval_estimand_audit",
        json!({
            "estimand": {
                "intervention": "knockdown",
                "comparator": "control",
                "unit": "cell line",
                "outcome": "viability",
                "horizon": "72h",
                "scope": "pdac-twin"
            },
            "kind": "association",
            "basis": { "evidentiary": "model_conditional", "model": "pdac-twin-v2" },
            "corroborations": [
                { "source": "pdac-twin-v2", "kind": "association", "detail": "ran again" }
            ]
        }),
    );
    assert_eq!(same_model["__isError"], json!(false));
    assert_eq!(same_model["ok"], json!(false));
    assert_eq!(same_model["stage"], json!("corroboration_validation"));
    assert_eq!(same_model["fail_closed"], json!(true));
}

#[test]
fn bioeval_evaluator_audit_separates_harness_health_task_outcomes_and_hidden_data() {
    let result = call(
        &mut server(),
        "bioeval_evaluator_audit",
        json!({
            "runs": [
                {
                    "evaluator": "grader-a",
                    "health": { "health": "healthy" },
                    "reached": "met",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "grader-b",
                    "health": { "health": "healthy" },
                    "reached": "not_met",
                    "diagnostic": { "command": "pytest", "exit_state": "1", "diff": "expected output missing", "logs": [], "hidden_data_access": [] }
                },
                {
                    "evaluator": "grader-b",
                    "health": { "health": "healthy" },
                    "reached": "inapplicable",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "grader-c",
                    "health": { "health": "healthy" },
                    "reached": "not_met",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "timeout",
                    "health": { "health": "timed_out", "after": "120s" },
                    "reached": null,
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "broken-fixture",
                    "health": { "health": "fixture_broken", "detail": "expected file absent" },
                    "reached": "met",
                    "diagnostic": { "command": "grader", "exit_state": "fixture-error", "diff": "", "logs": [], "hidden_data_access": ["read expected_outputs/"] }
                }
            ],
            "max_items": 2
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-evaluator-audit/0.1")
    );
    assert_eq!(result["panel"]["run_count"], json!(6));
    assert_eq!(result["panel"]["healthy_count"], json!(4));
    assert_eq!(result["panel"]["unhealthy_count"], json!(2));
    assert_eq!(result["panel"]["task_evidence_count"], json!(3));
    assert_eq!(result["panel"]["refused_task_outcome_count"], json!(3));
    assert_eq!(result["panel"]["outcomes"]["met"], json!(1));
    assert_eq!(result["panel"]["outcomes"]["not_met"], json!(1));
    assert_eq!(result["panel"]["outcomes"]["inapplicable"], json!(1));
    assert_eq!(
        result["panel"]["posture"],
        json!("review_required_hidden_data")
    );
    assert_eq!(
        result["findings"]["duplicate_evaluator_ids"]["ids"],
        json!(["grader-b"])
    );
    assert_eq!(result["runs"]["omitted"], json!(4));
    assert_eq!(result["runs"]["rows"][1]["task_outcome"], json!("not_met"));

    let hidden_refusal = call(
        &mut server(),
        "bioeval_evaluator_audit",
        json!({
            "runs": [{
                "evaluator": "grader",
                "health": { "health": "healthy" },
                "reached": "met",
                "diagnostic": { "command": "grader", "exit_state": "0", "diff": "", "logs": [], "hidden_data_access": ["secret"] }
            }],
            "fail_on_hidden_data": true
        }),
    );
    assert_eq!(hidden_refusal["__isError"], json!(false));
    assert_eq!(hidden_refusal["ok"], json!(false));
    assert_eq!(hidden_refusal["stage"], json!("hidden_data_policy"));
    assert_eq!(hidden_refusal["fail_closed"], json!(true));
}

#[test]
fn bioeval_plane_audit_keeps_unscored_and_inapplicable_out_of_the_fold() {
    let incomplete = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "fixed-model",
                "tier": "fixed_input_model",
                "dimensions": [
                    { "id": "accuracy", "required": "fixed_input_model", "weight": 2.0 },
                    { "id": "assay-selection", "required": "tool_using_agent", "weight": 1.0 },
                    { "id": "calibration", "required": "fixed_input_model", "weight": 1.0 }
                ],
                "cells": {
                    "accuracy": { "state": "scored", "score": 0.8 },
                    "assay-selection": { "state": "inapplicable", "required": "tool_using_agent", "declared": "fixed_input_model" },
                    "calibration": { "state": "unscored", "reason": "no_reference_standard", "note": "reference panel pending" }
                }
            },
            "max_items": 2
        }),
    );
    assert_eq!(incomplete["__isError"], json!(false));
    assert_eq!(incomplete["ok"], json!(true));
    assert_eq!(
        incomplete["schema"],
        json!("bioprism-mcp/bioeval-plane-audit/0.1")
    );
    assert_eq!(incomplete["plane"]["scored_count"], json!(1));
    assert_eq!(incomplete["plane"]["unscored_count"], json!(1));
    assert_eq!(incomplete["plane"]["inapplicable_count"], json!(1));
    assert_eq!(incomplete["findings"]["fold_blocked"], json!(true));
    assert_eq!(
        incomplete["findings"]["unscored_dimensions"]["ids"],
        json!(["calibration"])
    );
    assert_eq!(incomplete["dimensions"]["omitted"], json!(1));
    assert!(incomplete["fold"]["value"].is_null());

    let required = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "fixed-model",
                "tier": "fixed_input_model",
                "dimensions": [{ "id": "accuracy", "required": "fixed_input_model", "weight": 1.0 }],
                "cells": { "accuracy": { "state": "unscored", "reason": "not_attempted" } }
            },
            "require_fold": true
        }),
    );
    assert_eq!(required["ok"], json!(false));
    assert_eq!(required["stage"], json!("fold_policy"));
    assert_eq!(required["fail_closed"], json!(true));

    let folded = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "pipeline",
                "tier": "workflow_pipeline",
                "dimensions": [
                    { "id": "accuracy", "required": "fixed_input_model", "weight": 2.0 },
                    { "id": "workflow", "required": "workflow_pipeline", "weight": 1.0 },
                    { "id": "agent-action", "required": "tool_using_agent", "weight": 1.0 }
                ],
                "cells": {
                    "accuracy": { "state": "scored", "score": 0.75 },
                    "workflow": { "state": "scored", "score": 0.9 },
                    "agent-action": { "state": "inapplicable", "required": "tool_using_agent", "declared": "workflow_pipeline" }
                }
            },
            "require_fold": true
        }),
    );
    assert_eq!(folded["ok"], json!(true));
    assert_eq!(folded["fold"]["folded"], json!(true));
    assert!((folded["fold"]["value"].as_f64().unwrap() - 0.8).abs() < 1e-12);
    assert_eq!(folded["fold"]["included"], json!(["accuracy", "workflow"]));
    assert_eq!(folded["fold"]["excluded"][0]["id"], json!("agent-action"));
}

#[test]
fn bioeval_metamorphic_audit_separates_failure_directions_and_undetermined_trials() {
    let result = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [
                {
                    "id": "formatting",
                    "relation": "invariant",
                    "trials": [
                        { "id": "same", "relation": "invariant", "response": { "response": "unchanged" } },
                        { "id": "filename-shortcut", "relation": "invariant", "response": { "response": "moved", "direction": "increase" } },
                        { "id": "incomparable-format", "relation": "invariant", "response": { "response": "incomparable" } }
                    ]
                },
                {
                    "id": "biology-change",
                    "relation": { "directional_change": { "expected": "increase" } },
                    "trials": [
                        { "id": "expected-change", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "moved", "direction": "increase" } },
                        { "id": "blind-spot", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "unchanged" } },
                        { "id": "wrong-way", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "moved", "direction": "decrease" } }
                    ]
                }
            ],
            "max_items": 2,
            "require_both_relations": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-metamorphic-audit/0.1")
    );
    assert_eq!(result["suite"]["family_count"], json!(2));
    assert_eq!(result["suite"]["trial_count"], json!(6));
    assert_eq!(
        result["suite"]["relation_coverage"]["complete"],
        json!(true)
    );
    assert_eq!(result["suite"]["failing_family_count"], json!(2));
    assert_eq!(result["suite"]["undetermined_trial_count"], json!(1));
    assert_eq!(result["suite"]["has_suite_wide_consistency"], json!(false));
    assert_eq!(
        result["findings"]["false_sensitivity_trials"]["ids"],
        json!(["filename-shortcut"])
    );
    assert_eq!(
        result["findings"]["false_invariance_trials"]["ids"],
        json!(["blind-spot"])
    );
    assert_eq!(
        result["findings"]["wrong_direction_trials"]["ids"],
        json!(["wrong-way"])
    );
    assert_eq!(
        result["findings"]["undetermined_families"]["ids"],
        json!(["formatting"])
    );
    assert_eq!(result["families"]["rows"][0]["trials"]["omitted"], json!(1));

    let undetermined_refusal = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [{
                "id": "oracle-gap",
                "relation": "invariant",
                "trials": [{ "id": "unknown", "relation": "invariant", "response": { "response": "incomparable" } }]
            }],
            "fail_on_undetermined": true
        }),
    );
    assert_eq!(undetermined_refusal["ok"], json!(false));
    assert_eq!(undetermined_refusal["stage"], json!("oracle_quality"));
    assert_eq!(undetermined_refusal["fail_closed"], json!(true));

    let coverage_refusal = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [{
                "id": "only-invariant",
                "relation": "invariant",
                "trials": [{ "id": "same", "relation": "invariant", "response": { "response": "unchanged" } }]
            }],
            "require_both_relations": true
        }),
    );
    assert_eq!(coverage_refusal["ok"], json!(false));
    assert_eq!(coverage_refusal["stage"], json!("relation_coverage"));
    assert_eq!(coverage_refusal["fail_closed"], json!(true));
}

#[test]
fn bioeval_waiver_audit_preserves_gate_verdicts_and_nonwaivable_vetoes() {
    let arguments = json!({
        "version": "release-2026.08",
        "at": "2026-08-16T12:00:00Z",
        "gates": [
            { "id": "health", "kind": "benchmark_health", "verdict": { "verdict": "violated", "detail": "calibration below floor" } },
            { "id": "unknown-rate", "kind": "maximum_unknown_rate", "verdict": { "verdict": "unevaluable", "missing": "reference panel" } },
            { "id": "safety", "kind": "safety_veto", "verdict": { "verdict": "violated", "detail": "forbidden action" } },
            { "id": "confidence", "kind": "confidence_requirement", "verdict": { "verdict": "met" } }
        ],
        "waivers": [{
            "gate": "health",
            "authoriser": "release-board",
            "rationale": "ship only the documented calibration exception",
            "expiry": "2026-09-01T00:00:00Z",
            "affected_versions": ["release-2026.08"],
            "follow_up": "recalibrate before the next release"
        }],
        "max_items": 3
    });
    let result = call(&mut server(), "bioeval_waiver_audit", arguments.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-waiver-audit/0.1")
    );
    assert_eq!(result["release"]["blocking_before"], json!(3));
    assert_eq!(result["release"]["blocking_after"], json!(2));
    assert_eq!(result["release"]["waived_count"], json!(1));
    assert_eq!(result["release"]["unevaluable_count"], json!(1));
    assert_eq!(result["release"]["releasable"], json!(false));
    assert_eq!(result["findings"]["waived_gates"]["ids"], json!(["health"]));
    assert_eq!(
        result["findings"]["still_blocking"]["ids"],
        json!(["safety", "unknown-rate"])
    );
    assert_eq!(
        result["gates"]["rows"][0]["verdict"]["verdict"],
        json!("violated")
    );
    assert_eq!(result["gates"]["rows"][0]["blocks_after"], json!(false));
    assert_eq!(
        result["waivers"]["rows"][0]["waiver"]["follow_up"],
        json!("recalibrate before the next release")
    );

    let release_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": arguments["gates"],
            "waivers": arguments["waivers"],
            "require_releasable": true
        }),
    );
    assert_eq!(release_refusal["ok"], json!(false));
    assert_eq!(release_refusal["stage"], json!("release_gate_policy"));
    assert_eq!(release_refusal["fail_closed"], json!(true));

    let unknown_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": arguments["gates"],
            "waivers": arguments["waivers"],
            "require_no_unevaluable": true
        }),
    );
    assert_eq!(unknown_refusal["ok"], json!(false));
    assert_eq!(unknown_refusal["stage"], json!("unknown_rate_policy"));

    let veto_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": [arguments["gates"][2]],
            "waivers": [{
                "gate": "safety",
                "authoriser": "release-board",
                "rationale": "attempted override",
                "expiry": "2026-09-01T00:00:00Z",
                "affected_versions": ["release-2026.08"],
                "follow_up": "review safety finding"
            }]
        }),
    );
    assert_eq!(veto_refusal["ok"], json!(false));
    assert_eq!(veto_refusal["stage"], json!("waiver_application"));
    assert_eq!(veto_refusal["fail_closed"], json!(true));

    let expiry_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-09-02T00:00:00Z",
            "gates": [arguments["gates"][0]],
            "waivers": arguments["waivers"]
        }),
    );
    assert_eq!(expiry_refusal["ok"], json!(false));
    assert_eq!(expiry_refusal["stage"], json!("waiver_application"));
}

#[test]
fn bioeval_design_audit_keeps_single_factor_contrasts_and_interaction_holes_visible() {
    let complete = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "p1", "levels": { "planner": "tree", "verifier": "off" }, "conclusion": "pass", "tier": "execution" },
            { "id": "v1", "levels": { "planner": "react", "verifier": "on" }, "conclusion": "pass", "tier": "execution" },
            { "id": "both", "levels": { "planner": "tree", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "controlled": true,
        "max_items": 2,
        "require_contrasts": true,
        "require_complete_interactions": true,
        "require_attribution": true
    });
    let result = call(&mut server(), "bioeval_design_audit", complete.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-design-audit/0.1")
    );
    assert_eq!(result["design"]["contrast_count"], json!(4));
    assert_eq!(result["design"]["unattributable_arm_count"], json!(1));
    assert_eq!(result["interactions"]["estimable_count"], json!(1));
    assert_eq!(result["interactions"]["missing_count"], json!(0));
    assert_eq!(result["attributions"]["total"], json!(4));
    assert_eq!(result["attributions"]["causal_count"], json!(4));
    assert_eq!(
        result["findings"]["unattributable_arms"]["ids"],
        json!(["both"])
    );
    assert_eq!(result["arms"]["omitted"], json!(2));
    assert_eq!(result["attributions"]["rows"][0]["causal"], json!(true));

    let incomplete = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "p1", "levels": { "planner": "tree", "verifier": "off" }, "conclusion": "pass", "tier": "execution" },
            { "id": "v1", "levels": { "planner": "react", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "require_complete_interactions": true
    });
    let interaction_refusal = call(&mut server(), "bioeval_design_audit", incomplete);
    assert_eq!(interaction_refusal["ok"], json!(false));
    assert_eq!(interaction_refusal["stage"], json!("interaction_coverage"));
    assert_eq!(interaction_refusal["fail_closed"], json!(true));

    let no_contrast = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "both", "levels": { "planner": "tree", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "require_contrasts": true
    });
    let contrast_refusal = call(&mut server(), "bioeval_design_audit", no_contrast);
    assert_eq!(contrast_refusal["ok"], json!(false));
    assert_eq!(contrast_refusal["stage"], json!("contrast_coverage"));
}

#[test]
fn bioeval_mesh_audit_collapses_shared_inputs_and_separates_disagreement_kinds() {
    let result = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "system_artifacts": ["system-weights"],
            "evaluators": [
                { "id": "reader-a", "kind": "expert_review", "inputs": ["report-77"] },
                { "id": "reader-b", "kind": "expert_review", "inputs": ["report-77"] },
                { "id": "imaging", "kind": "expert_review", "inputs": ["mri-4"] },
                { "id": "molecular", "kind": "executable_analysis", "inputs": ["panel-9"] },
                { "id": "silent", "kind": "statistical_reference", "inputs": ["reference-3"] }
            ],
            "verdicts": [
                { "evaluator": "reader-a", "position": "progression" },
                { "evaluator": "reader-b", "position": "treatment-effect" },
                { "evaluator": "imaging", "position": "progression" },
                { "evaluator": "molecular", "position": "pseudoprogression" },
                { "evaluator": "silent", "position": "", "abstained": true }
            ],
            "expected": "progression",
            "max_items": 3
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-mesh-audit/0.1")
    );
    assert_eq!(result["mesh"]["evaluator_count"], json!(5));
    assert_eq!(result["mesh"]["independent_class_count"], json!(4));
    assert_eq!(result["mesh"]["independence_verified"], json!(true));
    assert_eq!(result["disagreements"]["within_class_count"], json!(1));
    assert_eq!(result["disagreements"]["across_class_count"], json!(4));
    assert_eq!(
        result["findings"]["abstaining_evaluators"]["ids"],
        json!(["silent"])
    );
    assert_eq!(result["independent_ratings"]["status"], json!("refused"));
    assert_eq!(result["findings"]["rating_projection_refused"], json!(true));
    assert_eq!(result["contributions"]["status"], json!("accepted"));
    assert!(result["contributions"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["conclusion"] == json!("unknown")));
    assert_eq!(result["classes"]["rows"][0]["size"], json!(2));

    let circular_refusal = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "system_artifacts": ["system-weights"],
            "evaluators": [{ "id": "distilled", "kind": "calibrated_model_judge", "inputs": ["answer"], "derived_from": ["system-weights"] }]
        }),
    );
    assert_eq!(circular_refusal["ok"], json!(false));
    assert_eq!(circular_refusal["stage"], json!("evaluator_admission"));
    assert_eq!(circular_refusal["fail_closed"], json!(true));

    let independence_refusal = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "evaluators": [{ "id": "silent", "kind": "expert_review", "inputs": [] }],
            "require_independence": true
        }),
    );
    assert_eq!(independence_refusal["ok"], json!(false));
    assert_eq!(independence_refusal["stage"], json!("independence_policy"));
}

#[test]
fn bioeval_burden_audit_preserves_inherited_residuals_waste_and_fork_refusals() {
    let result = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [
                { "id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL" },
                { "id": "compute", "class": "compute_and_money", "initial": 10, "unit": "hour" }
            ],
            "branches": [
                { "id": "candidate-a", "parent": "root" },
                { "id": "candidate-b", "parent": "root" }
            ],
            "draws": [
                { "branch": "root", "action": "extract", "resource": "biopsy", "amount": 30, "unit": "uL", "outcome": "wasted", "destructive": true },
                { "branch": "candidate-a", "action": "sequence-a", "resource": "biopsy", "amount": 60, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "candidate-b", "action": "sequence-b", "resource": "biopsy", "amount": 60, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "candidate-a", "action": "retry", "resource": "compute", "amount": 2, "unit": "hour", "outcome": "wasted", "destructive": false }
            ],
            "inspect_branches": ["root", "candidate-a", "candidate-b"],
            "joint_branches": ["candidate-a", "candidate-b"],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-burden-audit/0.1")
    );
    assert_eq!(result["burden"]["resource_count"], json!(2));
    assert_eq!(result["burden"]["branch_count"], json!(3));
    assert_eq!(result["burden"]["draw_count"], json!(4));
    assert_eq!(result["joint_feasibility"]["status"], json!("refused"));
    assert_eq!(result["findings"]["joint_feasibility_refused"], json!(true));
    assert_eq!(result["wasted_nonrenewable"]["total"], json!(1));
    assert_eq!(result["findings"]["failed_draws_still_counted"], json!(2));
    assert_eq!(
        result["branches"]["rows"][1]["residual"]["biopsy"],
        json!(10)
    );

    let joint_policy_refusal = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [{ "id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL" }],
            "branches": [{ "id": "a" }, { "id": "b" }],
            "draws": [
                { "branch": "a", "action": "a", "resource": "biopsy", "amount": 80, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "b", "action": "b", "resource": "biopsy", "amount": 80, "unit": "uL", "outcome": "productive", "destructive": true }
            ],
            "joint_branches": ["a", "b"],
            "require_joint_feasible": true
        }),
    );
    assert_eq!(joint_policy_refusal["ok"], json!(false));
    assert_eq!(
        joint_policy_refusal["stage"],
        json!("joint_feasibility_policy")
    );
    assert_eq!(joint_policy_refusal["fail_closed"], json!(true));

    let unit_refusal = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [{ "id": "biopsy", "class": "tissue_aliquot", "initial": 10, "unit": "uL" }],
            "draws": [{ "branch": "root", "action": "bad-unit", "resource": "biopsy", "amount": 1, "unit": "mL", "outcome": "productive", "destructive": true }]
        }),
    );
    assert_eq!(unit_refusal["ok"], json!(false));
    assert_eq!(unit_refusal["stage"], json!("draw_admission"));
}

#[test]
fn bioeval_reveal_audit_freezes_rubric_and_retains_unrevealed_commitments() {
    let result = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [
                { "target": "case-a", "prediction": { "class": "stable" }, "analysis_plan": "plan-v1" },
                { "target": "case-b", "prediction": { "class": "progression" }, "analysis_plan": "plan-v1" }
            ],
            "rubric": { "version": 1, "rules": ["predeclared"] },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "case-a", "observed": { "class": "stable" } }],
            "score_rubric": { "version": 1, "rules": ["predeclared"] }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-reveal-audit/0.1")
    );
    assert_eq!(result["commitments"]["total"], json!(2));
    assert_eq!(result["outcomes"]["total"], json!(1));
    assert_eq!(result["scoring"]["status"], json!("accepted"));
    assert_eq!(result["scoring"]["complete"], json!(false));
    assert_eq!(result["findings"]["selective_publication"], json!(true));
    assert_eq!(
        result["findings"]["unrevealed_commitments"]["ids"],
        json!(["case-b"])
    );
    assert_eq!(result["seal_lock"]["status"], json!("refused"));
    assert_eq!(result["reveal_lock"]["status"], json!("refused"));

    let rubric_refusal = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [{ "target": "case-a", "prediction": "stable", "analysis_plan": "plan-v1" }],
            "rubric": { "version": 1 },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "case-a", "observed": "stable" }],
            "score_rubric": { "version": 2 },
            "require_rubric_match": true
        }),
    );
    assert_eq!(rubric_refusal["ok"], json!(false));
    assert_eq!(rubric_refusal["stage"], json!("rubric_integrity_policy"));
    assert_eq!(rubric_refusal["fail_closed"], json!(true));

    let uncommitted = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [{ "target": "case-a", "prediction": "stable", "analysis_plan": "plan-v1" }],
            "rubric": { "version": 1 },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "new-case", "observed": "stable" }],
            "score_rubric": { "version": 1 }
        }),
    );
    assert_eq!(uncommitted["ok"], json!(true));
    assert_eq!(uncommitted["scoring"]["status"], json!("refused"));
    assert_eq!(
        uncommitted["findings"]["uncommitted_outcome_refused"],
        json!(true)
    );
}

#[test]
fn bioeval_boundary_audit_separates_authorization_denial_violations_vetoes_and_bypass() {
    let result = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "policies": [{
                "id": "consent-study",
                "recipient": "evaluator",
                "information_type": "deidentified",
                "purpose": "study",
                "transmission_principle": "consent",
                "channels": ["inter_agent_messages"]
            }],
            "flows": [
                {
                    "id": "authorized",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "evaluator",
                    "information_type": "deidentified",
                    "purpose": "study",
                    "transmission_principle": "consent",
                    "channel": "inter_agent_messages",
                    "effect": { "effect": "materialized" },
                    "irreversible": false
                },
                {
                    "id": "respected-denial",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "vendor",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "external_queries",
                    "effect": { "effect": "proposed", "denied_by": "policy-deny" },
                    "irreversible": false
                },
                {
                    "id": "materialized-violation",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "vendor",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "external_queries",
                    "effect": { "effect": "materialized" },
                    "irreversible": false
                },
                {
                    "id": "irreversible-veto",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "public",
                    "information_type": "identifier",
                    "purpose": "publication",
                    "transmission_principle": "none",
                    "channel": "final_output",
                    "effect": { "effect": "materialized" },
                    "irreversible": true
                },
                {
                    "id": "bypass",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "logger",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "logs",
                    "effect": { "effect": "bypass_attempted", "detail": "used an alternate path" },
                    "irreversible": false
                }
            ],
            "utility": 0.8,
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-boundary-audit/0.1")
    );
    assert_eq!(result["boundary"]["authorised_count"], json!(1));
    assert_eq!(result["boundary"]["compliant_count"], json!(1));
    assert_eq!(result["boundary"]["violation_count"], json!(3));
    assert_eq!(result["boundary"]["veto_count"], json!(2));
    assert_eq!(
        result["findings"]["compliant_proposals"]["ids"],
        json!(["respected-denial"])
    );
    assert_eq!(result["composite"]["status"], json!("refused"));
    assert_eq!(result["findings"]["bypass_is_veto"], json!(true));

    let veto_policy_refusal = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "flows": [{
                "id": "veto",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "public",
                "information_type": "identifier",
                "purpose": "publication",
                "transmission_principle": "none",
                "channel": "final_output",
                "effect": { "effect": "materialized" },
                "irreversible": true
            }],
            "require_no_vetoes": true
        }),
    );
    assert_eq!(veto_policy_refusal["ok"], json!(false));
    assert_eq!(veto_policy_refusal["stage"], json!("veto_policy"));

    let missing_principle = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "flows": [{
                "id": "missing",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "vendor",
                "information_type": "identifier",
                "purpose": "debug",
                "transmission_principle": "",
                "channel": "logs",
                "effect": { "effect": "materialized" },
                "irreversible": false
            }]
        }),
    );
    assert_eq!(missing_principle["ok"], json!(false));
    assert_eq!(missing_principle["stage"], json!("flow_assessment"));
}

#[test]
fn evaluation_worldline_audit_separates_future_leakage_from_dangling_context() {
    let stamp = |value: &str| Timestamp::parse(value).unwrap();
    let mut worldline = Worldline::new();
    worldline
        .observe(
            EvalObservation::new(
                "early",
                stamp("2026-01-01T00:00:00Z"),
                stamp("2026-01-02T00:00:00Z"),
                stamp("2026-01-03T00:00:00Z"),
                stamp("2026-01-04T00:00:00Z"),
            )
            .unwrap(),
        )
        .unwrap();
    worldline
        .observe(
            EvalObservation::new(
                "future",
                stamp("2026-01-05T00:00:00Z"),
                stamp("2026-01-06T00:00:00Z"),
                stamp("2026-01-07T00:00:00Z"),
                stamp("2026-01-10T00:00:00Z"),
            )
            .unwrap(),
        )
        .unwrap();
    worldline.decide(Decision {
        id: "decision-1".into(),
        at: stamp("2026-01-08T00:00:00Z"),
        context: vec!["early".into(), "future".into(), "missing".into()],
    });

    let result = call(
        &mut server(),
        "evaluation_worldline_audit",
        json!({
            "worldline": serde_json::to_value(worldline).unwrap(),
            "at": "2026-01-08T00:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-worldline-audit/0.1")
    );
    assert_eq!(result["leak_count"], json!(1));
    assert_eq!(result["dangling_count"], json!(1));
    assert_eq!(result["leaks"][0]["observation"], json!("future"));
    assert_eq!(result["dangling_references"][0][1], json!("missing"));
    assert_eq!(result["admissible_at"][0], json!("early"));
}

#[test]
fn evaluation_reproduction_check_keeps_divergence_and_validity_refusal_visible() {
    let mut reexecution = Reexecution::declaring(
        "workflow-1",
        true,
        vec![
            OutputSpec::exact("digest"),
            OutputSpec::numeric("score", 0.1).unwrap(),
        ],
    )
    .unwrap();
    reexecution
        .observe(
            "digest",
            Observed::Digests {
                original: "abc".into(),
                rerun: "abc".into(),
            },
        )
        .unwrap();
    reexecution
        .observe(
            "score",
            Observed::Numbers {
                original: 1.0,
                rerun: 1.5,
            },
        )
        .unwrap();

    let result = call(
        &mut server(),
        "evaluation_reproduction_check",
        json!({
            "reexecution": serde_json::to_value(reexecution).unwrap(),
            "biological_claim": "the treatment is effective"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-reproduction-check/0.1")
    );
    assert_eq!(result["reproduced"], json!(false));
    assert_eq!(result["verdict_count"], json!(2));
    assert_eq!(result["matched_count"], json!(1));
    assert_eq!(result["diverged_count"], json!(1));
    assert_eq!(result["missing_count"], json!(0));
    assert_eq!(result["verdicts"][1]["output"], json!("score"));
    assert_eq!(result["verdicts"][1]["verdict"], json!("diverged"));
    assert_eq!(result["first_divergence"]["output"], json!("score"));
    assert_eq!(result["validity_claim"]["ok"], json!(false));
    assert_eq!(result["portability_demonstrated"], json!(false));
}

#[test]
fn evaluation_trajectory_check_reports_vacuity_and_bounded_suffix_separately() {
    let mut trajectory = Trajectory::of(vec![
        Step::new("edit").irreversible(),
        Step::new("verify").at_distance(2.0),
        Step::new("finish").at_distance(1.0),
    ]);
    trajectory
        .require(PathProperty::PrecededBy {
            before: "edit".into(),
            after: "inspect".into(),
        })
        .unwrap();

    let result = call(
        &mut server(),
        "evaluation_trajectory_check",
        json!({
            "trajectory": serde_json::to_value(trajectory).unwrap(),
            "step": 0,
            "horizon": 2
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-trajectory-check/0.1")
    );
    assert_eq!(result["step_records"].as_array().unwrap().len(), 3);
    assert_eq!(result["property_count"], json!(1));
    assert_eq!(result["violated_count"], json!(1));
    assert_eq!(result["vacuous_count"], json!(0));
    assert_eq!(result["property_outcomes"][0]["held"], json!(false));
    assert_eq!(result["recovery_count"], json!(0));
    assert_eq!(result["property_outcomes"][0]["violations"][0], json!(0));
    assert_eq!(result["bounded_suffix"]["complete"], json!(true));
    assert_eq!(result["bounded_suffix"]["value"]["downstream"], json!(1.0));
}

#[test]
fn runtime_effect_check_is_deny_by_default_and_never_executes() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileRead])
        .allowing_path("/work/");
    let allowed = call(
        &mut server(),
        "runtime_effect_check",
        json!({
            "policy": serde_json::to_value(&policy).unwrap(),
            "request": EffectRequest::FileRead { path: "/work/data.txt".into() }
        }),
    );
    assert_eq!(allowed["ok"], json!(true));
    assert_eq!(allowed["authorization"], json!("perform"));

    let denied = call(
        &mut server(),
        "runtime_effect_check",
        json!({
            "policy": serde_json::to_value(policy).unwrap(),
            "request": EffectRequest::FileRead { path: "/etc/passwd".into() }
        }),
    );
    assert_eq!(denied["ok"], json!(false));
    assert_eq!(denied["fail_closed"], json!(true));
    assert!(denied["refusal"].as_str().unwrap().contains("path"));
}

#[test]
fn runtime_tape_verify_accepts_only_verified_chain_state() {
    let tape = WorldTape::new(RunId::parse("run-mcp").unwrap());
    let result = call(
        &mut server(),
        "runtime_tape_verify",
        json!({ "tape": serde_json::to_value(tape).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-tape-verify/0.1")
    );
    assert_eq!(result["chain_verified"], json!(true));
    assert_eq!(result["entries"], json!(0));
    assert_eq!(result["checkpoint_count"], json!(0));
    assert_eq!(result["artifact_consumed_count"], json!(0));
    assert_eq!(result["artifact_created_count"], json!(0));
    assert_eq!(result["simulated_steps"].as_array().unwrap().len(), 0);
    assert_eq!(result["simulated_step_count"], json!(0));
}

#[test]
fn runtime_execution_simulate_records_replays_and_forks_without_live_effects() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::ClockNow,
            EffectKind::RandomBytes,
            EffectKind::FileRead,
            EffectKind::FileWrite,
        ])
        .allowing_path("/work/");
    let budget = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(4));
    let result = call(
        &mut server(),
        "runtime_execution_simulate",
        json!({
            "run": "run-execution-mcp",
            "policy": serde_json::to_value(policy).unwrap(),
            "requests": [
                { "kind": "clock_now" },
                { "kind": "file_read", "path": "/work/input.txt" },
                { "kind": "random_bytes", "count": 4 },
                { "kind": "file_write", "path": "/work/output.txt", "content": "done" }
            ],
            "world": {
                "seed": 7,
                "clock_start": 10,
                "clock_tick": 2,
                "base_files": { "/work/input.txt": "fixture" }
            },
            "budget": serde_json::to_value(budget).unwrap(),
            "fork": {
                "step": 2,
                "run": "run-execution-child",
                "requests": [{ "kind": "clock_now" }]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-execution-simulate/0.1")
    );
    assert_eq!(result["recorded_requests"], json!(4));
    assert_eq!(result["recording_complete"], json!(true));
    assert_eq!(result["partial_recording"], json!(false));
    assert_eq!(result["live_outcome_count"], json!(4));
    assert!(result["policy_journal_count"].as_u64().unwrap() >= 4);
    assert_eq!(result["replay"]["verified"], json!(true));
    assert_eq!(result["replay"]["matched"], json!(true));
    assert_eq!(result["replay_complete"], json!(true));
    assert_eq!(result["world"]["calls"], json!(4));
    assert_eq!(
        result["world"]["file_changes"][0]["path"],
        json!("/work/output.txt")
    );
    assert_eq!(result["fork"]["ok"], json!(true));
    assert_eq!(result["fork"]["inherited_steps"], json!(2));
    assert_eq!(result["fork"]["observed_state"]["fork_step"], json!(2));
    assert!(result["fork"]["child_tape"]["lineage"].is_object());
}

#[test]
fn runtime_execution_simulate_reports_budget_exhaustion_and_keeps_partial_replay_explicit() {
    let policy = EffectPolicy::evaluation_default().declaring([EffectKind::ClockNow]);
    let budget = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(1));
    let result = call(
        &mut server(),
        "runtime_execution_simulate",
        json!({
            "policy": serde_json::to_value(policy).unwrap(),
            "requests": [{ "kind": "clock_now" }, { "kind": "clock_now" }],
            "budget": serde_json::to_value(budget).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-execution-simulate/0.1")
    );
    assert!(result["execution_error"]
        .as_str()
        .unwrap()
        .contains("budget exhausted"));
    assert_eq!(result["recorded_requests"], json!(1));
    assert_eq!(result["recording_complete"], json!(false));
    assert_eq!(result["partial_recording"], json!(true));
    assert_eq!(result["replay"]["verified"], json!(true));
    assert_eq!(result["budget"]["aborted_on"], json!("tool_calls"));
}

#[test]
fn megafactory_twin_audit_requires_discrepancy_stable_direction_for_oracle_status() {
    let model = json!({
        "id": "reference",
        "compartments": ["a", "b", "c"],
        "rates": [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        "known_misspecification": "linear transfer has no saturation"
    });
    let stable = call(
        &mut server(),
        "megafactory_twin_audit",
        json!({
            "reference": model,
            "alternatives": [
                {
                    "id": "faster",
                    "compartments": ["a", "b", "c"],
                    "rates": [[0.0, 0.0, 0.5], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                    "known_misspecification": "linear transfer has no saturation"
                },
                {
                    "id": "slower",
                    "compartments": ["a", "b", "c"],
                    "rates": [[0.0, 0.0, 0.1], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                    "known_misspecification": "linear transfer has no saturation"
                }
            ],
            "initial": [1.0, 5.0, 0.0],
            "steps": 3,
            "intervention": { "compartment": "a", "hold_at": 1.0 },
            "outcome_compartment": "c"
        }),
    );
    assert_eq!(stable["ok"], json!(true));
    assert_eq!(stable["oracle_eligible"], json!(true));
    assert_eq!(stable["probe"]["models_disagreeing"], json!([]));

    let unstable = call(
        &mut server(),
        "megafactory_twin_audit",
        json!({
            "reference": {
                "id": "reference",
                "compartments": ["a", "b", "c"],
                "rates": [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                "known_misspecification": "linear transfer has no saturation"
            },
            "alternatives": [{
                "id": "refilled",
                "compartments": ["a", "b", "c"],
                "rates": [[0.0, 0.0, 0.3], [0.9, 0.0, 0.0], [0.0, 0.0, 0.0]],
                "known_misspecification": "the refill compartment is treated as a source"
            }],
            "initial": [1.0, 5.0, 0.0],
            "steps": 3,
            "intervention": { "compartment": "a", "hold_at": 1.0 },
            "outcome_compartment": "c"
        }),
    );
    assert_eq!(unstable["ok"], json!(true));
    assert_eq!(unstable["oracle_eligible"], json!(false));
    assert!(unstable["headline"]
        .as_str()
        .unwrap()
        .contains("not benchmark ground truth"));
}

#[test]
fn megafactory_placement_audit_exposes_transfer_fencing_and_non_idempotent_duplicates() {
    let job = Job::new(
        "job-placement-mcp",
        ResourceClass::Evaluate,
        Idempotency::NonIdempotent,
        json!({ "suite": "release" }),
    );
    let worker = WorkerProfile::new(
        WorkerCapability::new("worker-mcp", vec![ResourceClass::Evaluate]),
        TrustDomain::new("worker-domain"),
        Locale::new("us"),
        Attestation::Attested {
            measurement: ContentHash::of_bytes(b"worker-image"),
            vouched_by: "attestor".into(),
        },
    );
    let request = WorkRequest {
        data_locale: Locale::new("eu"),
        access_tier: PlacementAccessTier::Restricted,
        oracle_domain: TrustDomain::new("oracle-domain"),
        input_bytes: 4096,
    };
    let result = call(
        &mut server(),
        "megafactory_placement_audit",
        json!({
            "job": serde_json::to_value(job).unwrap(),
            "request": serde_json::to_value(request).unwrap(),
            "worker": serde_json::to_value(worker).unwrap(),
            "item": "item-1",
            "commit_count": 2,
            "supersede_fence": true
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["placement"]["data_local"], json!(false));
    assert_eq!(result["placement"]["transfer_bytes"], json!(4096));
    assert_eq!(result["fencing"]["stale_admission"]["ok"], json!(false));
    assert_eq!(result["fencing"]["current_admission"]["ok"], json!(true));
    assert_eq!(
        result["ledger"]["duplicates"]["repeated_effect_incidents"],
        json!(1)
    );
    assert_eq!(result["ledger"]["has_incidents"], json!(true));
}

#[test]
fn onco_boundary_check_releases_aggregate_work_and_escalates_individual_use() {
    let request = BoundaryRequest {
        purpose: "compare cohort response rates".into(),
        context: RequestContext::Research,
        claimed_role: "attending physician".into(),
        claimed_urgency: true,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: vec![
            OutputUse::CohortAnalysis,
            OutputUse::TreatmentRecommendation,
        ],
        direct_identifier_fields: Vec::new(),
    };
    let result = call(
        &mut server(),
        "onco_boundary_check",
        json!({ "request": serde_json::to_value(request).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-boundary-check/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("disposition"));
    assert_eq!(result["disposition_kind"], json!("release_partial"));
    assert_eq!(result["released"][0], json!("cohort_analysis"));
    assert_eq!(result["refused"][0], json!("treatment_recommendation"));
    assert_eq!(result["terminal_action"], json!("escalate"));
    assert_eq!(result["requested_use_count"], json!(2));
    assert_eq!(result["released_count"], json!(1));
    assert_eq!(result["refused_count"], json!(1));
    assert_eq!(result["escalation_present"], json!(true));
    assert_eq!(
        result["escalation_trigger"],
        json!("individual_clinical_request")
    );
    assert_eq!(result["escalation_route"], json!("treating_clinical_team"));
    assert_eq!(result["identifier_fields_present"], json!(false));

    let identifiers = BoundaryRequest {
        purpose: "research".into(),
        context: RequestContext::Research,
        claimed_role: "analyst".into(),
        claimed_urgency: false,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: vec![OutputUse::CohortAnalysis],
        direct_identifier_fields: vec!["name".into()],
    };
    let refused = call(
        &mut server(),
        "onco_boundary_check",
        json!({ "request": serde_json::to_value(identifiers).unwrap() }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/onco-boundary-check/0.1")
    );
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("identifiers_present"));
    assert_eq!(refused["requested_use_count"], json!(1));
    assert_eq!(refused["identifier_fields_present"], json!(true));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn onco_response_assess_withholds_post_treatment_progression() {
    let timestamp = |value: &str| AcquisitionTime::new(Timestamp::parse(value).unwrap());
    let lesion = |longest: f64, perpendicular: f64| {
        TargetLesion::new("target", longest, perpendicular).unwrap()
    };
    let scan = |lesions| ImagingObservation {
        modality: ImagingModality::MriT1PostContrast,
        compartment: Compartment::ContrastEnhancing,
        target_lesions: lesions,
        new_lesion: OncoObserved::Value(false),
        nonmeasurable_change: OncoObserved::Value(DirectionOfChange::Unchanged),
        comparable_to_baseline: true,
    };
    let clinical = ClinicalObservation {
        corticosteroid_dexamethasone_equivalent_mg_per_day: OncoObserved::Value(0.0),
        performance_status: OncoObserved::Value(Karnofsky::new(100).unwrap()),
        trend: OncoObserved::Value(ClinicalTrend::Stable),
    };
    let result = call(
        &mut server(),
        "onco_response_assess",
        json!({
            "criterion": serde_json::to_value(ResponseCriterion::high_grade_2010()).unwrap(),
            "baseline": serde_json::to_value(scan(vec![lesion(10.0, 10.0)])).unwrap(),
            "current": serde_json::to_value(scan(vec![lesion(13.0, 10.0)])).unwrap(),
            "current_acquired": "2026-02-01T00:00:00Z",
            "baseline_clinical": serde_json::to_value(&clinical).unwrap(),
            "current_clinical": serde_json::to_value(&clinical).unwrap(),
            "treatment": serde_json::to_value(TreatmentContext { modality: TreatmentModality::Radiotherapy, completed: timestamp("2026-01-01T00:00:00Z") }).unwrap(),
            "evidence": serde_json::to_value(ProgressionEvidence::default()).unwrap(),
            "measurement_error_fraction": 0.0
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-response-assess/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("assessment"));
    assert_eq!(result["call_kind"], json!("not_evaluable"));
    assert_eq!(result["unconfirmed_reading"], json!("progression"));
    assert_eq!(result["post_treatment_window_days"], json!(84));
    assert_eq!(result["pseudoresponse_possible"], json!(false));
    assert_eq!(result["criterion_divergence_present"], json!(true));
    assert_eq!(result["sensitivity_flips"], json!(false));
    assert_eq!(result["hypothesis_non_identifiable"], json!(true));
    assert_eq!(
        result["assessment"]["unconfirmed_reading"],
        json!("progression")
    );
    assert_eq!(result["call_label"], json!("not evaluable"));
    assert_eq!(result["withheld_progression"], json!(true));
    assert!(result["hypothesis_count"].as_u64().unwrap() >= 2);
}

#[test]
fn onco_worldline_view_separates_biological_record_and_visibility_orders() {
    let timestamp = |value: &str| Timestamp::parse(value).unwrap();
    let timepoint = |label: &str, acquired: &str, recorded: &str, released: &str, visible: &str| {
        Timepoint::new(
            label,
            Clocks {
                acquired: AcquisitionTime::new(timestamp(acquired)),
                recorded: bioprism_onco::RecordTime::new(timestamp(recorded)),
                released: bioprism_onco::ReleaseTime::new(timestamp(released)),
                visible: AvailabilityTime::new(timestamp(visible)),
            },
            OncoObservation::Molecular(MarkerPanel::nothing_collected()),
        )
        .unwrap()
    };
    let baseline = timepoint(
        "baseline",
        "2026-01-01T00:00:00Z",
        "2026-01-10T00:00:00Z",
        "2026-01-11T00:00:00Z",
        "2026-01-11T00:00:00Z",
    );
    let future = timepoint(
        "future",
        "2026-01-05T00:00:00Z",
        "2026-01-06T00:00:00Z",
        "2026-01-07T00:00:00Z",
        "2026-01-07T00:00:00Z",
    );
    let mut worldline = TumourWorldline::new(SubjectRef::new("S-1").unwrap(), baseline);
    worldline.push(future).unwrap();

    let result = call(
        &mut server(),
        "onco_worldline_view",
        json!({
            "worldline": serde_json::to_value(worldline).unwrap(),
            "visible_at": "2026-01-10T12:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-worldline-view/0.1")
    );
    assert_eq!(result["biological_order"], json!(["baseline", "future"]));
    assert_eq!(result["record_order"], json!(["future", "baseline"]));
    assert_eq!(result["record_order_differs"], json!(true));
    assert_eq!(
        result["clock_axes"],
        json!(["acquired", "recorded", "released", "visible"])
    );
    assert_eq!(result["clock_order_guaranteed"], json!(true));
    assert_eq!(result["visible_timepoints"], json!(["future"]));
    assert_eq!(result["hidden_from_agent"], json!(["baseline"]));
    assert_eq!(result["visible_count"], json!(1));
    assert_eq!(result["hidden_count"], json!(1));
    assert_eq!(result["timepoints"][0]["biological_index"], json!(0));
    assert_eq!(result["timepoints"][0]["record_index"], json!(1));
    assert_eq!(
        result["timepoints"][0]["visibility_state"],
        json!("hidden_from_agent")
    );
    assert_eq!(result["timepoints"][0]["visible_at_cutoff"], json!(false));
    assert_eq!(result["timepoints"][1]["biological_index"], json!(1));
    assert_eq!(result["timepoints"][1]["record_index"], json!(0));
    assert_eq!(
        result["timepoints"][1]["visibility_state"],
        json!("visible")
    );
    assert_eq!(result["timepoints"][1]["visible_at_cutoff"], json!(true));
    assert_eq!(result["visibility_partition"]["visible_count"], json!(1));
    assert_eq!(result["visibility_partition"]["hidden_count"], json!(1));
    assert_eq!(
        result["timepoints"][0]["clocks"]["acquired"],
        json!("2026-01-01T00:00:00Z")
    );
    assert_eq!(result["timepoints"][1]["days_from_baseline"], json!(4));
}

#[test]
fn onco_classification_check_preserves_unresolved_obligations_and_integrated_calls() {
    let unresolved = call(
        &mut server(),
        "onco_classification_check",
        json!({
            "histology": "diffuse_glioma",
            "panel": serde_json::to_value(MarkerPanel::nothing_collected()).unwrap()
        }),
    );
    assert_eq!(unresolved["ok"], json!(true));
    assert_eq!(
        unresolved["schema"],
        json!("bioprism-mcp/onco-classification-check/0.1")
    );
    assert_eq!(unresolved["is_integrated"], json!(false));
    assert_eq!(unresolved["resolution_kind"], json!("unresolved"));
    assert_eq!(unresolved["panel_state_count"], json!(0));
    assert_eq!(unresolved["observed_panel_state_count"], json!(0));
    assert_eq!(unresolved["unobserved_panel_state_count"], json!(0));
    assert_eq!(unresolved["resolution"]["resolution"], json!("unresolved"));
    assert!(!unresolved["obligations"].as_array().unwrap().is_empty());

    let integrated = MarkerPanel::nothing_collected()
        .observed(MolecularMarker::IdhMutation, MarkerCall::Present)
        .observed(MolecularMarker::Codeletion1p19q, MarkerCall::Present);
    let result = call(
        &mut server(),
        "onco_classification_check",
        json!({
            "histology": serde_json::to_value(Histology::DiffuseGlioma).unwrap(),
            "panel": serde_json::to_value(integrated).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["is_integrated"], json!(true));
    assert_eq!(result["resolution_kind"], json!("integrated"));
    assert_eq!(result["obligation_count"], json!(0));
    assert_eq!(result["observed_panel_state_count"], json!(2));
    assert_eq!(result["unobserved_panel_state_count"], json!(0));
    assert_eq!(
        result["entity"],
        json!("oligodendroglioma_idh_mutant1p19q_codeleted")
    );
    assert!(result["obligations"].as_array().unwrap().is_empty());
}

#[test]
fn oncoworlds_identity_join_returns_typed_refusals_and_accepts_a_warranted_bridge() {
    let left = OncoArtifact::new("left", Pseudonym::new("P-1"), DiseaseEpoch::Preoperative)
        .at(ArtifactLevel::Encounter, Pseudonym::new("E-1"))
        .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let right = OncoArtifact::new("right", Pseudonym::new("P-2"), DiseaseEpoch::Preoperative)
        .at(ArtifactLevel::Encounter, Pseudonym::new("E-2"))
        .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let refused = call(
        &mut server(),
        "oncoworlds_identity_join",
        json!({
            "left": serde_json::to_value(&left).unwrap(),
            "right": serde_json::to_value(&right).unwrap(),
            "unit": "specimen"
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-identity-join/0.1")
    );
    assert_eq!(refused["joinable"], json!(false));
    assert_eq!(refused["verdict_kind"], json!("declined"));
    assert_eq!(refused["refusal_kind"], json!("no_identity_evidence"));
    assert_eq!(refused["identity_evidence_present"], json!(false));
    assert_eq!(refused["identity_link_count"], json!(0));
    assert_eq!(refused["bridge_declared"], json!(false));
    assert_eq!(refused["epoch_bridge"], json!(null));
    assert_eq!(refused["bridge_warrant_present"], json!(false));
    assert!(refused["checked_dimensions"].as_array().unwrap().len() >= 8);
    assert_eq!(
        refused["report"]["verdict"]["reason"]["refusal"],
        json!("no_identity_evidence")
    );

    let same_participant_left =
        OncoArtifact::new("pre", Pseudonym::new("P-1"), DiseaseEpoch::Preoperative)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E-1"))
            .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let same_participant_right =
        OncoArtifact::new("post", Pseudonym::new("P-1"), DiseaseEpoch::Postoperative)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E-3"))
            .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let bridge = bioprism_oncoworlds::EpochBridge {
        from: DiseaseEpoch::Preoperative,
        to: DiseaseEpoch::Postoperative,
        warrant: "paired longitudinal sampling plan".into(),
    };
    let accepted = call(
        &mut server(),
        "oncoworlds_identity_join",
        json!({
            "left": serde_json::to_value(same_participant_left).unwrap(),
            "right": serde_json::to_value(same_participant_right).unwrap(),
            "unit": "specimen",
            "epoch_bridge": serde_json::to_value(bridge).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["joinable"], json!(true));
    assert_eq!(accepted["verdict_kind"], json!("joinable"));
    assert_eq!(accepted["refusal_kind"], json!(null));
    assert_eq!(accepted["bridge_declared"], json!(true));
    assert!(accepted["epoch_bridge"].is_object());
    assert_eq!(accepted["bridge_warrant_present"], json!(true));
}

#[test]
fn oncoworlds_model_transport_keeps_model_and_patient_claims_separate() {
    let result = ModelResult::new(
        ModelIdentity::new("ORG-1", ModelSystem::Organoid, "S-1", 3).verified(),
        "the compound reduced viability",
        ReplicateStructure {
            technical_wells: 6,
            biological_replicates: 3,
        },
    )
    .resting_on(FidelityAxis::Genomic);
    let fidelity = FidelityEvidence::new().measured(FidelityAxis::Genomic, 3);
    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S-1"),
        ScopeKey::new().exact("patient", "P-1"),
        "an ex vivo effect is transported to a bounded patient-relevant research claim",
    )
    .losing("microenvironment and immune compartment")
    .adding_uncertainty("passage and establishment selection");
    for assumption in bioprism_oncoworlds::models::REQUIRED_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "declared by the study protocol");
    }

    let accepted = call(
        &mut server(),
        "oncoworlds_model_transport",
        json!({
            "result": serde_json::to_value(&result).unwrap(),
            "fidelity": serde_json::to_value(fidelity).unwrap(),
            "establishment": serde_json::to_value(EstablishmentCohort::new(3, 3)).unwrap(),
            "claimed_n": 3,
            "transport": serde_json::to_value(&transport).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(
        accepted["schema"],
        json!("bioprism-mcp/oncoworlds-model-transport/0.1")
    );
    assert_eq!(accepted["supported"], json!(true));
    assert_eq!(accepted["outcome_kind"], json!("supported"));
    assert_eq!(
        accepted["model_identity"]["verified_against_source"],
        json!(true)
    );
    assert_eq!(accepted["fidelity_axes"][0]["axis"], json!("genomic"));
    assert_eq!(accepted["establishment"]["selected"], json!(false));
    assert_eq!(accepted["replicates"]["effective_biological_n"], json!(3));
    assert_eq!(accepted["replicates"]["claimed_n"], json!(3));
    assert_eq!(
        accepted["transport_assumption_names"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(accepted["effective_biological_n"], json!(3));
    assert!(accepted["patient_relevant_claim"].is_object());
    assert!(accepted["model_statement"]
        .as_str()
        .unwrap()
        .contains("organoid"));

    let refused = call(
        &mut server(),
        "oncoworlds_model_transport",
        json!({
            "result": serde_json::to_value(ModelResult::new(
                ModelIdentity::new("ORG-unverified", ModelSystem::Organoid, "S-1", 3),
                "effect",
                ReplicateStructure { technical_wells: 1, biological_replicates: 1 }
            )).unwrap(),
            "establishment": serde_json::to_value(EstablishmentCohort::new(1, 1)).unwrap(),
            "claimed_n": 1,
            "transport": serde_json::to_value(transport).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-model-transport/0.1")
    );
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("unverified_model_identity"));
    assert_eq!(
        refused["model_identity"]["verified_against_source"],
        json!(false)
    );
    assert_eq!(
        refused["refusal"]["refusal"],
        json!("unverified_model_identity")
    );
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn oncoworlds_methylation_tools_preserve_threshold_and_version_conditioning() {
    let threshold = ScoreValue::from_parts_per_ten_thousand(7_000).unwrap();
    let calibrated = RawScore(ScoreValue::from_parts_per_ten_thousand(8_500).unwrap())
        .calibrate(&Calibration::new("isotonic", "cal-1"));
    let classifier =
        ClassifierVersion::new("methylation-demo", "v1", "ref-1").reporting_at(threshold);
    let context = SampleContext::new(
        QcOutcome::Passed,
        OncoObserved::Unobserved(ObservationStatus::NotCollected),
    );
    let scores =
        std::collections::BTreeMap::from([(MethylationClass::new("class-a"), calibrated.clone())]);
    let classified = call(
        &mut server(),
        "oncoworlds_methylation_classify",
        json!({
            "classifier": serde_json::to_value(&classifier).unwrap(),
            "scores": serde_json::to_value(scores).unwrap(),
            "context": serde_json::to_value(context).unwrap()
        }),
    );
    assert_eq!(classified["ok"], json!(true));
    assert_eq!(
        classified["schema"],
        json!("bioprism-mcp/oncoworlds-methylation-classify/0.1")
    );
    assert_eq!(classified["outcome_kind"], json!("classified"));
    assert_eq!(classified["threshold_declared"], json!(true));
    assert_eq!(classified["score_count"], json!(1));
    assert_eq!(classified["score_classes"], json!(["class-a"]));
    assert_eq!(classified["nearest_present"], json!(false));
    assert_eq!(classified["caveat_count"], json!(1));
    assert_eq!(classified["classified"], json!(true));
    assert_eq!(classified["class"], json!("class-a"));
    assert!(!classified["report"]["caveats"]
        .as_array()
        .unwrap()
        .is_empty());

    let missing_threshold = call(
        &mut server(),
        "oncoworlds_methylation_classify",
        json!({
            "classifier": serde_json::to_value(ClassifierVersion::new("methylation-demo", "v2", "ref-2")).unwrap(),
            "scores": serde_json::to_value(std::collections::BTreeMap::from([(
                MethylationClass::new("class-a"), calibrated.clone()
            )])).unwrap(),
            "context": serde_json::to_value(SampleContext::new(
                QcOutcome::Passed,
                OncoObserved::Unobserved(ObservationStatus::NotCollected)
            )).unwrap()
        }),
    );
    assert_eq!(missing_threshold["ok"], json!(false));
    assert_eq!(missing_threshold["outcome_kind"], json!("refused"));
    assert_eq!(
        missing_threshold["refusal_kind"],
        json!("undeclared_threshold")
    );
    assert_eq!(missing_threshold["threshold_declared"], json!(false));
    assert_eq!(
        missing_threshold["refusal"]["refusal"],
        json!("undeclared_threshold")
    );

    let left = VersionedResult {
        classifier: classifier.clone(),
        outcome: MethylationOutcome::Classified {
            class: MethylationClass::new("class-a"),
            score: calibrated.clone(),
        },
    };
    let right = VersionedResult {
        classifier: ClassifierVersion::new("methylation-demo", "v2", "ref-2")
            .reporting_at(threshold),
        outcome: MethylationOutcome::Classified {
            class: MethylationClass::new("class-b"),
            score: calibrated,
        },
    };
    let comparison = call(
        &mut server(),
        "oncoworlds_methylation_compare",
        json!({
            "left": serde_json::to_value(left).unwrap(),
            "right": serde_json::to_value(right).unwrap()
        }),
    );
    assert_eq!(comparison["ok"], json!(true));
    assert_eq!(
        comparison["schema"],
        json!("bioprism-mcp/oncoworlds-methylation-compare/0.1")
    );
    assert_eq!(comparison["divergence_kind"], json!("version_conditioned"));
    assert_eq!(comparison["classifier_changed"], json!(true));
    assert_eq!(comparison["left_outcome_kind"], json!("classified"));
    assert_eq!(comparison["right_outcome_kind"], json!("classified"));
    assert_eq!(comparison["stable_evidence_count"], json!(0));
    assert_eq!(
        comparison["comparison"]["divergence"]["divergence"],
        json!("version_conditioned")
    );
    assert_eq!(
        comparison["comparison"]["divergence"]["under_left"],
        json!("class-a")
    );
    assert_eq!(
        comparison["comparison"]["divergence"]["under_right"],
        json!("class-b")
    );
}

#[test]
fn oncoworlds_radiogenomic_check_refuses_leaky_splits_before_claims() {
    let observation = SpecimenObservation::new(
        MolecularMarker::IdhMutation,
        SpecimenSampling::new("S-1").sampling(RegionId::new("core-1")),
        OncoObserved::Value(MarkerCall::Present),
    );
    let claim = RadiogenomicClaim {
        target: ClaimTarget::Mechanism,
        statement: "imaging predicts the molecular mechanism".into(),
    };
    let refused = call(
        &mut server(),
        "oncoworlds_radiogenomic_check",
        json!({
            "claim": serde_json::to_value(&claim).unwrap(),
            "design": serde_json::to_value(EvaluationDesign::new(SplitUnit::Image, "features-v1")).unwrap(),
            "observation": serde_json::to_value(&observation).unwrap(),
            "transport": serde_json::to_value(DeclaredTransport::new(
                ScopeKey::new().exact("specimen", "S-1"),
                ScopeKey::new().exact("patient", "P-1"),
                "cross-modal claim"
            )).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-radiogenomic-check/0.1")
    );
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["claim_target"], json!("mechanism"));
    assert_eq!(refused["design"]["split_unit"], json!("image"));
    assert_eq!(refused["design"]["mechanism_strata_present"], json!(false));
    assert_eq!(refused["refusal_kind"], json!("leaky_split"));
    assert_eq!(refused["refusal"]["refusal"], json!("leaky_split"));

    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S-1"),
        ScopeKey::new().exact("patient", "P-1"),
        "cross-modal claim with declared losses",
    )
    .losing("specimen heterogeneity and transport uncertainty");
    for assumption in bioprism_oncoworlds::radiogenomics::REQUIRED_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "declared by the evaluation protocol");
    }
    let design = EvaluationDesign::new(SplitUnit::Participant, "features-v1")
        .features_fitted_on_training_split()
        .validated_on(CohortSelection::PrespecifiedBeforeResults {
            cohort: "external-1".into(),
        })
        .stratified_by("site")
        .stratified_by("scanner");
    let accepted = call(
        &mut server(),
        "oncoworlds_radiogenomic_check",
        json!({
            "claim": serde_json::to_value(claim).unwrap(),
            "design": serde_json::to_value(design).unwrap(),
            "observation": serde_json::to_value(observation).unwrap(),
            "transport": serde_json::to_value(transport).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["supported"], json!(true));
    assert_eq!(accepted["outcome_kind"], json!("supported"));
    assert_eq!(accepted["design"]["split_unit"], json!("participant"));
    assert_eq!(
        accepted["design"]["feature_provenance"],
        json!("fitted_on_training_split_only")
    );
    assert_eq!(accepted["design"]["mechanism_strata_present"], json!(true));
    assert_eq!(accepted["claim_target"], json!("mechanism"));
    assert_eq!(
        accepted["transport_assumption_names"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(accepted["supported_claim"].is_object());
}

#[test]
fn onco_outcome_analyze_keeps_censoring_and_delayed_entry_explicit() {
    let timestamp = |value: &str| AcquisitionTime::new(Timestamp::parse(value).unwrap());
    let follow_up = FollowUp::new(
        SubjectRef::new("P-1").unwrap(),
        timestamp("2026-01-01T00:00:00Z"),
        timestamp("2026-01-11T00:00:00Z"),
        timestamp("2026-01-21T00:00:00Z"),
        TerminalFact::LostToFollowUp,
    )
    .unwrap();
    let estimand = EndpointKind::TimeToProgression.default_estimand(Population::IntentionToTreat);
    let result = call(
        &mut server(),
        "onco_outcome_analyze",
        json!({
            "follow_up": serde_json::to_value(follow_up).unwrap(),
            "estimand": serde_json::to_value(estimand).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-outcome-analyze/0.1")
    );
    assert_eq!(result["event"], json!(false));
    assert_eq!(result["censoring_reason"], json!("lost_to_follow_up"));
    assert_eq!(result["censoring_informative"], json!(true));
    assert_eq!(result["left_truncated"], json!(true));
    assert_eq!(result["at_risk_days"], json!(10));
    assert_eq!(result["immortal_time_days"], json!(10));
    assert_eq!(result["bias_count"], json!(2));
    assert_eq!(result["informative_bias_count"], json!(1));
    assert_eq!(
        result["outcome"],
        json!({"outcome": "censored", "lost_to_follow_up": null})
    );
    assert_eq!(result["analysis"]["subject"], json!("P-1"));
    assert_eq!(
        result["analysis"]["estimand"]["endpoint"],
        json!("time_to_progression")
    );
    assert_eq!(
        result["analysis"]["bias_flags"],
        json!(["left_truncation", "informative_loss_to_follow_up"])
    );
    assert_eq!(
        result["informative_bias_flags"][0],
        json!("informative_loss_to_follow_up")
    );
}

#[test]
fn oncoworlds_clonal_history_check_preserves_rejected_and_ambiguous_histories() {
    let population = TumourPopulation::new()
        .with(Subclone::new(
            SubcloneId::new("parent"),
            CellularFraction::from_parts_per_ten_thousand(10_000).unwrap(),
        ))
        .with(Subclone::new(
            SubcloneId::new("child"),
            CellularFraction::from_parts_per_ten_thousand(4_000).unwrap(),
        ));
    let compatible =
        ClonalHistory::new().descends(SubcloneId::new("parent"), SubcloneId::new("child"));
    let cyclic = ClonalHistory::new()
        .descends(SubcloneId::new("parent"), SubcloneId::new("child"))
        .descends(SubcloneId::new("child"), SubcloneId::new("parent"));
    let result = call(
        &mut server(),
        "oncoworlds_clonal_history_check",
        json!({
            "population": serde_json::to_value(population).unwrap(),
            "candidates": serde_json::to_value(vec![compatible, cyclic]).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["compatible_count"], json!(1));
    assert_eq!(result["rejected_count"], json!(1));
    assert_eq!(result["rejected"][0][1]["refusal"], json!("cyclic"));
    assert_eq!(result["unique_history"]["ok"], json!(true));
}

#[test]
fn oncoworlds_clonal_evidence_check_preserves_sampling_bounds_and_causal_refusal() {
    let core = RegionId::new("enhancing-core");
    let cellular = |parts| FractionEvidence::Cellular {
        fraction: CellularFraction::from_parts_per_ten_thousand(parts).unwrap(),
        derivation: FractionDerivation {
            purity: CellularFraction::from_parts_per_ten_thousand(8_000).unwrap(),
            local_copy_number: 2,
            multiplicity: 1,
            derived_by: "caller-copy-number-model-v1".into(),
        },
    };
    let diagnosis = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("diagnostic-core")
            .sampling(core.clone())
            .detecting_down_to(DetectionSensitivity {
                smallest_detectable_fraction: CellularFraction::from_parts_per_ten_thousand(500)
                    .unwrap(),
                declared_by: "assay-validation-v1".into(),
            }),
        OncoObserved::Value(MarkerCall::Absent),
    )
    .at_fraction(cellular(500));
    let recurrence = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("recurrence-core").sampling(core),
        OncoObserved::Value(MarkerCall::Present),
    )
    .at_fraction(cellular(2_000));
    let result = call(
        &mut server(),
        "oncoworlds_clonal_evidence_check",
        json!({
            "promotion": { "observation": serde_json::to_value(&recurrence).unwrap() },
            "resistance": { "diagnosis": serde_json::to_value(&diagnosis).unwrap(), "recurrence": serde_json::to_value(&recurrence).unwrap() },
            "attribution": { "treatment": "temozolomide", "alteration": "egfr_amplification", "design": "temporal_association_only" }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/oncoworlds-clonal-evidence-check/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("report"));
    assert_eq!(result["check_count"], json!(3));
    assert_eq!(result["refusal_count"], json!(1));
    assert_eq!(result["checks"]["promotion"]["allowed"], json!(true));
    assert_eq!(
        result["checks"]["promotion"]["outcome_kind"],
        json!("present_in_sampled_regions")
    );
    assert_eq!(result["checks"]["resistance"]["allowed"], json!(true));
    assert_eq!(
        result["checks"]["resistance"]["unique_explanation"],
        json!("de_novo_emergence")
    );
    assert_eq!(
        result["checks"]["resistance"]["de_novo_emergence_survives"],
        json!(true)
    );
    assert_eq!(result["checks"]["attribution"]["allowed"], json!(false));
    assert_eq!(
        result["checks"]["attribution"]["refusal_kind"],
        json!("unsupported_directionality")
    );

    let missing = call(
        &mut server(),
        "oncoworlds_clonal_evidence_check",
        json!({
            "promotion": { "observation": {
                "marker": "egfr_amplification",
                "sampling": { "specimen": "diagnostic-core", "regions": ["enhancing-core"] },
                "call": { "unobserved": "not_collected" }
            } }
        }),
    );
    assert_eq!(missing["all_admissible"], json!(false));
    assert_eq!(missing["refusal_count"], json!(1));
    assert_eq!(
        missing["checks"]["promotion"]["refusal_kind"],
        json!("not_an_absence")
    );
}

#[test]
fn oncoworlds_era_shift_and_equity_checks_preserve_mapping_resource_and_interval_evidence() {
    let comparable = call(
        &mut server(),
        "oncoworlds_era_shift_check",
        json!({
            "left": { "name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1"] },
            "right": { "name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"] },
            "mapping": { "from": "criteria-a", "to": "criteria-b", "fates": { "entity-1": { "fate": "renamed", "to": "entity-1a" } } },
            "assay_contexts": [{ "site": "site-b", "assay": "methylation", "availability": { "availability": "unavailable_at_site" } }],
            "descriptor_checks": [{ "descriptor": "self_reported_race_or_ethnicity", "use": "stratification" }, { "descriptor": "self_reported_race_or_ethnicity", "use": "mechanistic_variable" }]
        }),
    );
    assert_eq!(comparable["ok"], json!(true));
    assert_eq!(
        comparable["schema"],
        json!("bioprism-mcp/oncoworlds-era-shift-check/0.1")
    );
    assert_eq!(comparable["outcome_kind"], json!("comparable"));
    assert_eq!(comparable["evidence"]["mapping_fate_count"], json!(1));
    assert_eq!(
        comparable["evidence"]["mapping_versions_match"],
        json!(true)
    );
    assert_eq!(
        comparable["evidence"]["assay_contexts"][0]["negative_call_supported"],
        json!(false)
    );
    assert_eq!(
        comparable["evidence"]["assay_contexts"][0]["negative_call_refusal_kind"],
        json!("resource_absence_read_as_biology")
    );
    assert_eq!(
        comparable["evidence"]["descriptor_checks"][1]["allowed"],
        json!(false)
    );
    assert_eq!(
        comparable["evidence"]["descriptor_checks"][1]["refusal_kind"],
        json!("descriptor_used_as_mechanism")
    );

    let refused = call(
        &mut server(),
        "oncoworlds_era_shift_check",
        json!({
            "left": { "name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1", "entity-2"] },
            "right": { "name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"] },
            "mapping": { "from": "criteria-a", "to": "criteria-b", "fates": { "entity-1": { "fate": "renamed", "to": "entity-1a" } } }
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("incomplete_mapping"));
    assert_eq!(refused["fail_closed"], json!(true));

    let equity = call(
        &mut server(),
        "oncoworlds_equity_check",
        json!({
            "pooled": {
                "value": 0.91,
                "subgroups": [
                    { "subgroup": "large", "n": 900, "estimate": 0.93, "interval": { "low": 0.90, "high": 0.95 } },
                    { "subgroup": "small", "n": 3, "estimate": 0.55, "interval": { "low": 0.28, "high": 0.80 } }
                ]
            }
        }),
    );
    assert_eq!(equity["ok"], json!(true));
    assert_eq!(
        equity["schema"],
        json!("bioprism-mcp/oncoworlds-equity-check/0.1")
    );
    assert_eq!(equity["outcome_kind"], json!("equity_report"));
    assert_eq!(equity["subgroup_count"], json!(2));
    assert_eq!(equity["interval_count"], json!(2));
    assert_eq!(equity["all_intervals_present"], json!(true));

    let pooled_only = call(
        &mut server(),
        "oncoworlds_equity_check",
        json!({ "pooled": { "value": 0.91, "subgroups": [] } }),
    );
    assert_eq!(pooled_only["ok"], json!(false));
    assert_eq!(pooled_only["refusal_kind"], json!("pooled_score_only"));
    assert_eq!(pooled_only["fail_closed"], json!(true));
}

#[test]
fn oncoworlds_entity_world_check_keeps_independent_selection_and_event_refusals_visible() {
    let admissible = call(
        &mut server(),
        "oncoworlds_entity_world_check",
        json!({
            "provenance": { "left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": true },
            "alterations": { "left": "fusion", "right": "sequence_variant", "estimand": "time to next systemic therapy" },
            "benchmark": { "macro_score": 0.88, "per_class_counts": { "common": 300, "rare": 3 } },
            "lesion_analysis": { "lesions": 12, "participants": 12, "cluster_declared": false, "endpoint": "overall_survival", "event": "systemic_death", "handling": "event" }
        }),
    );
    assert_eq!(admissible["ok"], json!(true));
    assert_eq!(
        admissible["schema"],
        json!("bioprism-mcp/oncoworlds-entity-world-check/0.1")
    );
    assert_eq!(admissible["outcome_kind"], json!("report"));
    assert_eq!(admissible["all_admissible"], json!(true));
    assert_eq!(admissible["check_count"], json!(4));
    assert_eq!(admissible["refusal_count"], json!(0));
    assert_eq!(
        admissible["checks"]["benchmark"]["feasibility_kind"],
        json!("feasible")
    );
    assert_eq!(
        admissible["checks"]["lesion_analysis"]["event_allowed"],
        json!(true)
    );

    let refused = call(
        &mut server(),
        "oncoworlds_entity_world_check",
        json!({
            "provenance": { "left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": false },
            "alterations": { "left": "fusion", "right": "sequence_variant" },
            "benchmark": { "macro_score": 0.88, "per_class_counts": {} },
            "lesion_analysis": { "lesions": 41, "participants": 12, "cluster_declared": false, "endpoint": "local_control", "event": "systemic_death", "handling": "censoring" }
        }),
    );
    assert_eq!(refused["all_admissible"], json!(false));
    assert_eq!(refused["refusal_count"], json!(4));
    assert_eq!(
        refused["checks"]["provenance"]["refusal_kind"],
        json!("unmodelled_provenance_selection")
    );
    assert_eq!(
        refused["checks"]["alterations"]["refusal_kind"],
        json!("mechanism_collapse")
    );
    assert_eq!(
        refused["checks"]["benchmark"]["refusal_kind"],
        json!("macro_score_without_counts")
    );
    assert_eq!(
        refused["checks"]["lesion_analysis"]["cluster_refusal_kind"],
        json!("undeclared_cluster")
    );
    assert_eq!(
        refused["checks"]["lesion_analysis"]["event_refusal_kind"],
        json!("competing_event_as_censoring")
    );
}

#[test]
fn stress_profile_reports_breaking_points_and_generator_posture() {
    let cohort = Cohort::new(
        "cohort-1",
        vec![
            Subject::new("p1", "site-a", true, 3.0, 1000.0),
            Subject::new("p2", "site-b", true, 2.5, 1100.0),
            Subject::new("n1", "site-a", false, 0.5, 900.0),
            Subject::new("n2", "site-b", false, 0.2, 950.0),
        ],
    );
    let stress = Stress::new(
        "deployment-prevalence",
        Knob::PrevalenceShift {
            target_prevalence: 0.25,
        },
        Magnitude::FULL,
        7,
    );
    let result = call(
        &mut server(),
        "stress_profile",
        json!({
            "cohort": serde_json::to_value(cohort).unwrap(),
            "stress": serde_json::to_value(stress).unwrap(),
            "procedures": serde_json::to_value(vec![Procedure::MarkerRanking]).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert!(result["headline"].as_str().unwrap().contains("prevalence"));
    assert_eq!(result["profile"]["sweep"].as_array().unwrap().len(), 8);
    assert!(result["profile"]["generator_defects"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn bioethics_action_review_never_executes_and_requires_both_referral_acts() {
    let plan = ActionPlan::new("resistance-screen", OutputUse::CohortAnalysis)
        .with_step(PlannedStep::new(ActionKind::Analysis, "analyse the cohort"))
        .with_step(PlannedStep::new(
            ActionKind::LiquidHandler,
            "plate a dilution series",
        ));
    let refused = call(
        &mut server(),
        "bioethics_action_review",
        json!({ "plan": serde_json::to_value(&plan).unwrap() }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["physical_step_count"], json!(1));
    assert_eq!(refused["referral"]["status"], json!("not_attempted"));
    assert_eq!(refused["referral"]["fail_closed"], json!(true));

    let referred = call(
        &mut server(),
        "bioethics_action_review",
        json!({
            "plan": serde_json::to_value(&plan).unwrap(),
            "authorisation": serde_json::to_value(
                Authorisation::new()
                    .approved_by("principal investigator")
                    .safety_reviewed_by("institutional biosafety committee")
            ).unwrap()
        }),
    );
    assert_eq!(referred["referral"]["status"], json!("referred"));
    assert_eq!(
        referred["referral"]["executes_physical_action"],
        json!(false)
    );

    let clinical = ActionPlan::new("individual", OutputUse::TreatmentRecommendation)
        .with_step(PlannedStep::new(ActionKind::Analysis, "analyse"));
    let clinical_result = call(
        &mut server(),
        "bioethics_action_review",
        json!({ "plan": serde_json::to_value(clinical).unwrap() }),
    );
    assert_eq!(clinical_result["ok"], json!(false));
    assert_eq!(clinical_result["fail_closed"], json!(true));
}

#[test]
fn bioethics_human_subject_screen_keeps_review_consent_and_return_of_results_separate() {
    let study = StudyDescription::new(
        "reader-agreement-study",
        PurposeSet::of([Purpose::ResearchAnalysis]),
    )
    .engaging(EngagementKind::ExpertPerformanceStudy)
    .returning(ReturnOfResults::IndividualFindings);
    let consent = Consent::new("consent-1", PurposeSet::of([Purpose::ResearchAnalysis]));
    let result = call(
        &mut server(),
        "bioethics_human_subject_screen",
        json!({
            "study": serde_json::to_value(study).unwrap(),
            "consent": serde_json::to_value(consent).unwrap(),
            "at": "2026-01-01T00:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["requires_institutional_review"], json!(true));
    assert_eq!(result["consent"]["status"], json!("admitted"));
    assert_eq!(result["return_of_results"]["status"], json!("refused"));
    assert_eq!(result["clearance_issued"], json!(false));

    let undetermined = call(
        &mut server(),
        "bioethics_human_subject_screen",
        json!({
            "study": serde_json::to_value(StudyDescription::new(
                "undescribed",
                PurposeSet::of([Purpose::MethodDevelopment])
            )).unwrap()
        }),
    );
    assert_eq!(
        undetermined["determination"]["determination"],
        json!("undetermined")
    );
    assert_eq!(undetermined["clearance_issued"], json!(false));
}

#[test]
fn bioethics_dual_use_review_requires_assessment_before_the_safety_gate() {
    let release = CapabilityRelease::new(
        "variant-caller",
        SurfaceAssessment::assessed("biosafety reviewer", [MisuseSurface::SequenceDesign]),
    );
    let mut risk = RiskAssessment::for_subject("variant-caller")
        .in_category(SensitiveCategory::BiologicalDesign);
    for dimension in RiskDimension::ALL {
        risk = risk.rating(dimension, Rating::Low);
    }
    let accepted = call(
        &mut server(),
        "bioethics_dual_use_review",
        json!({
            "release": serde_json::to_value(&release).unwrap(),
            "risk": serde_json::to_value(&risk).unwrap(),
            "withhold": "exploit_detail",
            "finding": "screening gap"
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["decision"]["decision"], json!("cleared"));
    assert_eq!(accepted["withholding"]["status"], json!("admitted"));

    let unassessed = call(
        &mut server(),
        "bioethics_dual_use_review",
        json!({
            "release": serde_json::to_value(CapabilityRelease::new(
                "variant-caller",
                SurfaceAssessment::NotAssessed
            )).unwrap(),
            "risk": serde_json::to_value(risk).unwrap()
        }),
    );
    assert_eq!(unassessed["ok"], json!(false));
    assert!(unassessed["refusal"]
        .as_str()
        .unwrap()
        .contains("no misuse-surface"));
}

#[test]
fn bioethics_validation_check_does_not_mint_verification_from_missing_evidence() {
    let empty = call(
        &mut server(),
        "bioethics_validation_check",
        json!({
            "dossier": serde_json::to_value(ValidationDossier::new("module", "author-a")).unwrap()
        }),
    );
    assert_eq!(empty["ok"], json!(true));
    assert_eq!(empty["verification"]["status"], json!("refused"));
    assert!(empty["missing_count"].as_u64().unwrap() > 0);

    let mut complete = ValidationDossier::new("module", "author-a");
    for kind in EvidenceKind::ALL {
        complete = complete.with(BioethicsEvidenceRecord::new(
            kind,
            format!("reference-{}", kind.as_str()),
            if kind == EvidenceKind::IndependentReproduction {
                "reproducer-b"
            } else {
                "author-a"
            },
        ));
    }
    let verified = call(
        &mut server(),
        "bioethics_validation_check",
        json!({ "dossier": serde_json::to_value(complete).unwrap() }),
    );
    assert_eq!(verified["verification"]["status"], json!("verified"));
    assert_eq!(verified["missing_count"], json!(0));
}

#[test]
fn bioethics_representation_audit_preserves_unmeasured_and_suppressed_strata() {
    let observations = vec![
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::AgeAndSex, "adult"),
            StratumCoverage::Measured,
        ),
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::Geography, "rural"),
            StratumCoverage::Unmeasured,
        ),
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::SiteResources, "low-resource"),
            StratumCoverage::SuppressedSmallGroup { below: 10 },
        ),
    ];
    let result = call(
        &mut server(),
        "bioethics_representation_audit",
        json!({
            "subject": "cohort-1",
            "observations": serde_json::to_value(observations).unwrap(),
            "attribution": {
                "axis": "age_and_sex",
                "matched": [],
                "finding": "performance difference"
            }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["measured_count"], json!(1));
    assert_eq!(result["unmeasured_count"], json!(1));
    assert_eq!(result["suppressed_count"], json!(1));
    assert_eq!(result["complete"], json!(false));
    assert_eq!(
        result["attribution"]["standing"]["status"],
        json!("refused")
    );
}

#[test]
fn influence_analyze_reports_bounds_and_unknown_structural_inputs_distinctly() {
    let bounded = call(
        &mut server(),
        "influence_analyze",
        json!({
            "label": "small-region",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"], "table": [1.0, 2.0] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(bounded["ok"], json!(true));
    assert_eq!(bounded["execute"], json!(false));
    assert_eq!(bounded["analysis"]["estimate"]["kind"], json!("bounded"));
    assert!(bounded["analysis"]["estimate"]["value"].is_number());
    assert!(bounded["analysis"]["estimate"]["method"].is_string());

    let unknown = call(
        &mut server(),
        "influence_analyze",
        json!({
            "label": "structural-region",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(unknown["ok"], json!(true));
    assert_eq!(unknown["analysis"]["estimate"]["kind"], json!("unknown"));
    assert!(unknown["analysis"]["estimate"]["reason"].is_string());
}

fn routing_fingerprint_fixture() -> Fingerprint {
    Fingerprint {
        facts: 10,
        factors: 3,
        protected_tag_count: 2,
        protected_fact_fraction: 0.2,
        distractor_density: 0.8,
        tag_informativeness: 1.0,
        mean_factor_arity: 1.0,
        max_factor_arity: 2,
        arity_histogram: BTreeMap::from([(1, 2), (2, 1)]),
        max_unary_chain: 0,
        hub_share: 0.5,
        hub_is_derived: false,
        target_producer_count: 1,
    }
}

#[test]
fn routing_decide_abstains_without_two_architecture_supporting_panels() {
    let approved = ApprovedSet::new([
        RoutingArchitecture::FullContext,
        RoutingArchitecture::FiberCompiled,
    ])
    .unwrap();
    let policy = RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap();
    let result = call(
        &mut server(),
        "routing_decide",
        json!({
            "fingerprint": serde_json::to_value(routing_fingerprint_fixture()).unwrap(),
            "evidence": [],
            "policy": serde_json::to_value(policy).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["decision"]["abstained"], json!(true));
    assert_eq!(result["decision"]["confidence"], json!(0.0));
    assert_eq!(
        result["holdout_check"],
        json!("caller_must_supply_unseen_identity")
    );
}

#[test]
fn routing_decide_refuses_evidence_leakage_when_task_identity_is_supplied() {
    let fingerprint = routing_fingerprint_fixture();
    let ledger = EvidenceLedger::new([RoutingObservation {
        task_id: "seen-task".into(),
        fingerprint: fingerprint.clone(),
        architecture: RoutingArchitecture::FullContext,
        verdict_preserving: true,
        closure_complete: true,
        status: OracleStatus::Valid,
        facts_exposed: 10,
        total_facts: 10,
    }])
    .unwrap();
    let approved = ApprovedSet::new([RoutingArchitecture::FullContext]).unwrap();
    let policy = RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap();
    let result = call(
        &mut server(),
        "routing_decide",
        json!({
            "fingerprint": serde_json::to_value(fingerprint).unwrap(),
            "evidence": serde_json::to_value(ledger).unwrap(),
            "policy": serde_json::to_value(policy).unwrap(),
            "task_id": "seen-task",
        }),
    );
    assert_eq!(result["__isError"], json!(true));
    assert!(result["error"]
        .as_str()
        .unwrap()
        .contains("contains that task's own outcome"));
}

#[test]
fn routing_lab_run_keeps_holdout_comparators_and_bounded_task_rows_visible() {
    let reference = bioprism_worldgen::generate(&bioprism_worldgen::WorldSpec::reference_like(2));
    let discriminating =
        bioprism_worldgen::generate(&bioprism_worldgen::WorldSpec::discriminating(2));
    let approved = ApprovedSet::new([
        RoutingArchitecture::FullContext,
        RoutingArchitecture::FiberCompiled,
    ])
    .unwrap();
    let settings = bioprism_routing::LabSettings::new(
        RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap(),
        RoutingArchitecture::FullContext,
    )
    .unwrap();
    let result = call(
        &mut server(),
        "routing_lab_run",
        json!({
            "tasks": [
                { "task_id": "reference-task", "world": reference.world, "query": reference.query },
                { "task_id": "discriminating-task", "world": discriminating.world, "query": discriminating.query }
            ],
            "settings": serde_json::to_value(settings).unwrap(),
            "include_rows": true,
            "max_rows": 1
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/routing-lab-run/0.1"));
    assert_eq!(result["tasks"], json!(2));
    assert_eq!(result["holdout_label"], json!("leave-one-task-out"));
    assert_eq!(result["report"]["task_rows"].as_array().unwrap().len(), 1);
    assert_eq!(result["report"]["task_rows_omitted"], json!(1));
    assert!(result["report"]["account"]["router"].is_object());
    assert!(result["report"]["verdict"].is_string());
    assert!(result["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("route_unseen")));
}

#[test]
fn token_context_plan_keeps_dry_run_restricted_data_fail_closed() {
    let request = json!({
        "world_ref": "world/demo",
        "decision_ref": "decision/demo",
        "role": "researcher",
        "policy_id": "policy/minimal",
        "envelope": { "total": 100 },
        "depth": "dry_run",
        "compiler_version": "compiler/1.0.0",
    });
    let restricted = json!([{
        "node_id": "raw/secret",
        "kind": "evidence",
        "restricted": true,
        "estimate": { "tokens": 10, "method": { "method": "declared_by_caller" } },
    }]);
    let refused = call(
        &mut server(),
        "token_context_plan",
        json!({ "request": request, "candidates": restricted }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("restricted"));

    let accepted = call(
        &mut server(),
        "token_context_plan",
        json!({
            "request": {
                "world_ref": "world/demo",
                "decision_ref": "decision/demo",
                "role": "researcher",
                "policy_id": "policy/minimal",
                "envelope": { "total": 100 },
                "depth": "l1",
                "compiler_version": "compiler/1.0.0",
            },
            "candidates": [{
                "node_id": "invariant/identity",
                "kind": "invariant",
                "mandatory": true,
                "estimate": { "tokens": 20, "method": { "method": "declared_by_caller" } },
            }, {
                "node_id": "evidence/summary",
                "kind": "summary",
                "estimate": { "tokens": 30, "method": { "method": "declared_by_caller" } },
            }],
        }),
    );
    assert_eq!(accepted["__isError"], json!(false));
    assert_eq!(accepted["plan"]["mandatory_estimate"]["tokens"], json!(20));
    assert_eq!(accepted["plan"]["optional_estimate"]["tokens"], json!(30));
}

#[test]
fn lens_catalogue_exposes_questions_and_unimplemented_section_remainder() {
    let result = call(&mut server(), "lens_catalogue", json!({}));
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["section_42_module_count"], json!(31));
    assert_eq!(result["implemented_count"], json!(6));
    assert!(result["implemented"]
        .as_array()
        .unwrap()
        .iter()
        .any(|lens| {
            lens["id"] == json!("cohort_leakage")
                && lens["requires"].as_array().unwrap().len() >= 4
                && lens["refuses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|reason| reason == "scope_precondition_unmet")
        }));
    assert!(result["not_implemented"].as_array().unwrap().len() >= 10);
}

#[test]
fn lens_leakage_check_seals_nonvisual_findings_and_preserves_underdetermination() {
    let result = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": { "cohort": "C-1" },
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": ["ALT-77"],
                    "site": { "recorded": "known", "value": "MGH" },
                    "label_source_time": { "recorded": "known", "value": "2025-01-01T00:00:00Z" }
                }, {
                    "subject": "S003",
                    "split": "test",
                    "aliases": ["ALT-77"],
                    "site": { "recorded": "known", "value": "DFCI" },
                    "label_source_time": { "recorded": "known", "value": "2026-05-01T00:00:00Z" }
                }],
                "preprocessing": [{ "name": "normalizer", "fit_over": ["train", "test"] }],
                "decision_time": { "recorded": "known", "value": "2026-01-01T00:00:00Z" }
            },
            "include_spoken": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["outcome"], json!("answered"));
    assert_eq!(result["blueprint_module"], json!("42.10"));
    assert_eq!(result["witness_count"], json!(4));
    assert!(result["receipt"].as_str().unwrap().len() >= 32);
    assert!(result["report"]["outcome"]["witnesses"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["kind"] == "identity_leakage"));
    assert!(result["spoken"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| { line.as_str().is_some_and(|line| line.contains("ALT-77")) }));

    let underdetermined = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": { "cohort": "C-1" },
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": [],
                    "site": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } },
                    "label_source_time": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } }
                }],
                "preprocessing": [],
                "decision_time": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } }
            }
        }),
    );
    assert_eq!(underdetermined["__isError"], json!(false));
    assert_eq!(underdetermined["outcome"], json!("answered"));
    assert!(underdetermined["report"]["outcome"]["witnesses"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["kind"] == "check_not_runnable"));
}

#[test]
fn lens_leakage_check_refuses_unbound_scope_before_answering() {
    let result = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": {},
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": [],
                    "site": { "recorded": "known", "value": "MGH" },
                    "label_source_time": { "recorded": "known", "value": "2025-01-01T00:00:00Z" }
                }],
                "preprocessing": [],
                "decision_time": { "recorded": "known", "value": "2026-01-01T00:00:00Z" }
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["outcome"], json!("refused"));
    assert_eq!(result["report"]["outcome"]["outcome"], json!("refused"));
    assert_eq!(
        result["report"]["outcome"]["reason"],
        json!("scope_precondition_unmet")
    );
}

#[test]
fn scale_family_split_verify_keeps_lineage_families_intact() {
    let parent = GeneratedItem::parent("world-1", "decision", "digest-parent");
    let child = GeneratedItem::descendant(
        "world-1-mutated",
        "world-1",
        "fault",
        "signature",
        "digest-child",
        "decision",
    );
    let valid = call(
        &mut server(),
        "scale_family_split_verify",
        json!({
            "corpus": serde_json::to_value(vec![parent.clone(), child.clone()]).unwrap(),
            "assignment": { "world-1": "public", "world-1-mutated": "public" }
        }),
    );
    assert_eq!(valid["__isError"], json!(false));
    assert_eq!(valid["valid"], json!(true));
    assert_eq!(valid["family_count"], json!(1));
    assert_eq!(valid["report"]["intact_families"], json!(1));
    assert_eq!(valid["report"]["items_by_tier"]["public"], json!(2));

    let straddled = call(
        &mut server(),
        "scale_family_split_verify",
        json!({
            "corpus": serde_json::to_value(vec![parent, child]).unwrap(),
            "assignment": { "world-1": "public", "world-1-mutated": "hidden" }
        }),
    );
    assert_eq!(straddled["__isError"], json!(false));
    assert_eq!(straddled["valid"], json!(false));
    assert!(straddled["refusal"].as_str().unwrap().contains("straddles"));
}

#[test]
fn stewardship_review_check_issues_scoped_approval_and_refuses_self_review() {
    let revision = EvaluatorRevision::new(
        "evaluator",
        SchemaVersion::parse("1.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v1"),
        false,
    );
    let mut record = ReviewRecord::new(
        revision,
        Actor::author("author"),
        Actor::independent_reviewer("reviewer"),
    )
    .against(full_corpus(1));
    for dimension in ReviewDimension::mandatory_for(false) {
        record = record
            .finding(
                dimension,
                StewardshipFinding::passed("checked against the declared corpus"),
            )
            .unwrap();
    }
    let issued = call(
        &mut server(),
        "stewardship_review_check",
        json!({ "review": serde_json::to_value(record).unwrap() }),
    );
    assert_eq!(issued["__isError"], json!(false));
    assert_eq!(issued["decision"], json!("issued"));
    assert_eq!(issued["covered_dimensions"].as_array().unwrap().len(), 6);
    assert!(issued["unreviewed_dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["dimension"] == "adversarial_injection"));

    let revision = EvaluatorRevision::new(
        "evaluator",
        SchemaVersion::parse("1.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v1"),
        false,
    );
    let mut self_review = ReviewRecord::new(
        revision,
        Actor::author("same-person"),
        Actor::independent_reviewer("same-person"),
    )
    .against(full_corpus(1));
    for dimension in ReviewDimension::mandatory_for(false) {
        self_review = self_review
            .finding(
                dimension,
                StewardshipFinding::passed("checked against the declared corpus"),
            )
            .unwrap();
    }
    let refused = call(
        &mut server(),
        "stewardship_review_check",
        json!({ "review": serde_json::to_value(self_review).unwrap() }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["decision"], json!("refused"));
    assert!(refused["refusal"]
        .as_str()
        .unwrap()
        .contains("authored the evaluator"));
}

#[test]
fn quality_gate_run_keeps_failures_and_unrunnable_checks_separate() {
    let dataset = QualityDataset::new("patients")
        .unwrap()
        .with_column("subject", [json!("S1"), json!("S1")])
        .unwrap()
        .with_column("age", [json!(41), json!(null)])
        .unwrap();
    let gate = QualityGate::new("release-quality")
        .unwrap()
        .with(
            "subject_unique",
            QualityCheck::Unique {
                column: "subject".into(),
            },
        )
        .unwrap()
        .with(
            "foreign_site",
            QualityCheck::ForeignKey {
                column: "site".into(),
                reference: "sites".into(),
            },
        )
        .unwrap();
    let result = call(
        &mut server(),
        "quality_gate_run",
        json!({
            "dataset": serde_json::to_value(dataset).unwrap(),
            "gate": serde_json::to_value(gate).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["schema"], json!("bioprism-mcp/quality-gate/0.1"));
    assert_eq!(result["verdict"], json!("failed"));
    assert_eq!(result["passed"], json!(false));
    assert_eq!(
        result["report"]["outcomes"]["subject_unique"]["Fail"]["witness"]["row"],
        json!(1)
    );
    let foreign_site = result["report"]["outcomes"]["foreign_site"].to_string();
    assert!(
        foreign_site.contains("MissingReferenceSet"),
        "{foreign_site}"
    );
    assert!(foreign_site.contains("site"), "{foreign_site}");

    let indeterminate_dataset = QualityDataset::new("empty-measurement")
        .unwrap()
        .with_column("age", [json!(null), json!(null)])
        .unwrap();
    let indeterminate_gate = QualityGate::new("age-quality")
        .unwrap()
        .with(
            "age_range",
            QualityCheck::InRange {
                column: "age".into(),
                min: 0.0,
                max: 120.0,
            },
        )
        .unwrap();
    let indeterminate = call(
        &mut server(),
        "quality_gate_run",
        json!({
            "dataset": serde_json::to_value(indeterminate_dataset).unwrap(),
            "gate": serde_json::to_value(indeterminate_gate).unwrap(),
        }),
    );
    assert_eq!(indeterminate["verdict"], json!("indeterminate"));
    assert_eq!(indeterminate["passed"], json!(false));
}

#[test]
fn ledger_ingest_preserves_quarantine_idempotency_time_axes_and_projections() {
    let parent = ledger_event_fixture(
        "specimen.collected",
        "patient-7/specimen-1",
        "2025-01-01T00:00:00Z",
        "parent-key",
    );
    let child = ledger_event_fixture(
        "measurement.recorded",
        "patient-7/specimen-1",
        "2024-01-01T00:00:00Z",
        "child-key",
    )
    .caused_by([bioprism_ids::EventId::parse("evt-000000000000").unwrap()]);
    let result = call(
        &mut server(),
        "ledger_ingest",
        json!({
            "events": serde_json::to_value(vec![child, parent.clone(), parent]).unwrap(),
            "include_receipts": true,
            "cut": serde_json::to_value(TemporalCut::known_at(
                LedgerRecordTime::parse("2024-06-01T00:00:00Z").unwrap()
            )).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["schema"], json!("bioprism-mcp/ledger-ingest/0.1"));
    assert_eq!(result["entries"], json!(2));
    assert_eq!(result["admissions"]["recorded"], json!(2));
    assert_eq!(result["admissions"]["duplicates"], json!(1));
    assert_eq!(result["admissions"]["quarantined"], json!(1));
    assert_eq!(result["admissions"]["released"], json!(1));
    assert_eq!(result["chain"]["status"], json!("intact"));
    assert_eq!(result["clock_anomalies"].as_array().unwrap().len(), 1);
    assert_eq!(result["quarantine"]["count"], json!(0));
    assert_eq!(result["latest_by_subject"]["count"], json!(1));
    assert_eq!(result["cut"]["count"], json!(1));
    assert!(
        result["latest_by_subject"]["items"][0]["payload_digest"]
            .as_str()
            .unwrap()
            .len()
            >= 32
    );
}

#[test]
fn fabric_synthesize_keeps_hard_rejections_out_of_the_pareto_frontier() {
    let goal = FabricGoal::new("produce a bounded decision", "decision-artifact").unwrap();
    let admissible = FabricCandidate::new("minimal", RoleGraph::new()).terminating_at("done");
    let rejected = FabricCandidate::new("unfinished", RoleGraph::new());
    let result = call(
        &mut server(),
        "fabric_synthesize",
        json!({
            "goal": serde_json::to_value(goal).unwrap(),
            "candidates": serde_json::to_value(vec![admissible, rejected]).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["candidate_count"], json!(2));
    assert_eq!(result["admissible_count"], json!(1));
    assert_eq!(result["eliminated_count"], json!(1));
    assert_eq!(result["artifact"]["frontier"], json!(["minimal"]));
    assert!(result["artifact"]["eliminated"]["unfinished"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason["reason"] == "missing_terminal_states"));
    assert!(result["unimplemented_stages"].as_array().unwrap().len() >= 5);
}

#[test]
fn interweave_workflow_catalogue_derives_owed_deliverables_without_fabricating_artifacts() {
    let result = call(&mut server(), "interweave_workflow_catalogue", json!({}));
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["workflow_count"], json!(6));
    assert_eq!(result["deliverables_per_workflow"], json!(9));
    assert_eq!(result["workflows"].as_array().unwrap().len(), 6);
    assert_eq!(
        result["outstanding_deliverables"]
            .as_object()
            .unwrap()
            .len(),
        6
    );
    assert!(result["outstanding_deliverables"]
        .as_object()
        .unwrap()
        .values()
        .all(|count| count == 9));
    assert!(result["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|workflow| workflow["present"].as_array().unwrap().is_empty()));
}

#[test]
fn bioql_compile_returns_a_typed_contract_and_refuses_missing_access_declarations() {
    let schema = QuerySchema::new().with(
        CollectionDecl::new("lesions")
            .declare(
                "tumor_volume",
                BioType::quantity(Unit::parse("mm3").expect("mm3 is a known unit")),
            )
            .costing(10),
    );
    let result = call(
        &mut server(),
        "bioql_compile",
        json!({
            "query": "select tumor_volume from lesions where tumor_volume > 12.5 mm3 labels { \"phi:deidentified\" } cost limit 100",
            "schema": serde_json::to_value(&schema).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["typed_query"]["collection"], json!("lesions"));
    assert_eq!(result["typed_query"]["cost_estimate"], json!(20));
    assert_eq!(result["execution"], json!("not_performed"));

    let refused = call(
        &mut server(),
        "bioql_compile",
        json!({
            "query": "select tumor_volume from lesions cost limit 100",
            "schema": serde_json::to_value(schema).unwrap(),
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(refused["refusal"]
        .as_str()
        .unwrap()
        .contains("access labels"));
}

#[test]
fn epistemic_voi_keeps_gross_cost_net_and_action_change_separate() {
    let result = call(
        &mut server(),
        "epistemic_voi",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisition": {
                "id": "assay",
                "cost": 0.1,
                "outcomes": [
                    { "label": "positive", "likelihood": [0.9, 0.1] },
                    { "label": "negative", "likelihood": [0.1, 0.9] }
                ]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["mode"], json!("single"));
    assert!(result["value"]["gross"].as_f64().unwrap() > 0.0);
    assert_eq!(result["value"]["cost"], json!(0.1));
    assert!(result["value"]["net"].as_f64().unwrap() > 0.0);
    assert_eq!(result["actions"]["without"], json!("treat"));
    assert_eq!(result["value"]["action_after"], json!([0, 1]));
    assert_eq!(result["value"]["action_without"], json!(0));

    let invalid = call(
        &mut server(),
        "epistemic_voi",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisition": {
                "id": "broken",
                "cost": 0.1,
                "outcomes": [
                    { "label": "only", "likelihood": [0.2, 0.2] }
                ]
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"]
        .as_str()
        .unwrap()
        .contains("invariant failed"));
}

#[test]
fn epistemic_decision_quotient_keeps_permitted_boundary_and_merges_only_equivalent_models() {
    let result = call(
        &mut server(),
        "epistemic_decision_quotient",
        json!({
            "problem": {
                "actions": ["accept", "defer", "reject"],
                "models": ["m-a", "m-b", "m-c"],
                "loss": [
                    0.0, 7.0, 0.0,
                    4.0, 11.0, 5.0,
                    8.0, 15.0, 8.0
                ]
            },
            "permitted_actions": ["reject", "accept", "defer"]
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-decision-quotient/0.1")
    );
    assert_eq!(
        result["quotient"]["permitted_actions"],
        json!(["accept", "defer", "reject"])
    );
    assert_eq!(result["summary"]["original_model_count"], json!(3));
    assert_eq!(result["summary"]["quotient_model_count"], json!(2));
    assert_eq!(result["summary"]["merged_model_count"], json!(1));
    assert_eq!(
        result["quotient"]["model_to_class"]["m-a"],
        result["quotient"]["model_to_class"]["m-b"]
    );
    assert_ne!(
        result["quotient"]["model_to_class"]["m-a"],
        result["quotient"]["model_to_class"]["m-c"]
    );

    let refused = call(
        &mut server(),
        "epistemic_decision_quotient",
        json!({
            "problem": {
                "actions": ["accept"],
                "models": ["m"],
                "loss": []
            },
            "permitted_actions": ["accept"]
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("invariant failed"));
}

#[test]
fn epistemic_context_audit_keeps_frontier_sufficiency_and_subset_refusals_distinct() {
    let result = call(
        &mut server(),
        "epistemic_context_audit",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "evidence_pool": {
                "items": [
                    { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
                    { "id": "marker", "cost": 1.0, "likelihood": [0.1, 0.9] }
                ]
            },
            "criterion": "bayes_regret",
            "tolerance": 1.0,
            "compatibility_floor": 0.0,
            "subsets": [[0], [0, 1], [0, 0]],
            "include_frontier": true,
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-context-audit/0.1")
    );
    assert_eq!(result["criterion"], json!("bayes_regret"));
    assert_eq!(result["evidence_pool"]["item_count"], json!(2));
    assert_eq!(result["evidence_pool"]["full_rate"], json!(3.0));
    assert_eq!(result["frontier"]["evaluated"], json!(4));
    assert!(result["sufficiency"]["outcome"].is_string());
    assert_eq!(result["subset_count"], json!(3));
    assert_eq!(result["subset_refusal_count"], json!(1));
    assert_eq!(result["subset_rows"][2]["result"], json!("refused"));
    assert!(result["identification"]["status"].is_string());
}

#[test]
fn epistemic_selection_audit_gates_guarantees_and_exact_comparisons() {
    let result = call(
        &mut server(),
        "epistemic_selection_audit",
        json!({
            "problem": {
                "actions": ["treat", "defer"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.4, 0.6] },
            "evidence_pool": {
                "items": [
                    { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
                    { "id": "marker", "cost": 1.0, "likelihood": [0.8, 0.2] },
                    { "id": "uninformative", "cost": 1.0, "likelihood": [1.0, 1.0] }
                ]
            },
            "constraint": { "cardinality": 2 },
            "protected": [],
            "check_submodularity": true,
            "include_lazy": true,
            "compare_optimum": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-selection-audit/0.1")
    );
    assert_eq!(result["objective"], json!("regret_reduction"));
    assert_eq!(result["evidence_pool"]["count"], json!(3));
    assert_eq!(result["submodularity"]["status"], json!("evaluated"));
    assert_eq!(
        result["comparisons"]["exact_optimum"]["status"],
        json!("evaluated")
    );
    assert!(result["greedy"]["chosen"].is_array());
    assert!(result["lazy"]["chosen"].is_array());
    assert!(result["comparisons"]["greedy_lazy_agree"].is_boolean());
    assert!(result["greedy"]["guarantee"]["applicability"].is_string());
}

#[test]
fn benchmark_trace_analyze_keeps_causal_localization_and_segmentation_distinct() {
    let result = call(
        &mut server(),
        "benchmark_trace_analyze",
        json!({
            "failing": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "summary": "choose route", "alternatives": ["safe", "unsafe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "reference": {
                "trace_id": "reference-run",
                "succeeded": true,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "summary": "choose safe", "alternatives": ["safe", "unsafe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "succeeded" }, "caused_by": 1, "visible": ["task"] }
                ]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["trace_id"], json!("failed-run"));
    assert_eq!(result["event_count"], json!(3));
    assert_eq!(result["summary"]["episode_count"], json!(1));
    assert!(result["summary"]["boundary_count"].as_u64().unwrap() >= 1);
    assert!(result["analysis"]["candidates"].is_array());
    assert!(result["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("does not replay")));
}

#[test]
fn benchmark_decision_audit_preserves_firewall_coverage_and_failure_evidence() {
    let result = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({
            "trace": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "unsafe", "alternatives": ["safe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "reference": {
                "trace_id": "reference-run",
                "succeeded": true,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "safe", "alternatives": ["safe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "succeeded" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "actions": [
                {
                    "label": "future-safe",
                    "semantic_property": "avoid the irreversible side effect",
                    "provenance": { "source": "from_future", "from_step": 3 },
                    "feasibility": { "state": "feasible" },
                    "strong": true
                }
            ],
            "claims": [
                { "status": "evidenced", "claim": "the choice differed", "citations": [{ "cites": "event", "step": 1 }] },
                { "status": "hypothesis", "claim": "the tool was confusing", "why": "no direct evidence" }
            ],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-decision-audit/0.1")
    );
    assert_eq!(result["decision"]["selected_step"], json!(1));
    assert_eq!(result["decision"]["causal_alignment"], json!("aligned"));
    assert_eq!(result["decision"]["action_counts"]["all"], json!(3));
    assert_eq!(
        result["decision"]["action_counts"]["visible_to_agent"],
        json!(2)
    );
    assert_eq!(
        result["decision"]["action_counts"]["validation_only"],
        json!(1)
    );
    assert_eq!(result["failure_card"]["evidence_ratio"], json!(0.5));
    assert_eq!(
        result["failure_card"]["hypotheses"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(result["trace_digest"].as_str().unwrap().len(), 64);

    let leak = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({
            "trace": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "unsafe" } },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1 }
                ]
            },
            "decision_step": 1,
            "actions": [{
                "label": "leaked",
                "provenance": { "source": "visible_at_decision_time", "from_step": 2 },
                "feasibility": { "state": "feasible" },
                "strong": true
            }]
        }),
    );
    assert_eq!(leak["__isError"], json!(false));
    assert_eq!(leak["ok"], json!(false));
    assert_eq!(leak["stage"], json!("hindsight_firewall"));
    assert_eq!(leak["fail_closed"], json!(true));

    let invalid = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({ "trace": { "trace_id": "t", "succeeded": false, "events": [] }, "max_items": "100" }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].as_str().unwrap().contains("max_items"));
}

#[test]
fn benchmark_integrity_audit_keeps_duplicates_leaks_holdouts_and_effective_denominators_separate() {
    let result = call(
        &mut server(),
        "benchmark_integrity_audit",
        json!({
            "instances": [
                { "instance_id": "a", "content": { "world": "W", "sample": "A" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["A"] },
                { "instance_id": "b", "content": { "world": "W", "sample": "A" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["A"] },
                { "instance_id": "c", "content": { "world": "W", "sample": "B" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["B"] },
                { "instance_id": "d", "content": { "world": "W2", "sample": "D" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["D"] },
                { "instance_id": "e", "content": { "world": "W3", "sample": "E" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["E"] }
            ],
            "panel_runs": [
                { "instance_id": "a", "architecture": "strong", "tier": "strong", "passed": true },
                { "instance_id": "a", "architecture": "weak", "tier": "weak", "passed": false },
                { "instance_id": "d", "architecture": "weak", "tier": "weak", "passed": true },
                { "instance_id": "e", "architecture": "strong", "tier": "strong", "passed": true }
            ],
            "known_instances": ["a", "b", "c", "d", "e", "unmeasured"],
            "safety_vetoes": ["e"],
            "bench_instances": [
                { "instance_id": "x1", "parent_digest": "p1", "mutation_family": "f1", "oracle_signature": "o1" },
                { "instance_id": "x2", "parent_digest": "p1", "mutation_family": "f1", "oracle_signature": "o1" },
                { "instance_id": "x3", "parent_digest": "p1", "mutation_family": "f2", "oracle_signature": "o1" },
                { "instance_id": "x4", "parent_digest": "p2", "mutation_family": "f1", "oracle_signature": "o2" }
            ],
            "exposure": {
                "a": { "published": true, "repositories": ["repo-a"], "answer_searchable": false, "first_published": "2025-01-01", "assessed": true },
                "b": { "published": true, "repositories": ["repo-b"], "answer_searchable": true, "first_published": "2025-01-01", "assessed": true },
                "e": { "published": false, "repositories": [], "answer_searchable": false, "first_published": null, "assessed": true }
            },
            "probes": {
                "d": [{ "channel": "metadata_only", "solved": true, "note": "metadata disclosed the answer" }],
                "e": [{ "channel": "filename_only", "solved": false, "note": "probe did not solve" }]
            },
            "private_share": 100,
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-integrity-audit/0.1")
    );
    assert_eq!(result["counts"]["instances"], json!(5));
    assert_eq!(result["dedup"]["examined"], json!(5));
    assert_eq!(result["dedup"]["distinct"], json!(3));
    assert!(result["dedup"]["groups"].as_array().unwrap().len() >= 2);
    assert_eq!(result["holdout"]["counts"]["private"], json!(5));
    assert_eq!(result["contamination"]["counts"]["unassessed"], json!(1));
    assert_eq!(
        result["contamination"]["counts"]["leaks_through_channel"],
        json!(1)
    );
    assert_eq!(result["contamination"]["admissible"], json!(1));
    assert_eq!(result["calibration"]["unmeasured"], json!(3));
    assert_eq!(result["calibration"]["safety_vetoes"], json!(1));
    assert_eq!(result["effective_diversity"]["instances"], json!(4));
    assert_eq!(
        result["effective_diversity"]["equivalence_classes"],
        json!(3)
    );
    assert!(result["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("semantic similarity")));
}

#[test]
fn benchmark_counterfactual_check_enforces_one_factor_matching_and_grades_contrast() {
    let source = json!({
        "schema_version": "bioprism-decision-cell/0.1",
        "cell_id": "cell-source",
        "decision_point": "choose evidence",
        "world": { "locator": "world-a", "sha256": "a".repeat(64) },
        "query": { "locator": "query-a", "sha256": "b".repeat(64) },
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "require_protected_closure": true
    });
    let followup = json!({
        "schema_version": "bioprism-decision-cell/0.1",
        "cell_id": "cell-followup",
        "decision_point": "choose evidence",
        "world": { "locator": "world-a", "sha256": "a".repeat(64) },
        "query": { "locator": "query-b", "sha256": "c".repeat(64) },
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "require_protected_closure": true
    });
    let result = call(
        &mut server(),
        "benchmark_counterfactual_check",
        json!({
            "source": source,
            "followup": followup,
            "intervention": {
                "factor": "fresh evidence",
                "target": "evidence_availability",
                "from": { "available": false },
                "to": { "available": true },
                "changes": ["query"]
            },
            "expected": { "expect": "invariant", "rationale": "the correct verdict remains pass" },
            "source_verdict": "pass",
            "followup_verdict": "pass"
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["pair"]["differing_fields"], json!(["query"]));
    assert_eq!(result["pair"]["realism_reviewed"], json!(false));
    assert_eq!(result["outcome"]["outcome"], json!("as_predicted"));
    assert_eq!(result["satisfied"], json!(true));
    assert_eq!(result["cell_digests"]["source"].as_str().unwrap().len(), 64);

    let mut mismatched_followup = followup;
    mismatched_followup["acceptable_verdicts"] = json!(["abstain"]);
    let refused = call(
        &mut server(),
        "benchmark_counterfactual_check",
        json!({
            "source": source,
            "followup": mismatched_followup,
            "intervention": {
                "factor": "fresh evidence",
                "target": "evidence_availability",
                "from": { "available": false },
                "to": { "available": true },
                "changes": ["query"]
            },
            "expected": { "expect": "invariant", "rationale": "unchanged" },
            "source_verdict": "pass",
            "followup_verdict": "abstain"
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("matched_pair"));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(refused["refusal"].as_str().unwrap().contains("not matched"));
}

#[test]
fn benchmark_oracle_review_requires_gate_before_grading_or_cell_packaging() {
    let proposal = json!({
        "oracle_id": "oracle-demo",
        "decision_point": "choose evidence",
        "strength": "exact_state_predicate",
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "can_see": ["declared world"],
        "blind_spots": ["hidden grader state"],
        "exploits": []
    });
    let result = call(
        &mut server(),
        "benchmark_oracle_review",
        json!({
            "proposal": proposal,
            "reviewer": "reviewer-1",
            "grade": { "verdict": "pass", "witnesses": ["evidence"], "closure_complete": true },
            "cell": {
                "cell_id": "cell-reviewed",
                "world": { "locator": "world.json", "sha256": "a".repeat(64) },
                "query": { "locator": "query.json", "sha256": "b".repeat(64) }
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["grade"]["acceptance"]["outcome"], json!("passed"));
    assert_eq!(result["grade"]["passed"], json!(true));
    assert_eq!(result["cell"]["cell_id"], json!("cell-reviewed"));
    assert_eq!(result["cell"]["acceptable_verdicts"], json!(["pass"]));
    assert_eq!(result["reviewer"], json!("reviewer-1"));
    assert_eq!(result["review_digest"].as_str().unwrap().len(), 64);
    assert_eq!(result["reviewed_oracle"]["reviewer"], json!("reviewer-1"));

    let exploit = call(
        &mut server(),
        "benchmark_oracle_review",
        json!({
            "proposal": {
                "oracle_id": "oracle-exploit",
                "decision_point": "choose",
                "strength": "exact_state_predicate",
                "acceptable_verdicts": ["pass"],
                "required_witnesses": [],
                "can_see": ["world"],
                "blind_spots": ["grader"],
                "exploits": [{ "name": "grader-read", "description": "read grader", "scored_as_pass": true, "fulfils_task_intent": false }]
            },
            "reviewer": "reviewer-1"
        }),
    );
    assert_eq!(exploit["__isError"], json!(false));
    assert_eq!(exploit["ok"], json!(false));
    assert_eq!(exploit["stage"], json!("oracle_review"));
    assert_eq!(exploit["fail_closed"], json!(true));
    assert!(exploit["refusal"].as_str().unwrap().contains("exploit"));
}

#[test]
fn benchmark_compile_composes_causal_minimization_and_oracle_synthesis_without_execution() {
    let trace = |trace_id: &str, tool: &str, succeeded: bool| {
        json!({
            "trace_id": trace_id,
            "succeeded": succeeded,
            "events": [
                { "step": 0, "kind": "goal", "payload": { "summary": "rank the candidates" } },
                { "step": 1, "kind": "action", "payload": { "tool": "choose_assay", "irreversible": true }, "caused_by": 0 },
                { "step": 2, "kind": "result", "payload": { "summary": "assay selected" }, "caused_by": 1 },
                { "step": 3, "kind": "action", "payload": { "tool": tool }, "caused_by": 2 },
                { "step": 4, "kind": "claim", "payload": { "summary": "reported a hit" }, "caused_by": 3 },
                { "step": 5, "kind": "termination", "payload": { "summary": "done" }, "caused_by": 4 }
            ]
        })
    };
    let signature = |invalid: bool| {
        if invalid {
            json!({ "verdict": "invalid", "witnesses": ["identity_leakage"], "divergence_step": 3 })
        } else {
            json!({ "verdict": "valid", "witnesses": [], "divergence_step": 3 })
        }
    };
    let subsets = vec![
        (vec![], false),
        (vec!["panel_manifest"], true),
        (vec!["unused_service"], false),
        (vec!["stale_memory"], false),
        (vec!["panel_manifest", "unused_service"], true),
        (vec!["panel_manifest", "stale_memory"], true),
        (vec!["unused_service", "stale_memory"], false),
        (
            vec!["panel_manifest", "unused_service", "stale_memory"],
            true,
        ),
    ];
    let observations = subsets
        .into_iter()
        .map(|(kept, invalid)| json!({ "kept": kept, "signature": signature(invalid) }))
        .collect::<Vec<_>>();
    let arguments = json!({
        "trace": trace("run_fail", "run_wrong_panel", false),
        "reference": trace("run_pass", "run_right_panel", true),
        "context": [
            { "id": "panel_manifest", "tier": "artifact", "guard": "removable" },
            { "id": "unused_service", "tier": "service", "guard": "removable" },
            { "id": "stale_memory", "tier": "memory_entry", "guard": "removable" }
        ],
        "probe_observations": observations,
        "budget": { "max_evaluations": 100 }
    });
    let result = call(&mut server(), "benchmark_compile", arguments.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-compile/0.1")
    );
    assert_eq!(
        result["class"],
        json!({ "class": "candidate_research_cell" })
    );
    assert_eq!(result["cell_step"], json!(3));
    assert_eq!(result["minimization"]["minimal"], json!(["panel_manifest"]));
    assert_eq!(
        result["minimization"]["removed"].as_array().unwrap().len(),
        2
    );
    assert_eq!(result["oracle"]["strength"], json!("exact_state_predicate"));
    assert!(result["unmeasured_stages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|stage| stage == "state_reconstruction"));
    assert_eq!(
        result["probe"]["execution"],
        json!("caller-supplied observation table; no world or architecture was run")
    );

    let mut missing = arguments.clone();
    missing["probe_observations"].as_array_mut().unwrap().pop();
    let refused = call(&mut server(), "benchmark_compile", missing);
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("minimization_probe"));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(refused["refusal"].as_str().unwrap().contains("observation"));

    let mut malformed_claim = arguments.clone();
    malformed_claim["claims"] = json!([{
        "status": "evidenced",
        "claim": "the panel caused the failure",
        "citations": []
    }]);
    let claim_refused = call(&mut server(), "benchmark_compile", malformed_claim);
    assert_eq!(claim_refused["__isError"], json!(false));
    assert_eq!(claim_refused["ok"], json!(false));
    assert_eq!(claim_refused["stage"], json!("claim_attribution"));
    assert_eq!(claim_refused["fail_closed"], json!(true));

    let mut reviewed_arguments = arguments;
    reviewed_arguments["reviewer"] = json!("reviewer-1");
    reviewed_arguments["world"] = json!({ "locator": "world.json", "sha256": "a".repeat(64) });
    reviewed_arguments["query"] = json!({ "locator": "query.json", "sha256": "b".repeat(64) });
    reviewed_arguments["grade"] = json!({ "verdict": "invalid", "witnesses": ["identity_leakage"], "closure_complete": true });
    let reviewed = call(
        &mut server(),
        "benchmark_compile_review",
        reviewed_arguments,
    );
    assert_eq!(reviewed["__isError"], json!(false));
    assert_eq!(reviewed["ok"], json!(true));
    assert_eq!(
        reviewed["schema"],
        json!("bioprism-mcp/benchmark-compile-review/0.1")
    );
    assert_eq!(reviewed["reviewer"], json!("reviewer-1"));
    assert_eq!(reviewed["grade"]["acceptance"]["outcome"], json!("passed"));
    assert_eq!(reviewed["cell"]["cell_id"], json!("dc_run_fail#step3"));
}

#[test]
fn pack_catalogue_exposes_agent_and_biological_declarations_without_scores() {
    let result = call(
        &mut server(),
        "pack_catalogue",
        json!({ "section": "29", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["section"], json!("29"));
    assert_eq!(result["section_counts"]["15"], json!(25));
    assert_eq!(result["section_counts"]["29"], json!(21));
    assert_eq!(result["returned"].as_array().unwrap().len(), 3);
    assert_eq!(result["omitted"], json!(18));
    assert!(result["returned"]
        .as_array()
        .unwrap()
        .iter()
        .all(|pack| pack["blueprint_module"]
            .as_str()
            .unwrap()
            .starts_with("29.")));
    assert!(result["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("not measured")));
}

#[test]
fn pack_coverage_audit_exposes_portfolio_gaps_and_refuses_unknown_subsets() {
    let result = call(
        &mut server(),
        "pack_coverage_audit",
        json!({ "section": "15", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/pack-coverage-audit/0.1")
    );
    assert_eq!(result["selected_pack_count"], json!(25));
    assert!(result["summary"]["families"].as_u64().unwrap() > 0);
    assert!(result["summary"]["covered"].as_u64().unwrap() > 0);
    assert_eq!(result["rows"].as_array().unwrap().len(), 3);
    assert!(result["rows_omitted"].as_u64().unwrap() > 0);
    assert!(result["summary"]["gap_summary"]
        .as_str()
        .unwrap()
        .contains("capability families"));

    let refused = call(
        &mut server(),
        "pack_coverage_audit",
        json!({ "pack_ids": ["pack-does-not-exist"] }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("pack_selection"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn pack_release_audit_preserves_stable_order_and_unsequenced_remainder() {
    let result = call(
        &mut server(),
        "pack_release_audit",
        json!({ "section": "15", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/pack-release-audit/0.1")
    );
    assert_eq!(result["selected_pack_count"], json!(25));
    assert_eq!(result["sequenced_count"], json!(13));
    assert_eq!(result["unsequenced_count"], json!(12));
    assert_eq!(result["release_order"].as_array().unwrap().len(), 3);
    assert_eq!(result["release_order_omitted"], json!(10));
    assert_eq!(result["unsequenced"].as_array().unwrap().len(), 3);
    assert_eq!(result["unsequenced_omitted"], json!(9));
    assert_eq!(result["release_order"][0]["selected_position"], json!(1));
    assert_eq!(result["release_order"][0]["portfolio_position"], json!(1));
    assert!(result["wave_counts"].is_object());
    assert!(result["axis_counts"].is_object());
    assert!(result["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("not an approval")));

    let refused = call(
        &mut server(),
        "pack_release_audit",
        json!({ "section": "15", "pack_ids": ["bio.statistical-estimands"] }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("pack_selection"));
    assert_eq!(
        refused["out_of_section_pack_ids"],
        json!(["bio.statistical-estimands"])
    );
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn foundation_contract_check_keeps_admissibility_world_authority_and_plane_checks_separate() {
    let result = call(
        &mut server(),
        "foundation_contract_check",
        json!({
            "contract": {
                "id": "fbc:test:001",
                "intent": "distinguish two declared outcomes",
                "evidence_obligations": ["reference"],
                "actions": ["inspect", "abstain"],
                "claim_schema": "typed-result-v1",
                "falsifiers": ["reference-disagrees"],
                "reference_standard": "deterministic-fixture",
                "minimum_reviewers": 1,
                "uncertainty_required": true,
                "terminations": ["success", "underdetermined"]
            },
            "world": {
                "id": "observed-world",
                "class": "observed_replay",
                "design_support": [],
                "reveal_policy": null,
                "withholds_for_scoring": false
            },
            "claim": "real_treatment_effect",
            "transition": {
                "plane": "observation",
                "effects": ["latent_biological_state"]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["contract"]["ok"], json!(true));
    assert_eq!(result["world"]["ok"], json!(false));
    assert!(result["world"]["claim"]
        .as_str()
        .unwrap()
        .contains("real treatment effect"));
    assert_eq!(result["transition"]["ok"], json!(false));
    assert_eq!(result["verdict"], json!("refused"));

    let missing = call(
        &mut server(),
        "foundation_contract_check",
        json!({
            "contract": {
                "id": "fbc:test:missing",
                "intent": "not falsifiable",
                "actions": ["inspect"],
                "claim_schema": "typed-result-v1",
                "reference_standard": "deterministic-fixture",
                "terminations": ["success"]
            }
        }),
    );
    assert_eq!(missing["__isError"], json!(false));
    assert_eq!(missing["verdict"], json!("refused"));
    assert_eq!(missing["contract"]["ok"], json!(false));
    assert!(missing["contract"]["refusal"]
        .as_str()
        .unwrap()
        .contains("falsifier"));
}

#[test]
fn weavelang_compile_returns_digests_and_replay_is_explicitly_local() {
    let result = call(
        &mut server(),
        "weavelang_compile",
        json!({
            "source": bioprism_weavelang::reference::COMPLETE_PROGRAM,
            "execute": false,
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert!(result["program"]["digest"].as_str().unwrap().len() >= 32);
    assert!(result["program"]["semantic_digest"].as_str().unwrap().len() >= 32);
    assert_eq!(result["execution"]["status"], json!("not_requested"));
    assert_eq!(result["execution"]["mode"], json!("replay"));

    let invalid = call(
        &mut server(),
        "weavelang_compile",
        json!({ "source": "not a weave program" }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"]
        .as_str()
        .unwrap()
        .contains("WeaveLang compilation refused"));
}

#[test]
fn choreography_check_separates_projection_refusal_from_bounded_model_results() {
    let valid = call(
        &mut server(),
        "choreography_check",
        json!({
            "global": {
                "node": "interaction",
                "from": "lead",
                "to": "reviewer",
                "branches": [{
                    "label": "approve",
                    "continuation": { "node": "end" }
                }]
            },
            "bound": { "max_states": 100, "max_depth": 20, "channel_capacity": 2 }
        }),
    );
    assert_eq!(valid["__isError"], json!(false));
    assert_eq!(valid["well_formed"], json!(true));
    assert_eq!(valid["projection_count"], json!(2));
    assert!(valid["model_check"]["deadlock"].is_object());

    let invalid = call(
        &mut server(),
        "choreography_check",
        json!({
            "global": {
                "node": "interaction",
                "from": "lead",
                "to": "lead",
                "branches": [{
                    "label": "self",
                    "continuation": { "node": "end" }
                }]
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["well_formed"], json!(false));
    assert_eq!(invalid["fail_closed"], json!(true));
    assert!(invalid["protocol_error"].is_object());
}

#[test]
fn conformance_run_verifies_fixtures_before_returning_release_evidence() {
    let result = call(
        &mut server(),
        "conformance_run",
        json!({ "include_details": true, "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["suite"]["fixture_drift"], json!([]));
    assert!(result["suite"]["case_count"].as_u64().unwrap() > 0);
    assert!(result["results"].as_array().unwrap().len() <= 3);
    assert!(result["release_decision"].is_object());
    assert!(result["summary"]
        .as_str()
        .unwrap()
        .contains("fiber-compiler-conformance"));
}

#[test]
fn provider_capability_gate_does_not_turn_untested_runtime_checks_into_claims() {
    let result = call(
        &mut server(),
        "provider_capability_gate",
        json!({
            "card": { "provider": "runtime-a", "states": {}, "measurements": [] },
            "required": ["host_escape"],
            "other_card": { "provider": "runtime-b", "states": {}, "measurements": [] }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["gate"]["outcome"], json!("blocked"));
    assert!(result["gate"]["unproven"][0]
        .as_str()
        .unwrap()
        .contains("untested"));
    assert_eq!(
        result["differential"]["HostEscape"]["drift"],
        json!("indeterminate")
    );
    assert_eq!(result["claims"], json!([]));
}

#[test]
fn projection_bundle_keeps_four_views_bound_to_one_compiled_certificate() {
    let result = call(
        &mut server(),
        "projection_bundle",
        json!({
            "world": WORLD,
            "query": QUERY,
            "include_views": false,
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(
        result["projections"],
        json!(["graph", "hypergraph", "timeline", "table"])
    );
    assert!(
        result["provenance"]["section_sha256"]
            .as_str()
            .unwrap()
            .len()
            >= 32
    );
    assert_eq!(result["fidelity"].as_array().unwrap().len(), 4);
    assert!(result["views"].is_null());
}
