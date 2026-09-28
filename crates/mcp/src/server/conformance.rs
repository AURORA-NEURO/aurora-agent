//! Conformance context and executable suite evaluation handlers.

use super::*;

impl Server {
    pub(super) fn conformance_context_compilation_federated_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ContextCompilationFederatedControlRequest")?;
        let receipt =
            crate::research_contracts::run_context_compilation_federated_control_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_conformance::context_compilation_federated_control_plane::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "A2 prospective high-throughput context-compilation admission is pinned to a suite/protocol contract and gated by peer quorum, fixture identity, capacity, policy, protected closure, signed approval, locality, and replay",
                "unknown/speculative evidence remains conditional, contradicted evidence and failed gates block, and negative results plus omissions remain explicit",
                "the control artifact exchanges only digest-bound federation metadata; private context and raw preclinical data remain local"
            ],
            "limitations": [
                "the contract admits and records validation work but does not compile private context or execute conformance suites",
                "the capability is not a clinical decision system and excludes human-subject and clinical-source data"
            ]
        }))
    }

    pub(super) fn conformance_context_compilation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_conformance_context_compilation_assurance_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_conformance::context_compilation_assurance::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "multimodal facts and federation peers are deterministically partitioned into selected, unresolved, blocked, and missing closure states",
                "semantic profile, replay, provenance, policy, protected-closure, locality, aggregate-only, and adversarial gates fail closed",
                "omissions, uncertainty, contradictions, negative evidence, and missing study/modality/peer coverage remain explicit"
            ],
            "limitations": [
                "the harness validates caller-supplied summaries and does not retrieve evidence, execute workflows, or move raw data",
                "the capability is preclinical research infrastructure only and never makes clinical decisions"
            ]
        }))
    }

    pub(super) fn conformance_run(&self, arguments: &Value) -> Result<Value, String> {
        let include_details = arguments
            .get("include_details")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }

        let suite = fiber_suite();
        let fixtures = FixtureStore::load(workspace_fixture_root(), &suite.manifest)
            .map_err(|error| format!("conformance fixture load refused: {error}"))?;
        fixtures
            .verify()
            .map_err(|error| format!("conformance fixture verification refused: {error}"))?;
        let report = suite
            .run(&FiberReference, &fixtures)
            .map_err(|error| format!("conformance run refused: {error}"))?
            .declaring_baseline(shipped_baseline());
        let decision = assess_conformance(&report);
        let results = if include_details {
            Some(
                report
                    .results
                    .iter()
                    .take(max_items as usize)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        Ok(json!({
            "ok": true,
            "suite": {
                "id": report.suite_id.clone(),
                "version": report.suite_version.clone(),
                "digest": report.suite_digest.clone(),
                "fixture_manifest_id": report.fixture_manifest_id.clone(),
                "fixture_count": report.fixture_count,
                "synthetic_fixture_count": report.synthetic_fixtures.len(),
                "case_count": report.results.len(),
                "passed": report.passed(),
                "failed": report.failed(),
                "unsupported": report.unsupported(),
                "errored": report.errored(),
                "fixture_drift": report.fixture_drift.clone(),
                "pyramid": report.pyramid(),
                "fully_conformant": report.is_fully_conformant(),
            },
            "release_decision": decision,
            "summary": report.summary(),
            "results": results,
            "guarantees": [
                "fixture digests are verified before any case result is trusted",
                "required failures, unsupported artifacts, and fixture drift remain distinct",
                "the equal-engineering baseline is declared in the report and is not silently invented",
                "a conformance run is evidence for release review, not an assertion of clinical or production readiness",
            ],
        }))
    }
}
