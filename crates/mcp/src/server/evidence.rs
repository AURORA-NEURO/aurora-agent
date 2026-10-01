//! Measurement, observation, trace, lineage, and pre-analytic evidence handlers.

use super::*;

impl Server {
    pub(super) fn measurement_compare(&self, arguments: &Value) -> Result<Value, String> {
        let left: Measurement = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a serialized Measurement")?,
        )
        .map_err(|error| format!("invalid left measurement: {error}"))?;
        let right: Measurement = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a serialized Measurement")?,
        )
        .map_err(|error| format!("invalid right measurement: {error}"))?;
        let policy = StandardsComparabilityPolicy {
            require_bound_terms: arguments
                .get("require_bound_terms")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        let report = comparability_report(&left, &right, policy);
        let comparable = report.verdict.is_comparable();
        let digest = report
            .digest()
            .map_err(|error| format!("could not hash comparability report: {error}"))?;
        Ok(json!({
            "ok": true,
            "comparable": comparable,
            "policy": policy,
            "report": report,
            "report_sha256": digest.to_string(),
            "guarantees": [
                "kind, frame, dimension, unit, and ontology mismatches remain typed refusals",
                "unit conversion is explicit and recorded rather than silently applied",
                "unbound ontology terms are reported as caveats by default and can be refused by policy",
            ],
            "limitations": [
                "this compares caller-supplied declarations; it does not parse source files, validate instrument provenance, load ontologies, or perform coordinate registration",
                "the closed unit vocabulary is the standards crate's declared table, not a general UCUM parser",
            ],
        }))
    }

    pub(super) fn tabular_ingest(&self, arguments: &Value) -> Result<Value, String> {
        let profile: TabularProfile = serde_json::from_value(
            arguments
                .get("profile")
                .cloned()
                .ok_or("profile is required and must be a serialized TabularProfile")?,
        )
        .map_err(|error| format!("invalid tabular profile: {error}"))?;
        let source_id = arguments
            .get("source_id")
            .and_then(Value::as_str)
            .ok_or("source_id is required")?;
        let inline = arguments.get("csv").and_then(Value::as_str);
        let document = arguments.get("document").and_then(Value::as_str);
        if inline.is_some() && document.is_some() {
            return Err("provide either csv or document, not both".into());
        }
        let bytes = match (inline, document) {
            (Some(csv), None) => csv.as_bytes().to_vec(),
            (None, Some(relative)) => {
                let path = self.resolve(relative)?;
                if path.is_dir() {
                    return Err("tabular_ingest requires a file, not a directory".into());
                }
                std::fs::read(&path).map_err(|error| {
                    format!("cannot read tabular source {}: {error}", path.display())
                })?
            }
            (None, None) => return Err("tabular_ingest requires csv or document".into()),
            (Some(_), Some(_)) => unreachable!("the mutually exclusive inputs were checked"),
        };
        let max_bytes = arguments
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(10_000_000);
        if max_bytes == 0 || max_bytes > 10_000_000 {
            return Err("max_bytes must be between 1 and 10000000".into());
        }
        if bytes.len() as u64 > max_bytes {
            return Err(format!(
                "tabular source is {} bytes, above max_bytes {}",
                bytes.len(),
                max_bytes
            ));
        }
        let mut source = Source::bytes(source_id, bytes);
        if let Some(format) = arguments.get("format").and_then(Value::as_str) {
            source = source.with_format(format);
        }
        if let Some(raw_provenance) = arguments.get("provenance") {
            let provenance: SourceProvenance = serde_json::from_value(raw_provenance.clone())
                .map_err(|error| format!("invalid source provenance: {error}"))?;
            source = source.with_provenance(provenance);
        }
        let adapter = TabularAdapter::new(profile);
        let (report, ingestion) = certify(&adapter, &source)
            .map_err(|error| format!("tabular ingestion refused: {error}"))?;
        let digest = ingestion
            .digest()
            .map_err(|error| format!("could not hash ingestion: {error}"))?;
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let include_facts = arguments
            .get("include_facts")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut output = json!({
            "ok": true,
            "source_id": source_id,
            "fact_count": ingestion.fact_count(),
            "ingestion_sha256": digest,
            "manifest": ingestion.manifest(),
            "semantic_loss": ingestion.loss(),
            "conformance": {
                "report": report,
                "passed": report.passed(),
                "verified": report.verified(),
                "summary": report.summary(),
            },
            "max_items": max_items,
            "limitations": [
                "this executes the in-process CSV/TSV tabular adapter over caller-supplied bytes; it does not parse DICOM, NIfTI, AnnData, OME-Zarr, BAM/CRAM, or VCF",
                "conformance verifies deterministic output and loss accounting, not the truth of the source declarations",
            ],
        });
        if include_facts {
            output["facts"] = json!(
                ingestion
                    .facts()
                    .iter()
                    .take(max_items as usize)
                    .cloned()
                    .collect::<Vec<_>>()
            );
            output["omitted_facts"] =
                json!(ingestion.fact_count().saturating_sub(max_items as usize));
        }
        Ok(output)
    }

    pub(super) fn observed_world_declare(&self, arguments: &Value) -> Result<Value, String> {
        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .ok_or("id is required")?;
        let sources: Vec<ObservedSourceRef> = serde_json::from_value(
            arguments
                .get("sources")
                .cloned()
                .ok_or("sources is required and must be an array of SourceRef values")?,
        )
        .map_err(|error| format!("invalid observed-world sources: {error}"))?;
        let mut source_names = BTreeSet::new();
        for source in &sources {
            if !source_names.insert(source.name.clone()) {
                return Err(format!("duplicate observed-world source {:?}", source.name));
            }
        }
        let design: StudyDesign = serde_json::from_value(
            arguments
                .get("design")
                .cloned()
                .ok_or("design is required and must be a serialized StudyDesign")?,
        )
        .map_err(|error| format!("invalid observed-world study design: {error}"))?;
        let labels = arguments
            .get("outcome_labels")
            .and_then(Value::as_array)
            .ok_or("outcome_labels is required and must be an array of strings")?;
        let mut outcome_labels = BTreeSet::new();
        for (index, label) in labels.iter().enumerate() {
            let label = label
                .as_str()
                .ok_or_else(|| format!("outcome_labels[{index}] must be a string"))?;
            if !outcome_labels.insert(label.to_string()) {
                return Err(format!("duplicate outcome label {label:?}"));
            }
        }
        let world = declare_observed_world(id, sources, design, outcome_labels)
            .map_err(|error| format!("observed-world declaration refused: {error}"))?;
        let provenance = world.provenance();
        let controlled = world
            .controlled_sources()
            .iter()
            .map(|source| source.name.clone())
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "world": world,
            "provenance": provenance,
            "world_id": world.id(),
            "source_count": world.sources().len(),
            "controlled_sources": controlled,
            "outcome_label_count": world.outcome_labels().len(),
            "guarantees": [
                "unpinned sources, unreconciled strata, and undeclared selection for population claims refuse before publication",
                "the observed rung remains distinct from synthetic and mechanistic provenance",
                "controlled sources are retained as a redistribution boundary rather than silently embedded",
            ],
        }))
    }

    pub(super) fn world_claim_check(&self, arguments: &Value) -> Result<Value, String> {
        let provenance: WorldProvenance = serde_json::from_value(
            arguments
                .get("provenance")
                .cloned()
                .ok_or("provenance is required and must be a serialized world provenance")?,
        )
        .map_err(|error| format!("invalid world provenance: {error}"))?;
        let claim: Claim = serde_json::from_value(
            arguments
                .get("claim")
                .cloned()
                .ok_or("claim is required and must be a serialized Claim")?,
        )
        .map_err(|error| format!("invalid world claim: {error}"))?;
        match support_world_claim(&provenance, claim.clone()) {
            Ok(grounded) => Ok(json!({
                "ok": true,
                "supported": true,
                "claim": grounded.claim(),
                "grounded": grounded,
                "caveat": grounded.caveat(),
                "provenance": provenance,
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "supported": false,
                "claim": claim,
                "refusal": error.to_string(),
                "provenance": provenance,
                "fail_closed": true,
            })),
        }
    }

    pub(super) fn trace_otel_ingest(&self, arguments: &Value) -> Result<Value, String> {
        let trace_id = arguments
            .get("trace_id")
            .and_then(Value::as_str)
            .ok_or("trace_id is required")?;
        if trace_id.trim().is_empty() {
            return Err("trace_id must not be empty".into());
        }

        let max_bytes = arguments
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(10_000_000);
        if max_bytes == 0 || max_bytes > 10_000_000 {
            return Err("max_bytes must be between 1 and 10000000".into());
        }
        let max_spans = arguments
            .get("max_spans")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_OTEL_SPANS as u64);
        if max_spans == 0 || max_spans > MAX_OTEL_SPANS as u64 {
            return Err(format!("max_spans must be between 1 and {MAX_OTEL_SPANS}"));
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }

        let inline = arguments.get("otlp_json").and_then(Value::as_str);
        let document = arguments.get("document").and_then(Value::as_str);
        if inline.is_some() && document.is_some() {
            return Err("provide either otlp_json or document, not both".into());
        }
        let text = match (inline, document) {
            (Some(text), None) => {
                if text.len() as u64 > max_bytes {
                    return Err(format!(
                        "otlp_json is {} bytes, above max_bytes {}",
                        text.len(),
                        max_bytes
                    ));
                }
                text.to_owned()
            }
            (None, Some(relative)) => {
                let path = self.resolve(relative)?;
                if path.is_dir() {
                    return Err("document requires an OTLP JSON file, not a directory".into());
                }
                let metadata = std::fs::metadata(&path).map_err(|error| {
                    format!("cannot inspect OTLP document {}: {error}", path.display())
                })?;
                if metadata.len() > max_bytes {
                    return Err(format!(
                        "OTLP document is {} bytes, above max_bytes {}",
                        metadata.len(),
                        max_bytes
                    ));
                }
                std::fs::read_to_string(&path).map_err(|error| {
                    format!("cannot read OTLP document {}: {error}", path.display())
                })?
            }
            (None, None) => return Err("trace_otel_ingest requires otlp_json or document".into()),
            (Some(_), Some(_)) => unreachable!("OTLP inputs were checked as exclusive"),
        };

        let ingestion = from_otlp_json(
            trace_id,
            &text,
            arguments
                .get("succeeded")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_spans as usize,
        )
        .map_err(|error| format!("OTLP import refused: {error}"))?;
        let validation_error = validate_trace(ingestion.trace())
            .err()
            .map(|error| error.to_string());
        let valid = validation_error.is_none();
        let event_count = ingestion.trace().len();
        let event_preview = arguments
            .get("include_events")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            .then(|| {
                ingestion
                    .trace()
                    .events
                    .iter()
                    .take(max_items as usize)
                    .cloned()
                    .collect::<Vec<_>>()
            });

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/trace-otel-ingest/0.1",
            "trace_id": ingestion.trace().trace_id,
            "event_count": event_count,
            "succeeded": ingestion.trace().succeeded,
            "trace_sha256": ingestion.trace().digest().as_str(),
            "valid": valid,
            "validation_error": validation_error,
            "mapping": ingestion.mapping(),
            "loss": ingestion.loss(),
            "lossless": ingestion.loss().is_lossless(),
            "dropped_events": ingestion.loss().dropped_events(),
            "compilable": valid && ingestion.is_compilable(),
            "events_included": event_preview.is_some(),
            "events": event_preview,
            "omitted_events": event_preview
                .as_ref()
                .map(|events| event_count.saturating_sub(events.len()))
                .unwrap_or(0),
            "guarantees": [
                "OTLP JSON is bounded by both input bytes and source span count before unbounded work",
                "source spans are retained inside normalized Event payloads, while unsupported fields remain explicit in the loss report",
                "parentSpanId becomes caused_by only when the parent is present and earlier after timestamp ordering",
                "event kinds inferred from names, missing timestamps, duplicate attributes, and multi-trace exports are not called lossless or compilable",
                "the caller supplies trajectory success; the adapter never infers benchmark outcome from span status",
            ],
            "limitations": [
                "this is a deterministic OTLP JSON importer, not an OTLP exporter, collector client, network publisher, or clock reader",
                "vendor-specific conventions are preserved as source data but are not interpreted unless they use prism.event.kind or aurora.event.kind",
                "span links are retained but are not converted into Event IR causal parents",
            ],
        }))
    }

    pub(super) fn trace_analyze(&self, arguments: &Value) -> Result<Value, String> {
        let trace_id = arguments
            .get("trace_id")
            .and_then(Value::as_str)
            .ok_or("trace_id is required")?;
        if trace_id.trim().is_empty() {
            return Err("trace_id must not be empty".into());
        }

        let max_bytes = arguments
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(10_000_000);
        if max_bytes == 0 || max_bytes > 10_000_000 {
            return Err("max_bytes must be between 1 and 10000000".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }

        let load_text = |inline_key: &str, document_key: &str| -> Result<Option<String>, String> {
            let inline = arguments.get(inline_key).and_then(Value::as_str);
            let document = arguments.get(document_key).and_then(Value::as_str);
            if inline.is_some() && document.is_some() {
                return Err(format!(
                    "provide either {inline_key} or {document_key}, not both"
                ));
            }
            match (inline, document) {
                (Some(text), None) => {
                    if text.len() as u64 > max_bytes {
                        return Err(format!(
                            "{inline_key} is {} bytes, above max_bytes {}",
                            text.len(),
                            max_bytes
                        ));
                    }
                    Ok(Some(text.to_string()))
                }
                (None, Some(relative)) => {
                    let path = self.resolve(relative)?;
                    if path.is_dir() {
                        return Err(format!(
                            "{document_key} requires a JSONL file, not a directory"
                        ));
                    }
                    let metadata = std::fs::metadata(&path).map_err(|error| {
                        format!("cannot inspect trace document {}: {error}", path.display())
                    })?;
                    if metadata.len() > max_bytes {
                        return Err(format!(
                            "trace document is {} bytes, above max_bytes {}",
                            metadata.len(),
                            max_bytes
                        ));
                    }
                    std::fs::read_to_string(&path).map(Some).map_err(|error| {
                        format!("cannot read trace document {}: {error}", path.display())
                    })
                }
                (None, None) => Ok(None),
                (Some(_), Some(_)) => unreachable!("trace inputs were checked as exclusive"),
            }
        };

        let failing_text = load_text("jsonl", "document")?
            .ok_or("trace_analyze requires jsonl or document for the failing trace")?;
        let succeeded = arguments
            .get("succeeded")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let failing = trace_from_jsonl(trace_id, &failing_text, succeeded);
        validate_trace(failing.trace())
            .map_err(|error| format!("trace validation refused: {error}"))?;

        let passing = load_text("passing_jsonl", "passing_document")?.map(|text| {
            trace_from_jsonl(
                arguments
                    .get("passing_trace_id")
                    .and_then(Value::as_str)
                    .unwrap_or("passing"),
                &text,
                arguments
                    .get("passing_succeeded")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            )
        });
        if let Some(passing) = &passing {
            validate_trace(passing.trace())
                .map_err(|error| format!("passing trace validation refused: {error}"))?;
        }

        let usable_divergence = passing.as_ref().and_then(|passing| {
            (failing.is_compilable() && passing.is_compilable())
                .then(|| first_divergence(failing.trace(), passing.trace()))
        });
        let divergence_refusal = passing.as_ref().and_then(|passing| {
            if failing.is_compilable() && passing.is_compilable() {
                None
            } else {
                Some("divergence is withheld because both traces must be non-empty and lossless")
            }
        });
        let divergence_step = usable_divergence
            .as_ref()
            .and_then(|item| item.failing_step());
        let candidates = segment_trace(failing.trace(), divergence_step);
        let excluded = excluded_trace(failing.trace());
        let candidate_count = candidates.len();
        let excluded_count = excluded.len();
        let bounded_candidates = candidates
            .iter()
            .take(max_items as usize)
            .cloned()
            .collect::<Vec<_>>();
        let proposals = bounded_candidates
            .iter()
            .filter_map(|candidate| {
                CellProposal::from_candidate(failing.trace(), candidate, usable_divergence.as_ref())
                    .ok()
            })
            .collect::<Vec<_>>();
        let bounded_excluded = excluded
            .iter()
            .take(max_items as usize)
            .map(|(step, reason)| json!({ "step": step, "reason": reason }))
            .collect::<Vec<_>>();
        let failing_digest = failing.trace().digest().as_str().to_string();

        Ok(json!({
            "ok": true,
            "trace_id": failing.trace().trace_id,
            "event_count": failing.trace().len(),
            "succeeded": failing.trace().succeeded,
            "trace_sha256": failing_digest,
            "valid": true,
            "loss": failing.loss(),
            "lossless": failing.loss().is_lossless(),
            "dropped_events": failing.loss().dropped_events(),
            "compilable": failing.is_compilable(),
            "passing": passing.as_ref().map(|item| json!({
                "trace_id": item.trace().trace_id,
                "event_count": item.trace().len(),
                "succeeded": item.trace().succeeded,
                "trace_sha256": item.trace().digest().as_str().to_string(),
                "loss": item.loss(),
                "lossless": item.loss().is_lossless(),
                "dropped_events": item.loss().dropped_events(),
                "compilable": item.is_compilable(),
            })),
            "divergence": usable_divergence,
            "divergence_actionable": usable_divergence
                .as_ref()
                .map(|item| divergence_is_actionable(item, failing.trace()))
                .unwrap_or(false),
            "divergence_refusal": divergence_refusal,
            "candidate_count": candidate_count,
            "candidates": bounded_candidates,
            "omitted_candidates": candidate_count.saturating_sub(max_items as usize),
            "proposals": proposals,
            "approval_required": true,
            "excluded_count": excluded_count,
            "excluded": bounded_excluded,
            "omitted_excluded": excluded_count.saturating_sub(max_items as usize),
            "review_reduction": review_reduction(failing.trace(), &candidates),
            "guarantees": [
                "JSONL import always returns an explicit loss report; untyped or unparsed lines are never guessed into events",
                "structural validation runs before divergence or candidate generation",
                "divergence compares semantic event content and only becomes usable when both traces are lossless and non-empty",
                "candidate scores expose alternatives, newly visible evidence, downstream reach, and divergence bonus",
                "cell proposals retain trace digest and review context but cannot become Decision Cells without a named reviewer",
            ],
            "limitations": [
                "this tool accepts native JSONL only; trace_otel_ingest handles bounded OTLP JSON imports",
                "segmentation is a transparent arithmetic heuristic, not a validated model or claim that the top candidate is correct",
                "this tool does not minimize state, replay tools, run a benchmark, or publish a Decision Cell",
            ],
        }))
    }

    pub(super) fn lineage_audit(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let registry: SpecimenRegistry = serde_json::from_value(
            arguments
                .get("registry")
                .cloned()
                .ok_or("registry is required and must be a serialized SpecimenRegistry")?,
        )
        .map_err(|error| format!("invalid specimen registry: {error}"))?;
        if registry.nodes.len() > 10_000 {
            return Err("registry must contain at most 10000 specimens".into());
        }
        if registry.artifacts.len() > 20_000 {
            return Err("registry must contain at most 20000 artifacts".into());
        }

        let audit = audit_lineage(&registry);
        let finding_count = audit.findings.len();
        let fingerprint_count = audit.fingerprints.len();
        let unchecked = audit.unchecked_specimens();
        let unchecked_count = unchecked.len();
        let identity_complete = audit.fingerprints.iter().all(|check| check.is_consistent());
        Ok(json!({
            "ok": true,
            "specimen_count": registry.nodes.len(),
            "artifact_count": registry.artifacts.len(),
            "finding_count": finding_count,
            "clean": audit.is_clean(),
            "identity_complete": identity_complete,
            "fingerprint_count": fingerprint_count,
            "fingerprints": audit.fingerprints.iter().take(max_items as usize).collect::<Vec<_>>(),
            "omitted_fingerprints": fingerprint_count.saturating_sub(max_items as usize),
            "unchecked_identity_count": unchecked_count,
            "unchecked_identity": unchecked
                .iter()
                .take(max_items as usize)
                .map(|specimen| specimen.as_str())
                .collect::<Vec<_>>(),
            "finding_count_returned": finding_count.min(max_items as usize),
            "findings": audit
                .findings
                .iter()
                .take(max_items as usize)
                .collect::<Vec<_>>(),
            "omitted_findings": finding_count.saturating_sub(max_items as usize),
            "guarantees": [
                "lineage cycles, mass over-allocation, temporal implausibility, duplicate material, artifact disagreement, and fingerprint mismatch remain typed findings",
                "missing fingerprint evidence is reported separately from a consistent fingerprint and never counts as a pass",
                "the audit consumes declared registry records without silently sealing or rewriting artifact observations",
            ],
            "limitations": [
                "fingerprints are opaque donor identifiers compared by equality; no genotyping, call-rate, or discrimination calculation is performed",
                "access_domain is a declared boundary, not an access-control enforcement point",
                "the audit checks the supplied registry and cannot establish that source records or laboratory assertions are true",
            ],
        }))
    }

    pub(super) fn preanalytic_apply(&self, arguments: &Value) -> Result<Value, String> {
        let specimen: Specimen = serde_json::from_value(
            arguments
                .get("specimen")
                .cloned()
                .ok_or("specimen is required and must be a serialized Specimen")?,
        )
        .map_err(|error| format!("invalid pre-analytic specimen: {error}"))?;
        let mutation: PreanalyticMutation = serde_json::from_value(
            arguments
                .get("mutation")
                .cloned()
                .ok_or("mutation is required and must be a serialized PreanalyticMutation")?,
        )
        .map_err(|error| format!("invalid pre-analytic mutation: {error}"))?;
        if mutation.edits.len() > 100 {
            return Err("mutation may contain at most 100 edits".into());
        }
        let before_biology = specimen.biology_digest();
        let before_digest = specimen.digest();

        let available_actions = arguments
            .get("available_actions")
            .map(|raw| {
                raw.as_array()
                    .ok_or("available_actions must be an array of strings")?
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        value
                            .as_str()
                            .map(ToString::to_string)
                            .ok_or_else(|| format!("available_actions[{index}] must be a string"))
                    })
                    .collect::<Result<std::collections::BTreeSet<_>, _>>()
            })
            .transpose()?;
        let response_check = available_actions.as_ref().map(|actions| {
            check_preanalytic_response(&mutation, actions)
                .map(|()| json!({ "ok": true }))
                .unwrap_or_else(|error| {
                    json!({ "ok": false, "refusal": error.to_string(), "fail_closed": true })
                })
        });

        let family = arguments
            .get("family")
            .map(|raw| {
                serde_json::from_value::<Vec<PreanalyticMutation>>(raw.clone())
                    .map_err(|error| format!("invalid pre-analytic family: {error}"))
            })
            .transpose()?;
        if family.as_ref().is_some_and(|members| members.len() > 100) {
            return Err("pre-analytic family may contain at most 100 mutations".into());
        }
        let family_validation = family.as_ref().map(|members| {
            let family_name = arguments
                .get("family_name")
                .and_then(Value::as_str)
                .unwrap_or(&mutation.family);
            validate_family(&specimen, family_name, members)
                .map(|()| json!({ "ok": true, "family": family_name }))
                .unwrap_or_else(|error| {
                    json!({
                        "ok": false,
                        "family": family_name,
                        "refusal": error.to_string(),
                        "fail_closed": true
                    })
                })
        });
        let detectability = match (
            family.as_ref(),
            arguments.get("qc_field").and_then(Value::as_str),
            arguments.get("alert_at").and_then(Value::as_i64),
        ) {
            (Some(members), Some(qc_field), Some(alert_at)) if alert_at >= 0 => {
                detectability_floor(&specimen, members, qc_field, alert_at).map(|intensity| {
                    json!({ "qc_field": qc_field, "alert_at": alert_at, "intensity": intensity.get() })
                })
            }
            (Some(_), Some(_), Some(_)) => {
                return Err("alert_at must be non-negative".into())
            }
            _ => None,
        };

        match apply_preanalytic(&specimen, &mutation) {
            Ok(faulted) => Ok(json!({
                "ok": true,
                "applied": true,
                "mutation": mutation,
                "stage": faulted.stage,
                "faulted": faulted,
                "biology_digest_before": before_biology,
                "biology_digest_after": faulted.specimen.biology_digest(),
                "biology_unchanged": before_biology == faulted.specimen.biology_digest(),
                "specimen_digest_before": before_digest,
                "specimen_digest_after": faulted.specimen.digest(),
                "has_signature": faulted.has_signature(),
                "response_check": response_check,
                "family_validation": family_validation,
                "detectability": detectability,
                "guarantees": [
                    "a non-null pre-analytic mutation is admitted only when biology remains byte-identical and QC or measurability carries a signature",
                    "QC labels that name the injected fault, stale downstream state, biological edits, and absent signatures refuse",
                    "false-positive family controls and response availability are independently reported rather than folded into application success",
                ],
                "limitations": [
                    "fault kinds and effects are caller-declared abstractions; this does not model a laboratory, assay physics, or clinical thresholds",
                    "detectability_floor uses the caller's QC field and alert threshold and skips mutations that refuse application",
                ],
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "applied": false,
                "mutation": mutation,
                "biology_digest_before": before_biology,
                "specimen_digest_before": before_digest,
                "refusal": error.to_string(),
                "response_check": response_check,
                "family_validation": family_validation,
                "detectability": detectability,
                "fail_closed": true,
            })),
        }
    }

    pub(super) fn contradiction_review(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let left: Reading = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a serialized contradiction Reading")?,
        )
        .map_err(|error| format!("invalid left reading: {error}"))?;
        let right: Reading = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a serialized contradiction Reading")?,
        )
        .map_err(|error| format!("invalid right reading: {error}"))?;
        let intent: DiscordanceClass = serde_json::from_value(
            arguments
                .get("intent")
                .cloned()
                .ok_or("intent is required and must be expected, resolvable, or irreducible")?,
        )
        .map_err(|error| format!("invalid contradiction intent: {error}"))?;
        let hypotheses: Vec<Hypothesis> = serde_json::from_value(
            arguments
                .get("hypotheses")
                .cloned()
                .ok_or("hypotheses is required and must be an array of Hypothesis values")?,
        )
        .map_err(|error| format!("invalid contradiction hypotheses: {error}"))?;
        if hypotheses.is_empty() {
            return Err("hypotheses must contain at least one account".into());
        }
        if hypotheses.len() > 1_000 {
            return Err("hypotheses may contain at most 1000 accounts".into());
        }
        let mut hypothesis_ids = BTreeSet::new();
        for hypothesis in &hypotheses {
            if !hypothesis_ids.insert(hypothesis.id.to_string()) {
                return Err(format!(
                    "hypotheses must not contain duplicate id {:?}",
                    hypothesis.id.to_string()
                ));
            }
        }
        let actions: Vec<DiscriminatingAction> = serde_json::from_value(
            arguments
                .get("actions")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid discriminating actions: {error}"))?;
        if actions.len() > 1_000 {
            return Err("actions may contain at most 1000 entries".into());
        }
        let mut action_ids = BTreeSet::new();
        for action in &actions {
            if !action_ids.insert(action.evidence.to_string()) {
                return Err(format!(
                    "actions must not contain duplicate evidence {:?}",
                    action.evidence.to_string()
                ));
            }
        }
        let missing: Vec<MissingEvidence> = serde_json::from_value(
            arguments
                .get("missing_evidence")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid missing evidence: {error}"))?;
        let references: Vec<ReferenceDiscordance> = serde_json::from_value(
            arguments
                .get("references")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid reference discordance: {error}"))?;
        for (name, count) in [
            ("missing_evidence", missing.len()),
            ("references", references.len()),
        ] {
            if count > 1_000 {
                return Err(format!("{name} may contain at most 1000 entries"));
            }
        }
        let examine: Vec<String> = serde_json::from_value(
            arguments
                .get("examine")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid examine list: {error}"))?;
        if examine.len() > 1_000 {
            return Err("examine may contain at most 1000 evidence ids".into());
        }
        let mut examine_ids = BTreeSet::new();
        for evidence in &examine {
            if !examine_ids.insert(evidence.clone()) {
                return Err(format!(
                    "examine must not contain duplicate evidence {evidence:?}"
                ));
            }
        }

        let contradiction = match pose_contradiction(left, right) {
            Ok(contradiction) => contradiction,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "pose",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }));
            }
        };
        let mut hypothesis_set = HypothesisSet::new();
        for hypothesis in hypotheses {
            hypothesis_set = hypothesis_set.with(hypothesis);
        }
        let mut program = ContradictionProgram::new(contradiction, intent, hypothesis_set);
        for action in actions {
            program = program.with_action(action);
        }
        for item in missing {
            program = program.with_missing(item);
        }
        for reference in references {
            program = program.with_reference(reference);
        }
        let validated = match validate_contradiction(program) {
            Ok(validated) => validated,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "validation",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }));
            }
        };
        let validation_intent = validated.intent_check().clone();
        let admissible_count = validated.admissible().len();
        let mut examined_program = validated.program().clone();
        let mut examined = Vec::new();
        for evidence in &examine {
            match examined_program.examine(&EvidenceId::new(evidence)) {
                Ok(_) => examined.push(evidence.clone()),
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "validated": true,
                        "stage": "examine",
                        "examined": examined,
                        "refusal": error.to_string(),
                        "state": examined_program.state(),
                        "fail_closed": true,
                    }));
                }
            }
        }
        let expectedness = arguments
            .get("notable_below_per_ten_thousand")
            .and_then(Value::as_u64)
            .map(|threshold| {
                contradiction_expectedness(&examined_program, threshold)
                    .map(|value| json!({ "ok": true, "value": value, "threshold": threshold }))
                    .unwrap_or_else(|error| {
                        json!({
                            "ok": false,
                            "threshold": threshold,
                            "refusal": error.to_string(),
                            "fail_closed": true
                        })
                    })
            });
        let state = examined_program.state();
        let next_actions = contradiction_next_actions(&examined_program);
        let cues = cue_scan(&examined_program);
        let post_intent = check_intent(&examined_program);
        Ok(json!({
            "ok": true,
            "validated": true,
            "contradiction": examined_program.contradiction(),
            "intent": examined_program.intent(),
            "declared_hypothesis_count": examined_program.declared().len(),
            "admissible_hypothesis_count": admissible_count,
            "admissible_hypotheses": validated.admissible(),
            "validation_intent_check": validation_intent,
            "post_examination_intent_check": post_intent,
            "examined": examined,
            "state": state,
            "state_name": state.as_str(),
            "live_hypothesis_count": examined_program.live().len(),
            "next_actions": next_actions.iter().take(max_items as usize).collect::<Vec<_>>(),
            "omitted_next_actions": next_actions.len().saturating_sub(max_items as usize),
            "cue_count": cues.len(),
            "cues": cues.iter().take(max_items as usize).collect::<Vec<_>>(),
            "omitted_cues": cues.len().saturating_sub(max_items as usize),
            "expectedness": expectedness,
            "guarantees": [
                "readings are posed only when they concern the same quantity, overlap in scope, were both examined, and disagree beyond declared uncertainty",
                "hypotheses are filtered by structural admissibility and answer cues are refused rather than silently tolerated",
                "resolution keeps resolved, not-yet-examined, and unresolvable states distinct",
                "narrowing requires named discriminating evidence and refuses an action that would erase every live account",
                "next-action ranking is a declared set-cover surrogate with cost tie-breaking, not an invented probability or information-gain claim",
            ],
            "limitations": [
                "modality, quantity, scope, and reading values are caller-supplied declarations; no imaging, pathology, assay, or biological truth is loaded",
                "expectedness requires a caller-supplied reference discordance distribution and threshold",
                "this tool does not choose a correct modality, reconcile contradictory evidence, or execute missing acquisitions",
            ],
        }))
    }

    pub(super) fn ledger_ingest(&self, arguments: &Value) -> Result<Value, String> {
        let raw_events = arguments
            .get("events")
            .cloned()
            .ok_or("events is required and must be an array of serialized Event values")?;
        let event_values = raw_events
            .as_array()
            .ok_or("events must be an array of serialized Event values")?;
        if event_values.is_empty() || event_values.len() > 50_000 {
            return Err("events must contain between 1 and 50000 events".into());
        }
        let encoded = serde_json::to_vec(&raw_events)
            .map_err(|error| format!("cannot measure ledger envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("events exceed the 20000000-byte safety bound".into());
        }
        let events: Vec<Event> = serde_json::from_value(raw_events)
            .map_err(|error| format!("invalid ledger event stream: {error}"))?;
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let include_receipts = arguments
            .get("include_receipts")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut ledger = EventLedger::new();
        let mut admissions = Vec::new();
        let mut recorded = 0usize;
        let mut duplicates = 0usize;
        let mut quarantined = 0usize;
        let mut released = 0usize;
        for (index, event) in events.into_iter().enumerate() {
            match ledger.append(event) {
                Ok(receipt) => {
                    recorded += usize::from(matches!(
                        &receipt.admission,
                        bioprism_ledger::Admission::Recorded { .. }
                    )) + receipt.released.len();
                    duplicates += usize::from(receipt.admission.is_duplicate());
                    quarantined += usize::from(receipt.admission.is_quarantined());
                    released += receipt.released.len();
                    if include_receipts && admissions.len() < max_items {
                        admissions.push(json!({
                            "event_index": index,
                            "receipt": receipt,
                        }));
                    }
                }
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/ledger-ingest/0.1",
                        "stage": "append",
                        "event_index": index,
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "ledger_before_refusal": {
                            "recorded_entries": ledger.len(),
                            "quarantined": ledger.quarantined().len(),
                            "next_seq": ledger.next_seq(),
                            "chain": ledger.verify_chain(),
                        },
                        "guarantee": "events after the first append refusal are not processed or silently discarded"
                    }));
                }
            }
        }

        let cut = if let Some(raw_cut) = arguments.get("cut") {
            Some(
                serde_json::from_value::<TemporalCut>(raw_cut.clone())
                    .map_err(|error| format!("invalid temporal cut: {error}"))?,
            )
        } else {
            None
        };
        let cut_summary = if let Some(cut) = cut {
            match ledger.cut(&cut) {
                Ok(entries) => json!({
                    "requested": cut,
                    "count": entries.len(),
                    "entries": entries.iter().take(max_items).map(|entry| json!({
                        "seq": entry.seq,
                        "id": entry.id,
                        "class": entry.event.class,
                        "kind": entry.event.kind,
                        "subject": entry.event.subject,
                        "valid": entry.event.times.valid,
                        "record": entry.event.times.record,
                        "release": entry.event.times.release,
                    })).collect::<Vec<_>>(),
                    "omitted": entries.len().saturating_sub(max_items),
                }),
                Err(error) => json!({
                    "requested": cut,
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }),
            }
        } else {
            Value::Null
        };

        let subject_projection = ledger.project(&SubjectLatest);
        let latest = subject_projection
            .state
            .iter()
            .take(max_items)
            .map(|(subject, fact)| {
                json!({
                    "subject": subject,
                    "event": fact.event,
                    "seq": fact.seq,
                    "valid": fact.valid,
                    "payload_digest": fact.payload_digest,
                })
            })
            .collect::<Vec<_>>();
        let class_projection = ledger.project(&ClassCounts);
        let quarantined_entries = ledger
            .quarantined()
            .iter()
            .take(max_items)
            .map(|held| {
                json!({
                    "key": held.key,
                    "missing": held.missing,
                    "note": held.note,
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/ledger-ingest/0.1",
            "entries": ledger.len(),
            "next_seq": ledger.next_seq(),
            "head": ledger.head(),
            "admissions": {
                "recorded": recorded,
                "duplicates": duplicates,
                "quarantined": quarantined,
                "released": released,
                "receipts": include_receipts.then_some(admissions),
            },
            "chain": ledger.verify_chain(),
            "clock_anomalies": ledger.clock_consistency(),
            "quarantine": {
                "count": ledger.quarantined().len(),
                "items": quarantined_entries,
                "omitted": ledger.quarantined().len().saturating_sub(max_items),
            },
            "class_counts": class_projection.state,
            "latest_by_subject": {
                "count": subject_projection.state.len(),
                "items": latest,
                "omitted": subject_projection.state.len().saturating_sub(max_items),
            },
            "cut": cut_summary,
            "guarantees": [
                "valid, record, and release times remain separate and caller-supplied",
                "unknown causal parents quarantine instead of creating dangling history",
                "duplicate idempotent events converge while conflicting keys refuse",
                "hash-chain, clock-anomaly, quarantine, and projection states remain independently visible",
                "payload bodies are not returned by default; projections carry digests rather than copied payloads",
            ],
        }))
    }
}
