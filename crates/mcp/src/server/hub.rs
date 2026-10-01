//! MCP Public research-hub and submission handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn hub_search(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let federation: HubFederation =
            serde_json::from_value(arguments.get("federation").cloned().ok_or(
                "federation is required and must be a serialized bioprism-hubapi Federation",
            )?)
            .map_err(|error| format!("invalid federation: {error}"))?;
        let catalogs: Vec<HubCatalog> = serde_json::from_value(
            arguments
                .get("catalogs")
                .cloned()
                .ok_or("catalogs is required and must be an array of serialized Catalog values")?,
        )
        .map_err(|error| format!("invalid catalogs: {error}"))?;
        if catalogs.len() > 100 {
            return Err("catalogs must contain at most 100 catalogs".into());
        }
        let release_count = catalogs.iter().map(HubCatalog::len).sum::<usize>();
        if release_count > 10_000 {
            return Err("catalogs may contain at most 10000 releases in one request".into());
        }
        let query: HubQuery = serde_json::from_value(
            arguments
                .get("query")
                .cloned()
                .ok_or("query is required and must be a serialized bioprism-hubapi Query")?,
        )
        .map_err(|error| format!("invalid hub query: {error}"))?;
        if query.limit == Some(0) {
            return Err("query.limit must be at least 1 when supplied".into());
        }
        let requested_limit = query.limit;
        let effective_limit =
            requested_limit.map_or(max_items as usize, |limit| limit.min(max_items as usize));
        let mut bounded_query = query;
        bounded_query.limit = Some(effective_limit);
        let results = hub_search_query(&federation, &catalogs, &bounded_query)
            .map_err(|error| format!("hub search refused: {error}"))?;
        let match_count = results.matches.len();
        let excluded_count = results.excluded.len();
        let matches = results.matches;
        let excluded = results
            .excluded
            .iter()
            .take(max_items as usize)
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "catalog_count": catalogs.len(),
            "release_count": release_count,
            "requested_limit": requested_limit,
            "effective_limit": effective_limit,
            "matches": matches,
            "match_count": match_count,
            "excluded": excluded,
            "excluded_count": excluded_count,
            "omitted_excluded": excluded_count.saturating_sub(max_items as usize),
            "truncated": results.truncated || excluded_count > max_items as usize,
            "guarantees": [
                "empty facet queries refuse rather than returning unexplained recommendations",
                "every match carries its matching facets, authority, tier, digest, and freshness",
                "near misses retain the facet that excluded them",
                "ranking uses declared catalog evidence and tier, never popularity telemetry",
            ],
            "limitations": [
                "federation membership, catalog contents, and freshness epochs are caller-supplied in-memory values",
                "this is exact facet and lexical search; it does not fetch registries, verify signatures, embed text, or infer semantic similarity",
            ],
        }))
    }

    pub(super) fn hub_inputs(
        &self,
        arguments: &Value,
    ) -> Result<(HubFederation, Vec<HubCatalog>), String> {
        let federation: HubFederation =
            serde_json::from_value(arguments.get("federation").cloned().ok_or(
                "federation is required and must be a serialized bioprism-hubapi Federation",
            )?)
            .map_err(|error| format!("invalid federation: {error}"))?;
        let catalogs: Vec<HubCatalog> = serde_json::from_value(
            arguments
                .get("catalogs")
                .cloned()
                .ok_or("catalogs is required and must be an array of serialized Catalog values")?,
        )
        .map_err(|error| format!("invalid catalogs: {error}"))?;
        if catalogs.len() > 100 {
            return Err("catalogs must contain at most 100 catalogs".into());
        }
        let release_count = catalogs.iter().map(HubCatalog::len).sum::<usize>();
        if release_count > 10_000 {
            return Err("catalogs may contain at most 10000 releases in one request".into());
        }
        Ok((federation, catalogs))
    }

    pub(super) fn hub_resolve(&self, arguments: &Value) -> Result<Value, String> {
        let (federation, catalogs) = self.hub_inputs(arguments)?;
        let request: HubRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or("request is required and must be a serialized bioprism-hubapi Request")?,
        )
        .map_err(|error| format!("invalid hub resolution request: {error}"))?;
        let resolution = hub_resolve_in(&federation, &catalogs, &request)
            .map_err(|error| format!("hub resolution refused: {error}"))?;
        Ok(json!({
            "ok": true,
            "resolution": resolution,
            "answered_by": resolution.answered_by(),
            "authoritative": resolution.is_authoritative(),
            "catalog_count": catalogs.len(),
            "guarantees": [
                "the federation is checked before a catalog answer is accepted",
                "authoritative catalogs are preferred and every agreeing catalog is cross-checked for digest divergence",
                "freshness policy and lifecycle intent travel inside the resolution provenance",
            ],
            "limitations": [
                "catalogs, federation membership, signatures, and epochs are caller-supplied values; this process does not fetch or authenticate registries",
            ],
        }))
    }

    pub(super) fn hub_lock(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let (federation, catalogs) = self.hub_inputs(arguments)?;
        let root: HubRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or("request is required and must be a serialized root Request")?,
        )
        .map_err(|error| format!("invalid root dependency request: {error}"))?;
        let lock = hub_resolve_dependencies(&federation, &catalogs, &root)
            .map_err(|error| format!("dependency closure refused: {error}"))?;
        let entries = lock
            .entries()
            .take(max_items as usize)
            .map(|(name, locked)| json!({ "name": name, "locked": locked }))
            .collect::<Vec<_>>();
        let entry_count = lock.len();
        Ok(json!({
            "ok": true,
            "entry_count": entry_count,
            "fully_authoritative": lock.is_fully_authoritative(),
            "answering_registries": lock.answering_registries(),
            "remarked_entry_count": lock.remarked().len(),
            "entries": entries,
            "omitted_entries": entry_count.saturating_sub(max_items as usize),
            "max_items": max_items,
            "guarantees": [
                "transitive dependencies are fixed by a bounded deterministic fixpoint rather than silently omitted",
                "version collisions and missing satisfiers refuse with the imposing requirements named",
                "each lock entry preserves resolution authority, freshness, digest, lifecycle notes, and required-by provenance",
            ],
        }))
    }

    pub(super) fn hub_submission_review(&self, arguments: &Value) -> Result<Value, String> {
        let raw_draft = arguments
            .get("draft")
            .cloned()
            .ok_or("draft is required and must be a serialized SubmissionDraft")?;
        let raw_submitter = arguments
            .get("submitter")
            .cloned()
            .ok_or("submitter is required and must be a serialized Submitter")?;
        let encoded = serde_json::to_vec(&json!({
            "draft": raw_draft,
            "submitter": raw_submitter,
            "moderation": arguments.get("moderation"),
        }))
        .map_err(|error| format!("cannot measure public-hub input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("public-hub input exceeds the 20000000-byte safety bound".into());
        }
        let draft: SubmissionDraft = serde_json::from_value(raw_draft)
            .map_err(|error| format!("invalid submission draft: {error}"))?;
        let submitter: Submitter = serde_json::from_value(raw_submitter)
            .map_err(|error| format!("invalid submitter: {error}"))?;
        let submission = match accept_hub_submission(draft, &submitter) {
            Ok(submission) => submission,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/hub-submission/0.1",
                    "stage": "submission_acceptance",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "submission": Value::Null,
                    "ledger": Value::Null,
                    "guarantees": [
                        "required provenance, scope, licence, evidence scale, conflict, and nonclaim fields are checked before any moderation state exists",
                        "a refused draft is never represented as submitted or publishable",
                    ],
                }));
            }
        };
        let submission_value = serde_json::to_value(&submission)
            .map_err(|error| format!("cannot serialize accepted submission: {error}"))?;
        let submission_id = submission.id.clone();
        let Some(raw_moderation) = arguments.get("moderation") else {
            return Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/hub-submission/0.1",
                "stage": "submission_acceptance",
                "submission": submission_value,
                "limitation_card": submission.limitations_card(),
                "moderation": Value::Null,
                "guarantees": [
                    "acceptance is a contract check, not identity authentication or content verification",
                    "self-reported verification remains self-reported until an independent ledger attestation",
                    "public publication requires a separate moderation action",
                ],
            }));
        };
        let moderation = raw_moderation
            .as_object()
            .ok_or("moderation must be an object")?;
        let open_actor = moderation
            .get("actor")
            .and_then(Value::as_str)
            .unwrap_or("hub-mcp");
        let open_at = moderation
            .get("at")
            .cloned()
            .map(serde_json::from_value::<HubEpoch>)
            .transpose()
            .map_err(|error| format!("invalid moderation opening epoch: {error}"))?
            .unwrap_or(submission.submitted_at);
        let mut ledger = ModerationLedger::new();
        if let Err(error) = ledger.open(submission, open_actor, open_at) {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/hub-submission/0.1",
                "stage": "moderation_open",
                "refusal": error.to_string(),
                "fail_closed": true,
                "submission": submission_value,
                "ledger": Value::Null,
            }));
        }

        let transitions = moderation
            .get("transitions")
            .map(|value| {
                value
                    .as_array()
                    .ok_or("moderation.transitions must be an array")
            })
            .transpose()?
            .cloned()
            .unwrap_or_default();
        let attestations = moderation
            .get("attestations")
            .map(|value| {
                value
                    .as_array()
                    .ok_or("moderation.attestations must be an array")
            })
            .transpose()?
            .cloned()
            .unwrap_or_default();
        let revocations = moderation
            .get("revocations")
            .map(|value| {
                value
                    .as_array()
                    .ok_or("moderation.revocations must be an array")
            })
            .transpose()?
            .cloned()
            .unwrap_or_default();
        if transitions.len() > 32 || attestations.len() > 32 || revocations.len() > 32 {
            return Err("moderation action lists are bounded at 32 entries each".into());
        }

        for raw in transitions {
            let to: ModerationState = serde_json::from_value(
                raw.get("to")
                    .cloned()
                    .ok_or("each moderation transition requires `to`")?,
            )
            .map_err(|error| format!("invalid moderation state: {error}"))?;
            let decision: HubDecision = serde_json::from_value(
                raw.get("decision")
                    .cloned()
                    .ok_or("each moderation transition requires `decision`")?,
            )
            .map_err(|error| format!("invalid moderation decision: {error}"))?;
            if let Err(error) = ledger.transition(&submission_id, to, decision) {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/hub-submission/0.1",
                    "stage": "moderation_transition",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "submission": submission_value,
                    "ledger": ledger,
                }));
            }
        }
        for raw in attestations {
            let to: VerificationStatus = serde_json::from_value(
                raw.get("to")
                    .cloned()
                    .ok_or("each attestation requires `to`")?,
            )
            .map_err(|error| format!("invalid verification status: {error}"))?;
            let actor = raw
                .get("actor")
                .and_then(Value::as_str)
                .ok_or("each attestation requires an actor")?;
            let at: HubEpoch = serde_json::from_value(
                raw.get("at")
                    .cloned()
                    .ok_or("each attestation requires `at`")?,
            )
            .map_err(|error| format!("invalid attestation epoch: {error}"))?;
            if let Err(error) = ledger.attest(&submission_id, to, actor, at) {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/hub-submission/0.1",
                    "stage": "moderation_attestation",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "submission": submission_value,
                    "ledger": ledger,
                }));
            }
        }
        for raw in revocations {
            let actor = raw
                .get("actor")
                .and_then(Value::as_str)
                .ok_or("each attestation revocation requires an actor")?;
            let at: HubEpoch = serde_json::from_value(
                raw.get("at")
                    .cloned()
                    .ok_or("each attestation revocation requires `at`")?,
            )
            .map_err(|error| format!("invalid revocation epoch: {error}"))?;
            let reason = raw
                .get("reason")
                .and_then(Value::as_str)
                .ok_or("each attestation revocation requires a reason")?;
            if let Err(error) = ledger.revoke_attestation(&submission_id, actor, at, reason) {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/hub-submission/0.1",
                    "stage": "moderation_attestation_revocation",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "submission": submission_value,
                    "ledger": ledger,
                }));
            }
        }
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/hub-submission/0.1",
            "stage": "moderation_ledger",
            "submission": submission_value,
            "limitation_card": ledger.record(&submission_id).map(|record| record.submission.limitations_card()),
            "state": ledger.state(&submission_id).map(ModerationState::as_str),
            "verification": ledger.verification(&submission_id),
            "published": ledger.published(),
            "event_count": ledger.events().len(),
            "ledger": ledger,
            "guarantees": [
                "moderation is an append-only in-memory state machine with monotonic epochs",
                "rejection, withdrawal, reopening, and supersession carry typed transition rules and reasons",
                "self-review and self-asserted verification are refused",
                "this call does not persist, authenticate, publish to a network, or render a public web page",
            ],
        }))
    }

    pub(super) fn hub_disclosure_review(&self, arguments: &Value) -> Result<Value, String> {
        let raw_actions = arguments
            .get("actions")
            .and_then(Value::as_array)
            .ok_or("actions is required and must be an array of disclosure operations")?;
        if raw_actions.len() > 256 {
            return Err("disclosure actions are bounded at 256 entries".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "ledger": arguments.get("ledger"),
            "actions": raw_actions,
        }))
        .map_err(|error| format!("cannot measure disclosure input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("disclosure input exceeds the 20000000-byte safety bound".into());
        }
        let mut ledger = arguments
            .get("ledger")
            .cloned()
            .map(serde_json::from_value::<DisclosureLedger>)
            .transpose()
            .map_err(|error| format!("invalid disclosure ledger: {error}"))?
            .unwrap_or_else(DisclosureLedger::new);
        let mut trace = Vec::with_capacity(raw_actions.len());
        let mut failures = 0usize;
        for (index, raw_action) in raw_actions.iter().enumerate() {
            let kind = raw_action
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("<missing>")
                .to_string();
            let action_result: Result<Value, String> = (|| {
                let pack = content_hash_argument(raw_action.get("pack"), "pack")?;
                match kind.as_str() {
                    "declare_held_out" => {
                        ledger
                            .declare_held_out(&pack)
                            .map_err(|error| format!("declare held-out refused: {error}"))?;
                        Ok(json!({ "pack": pack, "state": ledger.state(&pack) }))
                    }
                    "disclose" => {
                        let at = HubEpoch(
                            raw_action
                                .get("at")
                                .and_then(Value::as_u64)
                                .ok_or("disclose requires non-negative integer at")?,
                        );
                        ledger
                            .disclose(&pack, at)
                            .map_err(|error| format!("disclosure refused: {error}"))?;
                        Ok(json!({ "pack": pack, "state": ledger.state(&pack) }))
                    }
                    "contaminate" => {
                        let witness: ContaminationWitness = serde_json::from_value(
                            raw_action
                                .get("witness")
                                .cloned()
                                .ok_or("contaminate requires witness")?,
                        )
                        .map_err(|error| format!("invalid contamination witness: {error}"))?;
                        ledger
                            .record_contamination(&pack, witness)
                            .map_err(|error| format!("contamination record refused: {error}"))?;
                        Ok(json!({ "pack": pack, "state": ledger.state(&pack) }))
                    }
                    "split_integrity" => {
                        let verdict: bioprism_section::OracleVerdict = serde_json::from_value(
                            raw_action
                                .get("verdict")
                                .cloned()
                                .ok_or("split_integrity requires verdict")?,
                        )
                        .map_err(|error| format!("invalid split-integrity verdict: {error}"))?;
                        let at = HubEpoch(
                            raw_action
                                .get("at")
                                .and_then(Value::as_u64)
                                .ok_or("split_integrity requires non-negative integer at")?,
                        );
                        let reported_by = raw_action
                            .get("reported_by")
                            .and_then(Value::as_str)
                            .ok_or("split_integrity requires reported_by")?;
                        let state = ledger
                            .record_split_integrity(&pack, &verdict, at, reported_by)
                            .map_err(|error| format!("split-integrity record refused: {error}"))?;
                        Ok(json!({ "pack": pack, "state": state }))
                    }
                    "headline_eligibility" => {
                        let computed_at = HubEpoch(
                            raw_action
                                .get("computed_at")
                                .and_then(Value::as_u64)
                                .ok_or("headline_eligibility requires computed_at")?,
                        );
                        let acknowledges = raw_action
                            .get("acknowledges_disclosure")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        match ledger.headline_eligibility(&pack, computed_at, acknowledges) {
                            Ok(label) => Ok(json!({
                                "pack": pack,
                                "eligible": true,
                                "label": label,
                                "caveat": label.caveat(),
                            })),
                            Err(error) => Ok(json!({
                                "pack": pack,
                                "eligible": false,
                                "refusal": error.to_string(),
                                "fail_closed": true,
                            })),
                        }
                    }
                    other => Err(format!("unknown disclosure action {other:?}")),
                }
            })();
            match action_result {
                Ok(result) => trace.push(json!({
                    "index": index,
                    "kind": kind,
                    "ok": true,
                    "result": result,
                })),
                Err(refusal) => {
                    failures += 1;
                    trace.push(json!({
                        "index": index,
                        "kind": kind,
                        "ok": false,
                        "refusal": refusal,
                        "fail_closed": true,
                    }));
                }
            }
        }
        let entries = ledger
            .entries()
            .map(|(pack, state)| json!({ "pack": pack, "state": state }))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": failures == 0,
            "schema": "bioprism-mcp/hub-disclosure/0.1",
            "action_count": raw_actions.len(),
            "action_failures": failures,
            "trace": trace,
            "entries": entries,
            "ledger": ledger,
            "guarantees": [
                "disclosure is keyed by immutable pack digest rather than a mutable name",
                "unknown, held-out, disclosed, and contaminated remain distinct states",
                "disclosure is a ratchet and contamination cannot be walked back",
                "headline eligibility returns a caveat or a typed refusal instead of a bare score",
                "the review is in-memory and records caller-supplied findings; it does not detect leaks or publish data",
            ],
        }))
    }

    pub(super) fn hub_card_render(&self, arguments: &Value) -> Result<Value, String> {
        let raw_ledger = arguments
            .get("moderation")
            .cloned()
            .ok_or("moderation is required and must be a serialized ModerationLedger")?;
        let moderation: ModerationLedger = serde_json::from_value(raw_ledger)
            .map_err(|error| format!("invalid moderation ledger: {error}"))?;
        let submission = arguments
            .get("submission")
            .cloned()
            .ok_or("submission is required and must be a SubmissionId string")?;
        let submission: SubmissionId = serde_json::from_value(submission)
            .map_err(|error| format!("invalid submission id: {error}"))?;
        let record = moderation
            .record(&submission)
            .ok_or_else(|| format!("submission {submission} is not on the moderation ledger"))?;
        let version = arguments
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("bioatlas-card/0.1");
        let mut card = BioAtlasCard::render(record, version);
        if let Some(raw_reason) = arguments.get("not_comparable") {
            let reason: bioprism_hub::UnrankableReason = serde_json::from_value(raw_reason.clone())
                .map_err(|error| format!("invalid not-comparable reason: {error}"))?;
            card = card.as_not_comparable(&reason);
        }

        let score_result = if let Some(raw_score) = arguments.get("score") {
            let score: HubScore = serde_json::from_value(raw_score.clone())
                .map_err(|error| format!("invalid hub score: {error}"))?;
            let pack = content_hash_argument(arguments.get("pack"), "pack")?;
            let computed_at = HubEpoch(
                arguments
                    .get("computed_at")
                    .and_then(Value::as_u64)
                    .ok_or("computed_at is required when score is supplied")?,
            );
            let acknowledges = arguments
                .get("acknowledges_disclosure")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let disclosure: DisclosureLedger = arguments
                .get("disclosure")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("invalid disclosure ledger: {error}"))?
                .unwrap_or_else(DisclosureLedger::new);
            let label = match disclosure.headline_eligibility(&pack, computed_at, acknowledges) {
                Ok(label) => label,
                Err(error) => {
                    let base = serde_json::to_value(&card)
                        .map_err(|serialize| format!("cannot serialize card: {serialize}"))?;
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/hub-card/0.1",
                        "stage": "card_disclosure_gate",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "card": base,
                        "score": Value::Null,
                    }));
                }
            };
            let base_card = serde_json::to_value(&card)
                .map_err(|serialize| format!("cannot serialize card: {serialize}"))?;
            match card.with_score(score, label) {
                Ok(card_with_score) => {
                    card = card_with_score;
                    json!({ "attached": true, "pack": pack, "computed_at": computed_at })
                }
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/hub-card/0.1",
                        "stage": "card_publication_gate",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "card": base_card,
                        "score": Value::Null,
                    }));
                }
            }
        } else {
            json!({ "attached": false })
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/hub-card/0.1",
            "card": card,
            "score": score_result,
            "moderation_state": record.state,
            "verification": record.verification,
            "guarantees": [
                "the card state is derived from moderation history, verification, withdrawal, supersession, and access terms",
                "a card starts with a withheld score and never uses zero or blank as a failure state",
                "scores require disclosure eligibility and an available publication state",
                "the result is a renderer-facing object; it does not render HTML, resolve links, or publish a page",
            ],
        }))
    }

    pub(super) fn hub_leaderboard_render(&self, arguments: &Value) -> Result<Value, String> {
        let raw_entries = arguments.get("entries").and_then(Value::as_array).ok_or(
            "entries is required and must be an array of serialized leaderboard Entry values",
        )?;
        if raw_entries.len() > 2_000 {
            return Err("leaderboard entries are bounded at 2000 entries".into());
        }
        let board: HubBoard = serde_json::from_value(
            arguments
                .get("board")
                .cloned()
                .ok_or("board is required and must be a serialized Board")?,
        )
        .map_err(|error| format!("invalid leaderboard board: {error}"))?;
        let entries: Vec<HubEntry> = raw_entries
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("invalid leaderboard entry: {error}"))?;
        let moderation: ModerationLedger = serde_json::from_value(
            arguments
                .get("moderation")
                .cloned()
                .ok_or("moderation is required and must be a serialized ModerationLedger")?,
        )
        .map_err(|error| format!("invalid moderation ledger: {error}"))?;
        let disclosure: DisclosureLedger = serde_json::from_value(
            arguments
                .get("disclosure")
                .cloned()
                .ok_or("disclosure is required and must be a serialized DisclosureLedger")?,
        )
        .map_err(|error| format!("invalid disclosure ledger: {error}"))?;
        let ranked = board.rank(&entries, &moderation, &disclosure);
        let include_details = arguments
            .get("include_details")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let rendered = if include_details {
            serde_json::to_value(&ranked)
                .map_err(|error| format!("cannot serialize ranked board: {error}"))?
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/hub-leaderboard/0.1",
            "board": ranked.board,
            "ranked_count": ranked.ranked.len(),
            "unranked_count": ranked.unranked.len(),
            "leader_count": ranked.leaders().len(),
            "headline": ranked.headline(),
            "rendered": rendered,
            "guarantees": [
                "ranking is computed only within declared comparability conditions",
                "withdrawn, under-review, below-floor, contaminated, undisclosed, and incomparable entries remain visible as unranked reasons",
                "evidence scale and disclosure eligibility are checked before an entry is rankable",
                "the board headline carries its conditions, caveat, and clinical/non-universal nonclaims",
            ],
        }))
    }

    pub(super) fn hub_policy_autonomy_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a PolicyInferenceRequest3")?;
        let receipt =
            crate::research_contracts::run_hub_policy_autonomy_inference_engine_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_hub::POLICY_AUTONOMY_INFERENCE_FEATURE_ID,
            "contract_version": bioprism_hub::POLICY_AUTONOMY_INFERENCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "prospective research actions are classified by scope, policy, authority, approval, autonomy tier, evidence, replay, and locality",
                "denied, approval-required, local-only, unresolved, and negative states remain explicit",
                "the A1 engine is read-only and never executes actions or makes clinical decisions"
            ],
            "limitations": [
                "the engine classifies caller-supplied action metadata and does not perform external effects",
                "a qualified policy receipt is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }
}
