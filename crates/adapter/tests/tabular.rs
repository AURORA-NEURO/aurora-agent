//! End-to-end invariants of the tabular adapter (blueprint 40.17, 28.00).
//!
//! Each test names the claim it defends. The claims are about what the adapter *refuses* to do
//! as much as about what it produces: guessing a type, dropping a column quietly, inventing an
//! identity, or losing a unit without saying so are all things a permissive CSV importer does
//! by default, and each one is checked here to be impossible.

use bioprism_adapter::{
    Adapter, AdapterError, FramePolicy, LossKind, LossSeverity, OntologyPolicy, SemanticLoss,
    Source, SourceProvenance, TabularAdapter, TabularProfile, UnitPolicy, ValueType,
    VariableMapping,
};
use bioprism_world::Fact;
use serde_json::Value;

fn csv(bytes: &'static [u8]) -> Source {
    Source::bytes("cohort", bytes.to_vec()).with_format("text/csv")
}

fn traced(bytes: &'static [u8]) -> Source {
    csv(bytes).with_provenance(SourceProvenance {
        accession: Some("EGAD00001".into()),
        version: Some("v3".into()),
        retrieved_at: Some("2026-01-02T00:00:00Z".into()),
    })
}

fn subject_profile() -> TabularProfile {
    TabularProfile::new("RG-DEMO-001").scope("subject", "subject")
}

#[test]
fn a_column_with_no_mapping_rule_is_declared_lost_rather_than_dropped_in_silence() {
    let profile = subject_profile().variable("age", VariableMapping::new("age"));
    let source = traced(b"subject,age,free_text_note\nS1,41,anything\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();

    let entries = ingestion.loss().entries();
    let unmapped: Vec<_> = entries
        .iter()
        .filter(|entry| entry.kind == LossKind::UnmappedColumn)
        .collect();
    assert_eq!(unmapped.len(), 1);
    assert_eq!(unmapped[0].location.field.as_deref(), Some("free_text_note"));
}

#[test]
fn a_column_the_adapter_cannot_type_is_declared_lost_not_guessed() {
    let profile = subject_profile().variable("mixed", VariableMapping::new("mixed"));
    let source = traced(b"subject,mixed\nS1,12\nS2,not-a-number\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();

    assert!(ingestion.loss().kinds().contains(&LossKind::TypeUndetermined));
    let values: Vec<&Value> = ingestion
        .facts()
        .iter()
        .map(|fact| &fact["value"])
        .collect();
    assert!(
        values.iter().all(|value| value.is_string()),
        "an untypable column is carried verbatim as text: {values:?}"
    );
}

#[test]
fn a_column_whose_cells_all_parse_as_integers_is_determined_not_reported_as_loss() {
    let profile = subject_profile().variable("age", VariableMapping::new("age"));
    let source = traced(b"subject,age\nS1,41\nS2,7\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();

    assert!(!ingestion.loss().kinds().contains(&LossKind::TypeUndetermined));
    assert_eq!(ingestion.facts()[0]["value"], Value::from(41));
}

#[test]
fn a_column_of_only_empty_cells_cannot_be_typed_and_says_so() {
    let profile = subject_profile().variable("blank", VariableMapping::new("blank"));
    let source = traced(b"subject,blank\nS1,\nS2,\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();

    assert!(ingestion.loss().kinds().contains(&LossKind::TypeUndetermined));
}

#[test]
fn boolean_columns_accept_only_the_two_literals_and_never_yes_or_one() {
    let profile = subject_profile().variable("relapsed", VariableMapping::new("relapsed"));
    let determined = TabularAdapter::new(profile.clone())
        .ingest(&traced(b"subject,relapsed\nS1,true\nS2,false\n"))
        .unwrap();
    assert_eq!(determined.facts()[0]["value"], Value::Bool(true));

    let guessed = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,relapsed\nS1,yes\nS2,no\n"))
        .unwrap();
    assert!(guessed.loss().kinds().contains(&LossKind::TypeUndetermined));
}

#[test]
fn a_declared_type_that_the_data_contradict_is_an_error_rather_than_a_coercion() {
    let profile = subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Integer),
    );
    let error = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,forty\n"))
        .unwrap_err();
    assert!(matches!(error, AdapterError::ValueTypeMismatch { .. }));
}

#[test]
fn a_carried_unit_travels_inside_the_value_and_produces_no_loss() {
    let profile = subject_profile().variable(
        "volume",
        VariableMapping::new("tumor_volume")
            .typed(ValueType::Real)
            .unit(UnitPolicy::Carried("mm^3".into())),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,volume\nS1,2.5\n"))
        .unwrap();

    assert_eq!(ingestion.facts()[0]["value"]["unit"], Value::from("mm^3"));
    assert!(!ingestion.loss().kinds().contains(&LossKind::UnpreservedUnit));
}

#[test]
fn a_unit_the_mapping_cannot_carry_is_reported_as_blocking_loss() {
    let profile = subject_profile().variable(
        "dose",
        VariableMapping::new("dose").typed(ValueType::Real).unit(UnitPolicy::Dropped {
            unit: "mg/kg".into(),
            reason: "the target variable is defined as an absolute milligram dose".into(),
        }),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,dose\nS1,2.5\n"))
        .unwrap();

    let entry = ingestion
        .loss()
        .entries()
        .iter()
        .find(|entry| entry.kind == LossKind::UnpreservedUnit)
        .expect("a dropped unit is reported");
    assert_eq!(entry.severity, LossSeverity::Blocking);
    assert_eq!(entry.location.field.as_deref(), Some("dose"));
    assert!(entry.detail.contains("mg/kg"));
}

#[test]
fn a_coordinate_frame_that_is_not_carried_is_reported_at_the_column_that_lost_it() {
    let profile = subject_profile().variable(
        "position",
        VariableMapping::new("variant_position")
            .typed(ValueType::Integer)
            .frame(FramePolicy::Dropped {
                frame: "GRCh37".into(),
                reason: "the world's coordinate contract is GRCh38 and no liftover was run".into(),
            }),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,position\nS1,140753336\n"))
        .unwrap();

    let entry = ingestion
        .loss()
        .entries()
        .iter()
        .find(|entry| entry.kind == LossKind::CoordinateFrameNotCarried)
        .expect("a dropped frame is reported");
    assert_eq!(entry.location.field.as_deref(), Some("position"));
    assert!(entry.detail.contains("GRCh37"));
}

#[test]
fn an_ontology_term_carried_as_free_text_is_reported_and_a_versioned_mapping_is_not() {
    let unmapped = subject_profile().variable(
        "gene",
        VariableMapping::new("gene").typed(ValueType::Text).ontology(
            OntologyPolicy::Unmapped {
                ontology: "HGNC".into(),
            },
        ),
    );
    let ingestion = TabularAdapter::new(unmapped)
        .ingest(&traced(b"subject,gene\nS1,BRAF\n"))
        .unwrap();
    assert!(ingestion
        .loss()
        .kinds()
        .contains(&LossKind::OntologyTermUnmapped));

    let mapped = subject_profile().variable(
        "gene",
        VariableMapping::new("gene").typed(ValueType::Text).ontology(
            OntologyPolicy::Mapped {
                ontology: "HGNC".into(),
                version: "2026-01".into(),
            },
        ),
    );
    let ingestion = TabularAdapter::new(mapped)
        .ingest(&traced(b"subject,gene\nS1,BRAF\n"))
        .unwrap();
    assert!(!ingestion
        .loss()
        .kinds()
        .contains(&LossKind::OntologyTermUnmapped));
    assert_eq!(
        ingestion.facts()[0]["value"]["ontology_version"],
        Value::from("2026-01")
    );
}

#[test]
fn a_decimal_literal_that_f64_cannot_hold_is_reported_at_the_exact_cell() {
    let profile = subject_profile().variable(
        "measure",
        VariableMapping::new("measure").typed(ValueType::Real),
    );
    let source = traced(b"subject,measure\nS1,2.5\nS2,0.12345678901234567890123\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();

    let entries: Vec<_> = ingestion
        .loss()
        .entries()
        .iter()
        .filter(|entry| entry.kind == LossKind::PrecisionReduced)
        .collect();
    assert_eq!(entries.len(), 1, "only the second row loses precision");
    assert_eq!(entries[0].location.record, Some(2));
    assert_eq!(entries[0].location.field.as_deref(), Some("measure"));
}

#[test]
fn an_empty_identity_cell_is_an_error_because_the_adapter_must_not_invent_a_subject() {
    let profile = subject_profile().variable("age", VariableMapping::new("age"));
    let error = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,41\n,42\n"))
        .unwrap_err();
    match error {
        AdapterError::AmbiguousIdentity {
            location,
            dimension,
        } => {
            assert_eq!(location.record, Some(2));
            assert_eq!(dimension, "subject");
        }
        other => panic!("expected an ambiguous identity error, got {other}"),
    }
}

#[test]
fn a_mapping_rule_for_a_column_the_source_lacks_is_schema_drift_not_a_silent_skip() {
    let profile = subject_profile().variable("stage", VariableMapping::new("stage"));
    let error = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,41\n"))
        .unwrap_err();
    assert!(matches!(error, AdapterError::SchemaDrift { .. }));
}

#[test]
fn a_source_with_no_upstream_accession_declares_provenance_unavailable() {
    let profile = subject_profile().variable("age", VariableMapping::new("age"));
    let ingestion = TabularAdapter::new(profile)
        .ingest(&csv(b"subject,age\nS1,41\n"))
        .unwrap();
    assert!(ingestion
        .loss()
        .kinds()
        .contains(&LossKind::ProvenanceUnavailable));
}

#[test]
fn a_fully_specified_mapping_over_a_traced_source_is_lossless_not_merely_unaudited() {
    let profile = subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Integer),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,41\n"))
        .unwrap();
    assert!(matches!(ingestion.loss(), SemanticLoss::Lossless { .. }));
    assert!(ingestion.loss().is_audited());
}

#[test]
fn an_ignored_column_is_still_a_loss_carrying_the_authors_reason() {
    let profile = subject_profile()
        .variable("age", VariableMapping::new("age"))
        .ignore(
            "internal_row_key",
            "an export artefact with no biological meaning",
            LossSeverity::Advisory,
        );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age,internal_row_key\nS1,41,88213\n"))
        .unwrap();

    let entry = ingestion
        .loss()
        .entries()
        .iter()
        .find(|entry| entry.location.field.as_deref() == Some("internal_row_key"))
        .expect("an ignored column is reported");
    assert_eq!(entry.severity, LossSeverity::Advisory);
    assert!(entry.detail.contains("no biological meaning"));
}

#[test]
fn quoted_fields_with_embedded_commas_and_crlf_survive_into_the_facts() {
    let profile = subject_profile().variable(
        "note",
        VariableMapping::new("note").typed(ValueType::Text),
    );
    let source = traced(b"subject,note\r\nS1,\"left, then right\"\r\n");
    let ingestion = TabularAdapter::new(profile).ingest(&source).unwrap();
    assert_eq!(
        ingestion.facts()[0]["value"],
        Value::from("left, then right")
    );
}

#[test]
fn every_emitted_fact_parses_under_the_world_schema_and_names_its_cell() {
    let profile = subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Integer),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,41\nS2,7\n"))
        .unwrap();

    let facts: Vec<Fact> = ingestion.typed_facts().unwrap();
    assert_eq!(facts.len(), 2);
    for fact in &facts {
        assert!(fact
            .provenance
            .iter()
            .any(|entry| entry.contains("cohort#record=")));
        assert!(fact.provenance.iter().any(|entry| entry == "version=v3"));
        assert!(fact.scope.get("subject").is_some());
    }
}

#[test]
fn an_empty_cell_becomes_null_rather_than_zero() {
    let profile = subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Integer),
    );
    let ingestion = TabularAdapter::new(profile)
        .ingest(&traced(b"subject,age\nS1,\n"))
        .unwrap();
    assert_eq!(ingestion.facts()[0]["value"], Value::Null);
}

#[test]
fn a_tab_separated_source_is_certified_the_same_way_a_comma_separated_one_is() {
    let profile = TabularProfile::new("RG-DEMO-001")
        .with_delimiter(b'\t')
        .scope("subject", "subject")
        .variable(
            "age",
            VariableMapping::new("age").typed(ValueType::Integer),
        );
    let source = Source::bytes("cohort", b"subject\tage\nS1\t41\n".to_vec())
        .with_format("text/tab-separated-values")
        .with_provenance(SourceProvenance {
            accession: Some("EGAD00001".into()),
            version: None,
            retrieved_at: None,
        });

    let (report, ingestion) =
        bioprism_adapter::conformance::certify(&TabularAdapter::new(profile), &source).unwrap();
    assert!(report.verified(), "{}", report.summary());
    assert_eq!(ingestion.facts()[0]["value"], Value::from(41));
}

#[test]
fn the_profile_digest_distinguishes_two_worlds_built_from_the_same_bytes() {
    let bytes = b"subject,age\nS1,41\n";
    let integer = TabularAdapter::new(subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Integer),
    ))
    .ingest(&traced(bytes))
    .unwrap();
    let text = TabularAdapter::new(subject_profile().variable(
        "age",
        VariableMapping::new("age").typed(ValueType::Text),
    ))
    .ingest(&traced(bytes))
    .unwrap();

    assert_eq!(
        integer.manifest().source_digest,
        text.manifest().source_digest
    );
    assert_ne!(
        integer.manifest().profile_digest,
        text.manifest().profile_digest
    );
}
