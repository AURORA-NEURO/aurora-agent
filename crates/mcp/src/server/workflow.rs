//! Typed BioQL, fabric synthesis, and retained Interweave workflow execution evidence.

use super::*;

impl Server {
    pub(super) fn fabric_synthesize(&self, arguments: &Value) -> Result<Value, String> {
        let raw_goal = arguments
            .get("goal")
            .cloned()
            .ok_or("goal is required and must be a serialized fabric Goal")?;
        let raw_candidates = arguments
            .get("candidates")
            .cloned()
            .ok_or("candidates is required and must be an array of fabric Candidate values")?;
        let candidate_values = raw_candidates
            .as_array()
            .ok_or("candidates must be an array of fabric Candidate values")?;
        if candidate_values.is_empty() || candidate_values.len() > 1_000 {
            return Err("candidates must contain between 1 and 1000 candidates".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "goal": raw_goal.clone(),
            "candidates": raw_candidates.clone(),
        }))
        .map_err(|error| format!("cannot measure fabric synthesis envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("fabric synthesis input exceeds the 20000000-byte safety bound".into());
        }
        let goal: FabricGoal = serde_json::from_value(
            arguments
                .get("goal")
                .cloned()
                .expect("goal was checked above"),
        )
        .map_err(|error| format!("invalid fabric goal: {error}"))?;
        let candidates: Vec<FabricCandidate> = serde_json::from_value(raw_candidates)
            .map_err(|error| format!("invalid fabric candidates: {error}"))?;
        let mut names = BTreeSet::new();
        for candidate in &candidates {
            if candidate.name.trim().is_empty() || candidate.name.len() > 256 {
                return Err(
                    "each fabric candidate name must contain between 1 and 256 bytes".into(),
                );
            }
            if !names.insert(candidate.name.as_str()) {
                return Err(format!(
                    "fabric candidates contain duplicate name {:?}",
                    candidate.name
                ));
            }
            if candidate.graph.roles.len() > 1_000
                || candidate.graph.verifies.len() > 10_000
                || candidate.bindings.len() > 1_000
                || candidate.cards.len() > 1_000
            {
                return Err(format!(
                    "fabric candidate {:?} exceeds a role, verification, binding, or card bound",
                    candidate.name
                ));
            }
        }
        let artifact = synthesize_fabric(&goal, &candidates);
        Ok(json!({
            "ok": true,
            "goal": goal,
            "candidate_count": candidates.len(),
            "admissible_count": artifact.scores.len(),
            "eliminated_count": artifact.eliminated.len(),
            "artifact": artifact,
            "unimplemented_stages": unimplemented_stages(),
            "guarantees": [
                "hard constraint failures are returned as rejection reasons and never converted into soft penalties",
                "the frontier is Pareto-based and does not smuggle in caller-unstated weights",
                "role graphs and assurance requirements are checked before a candidate can score",
                "this endpoint performs no participant binding, runtime execution, registry lookup, or effect",
            ],
        }))
    }

    pub(super) fn bioql_compile(&self, arguments: &Value) -> Result<Value, String> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or("query is required and must be a BioQL source string")?;
        if query.trim().is_empty() || query.len() > 1_000_000 {
            return Err("query must contain between 1 and 1000000 bytes".into());
        }
        let raw_schema = arguments
            .get("schema")
            .cloned()
            .ok_or("schema is required and must be a serialized BioQL QuerySchema")?;
        let encoded = serde_json::to_vec(&json!({
            "query": query,
            "schema": raw_schema.clone(),
        }))
        .map_err(|error| format!("cannot measure BioQL envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("BioQL input exceeds the 20000000-byte safety bound".into());
        }
        let schema: QuerySchema = serde_json::from_value(raw_schema)
            .map_err(|error| format!("invalid BioQL schema: {error}"))?;
        let field_count = schema
            .names()
            .map(|name| {
                schema
                    .get(name)
                    .map(|collection| collection.len())
                    .unwrap_or(0)
            })
            .sum::<usize>();
        if schema.len() > 1_000 || field_count > 100_000 {
            return Err("BioQL schema exceeds the 1000-collection or 100000-field bound".into());
        }

        match compile_bioql(query, &schema) {
            Ok(typed) => Ok(json!({
                "ok": true,
                "typed_query": typed,
                "schema": {
                    "collections": schema.len(),
                    "fields": field_count,
                },
                "execution": "not_performed",
                "guarantees": [
                    "unknown fields and collections refuse instead of being inferred",
                    "units, frames, genome builds, ontology expansion, clocks, labels, provenance, and cost bounds are checked before execution",
                    "the cost estimate is syntactic and declared; no rows, statistics, store, planner, or permission decision is consulted",
                    "successful compilation returns a typed contract, not evidence or a result set",
                ],
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "stage": "bioql_typecheck",
                "refusal": error.to_string(),
                "fail_closed": true,
                "schema": {
                    "collections": schema.len(),
                    "fields": field_count,
                },
                "guarantees": [
                    "parse and type failures remain distinct from execution results",
                    "no partial query is returned as if it were typed",
                    "no execution, permission bypass, ontology expansion, or unit conversion occurs",
                ],
            })),
        }
    }

    pub(super) fn interweave_workflow_execute(&self, arguments: &Value) -> Result<Value, String> {
        let mode = arguments
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("simulate");
        if mode != "simulate" && mode != "replay" {
            return Err("mode must be \"simulate\" or \"replay\"".into());
        }
        let workflow: InterweaveWorkflowId = serde_json::from_value(
            arguments
                .get("workflow")
                .cloned()
                .ok_or("workflow is required and must be one of the six reference workflow ids")?,
        )
        .map_err(|error| format!("invalid workflow: {error}"))?;
        let problem: EpistemicDecisionProblem = serde_json::from_value(
            arguments
                .get("problem")
                .cloned()
                .ok_or("problem is required")?,
        )
        .map_err(|error| format!("invalid decision problem: {error}"))?;
        let belief: EpistemicBelief = serde_json::from_value(
            arguments
                .get("belief")
                .cloned()
                .ok_or("belief is required")?,
        )
        .map_err(|error| format!("invalid belief: {error}"))?;
        let acquisitions: Vec<EpistemicAcquisition> = serde_json::from_value(
            arguments
                .get("acquisitions")
                .cloned()
                .ok_or("acquisitions is required")?,
        )
        .map_err(|error| format!("invalid acquisitions: {error}"))?;
        let budget = arguments
            .get("budget")
            .and_then(Value::as_f64)
            .ok_or("budget is required and must be a finite non-negative number")?;
        let max_steps = arguments
            .get("max_steps")
            .and_then(Value::as_u64)
            .ok_or("max_steps is required and must be an integer")?
            as usize;
        let plan = EpistemicAdaptivePlan::new(problem, belief, acquisitions, budget, max_steps)
            .map_err(|error| format!("adaptive plan refused: {error}"))?;
        let plan_digest = plan
            .digest()
            .map_err(|error| format!("cannot digest adaptive plan: {error}"))?;
        let provider = arguments
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("mcp-simulated")
            .to_string();
        let capabilities = arguments
            .get("capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("capabilities[{index}] must be a non-empty string"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let binding = InterweaveWorkflowExecutionBinding::bind(
            workflow,
            &plan,
            provider.clone(),
            capabilities,
        )
        .map_err(|error| format!("workflow binding refused: {error}"))?;

        let receipt = if mode == "replay" {
            let prior: InterweaveWorkflowExecutionReceipt = serde_json::from_value(
                arguments
                    .get("receipt")
                    .cloned()
                    .ok_or("receipt is required in replay mode")?,
            )
            .map_err(|error| format!("invalid workflow execution receipt: {error}"))?;
            binding
                .replay(&plan, &prior)
                .map_err(|error| format!("workflow replay refused: {error}"))?
        } else {
            let grant = match arguments.get("authorization") {
                None => None,
                Some(raw) => {
                    let object = raw.as_object().ok_or("authorization must be an object")?;
                    let grant_id = object
                        .get("grant_id")
                        .and_then(Value::as_str)
                        .ok_or("authorization.grant_id is required")?;
                    let authorized_provider = object
                        .get("provider")
                        .and_then(Value::as_str)
                        .ok_or("authorization.provider is required")?;
                    Some(
                        EpistemicExecutionGrant::issue(
                            grant_id,
                            plan_digest.clone(),
                            authorized_provider,
                        )
                        .map_err(|error| format!("authorization refused: {error}"))?,
                    )
                }
            };
            let raw_observations = arguments
                .get("observations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if raw_observations.len() > 16 {
                return Err("observations cannot exceed the exact 16-step bound".into());
            }
            let script = raw_observations
                .iter()
                .enumerate()
                .map(|(index, raw)| {
                    let object = raw
                        .as_object()
                        .ok_or_else(|| format!("observations[{index}] must be an object"))?;
                    let acquisition_id = object
                        .get("acquisition_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("observations[{index}].acquisition_id is required")
                        })?;
                    let outcome_label = object
                        .get("outcome_label")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("observations[{index}].outcome_label is required")
                        })?;
                    Ok((acquisition_id.to_string(), outcome_label.to_string()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let mut executor = EpistemicScriptedExecutor::simulated(provider, script);
            binding
                .execute(&plan, grant.as_ref(), &mut executor)
                .map_err(|error| format!("workflow execution refused: {error}"))?
        };
        let (observed, simulated, replayed) = receipt.provenance_counts();
        let mut output = json!({
            "ok": true,
            "schema": INTERWEAVE_WORKFLOW_EXECUTION_SCHEMA,
            "mode": mode,
            "workflow": workflow,
            "plan_digest": plan_digest,
            "binding_digest": binding.digest(),
            "binding": binding,
            "completed": receipt.is_completed(),
            "release_posture": receipt.release_posture(),
            "receipt": receipt,
            "provenance_counts": {
                "observed": observed,
                "simulated": simulated,
                "replayed": replayed,
            },
            "guarantees": [
                "workflow identity, workflow specification, adaptive plan, provider, and capability declarations are digest-bound",
                "no provider call occurs without an explicit plan-scoped authorization grant",
                "simulation rows remain simulated and replay rows remain replayed",
                "workflow effect prohibitions are returned as metadata and are not silently treated as release authority",
            ],
            "limitations": [
                "the built-in MCP adapter is a deterministic simulator; real providers remain external Rust acquisition executors",
                "capability labels and provider provenance are declarations until a domain authority verifies them",
                "this route produces a receipt and never schedules participants, publishes results, or authorizes an external effect",
            ],
        });
        if let Some(config) = arguments.get("evidence") {
            let config_object = config
                .as_object()
                .ok_or("evidence must be an object when supplied")?;
            let mut evidence_arguments = Value::Object(config_object.clone());
            evidence_arguments["binding"] = output["binding"].clone();
            evidence_arguments["receipt"] = output["receipt"].clone();
            output["workflow_execution_evidence"] =
                self.interweave_workflow_execution_evidence(&evidence_arguments)?;
        }
        Ok(output)
    }

    /// Convert an already-produced workflow receipt into portable evidence and index it. This
    /// route is intentionally receipt-only: it never replays, executes, dereferences a provider,
    /// or upgrades a simulated/replayed provenance label.
    pub(super) fn interweave_workflow_execution_evidence(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let binding = arguments.get("binding").ok_or("binding is required")?;
        let receipt = arguments.get("receipt").ok_or("receipt is required")?;
        let subject_id = arguments
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("subject_id is required and must be a non-empty string")?;
        let domains = arguments
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("domains is required and must be an array")?
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("domains[{index}] must be a non-empty string"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let parent_digests = arguments
            .get("parent_digests")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("parent_digests[{index}] must be a non-empty digest"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let evidence = build_workflow_execution_evidence(
            binding,
            receipt,
            subject_id,
            &domains,
            &parent_digests,
        )
        .map_err(|error| format!("workflow execution evidence refused: {error}"))?;
        let import = self
            .workflow_execution_evidence_registry
            .lock()
            .map_err(|_| "workflow execution evidence registry lock is poisoned".to_string())?
            .import(&evidence)
            .map_err(|error| format!("workflow execution evidence import refused: {error}"))?;
        let artifact_registry = self.index_artifact_projection(
            "workflow_execution_evidence",
            subject_id,
            domains,
            parent_digests,
            evidence.clone(),
        );
        Ok(json!({
            "ok": true,
            "schema": WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION,
            "workflow": WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW,
            "evidence_digest": evidence["evidence_digest"],
            "evidence": evidence,
            "registry": import,
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "guarantees": [
                "the binding and receipt were independently validated before evidence indexing",
                "the exact receipt digest and provenance counts remain recoverable",
                "artifact and evidence registry projections are content-addressed and idempotent"
            ],
            "does_not_claim": [
                "indexing proves provider authentication, consent, scientific truth, or release readiness",
                "a simulated or replayed receipt is an observed-world result",
                "this operation executes or authorizes any workflow effect"
            ]
        }))
    }

    pub(super) fn interweave_workflow_execution_evidence_import(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let evidence = arguments.get("evidence").ok_or("evidence is required")?;
        validate_workflow_execution_evidence(evidence)
            .map_err(|error| format!("workflow execution evidence refused: {error}"))?;
        let subject_id = evidence
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("validated evidence omitted subject_id")?;
        let domains = evidence
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("validated evidence omitted domains")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let parent_digests = evidence
            .get("parent_digests")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let import = self
            .workflow_execution_evidence_registry
            .lock()
            .map_err(|_| "workflow execution evidence registry lock is poisoned".to_string())?
            .import(evidence)
            .map_err(|error| format!("workflow execution evidence import refused: {error}"))?;
        let artifact_registry = self.index_artifact_projection(
            "workflow_execution_evidence",
            subject_id,
            domains,
            parent_digests,
            evidence.clone(),
        );
        Ok(json!({
            "ok": true,
            "schema": WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA_VERSION,
            "workflow": "interweave_workflow_execution_evidence_import",
            "evidence_digest": evidence["evidence_digest"],
            "registry": import,
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "guarantees": [
                "the portable record was revalidated before import",
                "identical evidence imports are idempotent",
                "import does not execute or replay a workflow"
            ],
            "does_not_claim": [
                "registry presence establishes external provenance or domain validity"
            ]
        }))
    }

    pub(super) fn interweave_workflow_execution_evidence_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let optional_text = |field: &str| -> Result<Option<&str>, String> {
            arguments
                .get(field)
                .map(|value| {
                    value
                        .as_str()
                        .filter(|text| !text.trim().is_empty())
                        .ok_or_else(|| format!("{field} must be a non-empty string"))
                })
                .transpose()
        };
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|value| {
                        usize::try_from(value).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        let include_records = arguments
            .get("include_records")
            .map(|value| value.as_bool().ok_or("include_records must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.workflow_execution_evidence_registry
            .lock()
            .map_err(|_| "workflow execution evidence registry lock is poisoned".to_string())?
            .query(&WorkflowExecutionEvidenceQuery {
                workflow_id: optional_text("workflow_id")?,
                subject_id: optional_text("subject_id")?,
                domain: optional_text("domain")?,
                plan_digest: optional_text("plan_digest")?,
                binding_digest: optional_text("binding_digest")?,
                receipt_status: optional_text("receipt_status")?,
                provenance_mode: optional_text("provenance_mode")?,
                after: optional_text("after")?,
                max_items,
                include_records,
            })
            .map_err(|error| format!("workflow execution evidence query refused: {error}"))
    }

    pub(super) fn interweave_workflow_execution_evidence_get(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let digest = arguments
            .get("evidence_digest")
            .and_then(Value::as_str)
            .ok_or("evidence_digest is required")?;
        self.workflow_execution_evidence_registry
            .lock()
            .map_err(|_| "workflow execution evidence registry lock is poisoned".to_string())?
            .get(digest)
            .map_err(|error| format!("workflow execution evidence get refused: {error}"))
    }

    pub(super) fn weavelang_compile(&self, arguments: &Value) -> Result<Value, String> {
        let source = arguments
            .get("source")
            .and_then(Value::as_str)
            .ok_or("source is required and must be WeaveLang text")?;
        if source.is_empty() {
            return Err("source must not be empty".into());
        }
        if source.len() > 2_000_000 {
            return Err("source exceeds the 2000000-byte safety bound".into());
        }
        let execute = arguments
            .get("execute")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mode_text = arguments
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("replay");
        let mode: ExecutionMode = serde_json::from_value(json!(mode_text))
            .map_err(|error| format!("mode must be replay or live: {error}"))?;
        let thread_id = arguments
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("mcp-weavelang");
        if thread_id.is_empty() || thread_id.len() > 256 {
            return Err("thread_id must contain between 1 and 256 bytes".into());
        }
        let include_ir = arguments
            .get("include_ir")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let include_trace = arguments
            .get("include_trace")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let ir = compile_weave(source)
            .map_err(|error| format!("WeaveLang compilation refused: {error}"))?;
        let digest = ir.digest().map_err(|error| error.to_string())?;
        let semantic_digest = ir.semantic_digest().map_err(|error| error.to_string())?;
        let mut machine = Machine::load(ir.clone(), mode, thread_id);
        let initial_state = machine.state().to_string();
        let initial_invariants = machine.check_invariants();
        let initial_liveness = machine.liveness();
        let execution = if execute {
            match machine.run() {
                Ok(trace) => json!({
                    "status": "completed",
                    "mode": mode,
                    "state": machine.state(),
                    "event_count": trace.events.len(),
                    "trace_digest": trace.digest,
                    "trace": include_trace.then_some(trace),
                    "liveness": machine.liveness(),
                    "invariant_violations": machine.check_invariants(),
                }),
                Err(error) => json!({
                    "status": "refused",
                    "mode": mode,
                    "error": error.to_string(),
                    "fail_closed": true,
                    "state": machine.state(),
                    "liveness": machine.liveness(),
                    "invariant_violations": machine.check_invariants(),
                }),
            }
        } else {
            json!({
                "status": "not_requested",
                "mode": mode,
                "state": initial_state,
                "liveness": initial_liveness,
                "invariant_violations": initial_invariants,
            })
        };

        Ok(json!({
            "ok": true,
            "program": {
                "program_id": ir.program_id.clone(),
                "digest": digest,
                "semantic_digest": semantic_digest,
                "weave_ir_version": ir.weave_ir_version.clone(),
                "roles": ir.roles.len(),
                "participants": ir.participants.len(),
                "interfaces": ir.interfaces.len(),
                "policies": ir.policies.len(),
                "state_nodes": ir.state_graph.nodes.len(),
                "transitions": ir.state_graph.transitions.len(),
                "monitors": ir.monitors.len(),
                "initial_state": ir.choreography.initial_state.clone(),
                "terminal_states": ir.choreography.terminal_states.clone(),
            },
            "execution": execution,
            "ir": include_ir.then_some(ir),
            "guarantees": [
                "compilation is deterministic and returns both whole-document and semantic digests",
                "replay is the default execution mode and refuses world-mutating transitions",
                "execution is a local semantic trace; it performs no network, model, or tool call",
                "unimplemented language phases remain visible as compiler limitations rather than inferred capabilities",
            ],
        }))
    }

    pub(super) fn choreography_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_global = arguments
            .get("global")
            .cloned()
            .ok_or("global is required and must be a serialized GlobalType")?;
        let encoded = serde_json::to_vec(&raw_global).map_err(|error| error.to_string())?;
        if encoded.len() > 2_000_000 {
            return Err("global choreography exceeds the 2000000-byte safety bound".into());
        }
        let global: GlobalType = serde_json::from_value(raw_global)
            .map_err(|error| format!("invalid global choreography: {error}"))?;
        let bound: ExplorationBound = match arguments.get("bound") {
            None => ExplorationBound::default(),
            Some(raw) => serde_json::from_value(raw.clone())
                .map_err(|error| format!("invalid exploration bound: {error}"))?,
        };
        if bound.max_states == 0
            || bound.max_states > 100_000
            || bound.max_depth == 0
            || bound.max_depth > 10_000
            || bound.channel_capacity > 64
        {
            return Err(
                "bound must use 1..=100000 states, 1..=10000 depth, and at most 64 queued messages"
                    .into(),
            );
        }
        let roles = global
            .roles()
            .into_iter()
            .map(|role| role.to_string())
            .collect::<Vec<_>>();
        let checked = match WellFormedGlobal::check(global) {
            Ok(checked) => checked,
            Err(error) => {
                return Ok(json!({
                    "ok": true,
                    "well_formed": false,
                    "roles": roles,
                    "refusal": error.to_string(),
                    "protocol_error": error,
                    "fail_closed": true,
                    "guarantees": [
                        "an ill-formed global type yields no local projections",
                        "projection failures are returned before model checking",
                        "no protocol repair is inferred or silently applied",
                    ],
                }));
            }
        };
        let digest = checked.digest().map_err(|error| error.to_string())?;
        let system = ChoreographySystem::from_projection(&checked);
        let report = system.check(bound);

        Ok(json!({
            "ok": true,
            "well_formed": true,
            "digest": digest,
            "roles": roles,
            "projection_count": checked.projections().len(),
            "bound": bound,
            "model_check": report,
            "all_proved": report.all_proved(),
            "omissions": report.omissions(),
            "guarantees": [
                "well-formedness is established by projecting every named role",
                "deadlock, liveness, orphan, and unexpected-message verdicts preserve holds, fails, and inconclusive states",
                "a bounded search is never presented as universal proof when it hit a bound",
                "model checking is local and performs no network or participant execution",
            ],
        }))
    }
}
