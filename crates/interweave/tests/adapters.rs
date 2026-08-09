//! 23.24: grades, the partial bridge to 23.01, loss reports, envelopes, and the conformance matrix.

use bioprism_interweave::adapter::{
    authorize_mcp_call, build_envelope, grade_of_stack_adapter, project, AdapterProfile,
    CarriedSurface, CellOutcome, CliCapture, CliCaptureItem, ConformanceDimension,
    ConformanceMatrix, CloudEventEnvelope, EnvelopeRefusal, Extraction, Grade, McpExposure, Payload,
    Protocol, PublishedResult, UNBRIDGED_FEATURES, UNBRIDGED_SURFACES,
};
use bioprism_fabric::flow::{FlowLabel, Labelling, Sensitivity};
use bioprism_fabric::stack::{Adapter, AdapterRung, LossPolicy, SemanticFeature};
use bioprism_weave::{AuthorityError, AuthorityTable, Capability};
use std::collections::BTreeSet;

fn full_profile(name: &str) -> AdapterProfile {
    CarriedSurface::ALL
        .into_iter()
        .fold(AdapterProfile::new(Protocol::A2A, name), |profile, s| {
            profile.carrying(s)
        })
}

fn labelled(sensitivity: Sensitivity) -> Labelling {
    Labelling::Labelled(FlowLabel::open_at(sensitivity))
}

#[test]
fn grade_requirements_are_cumulative_so_a_higher_grade_contains_a_lower_one() {
    for pair in Grade::ALL.windows(2) {
        assert!(
            pair[0].requires().is_subset(&pair[1].requires()),
            "{:?} is not contained in {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn every_carried_surface_belongs_to_exactly_one_grade() {
    for surface in CarriedSurface::ALL {
        let owners: Vec<Grade> = Grade::ALL
            .into_iter()
            .filter(|g| g.introduces().contains(&surface))
            .collect();
        assert_eq!(owners.len(), 1, "{surface:?} is introduced by {owners:?}");
    }
}

#[test]
fn an_adapter_carrying_g5_surfaces_but_not_trace_correlation_grades_as_g1() {
    let profile = AdapterProfile::new(Protocol::A2A, "skips-g2")
        .carrying(CarriedSurface::StructuredMessages)
        .carrying(CarriedSurface::Artifacts)
        .carrying(CarriedSurface::TaskLifecycle)
        .carrying(CarriedSurface::Continuations)
        .carrying(CarriedSurface::ForkJoin)
        .carrying(CarriedSurface::ReplayHooks);
    assert_eq!(profile.grade(), Grade::G1);
    assert!(profile
        .shortfall(Grade::G2)
        .contains(&CarriedSurface::TraceCorrelation));
}

#[test]
fn an_adapter_carrying_everything_grades_as_g5() {
    assert_eq!(full_profile("complete").grade(), Grade::G5);
    assert!(full_profile("complete").shortfall(Grade::G5).is_empty());
}

#[test]
fn an_adapter_carrying_nothing_grades_as_g0_rather_than_being_ungraded() {
    let profile = AdapterProfile::new(Protocol::Cli, "bare");
    assert_eq!(profile.grade(), Grade::G0);
    assert!(Grade::G0.requires().is_empty());
}

#[test]
fn a_2301_adapter_declaration_cannot_be_graded_above_g1_because_trace_correlation_is_unbridged() {
    let adapter = SemanticFeature::TaskLifecycle;
    let declared = Adapter::new("a2a", "weave", AdapterRung::NativeWeaveSdk, LossPolicy::Reject)
        .supporting(SemanticFeature::Messages)
        .supporting(SemanticFeature::Artifacts)
        .supporting(adapter)
        .supporting(SemanticFeature::Commitments)
        .supporting(SemanticFeature::AuthorityDelegation)
        .supporting(SemanticFeature::ContinuationTransfer);
    let assessment = grade_of_stack_adapter(&declared);
    assert_eq!(assessment.ceiling, Grade::G1);
    assert_eq!(assessment.grade, Grade::G1);
    assert!(assessment
        .unexpressible
        .contains(&CarriedSurface::TraceCorrelation));
}

#[test]
fn the_two_vocabularies_are_bridged_on_exactly_six_pairs() {
    let bridged: Vec<SemanticFeature> = [
        SemanticFeature::Discovery,
        SemanticFeature::TaskLifecycle,
        SemanticFeature::Messages,
        SemanticFeature::Artifacts,
        SemanticFeature::Commitments,
        SemanticFeature::AuthorityDelegation,
        SemanticFeature::ContinuationTransfer,
        SemanticFeature::EpistemicStateDelta,
        SemanticFeature::SecurityLabels,
        SemanticFeature::MessageOrdering,
        SemanticFeature::ClaimVersusVerifiedFact,
    ]
    .into_iter()
    .filter(|f| bioprism_interweave::adapter::surface_of(*f).is_some())
    .collect();
    assert_eq!(bridged.len(), 6);
    assert_eq!(UNBRIDGED_FEATURES.len(), 5);
    assert_eq!(UNBRIDGED_SURFACES.len(), 7);
    assert_eq!(bridged.len() + UNBRIDGED_FEATURES.len(), 11);
}

#[test]
fn unbridged_surfaces_have_no_feature_mapping_into_them() {
    for surface in UNBRIDGED_SURFACES {
        let mapped = [
            SemanticFeature::Discovery,
            SemanticFeature::TaskLifecycle,
            SemanticFeature::Messages,
            SemanticFeature::Artifacts,
            SemanticFeature::Commitments,
            SemanticFeature::AuthorityDelegation,
            SemanticFeature::ContinuationTransfer,
            SemanticFeature::EpistemicStateDelta,
            SemanticFeature::SecurityLabels,
            SemanticFeature::MessageOrdering,
            SemanticFeature::ClaimVersusVerifiedFact,
        ]
        .into_iter()
        .any(|f| bioprism_interweave::adapter::surface_of(f) == Some(surface));
        assert!(!mapped, "{surface:?} is claimed to be unbridged but maps");
    }
}

#[test]
fn a_projection_cannot_be_obtained_without_the_surfaces_it_dropped() {
    let profile = AdapterProfile::new(Protocol::Mcp, "tools-only")
        .carrying(CarriedSurface::StructuredMessages)
        .carrying(CarriedSurface::Artifacts);
    let requested = BTreeSet::from([
        CarriedSurface::StructuredMessages,
        CarriedSurface::Commitments,
        CarriedSurface::Authority,
    ]);
    let projection = project(&profile, &requested);
    assert_eq!(
        projection.carried,
        BTreeSet::from([CarriedSurface::StructuredMessages])
    );
    assert_eq!(
        projection.loss.dropped,
        BTreeSet::from([CarriedSurface::Commitments, CarriedSurface::Authority])
    );
    assert!(!projection.loss.lossless_for_request());
}

#[test]
fn a_projection_that_drops_nothing_still_carries_a_report() {
    let projection = project(
        &full_profile("complete"),
        &BTreeSet::from([CarriedSurface::Commitments]),
    );
    assert!(projection.loss.lossless_for_request());
    assert_eq!(projection.loss.grade, Grade::G5);
    assert_eq!(
        projection.loss.preserved,
        BTreeSet::from([CarriedSurface::Commitments])
    );
}

fn envelope(payload: Payload) -> CloudEventEnvelope {
    CloudEventEnvelope {
        id: "evt-1".into(),
        source: "participant/patcher".into(),
        subject: "thread-9".into(),
        event_type: "aurora.weave.act.claim".into(),
        time: None,
        dataschema: "weave-ir/0.1".into(),
        datacontenttype: "application/json".into(),
        data: payload,
    }
}

#[test]
fn confidential_content_cannot_travel_inline_in_a_cloudevents_envelope() {
    let refusal = build_envelope(
        envelope(Payload::Inline {
            body: "patient-level".into(),
        }),
        &labelled(Sensitivity::Confidential),
    )
    .unwrap_err();
    assert_eq!(
        refusal,
        EnvelopeRefusal::SensitiveInClearMetadata {
            sensitivity: Sensitivity::Confidential
        }
    );
}

#[test]
fn the_same_confidential_content_is_accepted_when_referenced_rather_than_inline() {
    let built = build_envelope(
        envelope(Payload::Referenced {
            hash: "sha256:abc".into(),
        }),
        &labelled(Sensitivity::Confidential),
    );
    assert!(built.is_ok());
}

#[test]
fn unlabelled_payloads_are_refused_inline_rather_than_treated_as_public() {
    let refusal = build_envelope(
        envelope(Payload::Inline {
            body: "unknown".into(),
        }),
        &Labelling::Unlabelled,
    )
    .unwrap_err();
    assert_eq!(refusal, EnvelopeRefusal::LabellingUnknown);
}

#[test]
fn internal_content_may_travel_inline() {
    assert!(build_envelope(
        envelope(Payload::Inline {
            body: "internal note".into()
        }),
        &labelled(Sensitivity::Internal),
    )
    .is_ok());
}

#[test]
fn an_unnamespaced_event_type_is_refused() {
    let mut bare = envelope(Payload::Referenced {
        hash: "sha256:abc".into(),
    });
    bare.event_type = "claim".into();
    let refusal = build_envelope(bare, &labelled(Sensitivity::Public)).unwrap_err();
    assert_eq!(refusal, EnvelopeRefusal::UnnamespacedType("claim".into()));
}

#[test]
fn mcp_exposure_does_not_authorize_a_call_the_authority_table_never_granted() {
    let exposure = McpExposure::new("prism.evaluate", Capability::PublishResult);
    let table = AuthorityTable::new();
    let error = authorize_mcp_call(&exposure, &table, "grant-none").unwrap_err();
    assert!(matches!(error, AuthorityError::UnknownGrant(_)));
}

#[test]
fn mcp_exposure_of_one_tool_does_not_authorize_a_different_capability() {
    let mut table = AuthorityTable::new();
    let grant = table.issue("runner", [Capability::ReadWorld]);
    let exposure = McpExposure::new("prism.publish", Capability::PublishResult);
    let error = authorize_mcp_call(&exposure, &table, &grant).unwrap_err();
    assert!(matches!(error, AuthorityError::Insufficient { .. }));
}

#[test]
fn an_mcp_call_backed_by_a_matching_grant_is_authorized() {
    let mut table = AuthorityTable::new();
    let grant = table.issue("runner", [Capability::PublishResult]);
    let exposure = McpExposure::new("prism.publish", Capability::PublishResult);
    assert!(authorize_mcp_call(&exposure, &table, &grant).is_ok());
}

#[test]
fn a_cli_capture_missing_extraction_uncertainty_reports_it_as_missing() {
    let capture = CliCaptureItem::ALL
        .into_iter()
        .filter(|item| *item != CliCaptureItem::ExtractionUncertainty)
        .fold(CliCapture::new(Extraction::Structured), |capture, item| {
            capture.capturing(item)
        });
    assert_eq!(
        capture.missing(),
        BTreeSet::from([CliCaptureItem::ExtractionUncertainty])
    );
}

#[test]
fn an_undetermined_extraction_does_not_admit_minting_a_typed_act() {
    assert!(!Extraction::Undetermined {
        reason: "free text with no delimiters".into()
    }
    .admits_typed_act());
    assert!(Extraction::Inferred {
        basis: "leading JSON object".into()
    }
    .admits_typed_act());
    assert!(Extraction::Structured.admits_typed_act());
}

#[test]
fn a_conformance_dimension_never_recorded_reads_as_not_tested_rather_than_passed() {
    let matrix = ConformanceMatrix::new().recording(
        ConformanceDimension::Streaming,
        CellOutcome::Passed,
    );
    assert_eq!(
        matrix.outcome(ConformanceDimension::Cancellation),
        CellOutcome::NotTested
    );
    assert_eq!(matrix.untested().len(), 11);
    assert!(matrix.failed().is_empty());
}

#[test]
fn a_published_result_states_a_grade_and_separates_untested_from_failed() {
    let matrix = ConformanceDimension::ALL
        .into_iter()
        .filter(|d| *d != ConformanceDimension::SemanticLoss)
        .fold(ConformanceMatrix::new(), |matrix, dimension| {
            matrix.recording(dimension, CellOutcome::Passed)
        })
        .recording(ConformanceDimension::Retries, CellOutcome::Failed);
    let result = PublishedResult::new(&full_profile("complete"), matrix);
    assert_eq!(result.grade, Grade::G5);
    assert!(!result.fully_exercised());
    assert_eq!(
        result.matrix.untested(),
        BTreeSet::from([ConformanceDimension::SemanticLoss])
    );
    assert_eq!(
        result.matrix.failed(),
        BTreeSet::from([ConformanceDimension::Retries])
    );
}
