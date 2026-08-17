//! Deterministic consistency diagnostics for the bounded cross-domain registries.
//!
//! The artifact registry, evidence registry, and workflow-reconciliation registry deliberately
//! have different schemas and verification rules. This module does not merge those schemas. It
//! compares only the exact digest identities exposed by each store, so operators can see whether
//! a trusted source record has a corresponding artifact projection and whether a projection has a
//! source record still retained locally.
//!
//! A clean result means the compared bounded indexes agree at the time each store was observed.
//! It does not mean the stores were read in one transaction, that an omitted record never
//! existed, or that any scientific, clinical, causal, publication, or external-effect claim is
//! valid.

use crate::artifact_registry::ArtifactRecord;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const CROSS_DOMAIN_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-devplat-cross-domain-artifact-audit/0.1";
pub const CROSS_DOMAIN_AUDIT_WORKFLOW: &str = "artifact_registry_cross_store_audit";
pub const MAX_CROSS_DOMAIN_AUDIT_FINDINGS: usize = 1_024;

/// Build a bounded digest-only audit over the three local registry projections.
///
/// The input slices are expected to come from the registries' bounded in-memory indexes. The
/// function still defensively caps every returned set so a future caller cannot accidentally
/// turn a diagnostic endpoint into an unbounded fan-out response.
#[allow(clippy::too_many_arguments)]
pub fn build_cross_domain_audit(
    artifact_records: &[ArtifactRecord],
    evidence_digests: &[String],
    reconciliation_digests: &[String],
    artifact_generation: u64,
    evidence_generation: u64,
    reconciliation_generation: u64,
    artifact_state_digest: Option<String>,
    evidence_state_digest: Option<String>,
    reconciliation_state_digest: Option<String>,
) -> Value {
    let mut artifact_by_kind: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for record in artifact_records {
        artifact_by_kind
            .entry(record.kind.clone())
            .or_default()
            .insert(record.content_digest.clone());
    }
    let evidence = compare_source(
        "mission_evidence_bundle",
        evidence_digests,
        artifact_by_kind
            .get("mission_evidence_bundle")
            .into_iter()
            .flatten(),
        &artifact_by_kind,
    );
    let reconciliation = compare_source(
        "workflow_reconciliation",
        reconciliation_digests,
        artifact_by_kind
            .get("workflow_reconciliation")
            .into_iter()
            .flatten(),
        &artifact_by_kind,
    );

    let findings = evidence
        .findings
        .iter()
        .chain(reconciliation.findings.iter())
        .take(MAX_CROSS_DOMAIN_AUDIT_FINDINGS)
        .cloned()
        .collect::<Vec<_>>();
    let truncated = evidence.findings.len() + reconciliation.findings.len() > findings.len();
    let consistent = findings.is_empty() && !truncated;

    json!({
        "ok": true,
        "schema": CROSS_DOMAIN_AUDIT_SCHEMA_VERSION,
        "workflow": CROSS_DOMAIN_AUDIT_WORKFLOW,
        "consistent": consistent,
        "bounded": true,
        "truncated": truncated,
        "stores": {
            "artifact_registry": {
                "generation": artifact_generation,
                "record_count": artifact_records.len(),
                "state_digest": artifact_state_digest
            },
            "evidence_registry": {
                "generation": evidence_generation,
                "record_count": evidence_digests.len(),
                "state_digest": evidence_state_digest
            },
            "workflow_reconciliation_registry": {
                "generation": reconciliation_generation,
                "record_count": reconciliation_digests.len(),
                "state_digest": reconciliation_state_digest
            }
        },
        "coverage": {
            "mission_evidence_bundle": evidence.report,
            "workflow_reconciliation": reconciliation.report
        },
        "artifact_kind_counts": artifact_by_kind
            .iter()
            .map(|(kind, digests)| (kind, digests.len()))
            .collect::<BTreeMap<_, _>>(),
        "findings": findings,
        "execution": "not_started",
        "guarantees": [
            "source digests are compared only with exact artifact content digests",
            "expected artifact kinds are checked separately so a wrong-kind projection remains visible",
            "missing source rows and orphaned projections are reported instead of being silently reconciled",
            "each store exposes its own generation and digest-protected checkpoint identity"
        ],
        "does_not_claim": [
            "the three stores were read in one atomic transaction",
            "absence from a bounded local store means a record never existed",
            "a matching digest proves causal provenance, scientific validity, clinical meaning, publication approval, or external-effect completion"
        ]
    })
}

#[derive(Debug, Default)]
struct SourceComparison {
    report: Value,
    findings: Vec<Value>,
}

fn compare_source<'a>(
    expected_kind: &str,
    source_digests: &[String],
    expected_artifact_digests: impl IntoIterator<Item = &'a String>,
    artifact_by_kind: &BTreeMap<String, BTreeSet<String>>,
) -> SourceComparison {
    let source = source_digests.iter().cloned().collect::<BTreeSet<_>>();
    let expected = expected_artifact_digests
        .into_iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let all_artifacts = artifact_by_kind
        .values()
        .flat_map(|digests| digests.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = source.difference(&expected).cloned().collect::<Vec<_>>();
    let orphaned = expected.difference(&source).cloned().collect::<Vec<_>>();
    let wrong_kind = source
        .intersection(&all_artifacts)
        .filter(|digest| !expected.contains(*digest))
        .cloned()
        .collect::<Vec<_>>();
    let mut findings = Vec::new();
    for digest in &missing {
        findings.push(json!({
            "code": "source_missing_artifact_projection",
            "kind": expected_kind,
            "digest": digest,
            "severity": "warning"
        }));
    }
    for digest in &orphaned {
        findings.push(json!({
            "code": "orphaned_artifact_projection",
            "kind": expected_kind,
            "digest": digest,
            "severity": "warning"
        }));
    }
    for digest in &wrong_kind {
        let observed_kinds = artifact_by_kind
            .iter()
            .filter(|(_, digests)| digests.contains(digest))
            .map(|(kind, _)| kind.clone())
            .collect::<Vec<_>>();
        findings.push(json!({
            "code": "artifact_projection_kind_mismatch",
            "expected_kind": expected_kind,
            "observed_kinds": observed_kinds,
            "digest": digest,
            "severity": "error"
        }));
    }
    SourceComparison {
        report: json!({
            "source_record_count": source.len(),
            "expected_artifact_projection_count": expected.len(),
            "matching_count": source.intersection(&expected).count(),
            "missing_artifact_projections": missing,
            "orphaned_artifact_projections": orphaned,
            "wrong_kind_projections": wrong_kind,
            "complete": source == expected,
            "expected_artifact_kind": expected_kind
        }),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(digest: &str, kind: &str) -> ArtifactRecord {
        ArtifactRecord {
            content_digest: digest.into(),
            kind: kind.into(),
            subject_id: "subject".into(),
            domains: vec!["oncology".into()],
            parent_digests: Vec::new(),
            declared_digest: None,
            verification: json!({"method": "content_digest"}),
            artifact: json!({"digest": digest}),
        }
    }

    #[test]
    fn reports_missing_orphaned_and_wrong_kind_projection_states() {
        let result = build_cross_domain_audit(
            &[
                record("a", "mission_evidence_bundle"),
                record("b", "domain_report"),
                record("c", "workflow_reconciliation"),
            ],
            &["a".into(), "b".into()],
            &["d".into()],
            3,
            4,
            5,
            Some("artifact-state".into()),
            None,
            Some("reconciliation-state".into()),
        );
        assert_eq!(result["consistent"], json!(false));
        assert_eq!(
            result["coverage"]["mission_evidence_bundle"]["matching_count"],
            1
        );
        assert_eq!(
            result["coverage"]["mission_evidence_bundle"]["wrong_kind_projections"],
            json!(["b"])
        );
        assert_eq!(
            result["coverage"]["workflow_reconciliation"]["missing_artifact_projections"],
            json!(["d"])
        );
        assert_eq!(
            result["coverage"]["workflow_reconciliation"]["orphaned_artifact_projections"],
            json!(["c"])
        );
        assert_eq!(result["stores"]["artifact_registry"]["generation"], 3);
    }

    #[test]
    fn empty_stores_are_consistent() {
        let result = build_cross_domain_audit(&[], &[], &[], 0, 0, 0, None, None, None);
        assert_eq!(result["consistent"], json!(true));
        assert_eq!(result["findings"], json!([]));
    }
}
