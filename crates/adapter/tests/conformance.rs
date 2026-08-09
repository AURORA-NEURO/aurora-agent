//! Conformance certification (blueprint 40.17 verification plan, 40.32, 04.06).
//!
//! A conformance suite that only ever sees correct adapters proves nothing: it could be
//! returning `Pass` unconditionally and no test would notice. Every check in
//! [`bioprism_adapter::conformance`] is therefore exercised twice here — once against an
//! adapter that satisfies it, and once against an adapter written specifically to violate it
//! and to look plausible while doing so.
//!
//! The important one is [`SilentDropper`]. It is a perfectly ordinary CSV importer of the kind
//! most pipelines contain: it reads the columns it knows about, ignores the rest, and returns
//! a clean loss report saying it lost nothing. Its facts are valid, its ingest is
//! deterministic, its manifest is stable and its audit is present. The only thing wrong with
//! it is the thing that matters, and the only reason conformance can see it is that the field
//! list comes from an independent probe rather than from the adapter.

use bioprism_adapter::{
    conformance, Adapter, AdapterError, AdapterManifest, ConformanceLevel, Ingestion, LossAudit,
    LossKind, LossSeverity, SemanticLoss, Source, SourceLocation, SourceManifest, Status, Table,
    TabularAdapter, TabularProfile, ValueType, VariableMapping,
};
use bioprism_ids::ContentHash;
use bioprism_scope::ScopeKey;
use serde_json::Value;
use std::cell::Cell;

const COHORT: &[u8] = b"subject,age,site\nS1,41,berlin\nS2,7,lyon\n";

fn source() -> Source {
    Source::bytes("cohort", COHORT.to_vec()).with_format("text/csv")
}

fn manifest_for(source: &Source, adapter: &str) -> SourceManifest {
    SourceManifest {
        source_id: source.id.clone(),
        declared_format: source.declared_format.clone(),
        source_digest: ContentHash::of_bytes(COHORT),
        byte_length: Some(COHORT.len() as u64),
        adapter: adapter.to_string(),
        adapter_version: "0.1.0".into(),
        profile_digest: None,
        provenance: None,
    }
}

fn fact(id: &str, value: Value, scope: ScopeKey, provenance: &[&str]) -> Value {
    bioprism_adapter::FactDraft::new(id, "age", value, scope, SourceLocation::source("cohort"))
        .provenances(provenance.iter().copied())
        .build()
        .unwrap()
}

fn scoped() -> ScopeKey {
    ScopeKey::new().exact("dataset", "cohort")
}

fn honest_profile() -> TabularProfile {
    TabularProfile::new("cohort")
        .scope("subject", "subject")
        .variable(
            "age",
            VariableMapping::new("age").typed(ValueType::Integer),
        )
        .ignore(
            "site",
            "site is modelled as a scope dimension elsewhere",
            LossSeverity::Advisory,
        )
}

/// Reads one column, ignores the rest, and reports no loss. The adapter this suite exists for.
struct SilentDropper;

impl Adapter for SilentDropper {
    fn name(&self) -> &str {
        "test.silent_dropper"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.silent_dropper", "0.1.0", ConformanceLevel::Normalize)
            .accepting(["text/csv"])
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let table = Table::parse("cohort", source.as_bytes("test.silent_dropper")?)?;
        let mut audit = LossAudit::new();
        audit.mapped(SourceLocation::column("cohort", "subject"));

        let facts = table
            .records()
            .iter()
            .enumerate()
            .map(|(index, record)| {
                fact(
                    &format!("fact.subject.{index}"),
                    Value::from(record[0].clone()),
                    scoped(),
                    &["cohort"],
                )
            })
            .collect();

        Ingestion::new(manifest_for(source, "test.silent_dropper"), facts, audit.finish())
    }
}

/// Maps nothing and declares a single source-wide loss, hoping it discharges every column.
struct SourceLevelExcuse;

impl Adapter for SourceLevelExcuse {
    fn name(&self) -> &str {
        "test.source_level_excuse"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new(
            "test.source_level_excuse",
            "0.1.0",
            ConformanceLevel::Normalize,
        )
        .declaring([LossKind::ProvenanceUnavailable])
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let mut audit = LossAudit::new();
        audit.record(
            LossKind::ProvenanceUnavailable,
            LossSeverity::Advisory,
            SourceLocation::source("cohort"),
            "no upstream accession",
        );
        Ingestion::new(
            manifest_for(source, "test.source_level_excuse"),
            Vec::new(),
            audit.finish(),
        )
    }
}

/// Claims to have mapped a column the source does not contain.
struct Fabricator;

impl Adapter for Fabricator {
    fn name(&self) -> &str {
        "test.fabricator"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.fabricator", "0.1.0", ConformanceLevel::Normalize)
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let mut audit = LossAudit::new();
        for column in ["subject", "age", "site", "tumor_grade"] {
            audit.mapped(SourceLocation::column("cohort", column));
        }
        Ingestion::new(
            manifest_for(source, "test.fabricator"),
            Vec::new(),
            audit.finish(),
        )
    }
}

/// Emits a different value on every call.
struct Flaky {
    calls: Cell<u64>,
}

impl Adapter for Flaky {
    fn name(&self) -> &str {
        "test.flaky"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.flaky", "0.1.0", ConformanceLevel::Normalize)
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        self.calls.set(self.calls.get() + 1);
        let mut audit = LossAudit::new();
        for column in ["subject", "age", "site"] {
            audit.mapped(SourceLocation::column("cohort", column));
        }
        Ingestion::new(
            manifest_for(source, "test.flaky"),
            vec![fact(
                "fact.counter",
                Value::from(self.calls.get()),
                scoped(),
                &["cohort"],
            )],
            audit.finish(),
        )
    }
}

/// Describes itself differently on every call.
struct DriftingManifest {
    calls: Cell<u64>,
}

impl Adapter for DriftingManifest {
    fn name(&self) -> &str {
        "test.drifting_manifest"
    }

    fn manifest(&self) -> AdapterManifest {
        self.calls.set(self.calls.get() + 1);
        AdapterManifest::new(
            "test.drifting_manifest",
            format!("0.1.{}", self.calls.get()),
            ConformanceLevel::Normalize,
        )
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let mut audit = LossAudit::new();
        for column in ["subject", "age", "site"] {
            audit.mapped(SourceLocation::column("cohort", column));
        }
        Ingestion::new(
            manifest_for(source, "test.drifting_manifest"),
            Vec::new(),
            audit.finish(),
        )
    }
}

/// Returns facts without auditing anything.
struct Stub;

impl Adapter for Stub {
    fn name(&self) -> &str {
        "test.stub"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.stub", "0.1.0", ConformanceLevel::Normalize)
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        Ingestion::new(
            manifest_for(source, "test.stub"),
            vec![fact("fact.stub", Value::from(1), scoped(), &["cohort"])],
            SemanticLoss::Unaudited {
                reason: "the mapping layer is not written yet".into(),
            },
        )
    }
}

/// Emits a loss kind its manifest never declared.
struct Undeclaring;

impl Adapter for Undeclaring {
    fn name(&self) -> &str {
        "test.undeclaring"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.undeclaring", "0.1.0", ConformanceLevel::Normalize)
            .declaring([LossKind::UnmappedColumn])
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let mut audit = LossAudit::new();
        audit.mapped(SourceLocation::column("cohort", "subject"));
        audit.mapped(SourceLocation::column("cohort", "age"));
        audit.record(
            LossKind::PrecisionReduced,
            LossSeverity::Degrading,
            SourceLocation::column("cohort", "site"),
            "a kind this adapter never declared",
        );
        Ingestion::new(
            manifest_for(source, "test.undeclaring"),
            Vec::new(),
            audit.finish(),
        )
    }
}

/// Emits a fact that claims to hold everywhere, and one with no provenance.
struct Unscoped;

impl Adapter for Unscoped {
    fn name(&self) -> &str {
        "test.unscoped"
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new("test.unscoped", "0.1.0", ConformanceLevel::Normalize)
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let mut audit = LossAudit::new();
        for column in ["subject", "age", "site"] {
            audit.mapped(SourceLocation::column("cohort", column));
        }
        Ingestion::new(
            manifest_for(source, "test.unscoped"),
            vec![
                fact("fact.global", Value::from(1), ScopeKey::new(), &["cohort"]),
                fact("fact.untraceable", Value::from(2), scoped(), &[]),
            ],
            audit.finish(),
        )
    }
}

#[test]
fn an_adapter_that_maps_every_column_or_declares_it_lost_is_certified() {
    let (report, ingestion) =
        conformance::certify(&TabularAdapter::new(honest_profile()), &source()).unwrap();
    assert!(report.verified(), "{}", report.summary());
    assert_eq!(ingestion.fact_count(), 2);
}

#[test]
fn an_adapter_that_reads_one_column_and_ignores_the_rest_fails_loss_completeness() {
    let (report, _) = conformance::certify(&SilentDropper, &source()).unwrap();

    assert_eq!(report.status_of(conformance::Check::Determinism), Some(Status::Pass));
    assert_eq!(
        report.status_of(conformance::Check::LossAuditPerformed),
        Some(Status::Pass)
    );
    assert_eq!(
        report.status_of(conformance::Check::LossCompleteness),
        Some(Status::Fail),
        "the dropped columns must be the only failure"
    );

    let failure = report
        .failures()
        .find(|outcome| outcome.check == conformance::Check::LossCompleteness)
        .unwrap();
    assert!(failure.detail.contains("field=age"), "{}", failure.detail);
    assert!(failure.detail.contains("field=site"), "{}", failure.detail);
}

#[test]
fn a_source_wide_loss_entry_does_not_discharge_the_individual_columns() {
    let (report, _) = conformance::certify(&SourceLevelExcuse, &source()).unwrap();
    assert_eq!(
        report.status_of(conformance::Check::LossCompleteness),
        Some(Status::Fail)
    );
}

#[test]
fn claiming_to_have_mapped_a_column_the_source_lacks_fails_loss_completeness() {
    let (report, _) = conformance::certify(&Fabricator, &source()).unwrap();
    let failure = report
        .failures()
        .find(|outcome| outcome.check == conformance::Check::LossCompleteness)
        .expect("fabricated fields are caught");
    assert!(failure.detail.contains("tumor_grade"), "{}", failure.detail);
}

#[test]
fn an_adapter_whose_output_changes_between_runs_fails_determinism() {
    let adapter = Flaky {
        calls: Cell::new(0),
    };
    let (report, _) = conformance::certify(&adapter, &source()).unwrap();
    assert_eq!(
        report.status_of(conformance::Check::Determinism),
        Some(Status::Fail)
    );
    assert_eq!(
        adapter.calls.get(),
        2,
        "the check stops at the first divergence rather than running every trial"
    );
}

#[test]
fn an_adapter_whose_manifest_depends_on_the_call_fails_manifest_stability() {
    let adapter = DriftingManifest {
        calls: Cell::new(0),
    };
    let (report, _) = conformance::certify(&adapter, &source()).unwrap();
    assert_eq!(
        report.status_of(conformance::Check::ManifestStability),
        Some(Status::Fail)
    );
}

#[test]
fn an_adapter_that_returns_facts_without_auditing_loss_is_not_certified() {
    let (report, _) = conformance::certify(&Stub, &source()).unwrap();
    assert_eq!(
        report.status_of(conformance::Check::LossAuditPerformed),
        Some(Status::Fail)
    );
    assert!(!report.passed());
}

#[test]
fn emitting_a_loss_kind_the_manifest_does_not_declare_is_a_conformance_failure() {
    let (report, _) = conformance::certify(&Undeclaring, &source()).unwrap();
    let failure = report
        .failures()
        .find(|outcome| outcome.check == conformance::Check::DeclaredLossKinds)
        .expect("undeclared kinds are caught");
    assert!(failure.detail.contains("precision_reduced"));
}

#[test]
fn a_fact_with_an_empty_scope_or_no_provenance_fails_fact_integrity() {
    let (report, _) = conformance::certify(&Unscoped, &source()).unwrap();
    let failure = report
        .failures()
        .find(|outcome| outcome.check == conformance::Check::FactIntegrity)
        .expect("unscoped and untraceable facts are caught");
    assert!(failure.detail.contains("empty scope"), "{}", failure.detail);
    assert!(failure.detail.contains("no provenance"), "{}", failure.detail);
}

#[test]
fn loss_completeness_is_reported_as_not_applicable_when_the_source_cannot_be_probed() {
    let unprobeable = Source::bytes("cohort", COHORT.to_vec());
    let (report, _) = conformance::certify(&SilentDropper, &unprobeable).unwrap();
    assert_eq!(
        report.status_of(conformance::Check::LossCompleteness),
        Some(Status::NotApplicable),
        "an unverifiable source must not be reported as a pass"
    );
    assert!(report.passed());
}

#[test]
fn certification_is_itself_deterministic() {
    let adapter = TabularAdapter::new(honest_profile());
    let (first, one) = conformance::certify(&adapter, &source()).unwrap();
    let (second, two) = conformance::certify(&adapter, &source()).unwrap();
    assert_eq!(first, second);
    assert_eq!(one.digest().unwrap(), two.digest().unwrap());
}

#[test]
fn a_certified_report_names_every_check_it_ran() {
    let (report, _) = conformance::certify(&TabularAdapter::new(honest_profile()), &source()).unwrap();
    assert_eq!(report.checks.len(), 6);
    assert!(report.summary().contains("0 failed"));
}
