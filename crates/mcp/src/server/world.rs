//! World generation, query compilation, and bounded world document handlers.

use super::*;

impl Server {
    pub(super) fn fiber_compile(&self, arguments: &Value) -> Result<Value, String> {
        let (out, context, domain_surface) = self.compiled_with_domain(arguments)?;
        let layer = arguments
            .get("layer")
            .and_then(Value::as_str)
            .map(|text| Layer::parse(text).ok_or(format!("unknown layer {text:?}")))
            .transpose()?
            .unwrap_or(Layer::L0);

        let mut rendered = out.section.render(layer, &context);
        if let Some(map) = rendered.as_object_mut() {
            map.insert(
                "estimated_tokens".into(),
                json!({
                    "value": out.section.estimated_tokens(layer, &context),
                    "method": "four-characters-per-token heuristic, not a tokenizer",
                }),
            );

            if let Some(quotient) = &out.trace.decision_quotient {
                map.insert(
                    "decision_quotient".into(),
                    json!({
                        "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
                        "basis": quotient.basis,
                        "permitted_actions": quotient.permitted_actions,
                        "original_model_count": quotient.original_model_count,
                        "quotient_model_count": quotient.quotient_model_count,
                        "merged_model_count": quotient.merged_model_count,
                        "compressed": quotient.compressed(),
                        "compression_fraction": quotient.compression_fraction(),
                        "certificate_binding": {
                            "query_sha256": out.certificate.source_hashes.query_sha256,
                            "certificate_sha256": context.certificate_sha256,
                        },
                        "limitations": [
                            "decision-relative model compression only; this is not a biological, causal, predictive, clinical, or likelihood equivalence",
                            "the quotient is not a rate-distortion frontier and does not claim evidence sufficiency",
                        ],
                    }),
                );
            }

            if let Some(rate_distortion) = &out.trace.rate_distortion {
                map.insert(
                    "rate_distortion".into(),
                    json!({
                        "schema": "bioprism-mcp/epistemic-context-audit/0.2",
                        "criterion": rate_distortion.criterion,
                        "tolerance": rate_distortion.tolerance,
                        "compatibility_floor": rate_distortion.compatibility_floor,
                        "evidence_count": rate_distortion.evidence_count,
                        "full_rate": rate_distortion.full_rate,
                        "identification": rate_distortion.identification,
                        "sufficiency": rate_distortion.sufficiency,
                        "frontier": rate_distortion.frontier,
                        "certificate_binding": {
                            "query_sha256": out.certificate.source_hashes.query_sha256,
                            "certificate_sha256": context.certificate_sha256,
                        },
                        "guarantees": [
                            "frontier is exhaustive over the bounded observed evidence pool",
                            "rate is summed evidence cost and distortion is decision regret",
                            "identification, sufficiency and frontier remain separate evidence layers",
                        ],
                        "limitations": [
                            "evidence is caller-declared and already observed; this is not acquisition value",
                            "decision identification is not causal, biological, clinical or predictive identification",
                            "the compiler does not claim contexts beyond the 16-item exhaustive boundary",
                        ],
                    }),
                );
            }

            if let Some(adaptive_acquisition) = &out.trace.adaptive_acquisition {
                map.insert(
                    "adaptive_acquisition".into(),
                    project_fiber_adaptive_trace(
                        adaptive_acquisition,
                        &out.certificate.source_hashes.query_sha256,
                        context
                            .certificate_sha256
                            .as_deref()
                            .ok_or("compiled certificate has no reference digest")?,
                    ),
                );
            }

            let world = arguments
                .get("world")
                .and_then(Value::as_str)
                .ok_or("world is required (a path relative to the server root)")?;
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .ok_or("query is required (a path relative to the server root)")?;
            let digest = context
                .certificate_sha256
                .as_deref()
                .ok_or("compiled certificate has no reference digest")?;
            let mut handle = json!({
                "version": 1,
                "world": world,
                "query": query,
                "certificate_sha256": digest,
            });
            if let Some(domain) = arguments.get("domain").and_then(Value::as_str) {
                handle["domain"] = json!(domain);
            }
            if let Some(refine) = map.get_mut("refine").and_then(Value::as_object_mut) {
                refine.insert("handle".into(), handle);
            } else {
                map.insert("refine".into(), json!({ "handle": handle }));
            }
            if let Some(surface) = domain_surface {
                map.insert("domain".into(), surface);
            }
        }
        Ok(rendered)
    }

    pub(super) fn fiber_refine(&self, arguments: &Value) -> Result<Value, String> {
        let text = arguments
            .get("layer")
            .and_then(Value::as_str)
            .ok_or("layer is required, one of l0..l4")?;
        let layer = Layer::parse(text).ok_or(format!("unknown layer {text:?}"))?;
        let (out, context) = self.compiled(arguments)?;
        Ok(out.section.render(layer, &context))
    }

    pub(super) fn fiber_explain(&self, arguments: &Value) -> Result<Value, String> {
        let (out, context, domain_surface) = self.compiled_with_domain(arguments)?;
        let mut explained = json!({
            "ok": true,
            "backend": out.certificate.plan.backend.as_str(),
            "passes": out.trace.passes.iter().map(|pass| json!({
                "name": pass.name, "retained": pass.retained, "note": pass.note
            })).collect::<Vec<_>>(),
            "passes_not_run": out.trace.deferred_passes.iter().map(|(name, reason)| json!({
                "name": name, "reason": reason
            })).collect::<Vec<_>>(),
            "decision_quotient": out.trace.decision_quotient,
            "rate_distortion": out.trace.rate_distortion,
            "adaptive_acquisition": out.trace.adaptive_acquisition.as_ref().map(|trace| {
                project_fiber_adaptive_trace(
                    trace,
                    &out.certificate.source_hashes.query_sha256,
                    context.certificate_sha256.as_deref().unwrap_or_default(),
                )
            }),
            "selection": {
                "facts": out.certificate.plan.compiled_fact_count,
                "of_total": out.certificate.plan.total_fact_count,
                "fraction": out.certificate.plan.fact_selection_ratio(),
            },
            "omission_manifest": out.certificate.manifest,
            "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
            "protected_closure_satisfied": out.protected_closure_satisfied(),
            "unmatched_protected_tags": out.trace.unmatched_protected_tags,
        });
        if let Some(surface) = domain_surface {
            explained
                .as_object_mut()
                .expect("explain payload is an object")
                .insert("domain".into(), surface);
        }
        Ok(explained)
    }

    pub(super) fn fiber_verify(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("certificate")
            .and_then(Value::as_str)
            .ok_or("certificate is required (a path relative to the server root)")?;
        let path = self.resolve(relative)?;
        let document = self.read_json(&path)?;
        let verification = ContextCertificate::verify(&document).map_err(|e| e.to_string())?;

        use bioprism_section::CertificateVerification::*;
        Ok(match &verification {
            Valid => json!({ "ok": true, "verified": true, "detail": "digest verifies" }),
            DigestMismatch {
                claimed,
                recomputed,
            } => json!({
                "ok": true, "verified": false,
                "detail": format!("digest mismatch: claims {claimed}, recomputes to {recomputed}")
            }),
            Malformed(reason) => json!({
                "ok": true, "verified": false, "detail": format!("malformed: {reason}")
            }),
        })
    }

    /// The one tool with side effects. Without `confirm: true` it previews and writes nothing.
    pub(super) fn projection_bundle(&self, arguments: &Value) -> Result<Value, String> {
        let (out, _) = self.compiled(arguments)?;
        let source = ProjectionSource::bind(
            &out.section,
            &out.certificate,
            CertificateProfile::Reference,
        )
        .map_err(|error| format!("projection provenance refused: {error}"))?;
        let bundle = project_graph_bundle(&out.section, &[], source)
            .map_err(|error| format!("projection refused: {error}"))?;
        let include_views = arguments
            .get("include_views")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let fidelity = bundle
            .fidelity_summary()
            .into_iter()
            .map(|(kind, dropped)| json!({ "kind": kind, "dropped": dropped }))
            .collect::<Vec<_>>();
        let obstructions_survive = bundle.obstructions_survive_everywhere(&out.section);
        let source = bundle.graph.source().clone();

        Ok(json!({
            "ok": true,
            "provenance": source,
            "projections": ["graph", "hypergraph", "timeline", "table"],
            "fidelity": fidelity,
            "obstructions_survive_everywhere": obstructions_survive,
            "views": include_views.then_some(bundle),
            "guarantees": [
                "all four views are generated from one compiled section and one bound certificate",
                "projection provenance is recomputed and cannot be asserted from caller-supplied digests",
                "unresolved obligations and oracle conflicts cannot be silently dropped to close a view",
                "a projection is navigation data, never proof of evidence completeness or clinical truth",
            ],
        }))
    }

    pub(super) fn world_index(&self, arguments: &Value) -> Result<Value, String> {
        let world = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required")?;
        let store = arguments
            .get("store")
            .and_then(Value::as_str)
            .ok_or("store is required")?;
        let confirmed = arguments
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let world_path = self.resolve(world)?;
        let store_path = self.resolve(store)?;

        if !confirmed {
            return Ok(json!({
                "ok": true,
                "performed": false,
                "preview": {
                    "effect": "would write an index directory",
                    "reads": world_path.display().to_string(),
                    "writes": store_path.display().to_string(),
                },
                "hint": "call again with confirm=true to perform this write",
            }));
        }

        let raw = self.read_json(&world_path)?;
        let manifest = bioprism_store::build(&raw, &store_path).map_err(|e| e.to_string())?;
        Ok(json!({
            "ok": true,
            "performed": true,
            "world_id": manifest.world_id,
            "world_sha256": manifest.world_sha256,
            "facts": manifest.total_facts,
            "factors": manifest.total_factors,
        }))
    }

    pub(super) fn world_validate(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a path relative to the server root)")?;
        let path = self.resolve(relative)?;
        let raw = self.read_json(&path)?;
        let world = World::from_json(raw).map_err(|error| error.to_string())?;
        let report = validate(&world, &DimensionRegistry::default());
        let errors = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        let warnings = report.diagnostics.len() - errors;
        Ok(json!({
            "ok": errors == 0,
            "world_id": world.world_id,
            "world_sha256": world.content_hash(),
            "counts": {
                "facts": world.facts.len(),
                "factors": world.factors.len(),
                "events": world.events.len(),
            },
            "errors": errors,
            "warnings": warnings,
            "diagnostics": report.diagnostics,
        }))
    }

    pub(super) fn world_generate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_spec = arguments
            .get("spec")
            .cloned()
            .ok_or("spec is required and must be a serialized WorldSpec")?;
        let encoded = serde_json::to_vec(&raw_spec)
            .map_err(|error| format!("cannot measure world-generation input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("world-generation input exceeds the 20000000-byte safety bound".into());
        }
        let spec: WorldSpec = serde_json::from_value(raw_spec)
            .map_err(|error| format!("invalid world-generation spec: {error}"))?;
        if spec.subjects == 0 || spec.subjects > 1_000 {
            return Err("spec.subjects must be between 1 and 1000".into());
        }
        if spec.distractors > 10_000 {
            return Err("spec.distractors is bounded at 10000".into());
        }
        if spec.relay_depth > 64 {
            return Err("spec.relay_depth is bounded at 64".into());
        }
        if spec.events.len() > 128
            || spec.leakage.len() > 64
            || spec.hypotheses.len() > 128
            || spec.declared_absent.len() > 512
        {
            return Err(
                "spec events, leakage mechanisms, hypotheses, and declared absences exceed their safety bounds"
                    .into(),
            );
        }

        let generated = generate_world(&spec);
        let world_document = generated.world.clone();
        let query_document = generated.query.clone();
        let world = match World::from_json(world_document.clone()) {
            Ok(world) => world,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "generated_world_parse",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "world": Value::Null,
                    "query": Value::Null,
                }));
            }
        };
        let query = match Query::from_json(query_document.clone()) {
            Ok(query) => query,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "generated_query_parse",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "world": Value::Null,
                    "query": Value::Null,
                }));
            }
        };
        let report = validate(&world, &DimensionRegistry::default());
        let errors = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        if errors != 0 {
            return Ok(json!({
                "ok": false,
                "stage": "generated_world_validation",
                "refusal": "generated world contains validation errors",
                "fail_closed": true,
                "world_digest": world.content_hash(),
                "query_digest": bioprism_ids::ContentHash::of_value(&query_document)
                    .map_err(|error| error.to_string())?,
                "diagnostics": report.diagnostics,
            }));
        }

        let array_len = |document: &Value, field: &str| {
            document
                .get(field)
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        };
        let include_world = arguments
            .get("include_world")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let include_query = arguments
            .get("include_query")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(json!({
            "ok": true,
            "world_id": world.world_id,
            "query_id": query.query_id,
            "world_digest": world.content_hash(),
            "query_digest": bioprism_ids::ContentHash::of_value(&query_document)
                .map_err(|error| error.to_string())?,
            "counts": {
                "facts": world.facts.len(),
                "factors": world.factors.len(),
                "events": world.events.len(),
                "subjects": spec.subjects,
                "distractors": spec.distractors,
                "relay_depth": spec.relay_depth,
                "generated_query_targets": array_len(&query_document, "targets"),
            },
            "validation": {
                "errors": errors,
                "warnings": report.diagnostics.len() - errors,
                "diagnostics": report.diagnostics,
            },
            "world": include_world.then_some(world_document),
            "query": include_query.then_some(query_document),
            "guarantees": [
                "generation is a pure deterministic function of the serialized WorldSpec and seed",
                "both generated documents are parsed by their typed runtime models before success",
                "world and query digests bind the exact generated JSON documents returned or withheld",
                "generation performs no file, network, model, clinical, or publication side effect",
            ],
        }))
    }

    pub(super) fn context_compare(&self, arguments: &Value) -> Result<Value, String> {
        let world_relative = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a JSON world document relative to the server root)")?;
        let query_relative = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or("query is required (a query document relative to the server root)")?;
        let world_path = self.resolve(world_relative)?;
        let query_path = self.resolve(query_relative)?;
        if world_path.is_dir() {
            return Err(
                "context_compare requires a JSON world document, not an indexed store".into(),
            );
        }
        let world =
            World::from_json(self.read_json(&world_path)?).map_err(|error| error.to_string())?;
        let query = self.load_query(query_relative)?;
        let panel = bioprism_baseline::default_panel();
        let borrowed: Vec<&dyn bioprism_baseline::ContextStrategy> =
            panel.iter().map(|strategy| strategy.as_ref()).collect();
        let comparison =
            bioprism_baseline::compare(&world, &query, &borrowed).map_err(|error| {
                format!(
                    "comparing {} and {}: {error}",
                    world_path.display(),
                    query_path.display()
                )
            })?;
        Ok(comparison.to_json())
    }

    pub(super) fn bioworlds_catalog(&self, arguments: &Value) -> Result<Value, String> {
        let catalog = SliceCatalog::standard().map_err(|error| error.to_string())?;
        let include_render = arguments
            .get("include_render")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if let Some(slice_id) = arguments.get("slice").and_then(Value::as_str) {
            let report = catalog.run(slice_id).map_err(|error| error.to_string())?;
            let mut result = json!({
                "ok": true,
                "mode": "slice",
                "slice": slice_id,
                "report": report,
            });
            if include_render {
                result["render"] = json!(report.render());
            }
            return Ok(result);
        }

        let report = catalog.run_all().map_err(|error| error.to_string())?;
        let mut result = json!({
            "ok": true,
            "mode": "catalog",
            "slice_count": catalog.len(),
            "report": report,
            "unbuilt_worlds": bioprism_bioworlds::UNBUILT_BLUEPRINT_WORLDS,
        });
        if include_render {
            result["render"] = json!(report.render());
        }
        Ok(result)
    }

    pub(super) fn prism_minimize(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a JSON world document relative to the server root)")?;
        let path = self.resolve(relative)?;
        if path.is_dir() {
            return Err(
                "prism_minimize requires a JSON world document, not an indexed store".into(),
            );
        }
        let world = World::from_json(self.read_json(&path)?).map_err(|error| error.to_string())?;
        let minimization = if let Some(facts) = arguments.get("facts") {
            let facts = facts
                .as_array()
                .ok_or("facts must be an array of fact ids")?;
            let mut candidate = BTreeSet::new();
            for fact in facts {
                let fact = fact.as_str().ok_or("facts must be an array of fact ids")?;
                if world.fact(fact).is_none() {
                    return Err(format!("fact {fact:?} is not present in the world"));
                }
                candidate.insert(fact.to_string());
            }
            minimize(&world, &candidate).map_err(|error| error.to_string())?
        } else {
            minimize_world(&world).map_err(|error| error.to_string())?
        };
        let preservation = preserves(&world, &minimization);

        Ok(json!({
            "ok": true,
            "world": relative,
            "world_id": world.world_id,
            "world_sha256": world.content_hash(),
            "minimization": minimization,
            "preservation": preservation,
            "preserved": preservation.is_preserved(),
            "guarantee_is_not_global_minimum": true,
        }))
    }

    pub(super) fn token_context_plan(&self, arguments: &Value) -> Result<Value, String> {
        let raw_request = arguments
            .get("request")
            .cloned()
            .ok_or("request is required and must be a serialized ContextRequest")?;
        let request: ContextRequest = serde_json::from_value(raw_request)
            .map_err(|error| format!("invalid token context request: {error}"))?;
        if request.envelope.total > 10_000_000 {
            return Err("request.envelope.total must be at most 10000000".into());
        }

        let parse_candidates = |name: &str| -> Result<Vec<PlanCandidate>, String> {
            let raw = arguments
                .get(name)
                .cloned()
                .ok_or_else(|| format!("{name} is required and must be an array"))?;
            let items = raw
                .as_array()
                .ok_or_else(|| format!("{name} must be an array"))?;
            if items.is_empty() || items.len() > 10_000 {
                return Err(format!(
                    "{name} must contain between 1 and 10000 candidates"
                ));
            }
            let candidates: Vec<PlanCandidate> =
                serde_json::from_value(raw).map_err(|error| format!("invalid {name}: {error}"))?;
            let mut seen = BTreeSet::new();
            for candidate in &candidates {
                if candidate.node_id.is_empty() {
                    return Err(format!("{name} contains a candidate with an empty node_id"));
                }
                if candidate.estimate.tokens > 10_000_000 {
                    return Err(format!(
                        "candidate {:?} exceeds the 10000000-token safety bound",
                        candidate.node_id
                    ));
                }
                if !seen.insert(candidate.node_id.as_str()) {
                    return Err(format!(
                        "{name} contains duplicate node_id {:?}",
                        candidate.node_id
                    ));
                }
            }
            Ok(candidates)
        };

        let candidates = parse_candidates("candidates")?;
        let base_plan = plan_token_context(&request, &candidates)
            .map_err(|error| format!("token planning refused: {error}"))?;

        let has_variant_request = arguments.get("variant_request").is_some();
        let has_variant_candidates = arguments.get("variant_candidates").is_some();
        if has_variant_request != has_variant_candidates {
            return Err(
                "variant_request and variant_candidates must be supplied together for comparison"
                    .into(),
            );
        }
        let comparison = if has_variant_request {
            let variant_request: ContextRequest = serde_json::from_value(
                arguments
                    .get("variant_request")
                    .cloned()
                    .expect("variant request was checked above"),
            )
            .map_err(|error| format!("invalid variant token context request: {error}"))?;
            if variant_request.envelope.total > 10_000_000 {
                return Err("variant_request.envelope.total must be at most 10000000".into());
            }
            let variant_candidates = parse_candidates("variant_candidates")?;
            Some(
                compare_token_plans(
                    "mcp-token-policy-comparison",
                    &request,
                    &variant_request,
                    &candidates,
                    &variant_candidates,
                )
                .map_err(|error| format!("token comparison refused: {error}"))?,
            )
        } else {
            None
        };

        Ok(json!({
            "ok": true,
            "plan": base_plan,
            "comparison": comparison,
            "guarantees": [
                "dry_run refuses before touching any candidate marked restricted",
                "the mandatory closure is checked before a plan is returned",
                "candidate ids are unique and stable at the transport boundary",
                "policy comparisons are admitted only when request fields other than policy match",
                "token counts retain their estimation method and are never presented as measured by this server",
            ],
        }))
    }
}
