//! Deterministic cache invalidation and storage lifecycle simulations.

use super::*;

impl Server {
    pub(super) fn cache_invalidation_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cache simulation arguments are not serialisable: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("cache simulation input exceeds the 20 MB safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let schema_object = arguments
            .get("schema")
            .and_then(Value::as_object)
            .ok_or("schema is required and must declare name, components, and reuse")?;
        let schema_name = schema_object
            .get("name")
            .and_then(Value::as_str)
            .ok_or("schema.name is required")?;
        let components = schema_object
            .get("components")
            .and_then(Value::as_array)
            .ok_or("schema.components is required and must be an array")?;
        if components.is_empty() || components.len() > 128 {
            return Err("schema.components must contain between 1 and 128 names".into());
        }
        let component_names = components
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("schema.components[{index}] must be a string"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reuse = match schema_object
            .get("reuse")
            .and_then(Value::as_str)
            .unwrap_or("same_build_only")
        {
            "same_build_only" | "SameBuildOnly" | "same-build-only" => ReuseRule::SameBuildOnly,
            "across_builds" | "AcrossBuilds" | "across-builds" => ReuseRule::AcrossBuilds,
            other => {
                return Err(format!(
                    "unknown schema.reuse {other:?}; choose same_build_only or across_builds"
                ));
            }
        };
        let schema = KeySchema::declare(schema_name, component_names, reuse)
            .map_err(|error| format!("invalid cache key schema: {error}"))?;

        let parse_dependencies = |raw: Option<&Value>| -> Result<DependencyDeclaration, String> {
            let Some(raw) = raw else {
                return Ok(DependencyDeclaration::Undeclared);
            };
            if let Some(kind) = raw.as_str() {
                return match kind {
                    "undeclared" | "Undeclared" => Ok(DependencyDeclaration::Undeclared),
                    other => Err(format!(
                        "dependency declaration {other:?} is not valid; use undeclared or an object with kind declared"
                    )),
                };
            }
            let object = raw
                .as_object()
                .ok_or("dependencies must be undeclared or {kind: declared, resources: [...]}")?;
            let kind = object
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("declared");
            if !matches!(kind, "declared" | "Declared") {
                return Err(format!("unknown dependency declaration kind {kind:?}"));
            }
            let resources = object
                .get("resources")
                .and_then(Value::as_array)
                .ok_or("declared dependencies require a resources array")?;
            let parsed = resources
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    ResourceId::parse(
                        value
                            .as_str()
                            .ok_or_else(|| {
                                format!("dependencies.resources[{index}] must be a string")
                            })?
                            .to_string(),
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DependencyDeclaration::on(parsed))
        };
        let parse_key = |raw_components: &Value| -> Result<ComputationKey, String> {
            let object = raw_components
                .as_object()
                .ok_or_else(|| "components must be an object of string values".to_string())?;
            let values = object
                .iter()
                .map(|(name, value)| {
                    value
                        .as_str()
                        .map(|value| (name.clone(), value.to_string()))
                        .ok_or_else(|| format!("component {name:?} must be a string"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            ComputationKey::build(&schema, values).map_err(|error| error.to_string())
        };

        let raw_entries = arguments
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_entries.len() > 1_000 {
            return Err("entries must contain at most 1000 cache entries".into());
        }
        let mut cache = Cache::new(schema.clone());
        let mut entry_rows = Vec::with_capacity(raw_entries.len());
        let mut inserted_digests = BTreeSet::new();
        for (entry_index, raw_entry) in raw_entries.iter().enumerate() {
            let Some(object) = raw_entry.as_object() else {
                entry_rows.push(json!({
                    "index": entry_index,
                    "ok": false,
                    "refusal": "each entry must be an object",
                    "fail_closed": true,
                }));
                continue;
            };
            let key = object
                .get("components")
                .ok_or_else(|| format!("entry {entry_index} requires components"))?;
            let key = match parse_key(key) {
                Ok(key) => key,
                Err(error) => {
                    entry_rows.push(json!({
                        "index": entry_index,
                        "ok": false,
                        "refusal": error,
                        "fail_closed": true,
                    }));
                    continue;
                }
            };
            let digest = key.digest();
            let produced_by = CodeIdentity::parse(
                object
                    .get("produced_by")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("entry {entry_index} requires produced_by"))?
                    .to_string(),
            )
            .map_err(|error| format!("entry {entry_index} has invalid produced_by: {error}"))?;
            let written_at = object
                .get("written_at")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("entry {entry_index} requires integer written_at"))?;
            let dependencies = match parse_dependencies(object.get("dependencies")) {
                Ok(value) => value,
                Err(error) => {
                    entry_rows.push(json!({
                        "index": entry_index,
                        "digest": digest,
                        "ok": false,
                        "refusal": error,
                        "fail_closed": true,
                    }));
                    continue;
                }
            };
            match cache.insert(
                key,
                object.get("value").cloned().unwrap_or(Value::Null),
                produced_by,
                Epoch::new(written_at),
                dependencies.clone(),
            ) {
                Ok(inserted) => {
                    inserted_digests.insert(inserted.clone());
                    entry_rows.push(json!({
                        "index": entry_index,
                        "ok": true,
                        "digest": inserted,
                        "dependencies": dependencies,
                    }));
                }
                Err(error) => entry_rows.push(json!({
                    "index": entry_index,
                    "digest": digest,
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                })),
            }
        }

        let raw_graph = arguments
            .get("graph")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut graph = DependencyGraph::new();
        let declared = raw_graph
            .get("declared")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if declared.len() > 2_000 {
            return Err("graph.declared must contain at most 2000 declarations".into());
        }
        for (index, raw_declaration) in declared.iter().enumerate() {
            let object = raw_declaration
                .as_object()
                .ok_or_else(|| format!("graph.declared[{index}] must be an object"))?;
            let resource = ResourceId::parse(
                object
                    .get("resource")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("graph.declared[{index}] requires resource"))?
                    .to_string(),
            )
            .map_err(|error| format!("invalid graph.declared[{index}] resource: {error}"))?;
            let dependencies = object
                .get("depends_on")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("graph.declared[{index}] requires depends_on"))?
                .iter()
                .enumerate()
                .map(|(dependency_index, value)| {
                    ResourceId::parse(
                        value
                            .as_str()
                            .ok_or_else(|| {
                                format!(
                                    "graph.declared[{index}].depends_on[{dependency_index}] must be a string"
                                )
                            })?
                            .to_string(),
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            graph
                .declare(resource, dependencies)
                .map_err(|error| format!("graph declaration {index} refused: {error}"))?;
        }
        let opaque = raw_graph
            .get("opaque")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if opaque.len() > 2_000 {
            return Err("graph.opaque must contain at most 2000 resources".into());
        }
        for (index, raw_resource) in opaque.iter().enumerate() {
            let resource = ResourceId::parse(
                raw_resource
                    .as_str()
                    .ok_or_else(|| format!("graph.opaque[{index}] must be a string"))?
                    .to_string(),
            )
            .map_err(|error| format!("invalid graph.opaque[{index}]: {error}"))?;
            graph
                .declare_opaque(resource)
                .map_err(|error| format!("graph opaque declaration {index} refused: {error}"))?;
        }

        let changed = arguments
            .get("changed")
            .map(|raw| {
                ResourceId::parse(
                    raw.as_str()
                        .ok_or("changed must be a non-empty resource string")?
                        .to_string(),
                )
                .map_err(|error| error.to_string())
            })
            .transpose()?;
        let plan = changed.as_ref().map(|changed| {
            InvalidationPlan::compute(&graph, changed.clone(), cache.declarations())
        });
        let raw_lookups = arguments
            .get("lookups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_lookups.len() > 1_000 {
            return Err("lookups must contain at most 1000 requests".into());
        }
        let run_lookups = |cache: &mut Cache| {
            raw_lookups
                .iter()
                .enumerate()
                .map(|(index, raw_lookup)| {
                    let Some(object) = raw_lookup.as_object() else {
                        return json!({
                            "index": index,
                            "ok": false,
                            "refusal": "each lookup must be an object",
                            "fail_closed": true,
                        });
                    };
                    let key = object
                        .get("components")
                        .ok_or_else(|| "lookup requires components".to_string())
                        .and_then(parse_key);
                    let requested_by = object
                        .get("requested_by")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "lookup requires requested_by".to_string())
                        .and_then(|value| {
                            CodeIdentity::parse(value.to_string())
                                .map_err(|error| error.to_string())
                        });
                    match (key, requested_by) {
                        (Ok(key), Ok(requested_by)) => match cache.lookup(&key, &requested_by) {
                            Ok(bioprism_infra::Lookup::Hit(hit)) => json!({
                                "index": index,
                                "ok": true,
                                "hit": true,
                                "value": hit.value,
                                "proof": hit.proof,
                            }),
                            Ok(bioprism_infra::Lookup::Miss(reason)) => json!({
                                "index": index,
                                "ok": true,
                                "hit": false,
                                "miss_reason": reason,
                            }),
                            Err(error) => json!({
                                "index": index,
                                "ok": false,
                                "refusal": error.to_string(),
                                "fail_closed": true,
                            }),
                        },
                        (Err(error), _) | (_, Err(error)) => json!({
                            "index": index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        }),
                    }
                })
                .collect::<Vec<_>>()
        };
        let apply = arguments
            .get("apply")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let pre_lookup = if apply {
            Some(run_lookups(&mut cache))
        } else {
            None
        };
        let apply_report = if apply {
            let plan = plan
                .as_ref()
                .ok_or("apply requires changed so an invalidation plan can be computed")?;
            Some(
                cache
                    .apply(
                        plan,
                        Epoch::new(
                            arguments
                                .get("apply_at")
                                .and_then(Value::as_u64)
                                .ok_or("apply requires integer apply_at")?,
                        ),
                    )
                    .map_err(|error| format!("cache invalidation apply refused: {error}"))?,
            )
        } else {
            None
        };
        let raw_reprove = arguments
            .get("reprove")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_reprove.len() > 1_000 {
            return Err("reprove must contain at most 1000 entries".into());
        }
        let mut reprove_rows = Vec::new();
        for (index, raw_reproof) in raw_reprove.iter().enumerate() {
            let object = raw_reproof
                .as_object()
                .ok_or_else(|| format!("reprove[{index}] must be an object"))?;
            let digest = object
                .get("digest")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("reprove[{index}] requires digest"))?;
            let by = CodeIdentity::parse(
                object
                    .get("by")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("reprove[{index}] requires by"))?
                    .to_string(),
            )
            .map_err(|error| format!("reprove[{index}] has invalid by: {error}"))?;
            if cache.reprove(digest, &by).is_some() {
                reprove_rows.push(json!({
                    "index": index,
                    "ok": true,
                    "digest": digest,
                    "reproved_by": by,
                }));
            } else {
                reprove_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "digest": digest,
                    "refusal": "unknown cache digest",
                    "fail_closed": true,
                }));
            }
        }
        let post_lookup = if raw_lookups.is_empty() {
            Vec::new()
        } else {
            run_lookups(&mut cache)
        };
        let snapshots = inserted_digests
            .iter()
            .filter_map(|digest| {
                cache.get(digest).map(|entry| {
                    json!({
                        "digest": digest,
                        "key": entry.key,
                        "value": entry.value,
                        "produced_by": entry.produced_by,
                        "written_at": entry.written_at,
                        "dependencies": entry.dependencies,
                        "status": entry.status,
                    })
                })
            })
            .collect::<Vec<_>>();
        let miss_rows = cache
            .misses_by_reason()
            .iter()
            .map(|(reason, count)| json!({ "reason": reason, "count": count }))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/cache-invalidation/0.1",
            "max_items": max_items,
            "key_schema": schema,
            "entries": {
                "accepted": entry_rows.iter().filter(|row| row["ok"] == json!(true)).count(),
                "submitted": entry_rows.len(),
                "rows": entry_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_rows": entry_rows.len().saturating_sub(max_items),
            },
            "graph": {
                "known_resources": graph.known_resources(),
                "known_resource_count": graph.known_resources().len(),
                "opaque_resources": graph.opaque_resources(),
                "cycle": graph.find_cycle(),
                "cycle_is_a_scheduler_defect_not_an_invalidation_hang": graph.find_cycle().is_some(),
            },
            "invalidation": {
                "changed": changed,
                "plan": plan,
                "apply_requested": apply,
                "apply_report": apply_report,
            },
            "lookups": {
                "pre_apply": pre_lookup.map(|rows| json!(rows)).unwrap_or(Value::Null),
                "post_apply": post_lookup.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_post_apply": post_lookup.len().saturating_sub(max_items),
            },
            "reprove": reprove_rows,
            "cache": {
                "entry_count": cache.len(),
                "unproven": cache.unproven(),
                "hits": cache.hits(),
                "misses_by_reason": miss_rows,
                "hit_rate": cache.hit_rate(),
                "entries": snapshots.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_entries": snapshots.len().saturating_sub(max_items),
            },
            "guarantees": [
                "cache keys are rebuilt from every declared component and never from a bare digest",
                "cross-build reuse, key collisions, unproven entries, and cold misses remain distinct outcomes",
                "partial invalidation marks unknown entries unproven rather than serving them optimistically",
                "apply is explicit and requires a caller-supplied logical epoch; no wall clock is read",
                "re-proving names the digest and build that re-established currentness",
            ],
            "limitations": [
                "the cache and dependency graph are in-memory projections; no durable index, tenant isolation, eviction worker, or external invalidation feed is created",
                "a caller-supplied dependency declaration is not independently discovered or scientifically validated",
                "values are returned as supplied and are not treated as canonical truth outside the cache proof",
            ],
        }))
    }

    pub(super) fn storage_lifecycle_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("storage simulation arguments are not serialisable: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err("storage simulation input exceeds the 20 MB safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let now = Epoch::new(
            arguments
                .get("now")
                .and_then(Value::as_u64)
                .ok_or("now is required and must be an integer epoch")?,
        );

        let parse_tier = |raw: &Value| -> Result<Tier, String> {
            match raw
                .as_str()
                .ok_or("tier must be a string")?
                .to_ascii_lowercase()
                .as_str()
            {
                "hot" => Ok(Tier::Hot),
                "warm" => Ok(Tier::Warm),
                "cold" => Ok(Tier::Cold),
                other => Err(format!("unknown tier {other:?}; choose hot, warm, or cold")),
            }
        };
        let parse_class = |raw: &Value| -> Result<StorageClass, String> {
            match raw
                .as_str()
                .ok_or("storage class must be a string")?
                .to_ascii_lowercase()
                .replace('-', "_")
                .as_str()
            {
                "objects" => Ok(StorageClass::Objects),
                "events" => Ok(StorageClass::Events),
                "indexes" | "index" => Ok(StorageClass::Indexes),
                "results" | "result" => Ok(StorageClass::Results),
                "cache" => Ok(StorageClass::Cache),
                other => Err(format!(
                    "unknown storage class {other:?}; choose objects, events, indexes, results, or cache"
                )),
            }
        };
        let parse_purpose = |raw: &Value| -> Result<Purpose, String> {
            match raw
                .as_str()
                .ok_or("purpose must be a string")?
                .to_ascii_lowercase()
                .replace('_', "-")
                .as_str()
            {
                "ingest" => Ok(Purpose::Ingest),
                "evidence-finalization" | "evidence" => Ok(Purpose::EvidenceFinalization),
                "cleanup" => Ok(Purpose::Cleanup),
                other => Err(format!(
                    "unknown purpose {other:?}; choose ingest, evidence_finalization, or cleanup"
                )),
            }
        };
        let parse_charge =
            |raw: &Value, label: &str| -> Result<(StorageClass, Purpose, u64), String> {
                let object = raw
                    .as_object()
                    .ok_or_else(|| format!("{label} must be an object"))?;
                let class = parse_class(
                    object
                        .get("class")
                        .ok_or_else(|| format!("{label}.class is required"))?,
                )?;
                let purpose = parse_purpose(
                    object
                        .get("purpose")
                        .ok_or_else(|| format!("{label}.purpose is required"))?,
                )?;
                let bytes = object
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("{label}.bytes must be an integer"))?;
                Ok((class, purpose, bytes))
            };

        let policy_object = arguments
            .get("tiering_policy")
            .and_then(Value::as_object)
            .ok_or("tiering_policy is required")?;
        let policy = TieringPolicy::new(
            policy_object
                .get("demote_to_warm_after")
                .and_then(Value::as_u64)
                .ok_or("tiering_policy.demote_to_warm_after is required")?,
            policy_object
                .get("demote_to_cold_after")
                .and_then(Value::as_u64)
                .ok_or("tiering_policy.demote_to_cold_after is required")?,
            policy_object
                .get("promote_after_accesses")
                .and_then(Value::as_u64)
                .ok_or("tiering_policy.promote_after_accesses is required")?,
            policy_object
                .get("promote_within")
                .and_then(Value::as_u64)
                .ok_or("tiering_policy.promote_within is required")?,
        )
        .map_err(|error| format!("invalid tiering policy: {error}"))?;

        let raw_records = arguments
            .get("records")
            .and_then(Value::as_array)
            .ok_or("records is required and must be an array")?;
        if raw_records.len() > 1_000 {
            return Err("records must contain at most 1000 objects".into());
        }
        let mut records = Vec::with_capacity(raw_records.len());
        let mut record_rows = Vec::with_capacity(raw_records.len());
        for (index, raw_record) in raw_records.iter().enumerate() {
            let parsed = (|| -> Result<AccessRecord, String> {
                let object = raw_record.as_object().ok_or("record must be an object")?;
                let name = object
                    .get("object")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or("record.object must be a non-empty string")?;
                let mut record = AccessRecord::new(
                    name,
                    parse_tier(object.get("tier").ok_or("record.tier is required")?)?,
                    Epoch::new(
                        object
                            .get("last_access")
                            .and_then(Value::as_u64)
                            .ok_or("record.last_access must be an integer")?,
                    ),
                )
                .with_recent_accesses(
                    object
                        .get("recent_accesses")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                )
                .with_bytes(object.get("bytes").and_then(Value::as_u64).unwrap_or(0));
                if object
                    .get("pinned")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    record = record.pinned();
                }
                Ok(record)
            })();
            match parsed {
                Ok(record) => {
                    record_rows
                        .push(json!({ "index": index, "ok": true, "object": record.object }));
                    records.push(record);
                }
                Err(error) => record_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })),
            }
        }
        let plan = policy
            .plan(&records, now)
            .map_err(|error| format!("tiering plan refused: {error}"))?;
        let bytes_by_target = [Tier::Hot, Tier::Warm, Tier::Cold]
            .into_iter()
            .filter_map(|tier| {
                plan.bytes_by_target(&records)
                    .get(&tier)
                    .copied()
                    .filter(|bytes| *bytes > 0)
                    .map(|bytes| json!({ "tier": tier, "name": tier.name(), "bytes": bytes }))
            })
            .collect::<Vec<_>>();
        let apply_tiering = arguments
            .get("apply_tiering")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let tier_apply_report = if apply_tiering {
            let (applied, absent) = plan.apply_to(&mut records);
            Some(json!({ "applied": applied, "absent": absent }))
        } else {
            None
        };

        let quota_object = arguments
            .get("quota")
            .and_then(Value::as_object)
            .ok_or("quota is required and must contain limit and reserve")?;
        let mut quota = StorageQuota::new(
            quota_object
                .get("limit")
                .and_then(Value::as_u64)
                .ok_or("quota.limit must be an integer")?,
            quota_object
                .get("reserve")
                .and_then(Value::as_u64)
                .ok_or("quota.reserve must be an integer")?,
        )
        .map_err(|error| format!("invalid storage quota: {error}"))?;
        let raw_charges = arguments
            .get("charges")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_charges.len() > 1_000 {
            return Err("charges must contain at most 1000 actions".into());
        }
        let mut charge_rows = Vec::with_capacity(raw_charges.len());
        for (index, raw_charge) in raw_charges.iter().enumerate() {
            let action = parse_charge(raw_charge, &format!("charges[{index}]")).and_then(
                |(class, purpose, bytes)| {
                    quota
                        .charge(class, purpose, bytes)
                        .map(|remaining| (class, purpose, bytes, remaining))
                        .map_err(|error| error.to_string())
                },
            );
            match action {
                Ok((class, purpose, bytes, remaining)) => charge_rows.push(json!({
                    "index": index,
                    "ok": true,
                    "class": class,
                    "class_name": class.name(),
                    "purpose": purpose,
                    "purpose_name": purpose.name(),
                    "bytes": bytes,
                    "remaining_for_purpose": remaining,
                })),
                Err(error) => charge_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })),
            }
        }
        let raw_releases = arguments
            .get("releases")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_releases.len() > 1_000 {
            return Err("releases must contain at most 1000 actions".into());
        }
        let mut release_rows = Vec::with_capacity(raw_releases.len());
        for (index, raw_release) in raw_releases.iter().enumerate() {
            let action = (|| -> Result<(StorageClass, u64, u64), String> {
                let object = raw_release.as_object().ok_or("release must be an object")?;
                let class = parse_class(object.get("class").ok_or("release.class is required")?)?;
                let bytes = object
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .ok_or("release.bytes must be an integer")?;
                let remaining = quota
                    .release(class, bytes)
                    .map_err(|error| error.to_string())?;
                Ok((class, bytes, remaining))
            })();
            match action {
                Ok((class, bytes, remaining)) => release_rows.push(json!({
                    "index": index,
                    "ok": true,
                    "class": class,
                    "class_name": class.name(),
                    "bytes": bytes,
                    "remaining": remaining,
                })),
                Err(error) => release_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })),
            }
        }

        let raw_delegations = arguments
            .get("delegations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_delegations.len() > 100 {
            return Err("delegations must contain at most 100 children".into());
        }
        let mut delegated: Vec<Option<StorageQuota>> = Vec::new();
        let mut delegation_rows = Vec::with_capacity(raw_delegations.len());
        for (index, raw_delegation) in raw_delegations.iter().enumerate() {
            let action = (|| -> Result<(u64, Vec<Value>, StorageQuota), String> {
                let object = raw_delegation
                    .as_object()
                    .ok_or("delegation must be an object")?;
                let bytes = object
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .ok_or("delegation.bytes must be an integer")?;
                let mut child = quota.delegate(bytes).map_err(|error| error.to_string())?;
                let raw_child_charges = object
                    .get("charges")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if raw_child_charges.len() > 1_000 {
                    return Err("delegation.charges must contain at most 1000 actions".into());
                }
                let mut child_rows = Vec::with_capacity(raw_child_charges.len());
                for (charge_index, raw_charge) in raw_child_charges.iter().enumerate() {
                    let child_action = parse_charge(
                        raw_charge,
                        &format!("delegations[{index}].charges[{charge_index}]"),
                    )
                    .and_then(|(class, purpose, charge_bytes)| {
                        child
                            .charge(class, purpose, charge_bytes)
                            .map(|remaining| (class, purpose, charge_bytes, remaining))
                            .map_err(|error| error.to_string())
                    });
                    match child_action {
                        Ok((class, purpose, charge_bytes, remaining)) => child_rows.push(json!({
                            "index": charge_index,
                            "ok": true,
                            "class": class,
                            "purpose": purpose,
                            "bytes": charge_bytes,
                            "remaining_for_purpose": remaining,
                        })),
                        Err(error) => child_rows.push(json!({
                            "index": charge_index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        })),
                    }
                }
                Ok((bytes, child_rows, child))
            })();
            match action {
                Ok((bytes, child_rows, child)) => {
                    delegated.push(Some(child));
                    delegation_rows.push(json!({
                        "index": index,
                        "ok": true,
                        "child_index": delegated.len() - 1,
                        "bytes": bytes,
                        "charges": child_rows,
                    }));
                }
                Err(error) => delegation_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })),
            }
        }
        let raw_absorptions = arguments
            .get("absorb_delegated")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_absorptions.len() > 100 {
            return Err("absorb_delegated must contain at most 100 child indexes".into());
        }
        let mut absorption_rows = Vec::with_capacity(raw_absorptions.len());
        for (index, raw_index) in raw_absorptions.iter().enumerate() {
            let action = (|| -> Result<usize, String> {
                let child_index = raw_index
                    .as_u64()
                    .ok_or("absorb_delegated entries must be integer child indexes")?
                    as usize;
                let child = delegated
                    .get_mut(child_index)
                    .ok_or_else(|| format!("unknown child index {child_index}"))?
                    .take()
                    .ok_or_else(|| format!("child index {child_index} was already absorbed"))?;
                quota.absorb(child);
                Ok(child_index)
            })();
            match action {
                Ok(child_index) => absorption_rows.push(json!({
                    "index": index,
                    "ok": true,
                    "child_index": child_index,
                })),
                Err(error) => absorption_rows.push(json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })),
            }
        }
        let quota_classes = [
            StorageClass::Objects,
            StorageClass::Events,
            StorageClass::Indexes,
            StorageClass::Results,
            StorageClass::Cache,
        ]
        .into_iter()
        .map(|class| {
            json!({
                "class": class,
                "name": class.name(),
                "reconstructible": class.is_reconstructible(),
                "charged": quota.charged_by_class().get(&class).copied().unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();
        let remaining_children = delegated
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                child.as_ref().map(|child| {
                    json!({
                        "child_index": index,
                        "quota": child,
                    })
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/storage-lifecycle/0.1",
            "max_items": max_items,
            "now": now,
            "tiering": {
                "policy": policy,
                "plan": plan,
                "transition_count": plan.len(),
                "bytes_by_target": bytes_by_target,
                "apply_requested": apply_tiering,
                "apply_report": tier_apply_report,
                "records": records.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_records": records.len().saturating_sub(max_items),
                "input_rows": record_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_input_rows": record_rows.len().saturating_sub(max_items),
            },
            "quota": {
                "limit": quota.limit(),
                "reserve": quota.reserve(),
                "used": quota.used(),
                "remaining": quota.remaining(),
                "remaining_for_ingest": quota.remaining_for(Purpose::Ingest),
                "remaining_for_evidence_finalization": quota.remaining_for(Purpose::EvidenceFinalization),
                "remaining_for_cleanup": quota.remaining_for(Purpose::Cleanup),
                "classes": quota_classes,
                "charges": charge_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_charges": charge_rows.len().saturating_sub(max_items),
                "releases": release_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_releases": release_rows.len().saturating_sub(max_items),
                "delegations": delegation_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_delegations": delegation_rows.len().saturating_sub(max_items),
                "absorptions": absorption_rows.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_absorptions": absorption_rows.len().saturating_sub(max_items),
                "remaining_children": remaining_children.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_children": remaining_children.len().saturating_sub(max_items),
            },
            "guarantees": [
                "tiering is planned against a caller-supplied logical epoch, so the same records and policy replay to the same transitions",
                "pinned objects cannot be planned below warm, and skipped hot-to-cold moves remain explicit",
                "dry-run is the default; applying a plan reports absent objects instead of silently under-applying it",
                "ordinary ingest protects the quota reserve while evidence finalization and cleanup may use it",
                "delegation subtracts allowance from the parent and absorption consumes the child, so allowance is not copied",
                "storage classes retain raw attribution and identify reconstructible indexes and cache data",
            ],
            "limitations": [
                "this is a deterministic in-memory lifecycle projection; it does not move bytes, run a scheduler, or persist an audit event",
                "quota authorization is a typed calculation and does not enforce writes in an external backend or provide tenant isolation",
                "recency counts, logical epochs, pin state, and byte sizes are caller declarations; no access log or pin graph is discovered here",
                "no tier pricing, rehydration latency, calendar quota windows, encryption, replication, or garbage collector is created",
            ],
        }))
    }
}
