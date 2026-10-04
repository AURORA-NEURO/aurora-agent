//! MCP Cross-domain operational readiness handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Compose independently retained domain, routing, operations, release, and workflow
    /// evidence into one bounded control-plane posture.
    ///
    /// This is an evidence join, not a super-gate. Every component keeps its own state and
    /// authority boundary, while the caller's explicit policy decides which missing or unsatisfied
    /// components block the projection. The operation never dispatches a nested tool and never
    /// turns structural completeness into execution or release authority.
    pub(super) fn control_plane_readiness_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode control-plane readiness input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "control-plane readiness input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let object = arguments
            .as_object()
            .ok_or("control-plane readiness input must be an object")?;
        let subject_id = object
            .get("subject_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("subject_id must be a non-empty string")?;
        let policy_object = object
            .get("policy")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let policy_bool = |name: &str, default: bool| -> Result<bool, String> {
            match policy_object.get(name) {
                None => Ok(default),
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| format!("policy.{name} must be a boolean")),
            }
        };
        let require_domain_readiness = policy_bool("require_domain_readiness", true)?;
        let require_route_review = policy_bool("require_route_review", false)?;
        let require_route_plan = policy_bool("require_route_plan", false)?;
        let require_operations_acceptance = policy_bool("require_operations_acceptance", false)?;
        let require_release_ready = policy_bool("require_release_ready", false)?;
        let require_workflow_evidence = policy_bool("require_workflow_evidence", false)?;
        let policy = json!({
            "require_domain_readiness": require_domain_readiness,
            "require_route_review": require_route_review,
            "require_route_plan": require_route_plan,
            "require_operations_acceptance": require_operations_acceptance,
            "require_release_ready": require_release_ready,
            "require_workflow_evidence": require_workflow_evidence
        });

        let valid_digest = |value: Option<&Value>| -> Option<String> {
            value
                .and_then(Value::as_str)
                .filter(|digest| bioprism_ids::ContentHash::parse((*digest).to_string()).is_ok())
                .map(str::to_string)
        };
        let mut parent_digests = BTreeSet::new();
        let mut domains = BTreeSet::new();
        let mut components = Map::new();

        let readiness_component = if let Some(value) = object.get("readiness_audit") {
            let mut valid = true;
            let mut errors: Vec<String> = Vec::new();
            if value.get("workflow").and_then(Value::as_str)
                != Some(DOMAIN_DECISION_READINESS_WORKFLOW)
            {
                valid = false;
                errors.push("workflow must be domain_decision_readiness_audit".to_string());
            }
            if value.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_DECISION_READINESS_SCHEMA_VERSION)
            {
                valid = false;
                errors.push("schema must be the domain decision-readiness schema".to_string());
            }
            if value.get("ok").and_then(Value::as_bool) != Some(true)
                || value.get("readiness_claimed").and_then(Value::as_bool) != Some(false)
                || value.get("execution").and_then(Value::as_str) != Some("not_started")
            {
                valid = false;
                errors.push("readiness audit must be successful and non-executing".to_string());
            }
            let audit = value.get("audit");
            if let Some(audit) = audit {
                if let Err(error) = validate_domain_decision_readiness(audit) {
                    valid = false;
                    errors.push(format!("audit integrity validation failed: {error}"));
                }
                if audit.get("subject_id").and_then(Value::as_str) != Some(subject_id) {
                    valid = false;
                    errors.push(
                        "audit subject_id does not match the control-plane subject_id".into(),
                    );
                }
                if let Some(reports) = audit
                    .pointer("/harmonization/reports")
                    .and_then(Value::as_array)
                {
                    for report in reports {
                        if let Some(values) = report.get("domains").and_then(Value::as_array) {
                            domains.extend(
                                values.iter().filter_map(Value::as_str).map(str::to_string),
                            );
                        }
                    }
                }
            } else {
                valid = false;
                errors.push("audit is required".into());
            }
            let satisfied = valid
                && value
                    .pointer("/audit/policy_satisfied")
                    .and_then(Value::as_bool)
                    == Some(true);
            let state = value
                .pointer("/audit/decision_state")
                .and_then(Value::as_str)
                .unwrap_or(if valid { "incomplete" } else { "blocked" });
            let registry_digest = valid_digest(value.pointer("/artifact_registry/content_digest"));
            if value
                .pointer("/artifact_registry/indexed")
                .and_then(Value::as_bool)
                != Some(true)
                || value
                    .pointer("/artifact_registry/kind")
                    .and_then(Value::as_str)
                    != Some("domain_decision_readiness")
            {
                valid = false;
                errors.push(
                    "artifact_registry must identify an indexed domain_decision_readiness artifact"
                        .into(),
                );
            }
            if let Some(digest) = registry_digest.as_ref() {
                parent_digests.insert(digest.clone());
            } else {
                valid = false;
                errors.push("artifact_registry.content_digest must be a valid digest".into());
            }
            json!({
                "present": true,
                "valid": valid,
                "satisfied": satisfied && valid,
                "state": if valid { state } else { "blocked" },
                "workflow": DOMAIN_DECISION_READINESS_WORKFLOW,
                "evidence_digest": value.pointer("/audit/digest"),
                "content_digest": registry_digest,
                "errors": errors,
                "authority": "structural_domain_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        } else {
            json!({
                "present": false,
                "valid": false,
                "satisfied": false,
                "state": "incomplete",
                "workflow": DOMAIN_DECISION_READINESS_WORKFLOW,
                "errors": ["readiness_audit was not supplied"],
                "authority": "structural_domain_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        };
        components.insert("domain_decision_readiness".into(), readiness_component);

        let route_plan = object.get("route_plan");
        let route_review = object.get("route_review");
        let route_component = if let Some(value) = route_plan.or(route_review) {
            let is_plan = route_plan.is_some();
            let mut valid = true;
            let mut errors: Vec<String> = Vec::new();
            let review = if is_plan {
                value.get("review")
            } else {
                Some(value)
            };
            if value.get("workflow").and_then(Value::as_str)
                != Some(if is_plan {
                    "capability_route_plan"
                } else {
                    "capability_route_review"
                })
            {
                valid = false;
                errors.push("route packet workflow is not the declared route workflow".into());
            }
            if value.get("execution").and_then(Value::as_str) != Some("not_started")
                || (is_plan && value.get("dispatch").and_then(Value::as_str) != Some("not_started"))
            {
                valid = false;
                errors.push("route packet must remain non-executing".into());
            }
            let review_ready = review
                .and_then(|review| review.get("review_status").and_then(Value::as_str))
                == Some("ready");
            if !review_ready {
                errors.push("route review_status is not ready".into());
            }
            if review.is_none() {
                valid = false;
                errors.push("route plan/review must contain a review object".into());
            }
            let plan_ready = !is_plan
                || value.get("plan_status").and_then(Value::as_str)
                    == Some("ready_for_caller_inspection");
            if is_plan && !plan_ready {
                errors.push("route plan is not ready_for_caller_inspection".into());
            }
            let route_id = valid_digest(review.and_then(|row| row.get("route_id")));
            let review_id = valid_digest(review.and_then(|row| row.get("review_id")));
            let plan_digest = valid_digest(value.get("plan_digest"));
            if route_id.is_none() || review_id.is_none() || (is_plan && plan_digest.is_none()) {
                valid = false;
                errors
                    .push("route identities and plan digest must be valid content digests".into());
            }
            for digest in [route_id.clone(), review_id.clone(), plan_digest.clone()]
                .into_iter()
                .flatten()
            {
                parent_digests.insert(digest);
            }
            let satisfied = valid && review_ready && plan_ready;
            json!({
                "present": true,
                "valid": valid,
                "satisfied": satisfied,
                "state": if satisfied { "ready_for_human_review" } else if valid { "incomplete" } else { "blocked" },
                "workflow": value.get("workflow"),
                "route_id": route_id,
                "review_id": review_id,
                "plan_digest": plan_digest,
                "errors": errors,
                "authority": "non_executing_route_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        } else {
            json!({
                "present": false,
                "valid": false,
                "satisfied": false,
                "state": "incomplete",
                "workflow": "capability_route_review",
                "errors": ["route_review or route_plan was not supplied"],
                "authority": "non_executing_route_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        };
        components.insert("capability_route".into(), route_component);

        let operations_projection = object
            .get("operations_gate_projection")
            .or_else(|| object.get("operations_gate"));
        let operations_review = object.get("operations_gate_review");
        let operations_component = if let Some(value) = operations_projection {
            let mut valid = true;
            let mut errors: Vec<String> = Vec::new();
            if value.get("schema").and_then(Value::as_str)
                != Some("bioprism-operations-preflight-evidence/0.1")
            {
                valid = false;
                errors.push("operations projection schema is invalid".into());
            }
            if value.get("readiness_claimed").and_then(Value::as_bool) == Some(true)
                || value
                    .get("acceptance_matches_current_gates")
                    .and_then(Value::as_bool)
                    != Some(true)
                || value.get("review_present").and_then(Value::as_bool) != Some(true)
            {
                valid = false;
                errors
                    .push("operations projection is not bound to a current retained review".into());
            }
            let gate_digest = valid_digest(value.get("gate_digest"));
            let review_id = valid_digest(value.get("review_id"));
            if gate_digest.is_none() || review_id.is_none() {
                valid = false;
                errors.push(
                    "operations gate_digest and review_id must be valid content digests".into(),
                );
            }
            if let Some(review) = operations_review {
                if review.get("workflow").and_then(Value::as_str) != Some("operations_gate_review")
                    || review.get("readiness_claimed").and_then(Value::as_bool) == Some(true)
                    || valid_digest(review.get("review_id")) != review_id
                    || valid_digest(review.get("gate_digest")) != gate_digest
                {
                    valid = false;
                    errors.push(
                        "operations gate review does not match the projection identities".into(),
                    );
                }
            }
            for digest in [gate_digest.clone(), review_id.clone()]
                .into_iter()
                .flatten()
            {
                parent_digests.insert(digest);
            }
            let acceptance_valid =
                value.get("acceptance_valid").and_then(Value::as_bool) == Some(true);
            let satisfied = valid && acceptance_valid;
            json!({
                "present": true,
                "valid": valid,
                "satisfied": satisfied,
                "state": if satisfied { "ready_for_human_review" } else if valid { "review_required" } else { "blocked" },
                "workflow": "operations_gate_review",
                "gate_digest": gate_digest,
                "review_id": review_id,
                "acceptance_valid": acceptance_valid,
                "review_supplied": operations_review.is_some(),
                "errors": errors,
                "authority": "operator_gate_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        } else {
            json!({
                "present": false,
                "valid": false,
                "satisfied": false,
                "state": "incomplete",
                "workflow": "operations_gate_review",
                "errors": ["operations_gate_projection was not supplied"],
                "authority": "operator_gate_evidence_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        };
        components.insert("operations_acceptance".into(), operations_component);

        let release_component = if let Some(value) = object.get("release_audit") {
            let workflow = value.get("workflow").and_then(Value::as_str).unwrap_or("");
            let workflow_allowed = matches!(workflow, "release_audit" | "release_pipeline_audit");
            let valid = workflow_allowed
                && value.get("ok").and_then(Value::as_bool) == Some(true)
                && value.get("readiness_claimed").and_then(Value::as_bool) != Some(true)
                && value.get("execution").and_then(Value::as_str) != Some("started");
            let satisfied = valid
                && (value.get("release_ready").and_then(Value::as_bool) == Some(true)
                    || (workflow == "release_pipeline_audit"
                        && value.get("valid").and_then(Value::as_bool) == Some(true)));
            let digest = bioprism_ids::ContentHash::of_value(value)
                .map_err(|error| format!("release audit could not be digested: {error}"))?
                .to_string();
            parent_digests.insert(digest.clone());
            json!({
                "present": true,
                "valid": valid,
                "satisfied": satisfied,
                "state": if satisfied { "ready_for_human_review" } else if valid { "incomplete" } else { "blocked" },
                "workflow": workflow,
                "content_digest": digest,
                "release_ready": value.get("release_ready").or_else(|| value.get("valid")),
                "errors": if valid { json!([]) } else { json!(["release audit workflow, ok, or non-executing posture is invalid"]) },
                "authority": "local_release_structure_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        } else {
            json!({
                "present": false,
                "valid": false,
                "satisfied": false,
                "state": "incomplete",
                "workflow": "release_audit",
                "errors": ["release_audit was not supplied"],
                "authority": "local_release_structure_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        };
        components.insert("release".into(), release_component);

        let workflow_component = if let Some(value) = object.get("workflow_evidence") {
            let workflow = value.get("workflow").and_then(Value::as_str).unwrap_or("");
            let valid = !workflow.trim().is_empty()
                && value.get("ok").and_then(Value::as_bool) == Some(true)
                && value.get("readiness_claimed").and_then(Value::as_bool) != Some(true)
                && value.get("execution").and_then(Value::as_str) != Some("started");
            let digest = bioprism_ids::ContentHash::of_value(value)
                .map_err(|error| format!("workflow evidence could not be digested: {error}"))?
                .to_string();
            parent_digests.insert(digest.clone());
            json!({
                "present": true,
                "valid": valid,
                "satisfied": valid,
                "state": if valid { "ready_for_human_review" } else { "blocked" },
                "workflow": workflow,
                "content_digest": digest,
                "errors": if valid { json!([]) } else { json!(["workflow evidence must be successful and non-executing"]) },
                "authority": "workflow_structure_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        } else {
            json!({
                "present": false,
                "valid": false,
                "satisfied": false,
                "state": "incomplete",
                "workflow": "workflow_evidence",
                "errors": ["workflow_evidence was not supplied"],
                "authority": "workflow_structure_only",
                "readiness_claimed": false,
                "execution": "not_started"
            })
        };
        components.insert("workflow".into(), workflow_component);

        let requirements = [
            ("domain_decision_readiness", require_domain_readiness),
            (
                "capability_route",
                require_route_review || require_route_plan,
            ),
            ("operations_acceptance", require_operations_acceptance),
            ("release", require_release_ready),
            ("workflow", require_workflow_evidence),
        ];
        let mut blockers = Vec::new();
        let mut incomplete = false;
        let mut blocked = false;
        for (name, required) in requirements {
            if !required {
                continue;
            }
            let component = components
                .get(name)
                .expect("all control-plane components inserted");
            if component.get("present").and_then(Value::as_bool) != Some(true) {
                incomplete = true;
                blockers.push(json!({
                    "code": "required_component_missing",
                    "severity": "error",
                    "component": name,
                    "message": "the caller's control-plane policy requires this evidence component"
                }));
            } else if component.get("valid").and_then(Value::as_bool) != Some(true) {
                blocked = true;
                blockers.push(json!({
                    "code": "component_invalid",
                    "severity": "error",
                    "component": name,
                    "errors": component.get("errors").cloned().unwrap_or_else(|| json!([])),
                    "message": "a required evidence component failed its structural integrity checks"
                }));
            } else if component.get("satisfied").and_then(Value::as_bool) != Some(true) {
                let state = component
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or("incomplete");
                if state == "blocked" {
                    blocked = true;
                } else {
                    incomplete = true;
                }
                blockers.push(json!({
                    "code": "required_component_not_satisfied",
                    "severity": "error",
                    "component": name,
                    "state": state,
                    "errors": component.get("errors").cloned().unwrap_or_else(|| json!([])),
                    "message": "the required component is present but its own structural policy is not satisfied"
                }));
            }
        }
        if require_route_review && route_review.is_none() {
            incomplete = true;
            blockers.push(json!({
                "code": "required_component_missing",
                "severity": "error",
                "component": "route_review",
                "message": "policy.require_route_review requires the original capability_route_review packet"
            }));
        }
        if require_route_plan && route_plan.is_none() {
            incomplete = true;
            blockers.push(json!({
                "code": "required_component_missing",
                "severity": "error",
                "component": "route_plan",
                "message": "policy.require_route_plan requires the non-executing capability_route_plan packet"
            }));
        }
        let control_plane_state = if blocked {
            "blocked"
        } else if incomplete {
            "incomplete"
        } else if requirements.iter().any(|(_, required)| *required) {
            "ready_for_human_review"
        } else {
            "review_required"
        };
        let policy_satisfied = control_plane_state == "ready_for_human_review";
        let component_states = components
            .iter()
            .map(|(name, component)| {
                (
                    name.clone(),
                    json!({
                        "present": component.get("present"),
                        "valid": component.get("valid"),
                        "satisfied": component.get("satisfied"),
                        "state": component.get("state"),
                        "content_digest": component.get("content_digest").or_else(|| component.get("plan_digest")),
                        "authority": component.get("authority")
                    }),
                )
            })
            .collect::<Map<_, _>>();
        let mut audit = json!({
            "schema": "bioprism-control-plane-readiness/0.1",
            "workflow": "control_plane_readiness_audit",
            "subject_id": subject_id,
            "policy": policy,
            "components": components,
            "component_states": component_states,
            "component_count": 5,
            "parent_digests": parent_digests.iter().cloned().collect::<Vec<_>>(),
            "domains": domains.iter().cloned().collect::<Vec<_>>(),
            "control_plane_state": control_plane_state,
            "policy_satisfied": policy_satisfied,
            "blockers": blockers,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "each component retains its own structural state and non-authority boundary",
                "only explicitly required components can block policy_satisfied",
                "valid content digests are retained as parent edges for replayable lineage"
            ],
            "does_not_claim": [
                "structural completeness proves scientific, clinical, causal, regulatory, publication, or release validity",
                "an operations acceptance grants authorization to execute a mission",
                "a release audit proves deployment, signature verification, or external runner success",
                "absence of an optional component is a negative result"
            ]
        });
        audit["digest"] = Value::String(
            bioprism_ids::ContentHash::of_value(&audit)
                .map_err(|error| format!("control-plane readiness could not be digested: {error}"))?
                .to_string(),
        );
        let projection = self.index_artifact_projection(
            "control_plane_readiness",
            subject_id,
            domains.into_iter().collect(),
            parent_digests.into_iter().collect(),
            audit.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "control-plane readiness could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": "bioprism-control-plane-readiness/0.1",
            "workflow": "control_plane_readiness_audit",
            "audit": audit,
            "artifact_registry": projection,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "the projection joins supplied evidence without dispatching or recomputing nested tools",
                "component authority is not widened by the overall state",
                "the exact projection is retained in the digest-verified artifact registry"
            ],
            "does_not_claim": [
                "a ready_for_human_review state is execution, deployment, scientific, clinical, or regulatory authorization",
                "optional evidence absence is a failed domain result",
                "local retained evidence is complete external history"
            ]
        }))
    }

    /// Compare two digest-verified control-plane projections without rerunning nested evidence.
    ///
    /// This is intentionally a structural diff: a stronger state is not a scientific or
    /// deployment claim, and a weaker state is not a domain-result failure. Keeping the diff
    /// beside the original content digests makes readiness regressions and recoveries inspectable
    /// across every capability group without widening any component's authority.
    pub(super) fn control_plane_readiness_compare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode control-plane readiness comparison: {error}")
        })?;
        if encoded.len() > 40_000_000 {
            return Err(
                "control-plane readiness comparison input exceeds the 40000000-byte safety bound"
                    .into(),
            );
        }
        let object = arguments
            .as_object()
            .ok_or("control-plane readiness comparison input must be an object")?;

        let extract = |name: &str| -> Result<(String, Value), String> {
            let wrapper = object
                .get(name)
                .ok_or_else(|| format!("{name} is required"))?;
            if wrapper.get("ok").and_then(Value::as_bool) != Some(true)
                || wrapper.get("schema").and_then(Value::as_str)
                    != Some("bioprism-control-plane-readiness/0.1")
                || wrapper.get("workflow").and_then(Value::as_str)
                    != Some("control_plane_readiness_audit")
                || wrapper.get("readiness_claimed").and_then(Value::as_bool) != Some(false)
                || wrapper.get("execution").and_then(Value::as_str) != Some("not_started")
            {
                return Err(format!(
                    "{name} must be a successful, non-executing control_plane_readiness_audit wrapper"
                ));
            }
            if wrapper
                .pointer("/artifact_registry/indexed")
                .and_then(Value::as_bool)
                != Some(true)
            {
                return Err(format!("{name}.artifact_registry must be indexed"));
            }
            let audit = wrapper
                .get("audit")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("{name}.audit must be an object"))?;
            if audit.get("schema").and_then(Value::as_str)
                != Some("bioprism-control-plane-readiness/0.1")
                || audit.get("workflow").and_then(Value::as_str)
                    != Some("control_plane_readiness_audit")
                || audit.get("readiness_claimed").and_then(Value::as_bool) != Some(false)
                || audit.get("execution").and_then(Value::as_str) != Some("not_started")
            {
                return Err(format!(
                    "{name}.audit has an invalid workflow or authority posture"
                ));
            }
            let subject_id = audit
                .get("subject_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("{name}.audit.subject_id must be non-empty"))?
                .to_string();
            let digest = audit
                .get("digest")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{name}.audit.digest is required"))?;
            let parsed_digest = bioprism_ids::ContentHash::parse(digest.to_string())
                .map_err(|error| format!("{name}.audit.digest is invalid: {error}"))?;
            let mut unsigned = audit.clone();
            unsigned.remove("digest");
            let computed = bioprism_ids::ContentHash::of_value(&Value::Object(unsigned))
                .map_err(|error| format!("{name}.audit could not be hashed: {error}"))?;
            if parsed_digest != computed {
                return Err(format!("{name}.audit.digest does not match its content"));
            }
            if !audit.get("components").and_then(Value::as_object).is_some() {
                return Err(format!("{name}.audit.components must be an object"));
            }
            if !audit.get("blockers").is_none_or(Value::is_array)
                || !audit.get("domains").is_none_or(Value::is_array)
                || !audit.get("parent_digests").is_none_or(Value::is_array)
            {
                return Err(format!("{name}.audit arrays are malformed"));
            }
            Ok((subject_id, Value::Object(audit.clone())))
        };

        let (before_subject, before) = extract("before")?;
        let (after_subject, after) = extract("after")?;
        if before_subject != after_subject {
            return Err("before and after control-plane subjects must match".into());
        }
        if let Some(subject_id) = object.get("subject_id") {
            if subject_id.as_str() != Some(before_subject.as_str()) {
                return Err("subject_id must match both compared control-plane projections".into());
            }
        }
        let before = before
            .as_object()
            .expect("extracted control-plane audit is an object");
        let after = after
            .as_object()
            .expect("extracted control-plane audit is an object");
        let before_components = before
            .get("components")
            .and_then(Value::as_object)
            .expect("validated before components");
        let after_components = after
            .get("components")
            .and_then(Value::as_object)
            .expect("validated after components");
        let mut component_names = BTreeSet::new();
        component_names.extend(before_components.keys().cloned());
        component_names.extend(after_components.keys().cloned());
        let state_rank = |state: Option<&str>| match state {
            Some("ready_for_human_review") => 3,
            Some("review_required") => 2,
            Some("incomplete") => 1,
            Some("blocked") => 0,
            _ => -1,
        };
        let mut component_changes = Vec::new();
        let mut improvements = Vec::new();
        let mut regressions = Vec::new();
        for name in component_names {
            let before_component = before_components.get(&name);
            let after_component = after_components.get(&name);
            let before_state = before_component
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str);
            let after_state = after_component
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str);
            let fields = ["present", "valid", "satisfied", "state"];
            let changed_fields = fields
                .into_iter()
                .filter(|field| {
                    before_component.and_then(|value| value.get(*field))
                        != after_component.and_then(|value| value.get(*field))
                })
                .map(str::to_string)
                .collect::<Vec<_>>();
            if changed_fields.is_empty() {
                continue;
            }
            let row = json!({
                "component": name,
                "changed_fields": changed_fields,
                "before": before_component.cloned().unwrap_or_else(|| json!({})),
                "after": after_component.cloned().unwrap_or_else(|| json!({})),
                "state_rank_delta": state_rank(after_state) - state_rank(before_state)
            });
            let rank_delta = state_rank(after_state) - state_rank(before_state);
            let before_satisfied = before_component
                .and_then(|value| value.get("satisfied"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let after_satisfied = after_component
                .and_then(|value| value.get("satisfied"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if rank_delta > 0 || (!before_satisfied && after_satisfied) {
                improvements.push(json!({
                    "kind": if rank_delta > 0 { "component_state_improved" } else { "component_satisfied" },
                    "component": row["component"].clone(),
                    "before_state": before_state,
                    "after_state": after_state
                }));
            }
            if rank_delta < 0 || (before_satisfied && !after_satisfied) {
                regressions.push(json!({
                    "kind": if rank_delta < 0 { "component_state_regressed" } else { "component_unsatisfied" },
                    "component": row["component"].clone(),
                    "before_state": before_state,
                    "after_state": after_state
                }));
            }
            component_changes.push(row);
        }

        let digest_rows = |field: &str| -> Result<(Vec<Value>, Vec<Value>), String> {
            let digest = |value: &Value| -> Result<String, String> {
                bioprism_ids::ContentHash::of_value(value)
                    .map(|hash| hash.to_string())
                    .map_err(|error| format!("{field} row could not be hashed: {error}"))
            };
            let before_rows = before
                .get(field)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let after_rows = after
                .get(field)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let before_map = before_rows
                .iter()
                .map(|row| digest(row).map(|hash| (hash, row.clone())))
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let after_map = after_rows
                .iter()
                .map(|row| digest(row).map(|hash| (hash, row.clone())))
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok((
                after_map
                    .iter()
                    .filter(|(hash, _)| !before_map.contains_key(*hash))
                    .map(|(_, row)| row.clone())
                    .collect(),
                before_map
                    .iter()
                    .filter(|(hash, _)| !after_map.contains_key(*hash))
                    .map(|(_, row)| row.clone())
                    .collect(),
            ))
        };
        let (blockers_added, blockers_removed) = digest_rows("blockers")?;
        if !blockers_added.is_empty() {
            regressions.push(json!({
                "kind": "blockers_added",
                "count": blockers_added.len()
            }));
        }
        if !blockers_removed.is_empty() {
            improvements.push(json!({
                "kind": "blockers_removed",
                "count": blockers_removed.len()
            }));
        }
        let (domains_added, domains_removed) = digest_rows("domains")?;
        let (parents_added, parents_removed) = digest_rows("parent_digests")?;
        let before_state = before
            .get("control_plane_state")
            .and_then(Value::as_str)
            .unwrap_or("blocked");
        let after_state = after
            .get("control_plane_state")
            .and_then(Value::as_str)
            .unwrap_or("blocked");
        let before_policy = before.get("policy").cloned().unwrap_or_else(|| json!({}));
        let after_policy = after.get("policy").cloned().unwrap_or_else(|| json!({}));
        if before_policy != after_policy {
            improvements.push(json!({
                "kind": "policy_changed",
                "note": "policy changes alter the meaning of policy_satisfied and require human review"
            }));
        }
        let state_delta = state_rank(Some(after_state)) - state_rank(Some(before_state));
        let state_direction = if state_delta > 0 {
            "improved"
        } else if state_delta < 0 {
            "regressed"
        } else {
            "unchanged"
        };
        let evidence_direction = if !regressions.is_empty() && !improvements.is_empty() {
            "mixed"
        } else if !regressions.is_empty() {
            "regressed"
        } else if !improvements.is_empty() {
            "improved"
        } else {
            "unchanged"
        };
        let next_action = match after_state {
            "ready_for_human_review" => {
                "obtain the separate human or domain-authority review; this projection is not authorization"
            }
            "review_required" => "inspect optional evidence and obtain the required human review",
            "incomplete" => {
                "supply the missing required evidence components or revise the explicit policy"
            }
            "blocked" => "resolve structural integrity errors and re-run the non-executing audit",
            _ => "inspect the invalid control-plane state",
        };
        let mut comparison = json!({
            "schema": "bioprism-control-plane-readiness-compare/0.1",
            "workflow": "control_plane_readiness_compare",
            "subject_id": before_subject,
            "before": {
                "audit_digest": before.get("digest"),
                "control_plane_state": before_state,
                "policy_satisfied": before.get("policy_satisfied")
            },
            "after": {
                "audit_digest": after.get("digest"),
                "control_plane_state": after_state,
                "policy_satisfied": after.get("policy_satisfied")
            },
            "state_delta": state_delta,
            "state_direction": state_direction,
            "evidence_direction": evidence_direction,
            "policy_changed": before_policy != after_policy,
            "component_changes": component_changes,
            "blockers_added": blockers_added,
            "blockers_removed": blockers_removed,
            "domains_added": domains_added,
            "domains_removed": domains_removed,
            "parent_digests_added": parents_added,
            "parent_digests_removed": parents_removed,
            "improvements": improvements,
            "regressions": regressions,
            "next_action": next_action,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "both inputs are content-digest verified control-plane audits",
                "component states, policy changes, blocker changes, domains, and parent edges remain separately inspectable",
                "the comparison never reruns nested tools or infers a scientific, clinical, deployment, or release result"
            ],
            "does_not_claim": [
                "a state improvement is authorization or proof of external execution",
                "a state regression is failure of the underlying scientific or domain result",
                "unchanged structural state means unchanged external reality"
            ]
        });
        comparison["comparison_digest"] = Value::String(
            bioprism_ids::ContentHash::of_value(&comparison)
                .map_err(|error| {
                    format!("control-plane comparison could not be digested: {error}")
                })?
                .to_string(),
        );
        Ok(json!({
            "ok": true,
            "schema": "bioprism-control-plane-readiness-compare/0.1",
            "workflow": "control_plane_readiness_compare",
            "comparison": comparison,
            "readiness_claimed": false,
            "execution": "not_started"
        }))
    }

    /// Compare two complete control-plane audits already retained by the content-addressed
    /// artifact registry. The registry lookup is authoritative for identity and integrity; the
    /// comparison remains the same structural, non-executing diff as the inline route.
    pub(super) fn control_plane_readiness_compare_retained(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let object = arguments
            .as_object()
            .ok_or("retained control-plane comparison input must be an object")?;
        let digest_text = |name: &str| -> Result<String, String> {
            let value = object
                .get(name)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("{name} must be a non-empty content digest"))?;
            bioprism_ids::ContentHash::parse(value.to_string())
                .map(|digest| digest.to_string())
                .map_err(|error| format!("{name} is not a valid content digest: {error}"))
        };
        let before_content_digest = digest_text("before_content_digest")?;
        let after_content_digest = digest_text("after_content_digest")?;
        let subject_id = object
            .get("subject_id")
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "subject_id must be a non-empty string".to_string())
                    .map(str::to_string)
            })
            .transpose()?;

        let load = |digest: &str, name: &str| -> Result<(String, Value), String> {
            let response = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?
                .get(digest)
                .map_err(|error| format!("{name} retained readiness lookup refused: {error}"))?;
            let record = response
                .get("record")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("{name} retained artifact record is malformed"))?;
            if record.get("kind").and_then(Value::as_str) != Some("control_plane_readiness") {
                return Err(format!(
                    "{name} content digest is not a control_plane_readiness artifact"
                ));
            }
            let record_subject = record
                .get("subject_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("{name} retained artifact subject_id is missing"))?;
            let artifact = record
                .get("artifact")
                .cloned()
                .ok_or_else(|| format!("{name} retained readiness artifact body is missing"))?;
            if !artifact.is_object() {
                return Err(format!(
                    "{name} retained readiness artifact body must be an object"
                ));
            }
            let wrapper = json!({
                "ok": true,
                "schema": "bioprism-control-plane-readiness/0.1",
                "workflow": "control_plane_readiness_audit",
                "audit": artifact,
                "artifact_registry": {
                    "indexed": true,
                    "kind": "control_plane_readiness",
                    "content_digest": record.get("content_digest"),
                    "verification": record.get("verification")
                },
                "readiness_claimed": false,
                "execution": "not_started"
            });
            Ok((record_subject.to_string(), wrapper))
        };

        let (before_subject, before) = load(&before_content_digest, "before_content_digest")?;
        let (after_subject, after) = load(&after_content_digest, "after_content_digest")?;
        if before_subject != after_subject {
            return Err("retained before and after readiness subjects must match".into());
        }
        if subject_id
            .as_deref()
            .is_some_and(|value| value != before_subject)
        {
            return Err("subject_id must match both retained readiness artifacts".into());
        }
        let comparison = self.control_plane_readiness_compare(&json!({
            "subject_id": before_subject,
            "before": before,
            "after": after
        }))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-control-plane-readiness-compare-retained/0.1",
            "workflow": "control_plane_readiness_compare_retained",
            "subject_id": before_subject,
            "before_content_digest": before_content_digest,
            "after_content_digest": after_content_digest,
            "comparison": comparison.get("comparison"),
            "source": "content_addressed_artifact_registry",
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "both inputs were resolved by exact content digest from the verified artifact registry",
                "the retained records were required to be control_plane_readiness artifacts for one subject",
                "the structural comparison reused the digest-verified non-executing comparison contract"
            ],
            "does_not_claim": [
                "registry retention proves that the readiness state was current or externally complete",
                "an improved comparison is authorization or proof of execution",
                "a retained artifact is a scientific, clinical, deployment, release, or regulatory authority"
            ]
        }))
    }

    /// Query retained control-plane projections by structural state or explicit policy result.
    pub(super) fn control_plane_readiness_query(&self, arguments: &Value) -> Result<Value, String> {
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| format!("{name} must be a non-empty string"))
                })
                .transpose()
        };
        let subject_id = optional_string("subject_id")?;
        let control_plane_state = optional_string("control_plane_state")?;
        let after = optional_string("after")?;
        let policy_satisfied = arguments
            .get("policy_satisfied")
            .map(|value| value.as_bool().ok_or("policy_satisfied must be a boolean"))
            .transpose()?;
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
        let include_audits = arguments
            .get("include_audits")
            .map(|value| value.as_bool().ok_or("include_audits must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .control_plane_readiness_query(
                subject_id,
                control_plane_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            )
            .map_err(|error| format!("control-plane readiness query refused: {error}"))
    }
}
