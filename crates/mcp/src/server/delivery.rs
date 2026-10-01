//! CI, engineering delivery, release pipeline, and sandbox admission handlers.

use super::*;

impl Server {
    /// Reconcile caller-supplied CI evidence against a freshly generated workbench plan.
    ///
    /// This is deliberately a structural audit: it never contacts a provider or executes the
    /// checks. A release candidate requires the exact plan digest, complete passing evidence, and
    /// a successful conclusion, while the response keeps provider provenance and limitations
    /// explicit.
    pub(super) fn ci_execution_evidence_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode CI execution evidence input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "CI execution evidence input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let request: CiExecutionEvidenceRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid CI execution evidence input: {error}"))?;
        let audit = audit_ci_execution_evidence(&request)
            .map_err(|error| format!("CI execution evidence refused: {error}"))?;
        Ok(json!({
            "ok": true,
            "workflow": "ci_execution_evidence_audit",
            "schema": bioprism_devplat::CI_EXECUTION_EVIDENCE_SCHEMA,
            "valid": audit.structurally_valid,
            "ci_evidence_ready": audit.release_candidate,
            "plan_digest": audit.plan_digest,
            "evidence_digest": audit.evidence_digest,
            "audit": audit,
            "guarantees": [
                "the canonical plan is regenerated from the supplied CI request before evidence is assessed",
                "complete passing check evidence and a successful conclusion are required for the structural release-candidate signal",
                "the route remains non-executing and preserves missing, unknown, duplicate, failed, skipped, cancelled, and unknown states"
            ],
            "limitations": [
                "the route does not contact GitHub, verify provider signatures, fetch logs, or execute checks",
                "provider_observed and caller_attested are provenance labels rather than cryptographic trust decisions",
                "ci_evidence_ready is not deployment, security, scientific, clinical, or production approval"
            ]
        }))
    }

    /// Normalize a caller-supplied provider payload into canonical CI evidence.
    ///
    /// This is intentionally separate from the evidence audit: normalization understands
    /// provider-shaped fields, while the audit owns plan binding and readiness semantics.
    pub(super) fn ci_provider_normalize(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode CI provider normalization input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "CI provider normalization input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let request: CiProviderNormalizationRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid CI provider normalization input: {error}"))?;
        let normalized = normalize_ci_provider_payload(&request)
            .map_err(|error| format!("CI provider normalization refused: {error}"))?;
        Ok(json!({
            "ok": true,
            "workflow": "ci_provider_normalize",
            "schema": bioprism_devplat::CI_PROVIDER_NORMALIZATION_SCHEMA,
            "provider": normalized.provider,
            "source": normalized.source,
            "payload_digest": normalized.payload_digest,
            "run_id": normalized.run_id,
            "conclusion": normalized.conclusion,
            "check_count": normalized.check_count,
            "derived_result_digest_count": normalized.derived_result_digest_count,
            "warnings": normalized.warnings,
            "evidence": normalized.evidence,
            "normalization": normalized,
            "guarantees": [
                "provider-shaped input is converted into canonical CiRunEvidence before structural auditing",
                "missing result digests are derived deterministically and remain labeled as derived",
                "unknown and non-passing provider states remain visible to ci_execution_evidence_audit"
            ],
            "limitations": [
                "the route accepts caller-supplied payloads and does not contact or authenticate a provider",
                "normalization does not fetch logs, execute checks, verify signatures, or approve a release"
            ]
        }))
    }

    /// Audit normalized provider evidence together with optional artifact, log, and attestation rows.
    pub(super) fn ci_provider_evidence_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode CI provider evidence input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("CI provider evidence input exceeds the 20000000-byte safety bound".into());
        }
        let request: CiProviderEvidenceRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid CI provider evidence input: {error}"))?;
        let audit = audit_ci_provider_evidence(&request)
            .map_err(|error| format!("CI provider evidence audit refused: {error}"))?;
        Ok(json!({
            "ok": true,
            "workflow": "ci_provider_evidence_audit",
            "schema": bioprism_devplat::CI_PROVIDER_EVIDENCE_SCHEMA,
            "valid": audit.structurally_valid,
            "conformance_ready": audit.conformance_ready,
            "provider": audit.provider,
            "source": audit.source,
            "run_id": audit.run_id,
            "payload_digest": audit.payload_digest,
            "plan_digest": audit.plan_digest,
            "evidence_digest": audit.evidence_digest,
            "evidence": audit.evidence,
            "artifact_record_digest": audit.artifact_record_digest,
            "log_record_digest": audit.log_record_digest,
            "attestation_record_digest": audit.attestation_record_digest,
            "audit": audit,
            "guarantees": [
                "provider artifacts, logs, and attestations are retained and bound to normalized identities before a handoff signal is emitted",
                "record digests are deterministic and malformed, duplicate, unknown, or unbound rows remain visible as findings",
                "conformance_ready is structural evidence readiness and never external execution or provider authority"
            ],
            "limitations": [
                "the route does not fetch remote bytes, execute checks, authenticate providers, verify signatures, or approve deployment",
                "caller-supplied record digests identify declarations and do not prove the content at a remote URI"
            ]
        }))
    }

    /// Re-audit and retain one provider evidence request in the shared bounded registry.
    pub(super) fn ci_provider_evidence_import(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode CI provider evidence import input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "CI provider evidence import input exceeds the 20000000-byte safety bound".into(),
            );
        }
        self.ci_provider_evidence_registry
            .lock()
            .map_err(|_| "CI provider evidence registry lock is poisoned".to_string())?
            .import(arguments)
            .map_err(|error| format!("CI provider evidence import refused: {error}"))
    }

    /// Query retained provider evidence without contacting a provider or executing checks.
    pub(super) fn ci_provider_evidence_query(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode CI provider evidence query input: {error}"))?;
        if encoded.len() > 1_000_000 {
            return Err(
                "CI provider evidence query input exceeds the 1000000-byte safety bound".into(),
            );
        }
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| format!("{name} must be a string"))
                })
                .transpose()
        };
        let optional_usize = |name: &str| -> Result<Option<usize>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_u64()
                        .ok_or_else(|| format!("{name} must be an integer"))
                        .and_then(|number| {
                            usize::try_from(number)
                                .map_err(|_| format!("{name} is too large"))
                                .and_then(|value| {
                                    if value <= 128 {
                                        Ok(value)
                                    } else {
                                        Err(format!("{name} must be between 0 and 128"))
                                    }
                                })
                        })
                })
                .transpose()
        };
        let structurally_valid = arguments
            .get("structurally_valid")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or("structurally_valid must be a boolean")
            })
            .transpose()?;
        let conformance_ready = arguments
            .get("conformance_ready")
            .map(|value| value.as_bool().ok_or("conformance_ready must be a boolean"))
            .transpose()?;
        let min_local_byte_hash_artifacts = optional_usize("min_local_byte_hash_artifacts")?;
        let min_local_byte_hash_logs = optional_usize("min_local_byte_hash_logs")?;
        let min_attestation_subject_digest_bindings =
            optional_usize("min_attestation_subject_digest_bindings")?;
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|number| {
                        usize::try_from(number).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        let include_records = arguments
            .get("include_records")
            .map(|value| value.as_bool().ok_or("include_records must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.ci_provider_evidence_registry
            .lock()
            .map_err(|_| "CI provider evidence registry lock is poisoned".to_string())?
            .query(
                optional_string("provider")?,
                optional_string("run_id")?,
                optional_string("plan_digest")?,
                structurally_valid,
                conformance_ready,
                min_local_byte_hash_artifacts,
                min_local_byte_hash_logs,
                min_attestation_subject_digest_bindings,
                optional_string("after")?,
                max_items,
                include_records,
            )
            .map_err(|error| format!("CI provider evidence query refused: {error}"))
    }

    /// Fetch one retained provider evidence audit by its canonical record digest.
    pub(super) fn ci_provider_evidence_get(&self, arguments: &Value) -> Result<Value, String> {
        let digest = arguments
            .get("provider_evidence_digest")
            .and_then(Value::as_str)
            .ok_or("provider_evidence_digest is required and must be a content hash")?;
        self.ci_provider_evidence_registry
            .lock()
            .map_err(|_| "CI provider evidence registry lock is poisoned".to_string())?
            .get(digest)
            .map_err(|error| format!("CI provider evidence get refused: {error}"))
    }

    pub(super) fn engineering_manifest_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized EngineeringManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure engineering manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: EngineeringManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid engineering manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit engineering manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::IssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::IssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "engineering_manifest_audit",
            "schema": ENGINEERING_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "package topology, ticket references, ADR history, and ownership rows are audited independently",
                "the manifest digest is derived from canonical manifest content and is stable across equivalent JSON formatting",
                "dependency readiness is reported as a planning state and never as proof that work ran",
            ],
            "limitations": [
                "the route does not read the repository, run tests, execute CI, contact ticket systems, or authenticate people",
                "a valid manifest is an internally coherent engineering record, not a release approval or production readiness claim",
            ],
        }))
    }

    pub(super) fn engineering_execution_plan(&self, arguments: &Value) -> Result<Value, String> {
        let raw_request = arguments
            .get("request")
            .cloned()
            .ok_or("request is required and must be a serialized EngineeringPlanRequest")?;
        let encoded = serde_json::to_vec(&raw_request)
            .map_err(|error| format!("cannot measure engineering plan request: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("request exceeds the 20000000-byte safety bound".into());
        }
        let request: EngineeringPlanRequest = serde_json::from_value(raw_request)
            .map_err(|error| format!("invalid engineering plan request: {error}"))?;
        let audit = request
            .audit()
            .map_err(|error| format!("cannot derive engineering execution plan: {error}"))?;
        let request_digest = request
            .digest()
            .map_err(|error| format!("cannot digest engineering plan request: {error}"))?
            .to_string();
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::IssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::IssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "engineering_execution_plan",
            "schema": ENGINEERING_PLAN_AUDIT_SCHEMA,
            "request_digest": request_digest,
            "manifest_digest": audit.manifest_digest,
            "plan_digest": audit.plan_digest,
            "valid": audit.valid,
            "engineering_plan_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "dependency waves, package serialization, ticket readiness, and critical path are derived from the authoritative manifest",
                "omitted ticket windows and unfinished dependencies remain explicit instead of being silently scheduled",
                "the route proposes work and never creates tickets, runs CI, mutates a repository, or declares implementation complete",
            ],
            "limitations": [
                "ticket status, ownership, acceptance, and completion are caller-declared and are not verified against an external tracker",
                "the route does not execute a wave, reserve workers, run tests, or contact GitHub, CI, or deployment systems",
                "a valid plan is an auditable proposal, not evidence that any ticket was implemented or released",
            ],
        }))
    }

    pub(super) fn release_pipeline_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized ReleasePipelineManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure release pipeline manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: ReleasePipelineManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid release pipeline manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit release pipeline manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::PipelineIssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::PipelineIssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "release_pipeline_audit",
            "schema": RELEASE_PIPELINE_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "release_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "stage DAG, artifact lineage, attestation binding, promotion order, and rollback declarations are audited independently",
                "production protection, approval, provenance, and signature requirements remain explicit policy checks",
                "the manifest digest binds the declared release plan and is independent of JSON formatting",
            ],
            "limitations": [
                "the route does not execute commands, contact CI, verify cryptographic signatures, query registries, or mutate deployments",
                "issuer identity, approval authority, and external runner success remain caller-declared evidence",
                "a valid manifest is a coherent release plan, not proof that a release was built, approved, or deployed",
            ],
        }))
    }

    pub(super) fn operational_readiness_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized OperationalReadinessManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure operational-readiness manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: OperationalReadinessManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid operational-readiness manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit operational-readiness manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::OperationalIssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::OperationalIssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "operational_readiness_audit",
            "schema": OPERATIONAL_READINESS_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "operationally_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "service objectives, observed indicators, dependency fallbacks, runbooks, incidents, and controls remain separate evidence layers",
                "required operational controls are reported individually rather than hidden in one readiness score",
                "closed incidents retain a postmortem obligation and critical dependencies retain explicit fallback obligations",
            ],
            "limitations": [
                "the route does not query telemetry, page an on-call team, inspect a live dependency, or open an incident",
                "evidence digests, control booleans, ownership, and review status are caller-declared",
                "a valid declaration is operational-readiness evidence, not proof of uptime, recovery, or safe production behavior",
            ],
        }))
    }

    pub(super) fn sandbox_admission_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized SandboxManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure sandbox manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: SandboxManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid sandbox manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit sandbox manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::SandboxIssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::SandboxIssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "sandbox_admission_audit",
            "schema": SANDBOX_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "sandbox_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "artifact identity, content digests, producer/source lineage, and profile binding are audited before admission",
                "rootless execution, read-only roots, no privilege escalation, bounded networking, mounts, capabilities, and resources remain separate layers",
                "dangerous capabilities and released outputs require bounded targets, quarantine, lineage, and independent evidence",
            ],
            "limitations": [
                "the route does not execute code, mount a filesystem, open a socket, read a secret, or enforce a kernel policy",
                "runtime admission, credential revocation, scanners, quarantine storage, and operator response remain external",
                "a valid declaration is admission evidence, not proof that an external runtime enforced it",
            ],
        }))
    }

    pub(super) fn sandbox_runtime_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized SandboxRuntimeManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure sandbox runtime manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: SandboxRuntimeManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid sandbox runtime manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot simulate sandbox runtime: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::SandboxIssueSeverity::Blocking)
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| issue.severity == bioprism_devplat::SandboxIssueSeverity::Warning)
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "sandbox_runtime_simulate",
            "schema": SANDBOX_RUNTIME_AUDIT_SCHEMA,
            "manifest_digest": manifest.digest().map_err(|error| error.to_string())?.to_string(),
            "admission_digest": audit.admission_digest,
            "trace_digest": audit.trace_digest,
            "valid": audit.valid,
            "sandbox_runtime_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "admission, capability, target, and resource decisions are returned as one deterministic trace",
                "refused requests are not charged and stop later requests when stop_on_refusal is enabled",
                "runtime readiness remains false unless admission and every requested step are valid",
            ],
            "limitations": [
                "the route simulates decisions only; it does not start processes, execute code, mount paths, open sockets, or read secrets",
                "the route does not enforce kernel, namespace, cgroup, credential, syscall, or network policy",
                "an external runtime must consume and enforce this contract before any real effect is attempted",
            ],
        }))
    }
}
