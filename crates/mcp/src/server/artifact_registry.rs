//! Bounded artifact registry auditing and integrity-gated projections.

use super::*;

impl Server {
    /// Join bounded mission, evaluator, reconciliation, and domain artifacts by exact content
    /// identity while preserving each artifact's own verification posture.
    pub(super) fn artifact_registry_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode artifact registry input: {error}"))?;
        if encoded.len() > bioprism_devplat::MAX_ARTIFACT_REGISTRY_BYTES {
            return Err(format!(
                "artifact registry input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_ARTIFACT_REGISTRY_BYTES
            ));
        }
        let operation = arguments
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("query");
        if operation == "cross_store" {
            return self.cross_store_artifact_audit();
        }
        let mut registry = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?;
        match operation {
            "register" => {
                let registration = arguments
                    .get("registration")
                    .ok_or("registration is required for artifact registry register")?;
                registry
                    .register(registration)
                    .map_err(|error| format!("artifact registry registration refused: {error}"))
            }
            "query" => {
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
                let kind = optional_string("kind")?;
                let domain = optional_string("domain")?;
                let subject_id = optional_string("subject_id")?;
                let after = optional_string("after")?;
                let max_items = arguments
                    .get("max_items")
                    .map(|value| {
                        value
                            .as_u64()
                            .ok_or_else(|| "max_items must be an integer".to_string())
                            .and_then(|number| {
                                usize::try_from(number)
                                    .map_err(|_| "max_items is too large".to_string())
                            })
                    })
                    .transpose()?
                    .unwrap_or(100);
                let include_artifacts = arguments
                    .get("include_artifacts")
                    .map(|value| {
                        value
                            .as_bool()
                            .ok_or_else(|| "include_artifacts must be a boolean".to_string())
                    })
                    .transpose()?
                    .unwrap_or(false);
                registry
                    .query(
                        kind,
                        domain,
                        subject_id,
                        after,
                        max_items,
                        include_artifacts,
                    )
                    .map_err(|error| format!("artifact registry query refused: {error}"))
            }
            "get" => {
                let digest = arguments
                    .get("content_digest")
                    .and_then(Value::as_str)
                    .ok_or("content_digest is required for artifact registry get")?;
                registry
                    .get(digest)
                    .map_err(|error| format!("artifact registry get refused: {error}"))
            }
            "lineage" => {
                let digest = arguments
                    .get("content_digest")
                    .and_then(Value::as_str)
                    .ok_or("content_digest is required for artifact registry lineage")?;
                registry
                    .lineage(digest)
                    .map_err(|error| format!("artifact registry lineage refused: {error}"))
            }
            "domain_evidence_lineage" => {
                registry
                    .domain_evidence_lineage(arguments)
                    .map_err(|error| {
                        format!("artifact registry domain evidence lineage refused: {error}")
                    })
            }
            "verify_snapshot" => {
                let snapshot = arguments
                    .get("snapshot")
                    .ok_or("snapshot is required for artifact registry verification")?;
                let verified = ArtifactRegistry::from_snapshot(snapshot)
                    .map_err(|error| format!("artifact registry snapshot refused: {error}"))?;
                Ok(json!({
                    "ok": true,
                    "schema": bioprism_devplat::ARTIFACT_REGISTRY_SCHEMA_VERSION,
                    "workflow": "artifact_registry_snapshot_verify",
                    "valid": true,
                    "registry_generation": verified.generation(),
                    "registry_size": verified.len(),
                    "execution": "not_started",
                    "guarantees": [
                        "the outer state digest and every indexed artifact identity were rechecked",
                        "known evidence and reconciliation formats were independently re-verified"
                    ],
                    "does_not_claim": [
                        "scientific, clinical, publication, or external provenance validity"
                    ]
                }))
            }
            other => Err(format!(
                "unknown artifact registry operation {other:?}; choose register, query, get, lineage, domain_evidence_lineage, cross_store, or verify_snapshot"
            )),
        }
    }

    /// Compare the four bounded local registries by exact digest identity.
    ///
    /// Each mutex is observed independently so a slow or poisoned source store cannot hold the
    /// other stores hostage. The returned generations and checkpoint digests make that
    /// non-transactional observation explicit to callers.
    fn cross_store_artifact_audit(&self) -> Result<Value, String> {
        let (artifact_records, artifact_generation, artifact_state_digest) = {
            let registry = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?;
            let snapshot = registry
                .snapshot()
                .map_err(|error| format!("artifact registry snapshot failed: {error}"))?;
            (
                registry.records_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (evidence_digests, evidence_generation, evidence_state_digest) = {
            let registry = self
                .evidence_registry
                .lock()
                .map_err(|_| "evidence registry lock is poisoned".to_string())?;
            let snapshot = registry
                .snapshot()
                .map_err(|error| format!("evidence registry snapshot failed: {error}"))?;
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (reconciliation_digests, reconciliation_generation, reconciliation_state_digest) = {
            let registry = self
                .workflow_reconciliation_registry
                .lock()
                .map_err(|_| "workflow reconciliation registry lock is poisoned".to_string())?;
            let snapshot = registry.snapshot().map_err(|error| {
                format!("workflow reconciliation registry snapshot failed: {error}")
            })?;
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (
            workflow_execution_evidence_digests,
            workflow_execution_evidence_generation,
            workflow_execution_evidence_state_digest,
        ) = {
            let registry = self
                .workflow_execution_evidence_registry
                .lock()
                .map_err(|_| "workflow execution evidence registry lock is poisoned".to_string())?;
            let snapshot = registry.snapshot().map_err(|error| {
                format!("workflow execution evidence registry snapshot failed: {error}")
            })?;
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        Ok(bioprism_devplat::build_cross_domain_audit(
            &artifact_records,
            &evidence_digests,
            &reconciliation_digests,
            &workflow_execution_evidence_digests,
            artifact_generation,
            evidence_generation,
            reconciliation_generation,
            workflow_execution_evidence_generation,
            artifact_state_digest,
            evidence_state_digest,
            reconciliation_state_digest,
            workflow_execution_evidence_state_digest,
        ))
    }

    /// Attach an index projection only to outputs that crossed an explicit integrity boundary.
    ///
    /// This is intentionally a small allow-list. A generic domain tool result is not silently
    /// promoted to a durable artifact merely because it happens to contain a `domain` field.
    /// The projection is appended after registration, so the artifact's content digest remains
    /// the digest of the canonical report rather than a self-referential response envelope.
    pub(super) fn index_trusted_tool_output(
        &self,
        tool: &str,
        arguments: &Value,
        output: &mut Value,
    ) {
        let (kind, subject_id, domains, parent_digests, artifact) = match tool {
            "agent_mission" => return,
            "mission_evidence_bundle_import" => {
                let Some(bundle) = arguments.get("bundle") else {
                    return;
                };
                (
                    "mission_evidence_bundle",
                    bundle
                        .get("mission_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown-mission")
                        .to_string(),
                    evaluator_domains(bundle),
                    Vec::new(),
                    bundle.clone(),
                )
            }
            "domain_workflow_reconcile" => (
                "workflow_reconciliation",
                output
                    .get("mission_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown-mission")
                    .to_string(),
                // The canonical reconciliation record does not carry a domain-label field.
                // Keep this projection domain-neutral so a later explicit import of the exact
                // returned record is idempotent rather than metadata-conflicting.
                Vec::new(),
                Vec::new(),
                output.clone(),
            ),
            "domain_workflow_reconciliation_import" => {
                let Some(record) = arguments.get("record") else {
                    return;
                };
                let artifact = without_artifact_projection(record);
                (
                    "workflow_reconciliation",
                    artifact
                        .get("mission_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown-mission")
                        .to_string(),
                    explicit_domains(&artifact),
                    Vec::new(),
                    artifact,
                )
            }
            "mission_evaluator_replay" => (
                "evaluator_replay",
                output
                    .get("mission_id")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        arguments
                            .pointer("/mission/plan/mission_id")
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("unknown-mission")
                    .to_string(),
                evaluator_domains(output),
                projection_parent(arguments.pointer("/mission/artifact_registry")),
                output.clone(),
            ),
            _ => return,
        };
        let projection =
            self.index_artifact_projection(kind, &subject_id, domains, parent_digests, artifact);
        if let Some(object) = output.as_object_mut() {
            object.insert("artifact_registry".into(), projection);
        }
    }

    pub(super) fn index_artifact_projection(
        &self,
        kind: &str,
        subject_id: &str,
        domains: Vec<String>,
        parent_digests: Vec<String>,
        artifact: Value,
    ) -> Value {
        let registration = json!({
            "kind": kind,
            "subject_id": subject_id,
            "domains": domains,
            "parent_digests": parent_digests,
            "artifact": artifact,
        });
        let result = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())
            .and_then(|mut registry| {
                registry
                    .register(&registration)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(report) => {
                let digest = report.get("content_digest").cloned().unwrap_or(Value::Null);
                json!({
                    "indexed": true,
                    "kind": kind,
                    "subject_id": subject_id,
                    "content_digest": digest,
                    "created": report.get("created").cloned().unwrap_or(Value::Null),
                    "already_present": report.get("already_present").cloned().unwrap_or(Value::Null),
                    "verification": report.get("verification").cloned().unwrap_or(Value::Null),
                    "lookup": digest.as_str().map(|value| format!("/v1/artifacts/{value}")).unwrap_or_default(),
                    "execution": "not_started",
                    "does_not_claim": [
                        "artifact integrity establishes scientific, clinical, regulatory, publication, or external-effect validity",
                        "automatic indexing establishes causal provenance or external storage authority"
                    ]
                })
            }
            Err(error) => json!({
                "indexed": false,
                "kind": kind,
                "subject_id": subject_id,
                "error": error,
                "execution": "not_started",
                "does_not_claim": [
                    "the failed projection means the source report was invalid",
                    "absence from the artifact registry means the source report never existed"
                ]
            }),
        }
    }

    pub(super) fn index_mission_report(&self, mut result: Value) -> Result<Value, String> {
        let subject_id = result
            .pointer("/plan/mission_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "agent mission report omitted plan.mission_id".to_string())?;
        let parent_digests =
            projection_parent(result.pointer("/workflow_reconciliation/artifact_registry"));
        let projection = self.index_artifact_projection(
            "mission_report",
            subject_id,
            mission_domains(&result),
            parent_digests,
            result.clone(),
        );
        result["artifact_registry"] = projection;
        Ok(result)
    }
}
