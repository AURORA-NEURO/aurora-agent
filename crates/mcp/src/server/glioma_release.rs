//! Glioma research release handlers grouped by release workflow.

use super::*;

impl Server {
    /// Plan retention actions from immutable version lineage, storage health, and policy.
    pub(super) fn glioma_version_retention_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: RetentionGovernorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_version_retention_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma version retention request: {error}"))?;
        let plan = plan_glioma_version_retention(&request)
            .map_err(|error| format!("glioma version retention planning refused: {error}"))?;
        Ok(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "version lineage, immutability, pinning, legal hold, replica health, region policy, and minimum retention are evaluated before action planning",
                "protected versions are kept and deletion remains a proposed action subject to policy",
                "the route plans retention and never deletes or relocates artifacts"
            ]
        }))
    }

    /// Assess replica consistency and produce bounded repair tasks for distributed archives.
    pub(super) fn glioma_distributed_archive_mirror(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DistributedMirrorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_distributed_archive_mirror requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma distributed archive mirror request: {error}"))?;
        let status = evaluate_glioma_distributed_archive_mirror(&request)
            .map_err(|error| format!("glioma archive mirror assessment refused: {error}"))?;
        Ok(json!({
            "status": status,
            "dispatch": "not_started",
            "guarantees": [
                "replica content digests, availability, verification freshness, locality, region approval, and required replica count are checked",
                "repair work is bounded and represented as a plan rather than an archive write",
                "the route does not copy artifact bytes or change replica state"
            ]
        }))
    }

    /// Schedule release candidates under compute, reviewer, risk, deadline, and fairness limits.
    pub(super) fn glioma_release_queue_schedule(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleaseQueueScheduleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_queue_schedule requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release queue schedule request: {error}"))?;
        let schedule = schedule_glioma_release_queue(&request)
            .map_err(|error| format!("glioma release queue scheduling refused: {error}"))?;
        Ok(json!({
            "schedule": schedule,
            "dispatch": "not_started",
            "guarantees": [
                "only gate-ready, reviewer-ready candidates can be scheduled within explicit capacity and planning horizon",
                "risk, deadline, and fairness credit determine stable ordering while blocked work remains visible",
                "the schedule dispatches no reviewers, compute jobs, or release operations"
            ]
        }))
    }

    /// Plan a resumable, authorized chunk exchange for a signed research object.
    pub(super) fn glioma_research_object_exchange_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ResearchObjectExchangeRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_exchange_plan requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma research object exchange request: {error}"))?;
        let record = plan_glioma_research_object_exchange(&request).map_err(|error| {
            format!("glioma research object exchange planning refused: {error}")
        })?;
        Ok(json!({
            "record": record,
            "dispatch": "not_started",
            "guarantees": [
                "recipient grant, audience, region, signed manifest, byte cap, chunk identity, digest, and resume cursor are validated",
                "the plan records acknowledged chunks and resumable progress without copying payloads",
                "exchange requires a separate caller-owned transport adapter"
            ]
        }))
    }

    /// Audit an export dependency graph for leakage, unsafe paths, secrets, and restricted data.
    pub(super) fn glioma_research_object_dependency_leakage_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseDependencyLeakageRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_dependency_leakage_audit requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma release dependency audit request: {error}"))?;
        let audit = audit_glioma_release_dependency_leakage(&request)
            .map_err(|error| format!("glioma release dependency leakage audit refused: {error}"))?;
        Ok(json!({
            "audit": audit,
            "dispatch": "not_started",
            "guarantees": [
                "dependency closure, traversal bounds, export prefixes, path escapes, symlink escapes, embedded secrets, and sensitive content are checked",
                "findings and blocked export roots remain explicit even when downstream dependencies are unresolved",
                "the audit reads caller-declared metadata only and never exports artifacts"
            ]
        }))
    }

    /// Migrate an archived research object through an explicit, rollback-aware schema adapter.
    pub(super) fn glioma_archive_migration_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ArchiveMigrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_archive_migration_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma archive migration request: {error}"))?;
        let report = migrate_glioma_archive_object(&request)
            .map_err(|error| format!("glioma archive migration refused: {error}"))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "source and target schemas, field mappings, semantic loss, artifact identity, and rollback requirements are evaluated together",
                "unmapped required fields and excess semantic loss prevent migration readiness",
                "the adapter produces a migration report and does not overwrite archived source objects"
            ]
        }))
    }

    /// Compose digest-bound study objects into a cross-study comparative research object.
    pub(super) fn glioma_multistudy_release_compose(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComparativeReleaseRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multistudy_release_compose requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multistudy release request: {error}"))?;
        let comparative = compose_glioma_multistudy_release(&request)
            .map_err(|error| format!("glioma multistudy release composition refused: {error}"))?;
        Ok(json!({
            "comparative": comparative,
            "dispatch": "not_started",
            "guarantees": [
                "study identity, method and provenance digests, field mappings, limitations, and comparability policy remain explicit",
                "cross-study pooling is controlled by the requested comparable-pooling policy",
                "composition returns aggregate metadata and does not transfer study-level raw data"
            ]
        }))
    }

    /// Build an access-scoped comparative view over an existing multi-study release object.
    pub(super) fn glioma_comparative_release_explore(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComparativeReleaseExplorerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_comparative_release_explore requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma comparative release explorer request: {error}")
            })?;
        let view = explore_glioma_comparative_release(&request)
            .map_err(|error| format!("glioma comparative release exploration refused: {error}"))?;
        Ok(json!({
            "view": view,
            "dispatch": "not_started",
            "guarantees": [
                "requested audience, study scope, concept mapping, access epoch, and cache partition are enforced",
                "protected cross-study cache entries are evicted from the returned view",
                "the explorer emits bounded aggregate cells and does not expose raw study payloads"
            ]
        }))
    }

    /// Compile an event-reconciled, policy-bound continuous-release candidate.
    pub(super) fn glioma_continuous_release_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ContinuousReleaseRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_continuous_release_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma continuous release request: {error}"))?;
        let candidate = compile_glioma_continuous_release(&request)
            .map_err(|error| format!("glioma continuous release compilation refused: {error}"))?;
        Ok(json!({
            "candidate": candidate,
            "dispatch": "not_started",
            "guarantees": [
                "ordered release events are reconciled against required programs, artifacts, schema, policy digest, age, and negative-evidence rules",
                "stale, omitted, or policy-drifted event histories cannot silently become review-ready",
                "the candidate is a review input and is not signed, published, or uploaded"
            ]
        }))
    }

    /// Replay a digest-linked release event chain and verify its terminal lifecycle state.
    pub(super) fn glioma_release_event_protocol_replay(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseEventProtocolRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_release_event_protocol_replay requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma release event protocol request: {error}"))?;
        let report = replay_glioma_release_event_protocol(&request)
            .map_err(|error| format!("glioma release event replay refused: {error}"))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "event order, predecessor digests, authority, object identity, and lifecycle transitions are replay-checked",
                "gaps, forked histories, invalid transitions, and terminal-state mismatch remain explicit",
                "replay validates the event ledger and performs no lifecycle mutation"
            ]
        }))
    }

    /// Normalize release metadata under an approved field map and controlled vocabularies.
    pub(super) fn glioma_release_metadata_normalize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseMetadataNormalizationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_metadata_normalize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release metadata request: {error}"))?;
        let normalization = normalize_glioma_release_metadata(&request)
            .map_err(|error| format!("glioma release metadata normalization refused: {error}"))?;
        Ok(json!({
            "normalization": normalization,
            "dispatch": "not_started",
            "guarantees": [
                "source fields, approved mappings, vocabulary transforms, conflicts, omissions, and reversible changes are explicit",
                "required target fields cannot be inferred without a confirmed inference rule",
                "normalization emits metadata only and does not publish or transfer artifacts"
            ]
        }))
    }

    /// Issue a content-bound release attestation after policy and verification gates.
    pub(super) fn glioma_release_attestation_issue(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SignedReleaseAttestationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_attestation_issue requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release attestation request: {error}"))?;
        let attestation = attest_glioma_release(&request)
            .map_err(|error| format!("glioma release attestation refused: {error}"))?;
        Ok(json!({
            "attestation": attestation,
            "dispatch": "not_started",
            "guarantees": [
                "the attestation binds manifest, provenance, release-gate status, signer authority, policy scope, and verifier results",
                "revoked, inactive, out-of-scope authorities and failed required verifications cannot issue a publishable attestation",
                "this deterministic signature seam does not contact an external key service or publish the object"
            ]
        }))
    }

    /// Verify an attestation against caller-supplied trust roots and expected release identity.
    pub(super) fn glioma_release_signature_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseSignatureVerificationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_signature_verify requires request".to_string())?,
        )
        .map_err(|error| {
            format!("invalid glioma release signature verification request: {error}")
        })?;
        let report = verify_glioma_release_signature(&request)
            .map_err(|error| format!("glioma release signature verification refused: {error}"))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "manifest, provenance, release gate, policy scope, key authority, signature bytes, revocation, and age are independently checked",
                "unknown trust roots and mismatched expected identities fail verification",
                "verification does not authorize publication or external artifact transfer"
            ]
        }))
    }

    /// Check a research object against a target schema and its required evidence profile.
    pub(super) fn glioma_research_object_conformance_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ConformanceRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_conformance_check requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma research object conformance request: {error}")
            })?;
        let report = evaluate_glioma_research_object_conformance(&request).map_err(|error| {
            format!("glioma research object conformance check refused: {error}")
        })?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "required, allowed, forbidden, and extension fields are evaluated separately",
                "artifact presence and locality, provenance, uncertainty, negative evidence, signature, and migration constraints remain explicit",
                "conformance is a schema assessment and does not release or publish the research object"
            ]
        }))
    }

    /// Preview an audience-specific release while redacting restricted sections and artifacts.
    pub(super) fn glioma_release_preview(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleasePreviewRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_preview requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release preview request: {error}"))?;
        let preview = preview_glioma_release_audience(&request)
            .map_err(|error| format!("glioma release preview refused: {error}"))?;
        Ok(json!({
            "preview": preview,
            "dispatch": "not_started",
            "guarantees": [
                "audience, export profile, allowed and redacted sections, artifact visibility, and prior-preview comparison are explicit",
                "the preview returns a bounded release view without transferring or publishing source artifacts"
            ]
        }))
    }

    /// Reconcile the release event ledger and produce a bounded review queue snapshot.
    pub(super) fn glioma_release_queue_snapshot(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleaseQueueRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_queue_snapshot requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release queue request: {error}"))?;
        let snapshot = snapshot_glioma_release_queue(&request)
            .map_err(|error| format!("glioma release queue snapshot refused: {error}"))?;
        Ok(json!({
            "snapshot": snapshot,
            "dispatch": "not_started",
            "guarantees": [
                "candidate state is reconciled from the bounded event ledger with stale telemetry and conflicting observations exposed",
                "reviewer assignment capacity and unresolved release checks remain visible",
                "a queue snapshot schedules no reviewer work and performs no publication"
            ]
        }))
    }

    /// Evaluate transitive release rights, locality, embargo, and audience constraints.
    pub(super) fn glioma_release_shareability_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseShareabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_shareability_check requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release shareability request: {error}"))?;
        let decision = evaluate_glioma_release_shareability(&request)
            .map_err(|error| format!("glioma release shareability evaluation refused: {error}"))?;
        Ok(json!({
            "decision": decision,
            "dispatch": "not_started",
            "guarantees": [
                "transitive dependencies, field classifications, license terms, audience, embargo, rights, and locality are checked together",
                "unknown or restricted dependencies fail closed and remain visible in the decision",
                "a shareability decision authorizes no upload, publication, or artifact transfer"
            ]
        }))
    }
}
