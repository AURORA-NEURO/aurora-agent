//! Root-confined project ingestion, audit, and repair-plan handlers.

use super::*;

impl Server {
    /// Shared front half of the two project tools: a root-confined scan and assembly.
    ///
    /// `root` and the optional `issues` file resolve through the same [`Server::resolve`]
    /// confinement every other path parameter uses (`fiber_compile`'s `domain` included), so a
    /// project tool can never turn the server into a scanner of arbitrary directories. The
    /// project label is the root's last path segment — a display name for scopes, not an
    /// identity claim; the world id stays content-derived.
    fn project_scan_and_assemble(
        &self,
        arguments: &Value,
    ) -> Result<(ProjectScan, ProjectWorld), String> {
        let relative = arguments
            .get("root")
            .and_then(Value::as_str)
            .ok_or("root is required (a directory path relative to the server root)")?;
        let root = self.resolve(relative)?;
        if !root.is_dir() {
            return Err(format!(
                "root must name an existing directory inside the server root: {relative:?}"
            ));
        }
        let project = relative
            .split(['/', '\\'])
            .rfind(|segment| !segment.is_empty() && *segment != ".")
            .unwrap_or("project")
            .to_string();

        let issues = match arguments.get("issues") {
            None => Vec::new(),
            Some(value) => {
                let issues_relative = value
                    .as_str()
                    .ok_or("issues must be a string path relative to the server root")?;
                let issues_path = self.resolve(issues_relative)?;
                ProjectIssue::load(&issues_path).map_err(|error| error.to_string())?
            }
        };
        let decision_time = match arguments.get("decision_time") {
            None => String::new(),
            Some(value) => {
                let text = value
                    .as_str()
                    .ok_or("decision_time must be an RFC 3339 string")?;
                // Gated here, not inside assembly. A malformed timestamp reaches the world's
                // `event.scan` and the generated queries, so left ungated it surfaces as the
                // assembled world failing the reference validator — a message that blames the
                // emitter for a value only the caller can edit. The caller's exact bytes are
                // kept: this is a gate, not a normalisation.
                bioprism_scope::Timestamp::parse(text)
                    .map_err(|error| format!("decision_time must be RFC 3339: {error}"))?;
                text.to_string()
            }
        };

        let (scan, _ingestion) = ProjectScan::scan(&root, &ProjectScanOptions::new(project))
            .map_err(|error| error.to_string())?;
        let assembled = ProjectWorld::assemble(
            &scan,
            &ProjectAssemblyOptions {
                decision_time,
                issues,
                thresholds: Default::default(),
            },
        )
        .map_err(|error| error.to_string())?;
        Ok((scan, assembled))
    }

    /// Scan a project tree into a fiber world and optionally write the emitted documents.
    ///
    /// Writing follows the server's one side-effect convention (see `world_index`): with
    /// `out_dir` but without `confirm: true` the call previews exactly which files it would
    /// create and writes nothing. Written documents are canonical bytes, so re-ingesting the
    /// same tree overwrites with identical content rather than churning diffs.
    ///
    /// `performed` therefore carries three states, not two, and the third is the reason it is not
    /// a bool: `null` when no `out_dir` was named at all, `false` when one was named and the
    /// write is waiting on `confirm`, `true` when the files are on disk. Collapsing "nobody asked
    /// for a write" into "a write was declined" would report a refusal that never happened.
    pub(super) fn project_ingest(&self, arguments: &Value) -> Result<Value, String> {
        let (scan, assembled) = self.project_scan_and_assemble(arguments)?;

        let fact_count = assembled.world["facts"].as_array().map_or(0, Vec::len);
        let factor_count = assembled.world["factors"].as_array().map_or(0, Vec::len);
        let component_count = assembled.world["facts"].as_array().map_or(0, |facts| {
            facts
                .iter()
                .filter(|fact| {
                    fact["id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("fact.component."))
                })
                .count()
        });

        let mut documents: Vec<(String, &Value)> = vec![
            ("world.json".to_string(), &assembled.world),
            ("pack.json".to_string(), &assembled.pack),
            ("dimensions.json".to_string(), &assembled.dimensions),
            ("query.release.json".to_string(), &assembled.release_query),
        ];
        for (issue_id, query) in &assembled.issue_queries {
            documents.push((format!("query.issue.{issue_id}.json"), query));
        }

        let mut written: Vec<String> = Vec::new();
        let mut performed = Value::Null;
        if let Some(out_value) = arguments.get("out_dir") {
            let out_relative = out_value
                .as_str()
                .ok_or("out_dir must be a string path relative to the server root")?;
            let out_dir = self.resolve(out_relative)?;
            let display_dir = out_relative
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_string();
            let confirmed = arguments
                .get("confirm")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !confirmed {
                return Ok(json!({
                    "ok": true,
                    "performed": false,
                    "world_id": assembled.world_id,
                    "facts": fact_count,
                    "factors": factor_count,
                    "components": component_count,
                    "losses_by_kind": scan.loss_kind_counts(),
                    "written": [],
                    "preview": {
                        "effect": "would write the assembled world, pack, dimension and query documents",
                        "writes": documents
                            .iter()
                            .map(|(name, _)| format!("{display_dir}/{name}"))
                            .collect::<Vec<_>>(),
                    },
                    "hint": "call again with confirm=true to perform this write",
                }));
            }
            std::fs::create_dir_all(&out_dir)
                .map_err(|error| format!("cannot create out_dir {}: {error}", out_dir.display()))?;
            for (name, document) in &documents {
                let bytes = bioprism_ids::to_canonical_bytes(document)
                    .map_err(|error| format!("cannot canonicalise {name}: {error}"))?;
                std::fs::write(out_dir.join(name), bytes).map_err(|error| {
                    format!("cannot write {}: {error}", out_dir.join(name).display())
                })?;
                written.push(format!("{display_dir}/{name}"));
            }
            performed = Value::Bool(true);
        }

        Ok(json!({
            "ok": true,
            "performed": performed,
            "world_id": assembled.world_id,
            "facts": fact_count,
            "factors": factor_count,
            "components": component_count,
            "losses_by_kind": scan.loss_kind_counts(),
            "written": written,
            "limitations": [
                "the scan is static: tests are counted never run, markers are substring proxies, and requirement strings are never resolved",
                "every skipped or unread byte is declared in losses_by_kind; unmeasured is reported as absent, never as zero",
            ],
        }))
    }

    /// Scan, assemble, and judge a project end to end, with no filesystem writes.
    ///
    /// The verdict ships with its witnesses verbatim — checkable objects, never scores — and
    /// with the scan's loss summary, because a release verdict computed without seeing what the
    /// scan skipped would be a verdict about a tree nobody scanned. Each declared issue's query
    /// is compiled too, so the response names the exact fact region each issue would receive.
    pub(super) fn project_audit(&self, arguments: &Value) -> Result<Value, String> {
        let (scan, assembled) = self.project_scan_and_assemble(arguments)?;

        let world = World::from_json(assembled.world.clone()).map_err(|e| e.to_string())?;
        let pack = DomainPack::from_json(&assembled.pack).map_err(|e| e.to_string())?;
        let release_query =
            Query::from_json(assembled.release_query.clone()).map_err(|e| e.to_string())?;
        let out = compile_with_oracle(&world, &release_query, pack.oracle())
            .map_err(|e| e.to_string())?;

        // An issue's region is defined by the components it declares and nothing else, so the
        // declarations that produced it travel with it. Without `unresolved_components` on the
        // wire a region built from a component name that resolved to nothing is indistinguishable
        // from the region of an issue that declared nothing at all, and a reader will take the
        // second reading — which is the one that looks deliberate.
        let issue_declarations = |issue_id: &str, field: &str| -> Value {
            world
                .facts
                .iter()
                .find(|fact| fact.id.as_str() == format!("fact.issue.{issue_id}"))
                .and_then(|fact| fact.value.get(field))
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new()))
        };

        let mut issues = Map::new();
        for (issue_id, document) in &assembled.issue_queries {
            let query = Query::from_json(document.clone())
                .map_err(|error| format!("issue {issue_id:?} query: {error}"))?;
            let compiled = compile_with_oracle(&world, &query, pack.oracle())
                .map_err(|error| format!("issue {issue_id:?} compile: {error}"))?;
            issues.insert(
                issue_id.clone(),
                json!({
                    "query_id": document["query_id"],
                    "resolved_components": issue_declarations(issue_id, "components"),
                    "unresolved_components": issue_declarations(issue_id, "unresolved_components"),
                    "selected_facts": compiled.certificate.selected_facts,
                }),
            );
        }

        let loss_counts = scan.loss_kind_counts();
        Ok(json!({
            "ok": true,
            "world_id": assembled.world_id,
            "verdict": {
                "status": out.certificate.oracle.status.as_str(),
                "oracle_kind": out.certificate.oracle.oracle_kind,
                "witnesses": out.certificate.oracle.witnesses,
            },
            "facts": world.facts.len(),
            "selected_facts": out.certificate.selected_facts,
            "loss": {
                "total": loss_counts.values().sum::<u64>(),
                "by_kind": loss_counts,
            },
            "issues": issues,
            "limitations": [
                "every check is a static-scan proxy and says so in its witness detail; nothing is executed or resolved",
                "issue regions come from declared components only, resolved syntactically; there is no semantic relevance search, and a declaration that resolved to nothing is reported in unresolved_components rather than guessed at",
            ],
        }))
    }

    /// Reads the optional `criteria` document a caller declares its own plan items in.
    ///
    /// The same `bioprism-repair-declarations/0.1` document `bioprism project plan --criteria`
    /// reads, resolved through [`Server::resolve`] like every other path parameter, so declaring
    /// criteria cannot become a way to read a file outside the server root.
    ///
    /// Strict in the same way the plan reader is strict: an undeclared key is refused rather than
    /// ignored, because a misspelled `falsifier` would otherwise become a plan with no falsifier
    /// whose refusal then blames the author for something the reader dropped. An *absent* list is
    /// accepted and means the author declared none of that kind, which is a claim
    /// `Admissibility::Undeclared` already makes for obligations.
    ///
    /// A declared criterion must carry a `rationale`; obligations and falsifiers need none. That
    /// asymmetry is the plan document's own — `AcceptanceCriterion` is the one item type with the
    /// field — and it is enforced because a criterion is what a plan marks `declared` in order to
    /// say a person is accountable for it. An accountable claim with no stated reason is the shape
    /// of a criterion added to make a verification pass.
    fn repair_declarations(&self, arguments: &Value) -> Result<RepairPlanOptions, String> {
        let Some(value) = arguments.get("criteria") else {
            return Ok(RepairPlanOptions::default());
        };
        let relative = value
            .as_str()
            .ok_or("criteria must be a string path relative to the server root")?;
        let path = self.resolve(relative)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read criteria {relative:?}: {error}"))?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| format!("criteria {relative:?} is not valid JSON: {error}"))?;

        let map = document
            .as_object()
            .ok_or_else(|| format!("criteria {relative:?} must be an object"))?;
        let declared_fields = [
            "schema_version",
            "criteria",
            "obligations",
            "falsifiers",
            "limitations",
        ];
        if let Some(unknown) = map
            .keys()
            .find(|key| !declared_fields.contains(&key.as_str()))
        {
            return Err(format!(
                "undeclared field {unknown:?} on criteria {relative:?}; the declared fields are \
                 {declared_fields:?}"
            ));
        }
        match map.get("schema_version").and_then(Value::as_str) {
            Some(version) if version == REPAIR_DECLARATIONS_SCHEMA_VERSION => {}
            Some(other) => {
                return Err(format!(
                    "criteria {relative:?} declares schema_version {other:?}, expected \
                     {REPAIR_DECLARATIONS_SCHEMA_VERSION:?}"
                ));
            }
            None => {
                return Err(format!(
                    "criteria {relative:?} needs a string \"schema_version\" of \
                     {REPAIR_DECLARATIONS_SCHEMA_VERSION:?}"
                ));
            }
        }

        let items =
            |field: &str, with_rationale: bool| -> Result<Vec<RepairDeclaredItem>, String> {
                let Some(value) = map.get(field) else {
                    return Ok(Vec::new());
                };
                let entries = value.as_array().ok_or_else(|| {
                    format!("{field:?} on criteria {relative:?} must be an array")
                })?;
                entries
                .iter()
                .map(|entry| {
                    let fields: &[&str] = if with_rationale {
                        &["name", "statement", "predicate", "rationale"]
                    } else {
                        &["name", "statement", "predicate"]
                    };
                    let entry = entry
                        .as_object()
                        .ok_or_else(|| format!("every entry in {field:?} is an object"))?;
                    if let Some(unknown) = entry.keys().find(|key| !fields.contains(&key.as_str()))
                    {
                        return Err(format!(
                            "undeclared field {unknown:?} on an entry in {field:?}; the declared \
                             fields are {fields:?}"
                        ));
                    }
                    let text = |key: &str| -> Result<String, String> {
                        entry
                            .get(key)
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .ok_or_else(|| format!("an entry in {field:?} needs a string {key:?}"))
                    };
                    let predicate =
                        predicate_from_json(entry.get("predicate").ok_or_else(|| {
                            format!("an entry in {field:?} declares no \"predicate\"")
                        })?)
                        .map_err(|error| {
                            format!("an entry in {field:?} carries no predicate: {error}")
                        })?;
                    let item =
                        RepairDeclaredItem::new(text("name")?, text("statement")?, predicate);
                    Ok(if with_rationale {
                        item.with_rationale(text("rationale")?)
                    } else {
                        item
                    })
                })
                .collect()
            };

        let limitations = match map.get("limitations") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or("\"limitations\" must be an array of strings")?
                .iter()
                .map(|entry| {
                    entry
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| "\"limitations\" carries a non-string entry".to_string())
                })
                .collect::<Result<Vec<String>, String>>()?,
        };

        Ok(RepairPlanOptions {
            declared_criteria: items("criteria", true)?,
            declared_obligations: items("obligations", false)?,
            declared_falsifiers: items("falsifiers", false)?,
            limitations,
        })
    }

    /// Derive a repair plan for one declared issue and optionally write it.
    ///
    /// Writing follows the server's one side-effect convention (see `project_ingest`): with `out`
    /// but without `confirm: true` the call previews the exact path it would create and writes
    /// nothing. `preview.writes` and `written` are built from the same expression, so a caller
    /// cannot approve one effect and receive another.
    ///
    /// `performed` carries three states for the reason `project_ingest` gives: `null` when no
    /// `out` was named at all, `false` when one was named and the write is waiting on `confirm`,
    /// `true` when the file is on disk. Collapsing "nobody asked for a write" into "a write was
    /// declined" would report a refusal that never happened.
    ///
    /// Nothing here edits the scanned tree, produces a patch, builds, or runs a test. The one
    /// write is the plan document.
    pub(super) fn repair_plan(&self, arguments: &Value) -> Result<Value, String> {
        let issue_id = arguments
            .get("issue")
            .and_then(Value::as_str)
            .ok_or("issue is required (the id of an issue the issues file declares)")?
            .to_string();
        if arguments.get("issues").is_none() {
            return Err(
                "issues is required: a plan is for one declared issue, and a tree assembled \
                 without its declarations carries no issue to plan for"
                    .into(),
            );
        }
        let declared = self.repair_declarations(arguments)?;
        let (_scan, assembled) = self.project_scan_and_assemble(arguments)?;

        let query_document = assembled.issue_queries.get(&issue_id).ok_or_else(|| {
            let known: Vec<&str> = assembled.issue_queries.keys().map(String::as_str).collect();
            format!(
                "no issue {issue_id:?} is declared in the issues file; it declares {}",
                if known.is_empty() {
                    "no issues at all".to_string()
                } else {
                    known.join(", ")
                }
            )
        })?;

        let world = World::from_json(assembled.world.clone()).map_err(|e| e.to_string())?;
        let pack = DomainPack::from_json(&assembled.pack).map_err(|e| e.to_string())?;
        let query =
            Query::from_json(query_document.clone()).map_err(|e| format!("issue query: {e}"))?;
        let compiled = compile_with_oracle(&world, &query, pack.oracle())
            .map_err(|e| format!("issue {issue_id:?} compile: {e}"))?;
        let plan = plan_for_issue(&world, &pack, &issue_id, &compiled.certificate, &declared)
            .map_err(|e| e.to_string())?;
        let plan_document = plan.to_json().map_err(|e| e.to_string())?;

        let summary = json!({
            "ok": true,
            "plan_id": plan.plan_id(),
            "issue_id": plan.issue_id(),
            "goal": plan.goal(),
            "world_id": plan.evidence_binding().world_id,
            "region_fact_ids": plan.evidence_binding().region_fact_ids,
            "items": plan
                .criteria()
                .iter()
                .map(|item| json!({ "kind": "criterion", "name": item.name, "origin": item.origin.as_str() }))
                .chain(plan.obligations().iter().map(|item| {
                    json!({ "kind": "obligation", "name": item.name, "origin": item.origin.as_str() })
                }))
                .chain(plan.falsifiers().iter().map(|item| {
                    json!({ "kind": "falsifier", "name": item.name, "origin": item.origin.as_str() })
                }))
                .collect::<Vec<_>>(),
            "plan": plan_document,
            "limitations": [
                "this tool plans and never repairs: no file in the scanned tree is edited, no patch is produced, nothing is built and no test is run",
                "a derived criterion is a proxy for something the release-readiness pack could see, never for what the issue means; the plan's own limitations enumerate the rest",
                "meeting every criterion in the plan is not proof the issue is resolved, and repair_verify never claims it is",
            ],
        });

        let mut response = summary
            .as_object()
            .expect("the summary is an object")
            .clone();
        let Some(out_value) = arguments.get("out") else {
            response.insert("performed".into(), Value::Null);
            response.insert("written".into(), json!([]));
            return Ok(Value::Object(response));
        };

        let out_relative = out_value
            .as_str()
            .ok_or("out must be a string path relative to the server root")?;
        let out_path = self.resolve(out_relative)?;
        let display_path = out_relative.replace('\\', "/");
        let confirmed = arguments
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !confirmed {
            response.insert("performed".into(), Value::Bool(false));
            response.insert("written".into(), json!([]));
            response.insert(
                "preview".into(),
                json!({
                    "effect": "would write the repair plan document",
                    "writes": [display_path],
                }),
            );
            response.insert(
                "hint".into(),
                json!("call again with confirm=true to perform this write"),
            );
            return Ok(Value::Object(response));
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let bytes = bioprism_ids::to_canonical_bytes(&plan_document)
            .map_err(|error| format!("cannot canonicalise the plan: {error}"))?;
        std::fs::write(&out_path, bytes)
            .map_err(|error| format!("cannot write {}: {error}", out_path.display()))?;
        response.insert("performed".into(), Value::Bool(true));
        response.insert("written".into(), json!([display_path]));
        Ok(Value::Object(response))
    }

    /// Report which of a repair plan's declared criteria held in a freshly scanned tree.
    ///
    /// Three-valued and staleness-first, exactly as `bioprism_repair::verify` is. If the world
    /// scanned here is not the world the plan was bound to, the response is the crate's `stale`
    /// report — no outcome, no item statuses, nothing evaluated — because a verdict computed
    /// against a different world is not a verdict about this plan. `stale` is a finding and not an
    /// error: it arrives as a successful call carrying a report that says so, so a caller cannot
    /// discard it the way it would discard a transport failure.
    ///
    /// Nothing is written, built, or run, and the response never states that the issue is fixed.
    /// When the world changed, a caller may supply `succession: { declared_by, statement }` to
    /// assert that this is the plan's repaired successor. The named assertion is recorded
    /// verbatim and never independently verified; without it, a different world remains stale.
    pub(super) fn repair_verify(&self, arguments: &Value) -> Result<Value, String> {
        let plan_relative = arguments.get("plan").and_then(Value::as_str).ok_or(
            "plan is required (a path to a repair plan document, relative to the server root)",
        )?;
        let plan_path = self.resolve(plan_relative)?;
        let text = std::fs::read_to_string(&plan_path)
            .map_err(|error| format!("cannot read plan {plan_relative:?}: {error}"))?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| format!("plan {plan_relative:?} is not valid JSON: {error}"))?;
        let plan = RepairPlan::from_json(&document)
            .map_err(|error| format!("plan {plan_relative:?}: {error}"))?;

        let succession = match arguments.get("succession") {
            None => None,
            Some(value) => {
                let object = value
                    .as_object()
                    .ok_or("succession must be an object with declared_by and statement")?;
                let fields = ["declared_by", "statement"];
                if let Some(unknown) = object.keys().find(|key| !fields.contains(&key.as_str())) {
                    return Err(format!(
                        "undeclared field {unknown:?} on succession; the declared fields are {fields:?}"
                    ));
                }
                let declared_by = object
                    .get("declared_by")
                    .and_then(Value::as_str)
                    .ok_or("succession needs a string declared_by")?;
                let statement = object
                    .get("statement")
                    .and_then(Value::as_str)
                    .ok_or("succession needs a string statement")?;
                Some(
                    Succession::declare(declared_by, statement)
                        .map_err(|error| format!("invalid succession: {error}"))?,
                )
            }
        };

        let (_scan, assembled) = self.project_scan_and_assemble(arguments)?;
        let world = World::from_json(assembled.world.clone()).map_err(|e| e.to_string())?;
        let report = match &succession {
            Some(succession) => verify_repair_successor(&plan, &world, succession),
            None => verify_repair(&plan, &world),
        };

        Ok(json!({
            "ok": true,
            "plan_id": report.plan_id(),
            "issue_id": report.issue_id(),
            "stale": matches!(report, AcceptanceReport::Stale { .. }),
            "outcome": report.outcome().map(|outcome| outcome.as_str()),
            "admissibility": report.admissibility().map(|value| value.as_str()),
            "report": report.to_json(),
            "limitations": [
                "this report states which of the plan's declared criteria held in the tree scanned here; it does not state that the issue is resolved",
                "without a succession declaration, a stale report evaluated nothing at all: outcome and admissibility are null, and the item list is absent rather than empty",
                "a supplied succession is the caller's assertion; it is recorded verbatim and never independently verified",
                "criteria are evaluated against every fact in the assembled world rather than a recompiled evidence region, so no budget is applied; nothing is executed",
            ],
        }))
    }
}
