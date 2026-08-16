//! Protocol conformance and the security properties of 11.11.

use bioprism_adapter::{TabularProfile, ValueType, VariableMapping};
use bioprism_adaptive::{AdaptivePanel, PanelConfig};
use bioprism_atlas::{
    Atlas, CapabilityDimension, CapabilityFamily, CapabilityId, CapabilityNode, CapabilityOntology,
    EvidenceRecord, EvidenceTier, OracleTier, TrialOutcome, WeightingPolicy,
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
use bioprism_evalengine::{
    compose, Conclusion, Contribution, CoverageFloor, Observation, ReleaseGate, ScoreTier,
    UnknownPolicy,
};
use bioprism_fabric::synth::{Candidate as FabricCandidate, Goal as FabricGoal, RoleGraph};
use bioprism_factory::{Idempotency, Job, ResourceClass, WorkerCapability};
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
use bioprism_lab::{AcquisitionAction, AcquisitionCost, AcquisitionKind, PrivacyBoundary};
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
    ClassifierVersion, ClonalHistory, CohortSelection, DeclaredTransport, DiseaseEpoch,
    EstablishmentCohort, EvaluationDesign, FidelityAxis, FidelityEvidence, MethylationClass,
    MethylationOutcome, ModelIdentity, ModelResult, ModelSystem, Pseudonym, QcOutcome,
    RadiogenomicClaim, RawScore, RegionId, ReplicateStructure, SampleContext, ScoreValue,
    SpecimenObservation, SpecimenSampling, SplitUnit, Subclone, SubcloneId, TumourPopulation,
    VersionedResult,
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
use bioprism_standards::{Measurement, Quantity, Unit};
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
    assert_eq!(tools.len(), 122);
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
    assert_eq!(payload["schema"], json!("bioprism-mcp/telemetry-projection/0.1"));
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
                { "kind": "stage", "job_id": "a-idempotent", "worker_id": "worker-1", "now_nanos": 31_000_000_001i64, "output": { "digest": "a-out" } },
                { "kind": "commit", "job_id": "a-idempotent", "worker_id": "worker-1", "now_nanos": 31_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 31_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 61_000_000_003i64 },
                { "kind": "release_quarantine", "job_id": "b-nonidempotent", "operator": "reviewer-1" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 62_000_000_000i64 },
                { "kind": "fail", "job_id": "b-nonidempotent", "worker_id": "worker-1", "now_nanos": 62_000_000_001i64, "reason": "external service rejected the request" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_000i64 },
                { "kind": "stage", "job_id": "b-nonidempotent", "worker_id": "worker-1", "now_nanos": 63_000_000_001i64, "output": { "effect_id": "effect-1" } },
                { "kind": "commit", "job_id": "b-nonidempotent", "worker_id": "worker-1", "now_nanos": 63_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 93_000_000_003i64 },
                { "kind": "compensate", "job_id": "c-compensable" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 94_000_000_000i64 },
                { "kind": "stage", "job_id": "c-compensable", "worker_id": "worker-1", "now_nanos": 94_000_000_001i64, "output": { "compensated": true } },
                { "kind": "commit", "job_id": "c-compensable", "worker_id": "worker-1", "now_nanos": 94_000_000_002i64 }
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
    assert_eq!(leaderboard["schema"], json!("bioprism-mcp/hub-leaderboard/0.1"));
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
    assert_eq!(result["schema"], json!("bioprism-mcp/bioatlas-publication-audit/0.1"));
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
fn capability_audit_proves_catalogue_and_transport_schema_parity() {
    let mut server = server();
    let result = call(&mut server, "capability_audit", json!({}));
    assert_eq!(result["workflow"], json!("capability_audit"));
    assert_eq!(result["healthy"], json!(true));
    assert_eq!(result["total_groups"], json!(28));
    assert_eq!(result["unique_catalog_tools"], json!(122));
    assert_eq!(result["advertised_tool_count"], json!(122));
    assert_eq!(result["catalog_only_tools"], json!([]));
    assert_eq!(result["advertised_only_tools"], json!([]));
    assert_eq!(result["schema_quality"]["checked"], json!(122));
    assert_eq!(result["schema_quality"]["valid"], json!(122));
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
    assert_eq!(result["groups"].as_array().unwrap().len(), 28);

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
                "budget": 18000
            },
            "policy": "normative",
            "include_markdown": true,
            "max_markdown_chars": 100000
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
    assert_eq!(result["summary"]["measured"], json!(1));
    assert_eq!(result["summary"]["holes"], json!(2));
    assert_eq!(result["composite"]["ok"], json!(false));
    assert!(result["composite"]["refusal"]
        .as_str()
        .unwrap()
        .contains("unmeasured"));
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
    assert_eq!(result["reproduced"], json!(false));
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
    assert_eq!(result["chain_verified"], json!(true));
    assert_eq!(result["entries"], json!(0));
    assert_eq!(result["simulated_steps"].as_array().unwrap().len(), 0);
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
    assert_eq!(result["recorded_requests"], json!(4));
    assert_eq!(result["replay"]["verified"], json!(true));
    assert_eq!(result["replay"]["matched"], json!(true));
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
    assert!(result["execution_error"]
        .as_str()
        .unwrap()
        .contains("budget exhausted"));
    assert_eq!(result["recorded_requests"], json!(1));
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
    assert_eq!(result["released"][0], json!("cohort_analysis"));
    assert_eq!(result["refused"][0], json!("treatment_recommendation"));
    assert_eq!(result["terminal_action"], json!("escalate"));

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
    assert_eq!(result["biological_order"], json!(["baseline", "future"]));
    assert_eq!(result["record_order"], json!(["future", "baseline"]));
    assert_eq!(result["record_order_differs"], json!(true));
    assert_eq!(result["visible_timepoints"], json!(["future"]));
    assert_eq!(result["hidden_from_agent"], json!(["baseline"]));
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
    assert_eq!(unresolved["is_integrated"], json!(false));
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
    assert_eq!(refused["joinable"], json!(false));
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
    assert_eq!(accepted["bridge_declared"], json!(true));
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
    assert_eq!(result["event"], json!(false));
    assert_eq!(result["censoring_reason"], json!("lost_to_follow_up"));
    assert_eq!(result["at_risk_days"], json!(10));
    assert_eq!(result["immortal_time_days"], json!(10));
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
