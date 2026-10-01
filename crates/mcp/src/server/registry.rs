//! Registry contract validation and bounded lifecycle simulation.

use super::*;

impl Server {
    pub(super) fn registry_gate(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments.get("pack").and_then(Value::as_str).ok_or(
            "pack is required (a path to an attested benchmark pack relative to the server root)",
        )?;
        let path = self.resolve(relative)?;
        if path.is_dir() {
            return Err("registry_gate requires an attested JSON pack document".into());
        }
        let policy_name = arguments
            .get("policy")
            .and_then(Value::as_str)
            .unwrap_or("default");
        let policy = match policy_name {
            "default" => RegistryPolicy::default(),
            "experimental" => RegistryPolicy::experimental(),
            other => {
                return Err(format!(
                    "unknown registry policy {other:?}; choose default or experimental"
                ));
            }
        };
        let outcome = gate_document(&self.read_json(&path)?, &policy);
        Ok(json!({
            "ok": true,
            "pack": relative,
            "policy": policy_name,
            "passed": outcome.is_pass(),
            "blocked": outcome.is_block(),
            "outcome": outcome,
            "fail_closed": true,
            "note": "a pass establishes internal conformance for the earned tier, not scientific validity or runtime security",
        }))
    }

    pub(super) fn registry_lifecycle_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let input_bytes = serde_json::to_vec(arguments)
            .map_err(|error| format!("registry lifecycle arguments are not serialisable: {error}"))?
            .len();
        if input_bytes > 20_000_000 {
            return Err("registry lifecycle input exceeds the 20 MB safety bound".into());
        }

        let raw_packs = arguments
            .get("packs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_packs.len() > 64 {
            return Err("packs must contain at most 64 attested benchmark-pack documents".into());
        }
        let packs = raw_packs
            .iter()
            .map(|document| {
                BenchmarkPack::from_attested(document)
                    .map_err(|error| format!("attested pack preflight failed: {error}"))
            })
            .collect::<Vec<Result<BenchmarkPack, String>>>();
        let pack_rows = packs
            .iter()
            .enumerate()
            .map(|(index, result)| match result {
                Ok(pack) => json!({
                    "index": index,
                    "valid": true,
                    "name": pack.name(),
                    "artifact_digest": pack.digest().ok().map(|digest| digest.to_string()),
                    "core_digest": pack.core_digest().ok().map(|digest| digest.to_string()),
                    "publisher": pack.provenance.publisher,
                    "instance_count": pack.instances.len(),
                }),
                Err(error) => json!({
                    "index": index,
                    "valid": false,
                    "refusal": error,
                    "fail_closed": true,
                }),
            })
            .collect::<Vec<_>>();

        let mut index: RegistryIndex = arguments
            .get("index")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid serialized RegistryIndex: {error}"))?
            .unwrap_or_else(RegistryIndex::new);
        let policy: TierPolicy = arguments
            .get("policy")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid serialized TierPolicy: {error}"))?
            .unwrap_or_default();
        let actions = arguments
            .get("actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if actions.len() > 256 {
            return Err("actions must contain at most 256 lifecycle operations".into());
        }

        let initial_broken = index.verify_all();
        let initial_integrity = json!({
            "artifact_count": index.len(),
            "log_count": index.log().len(),
            "broken_count": initial_broken.len(),
            "broken": initial_broken.iter().map(|(digest, attestation)| json!({
                "digest": digest,
                "attestation": format!("{attestation:?}"),
            })).collect::<Vec<_>>(),
            "operations_allowed": initial_broken.is_empty(),
        });

        let mut action_rows = Vec::with_capacity(actions.len());
        for (action_index, action) in actions.iter().enumerate() {
            let Some(object) = action.as_object() else {
                action_rows.push(json!({
                    "index": action_index,
                    "ok": false,
                    "refusal": "each action must be an object",
                    "fail_closed": true,
                }));
                continue;
            };
            let Some(operation) = object.get("op").and_then(Value::as_str) else {
                action_rows.push(json!({
                    "index": action_index,
                    "ok": false,
                    "refusal": "each action requires a string op",
                    "fail_closed": true,
                }));
                continue;
            };
            let refusal = |message: String| {
                json!({
                    "index": action_index,
                    "op": operation,
                    "ok": false,
                    "refusal": message,
                    "fail_closed": true,
                })
            };
            if !initial_broken.is_empty() {
                action_rows.push(refusal(
                    "serialized registry integrity failed; no lifecycle mutation or lookup was run"
                        .into(),
                ));
                continue;
            }

            let digest = || {
                object
                    .get("digest")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "action requires a non-empty digest".to_string())
            };
            let tier = || {
                let raw = object
                    .get("tier")
                    .ok_or_else(|| "action requires tier".to_string())?;
                serde_json::from_value::<TrustTier>(raw.clone())
                    .map_err(|error| format!("invalid trust tier: {error}"))
            };
            let pack_index = |field: &str| {
                let raw = object
                    .get(field)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("action requires integer {field}"))?;
                let index = usize::try_from(raw)
                    .map_err(|_| format!("{field} is outside the supported range"))?;
                match packs.get(index) {
                    Some(Ok(pack)) => Ok(pack),
                    Some(Err(error)) => Err(format!("pack {index} is unavailable: {error}")),
                    None => Err(format!("pack index {index} is outside packs")),
                }
            };
            let reason = || {
                object
                    .get("reason")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| "action requires a non-empty reason".to_string())
            };

            let result = match operation {
                "publish" => {
                    let tier = tier();
                    let pack = pack_index("pack_index");
                    match (pack, tier) {
                        (Ok(pack), Ok(tier)) => index
                            .publish(pack, tier, &policy)
                            .map(|digest| {
                                json!({
                                    "digest": digest,
                                    "tier": tier,
                                    "status": "active",
                                })
                            })
                            .map_err(|error| error.to_string()),
                        (Err(error), _) | (_, Err(error)) => Err(error),
                    }
                }
                "promote" => match (digest(), tier()) {
                    (Ok(digest), Ok(tier)) => index
                        .promote(digest, tier, &policy)
                        .map(|promotion| {
                            json!({
                                "promotion": promotion,
                                "digest": digest,
                                "tier": tier,
                            })
                        })
                        .map_err(|error| error.to_string()),
                    (Err(error), _) | (_, Err(error)) => Err(error),
                },
                "reassess" => match digest() {
                    Ok(digest) => index
                        .reassess(digest, &policy)
                        .map(|verdict| {
                            json!({
                                "digest": digest,
                                "verdict": verdict,
                                "tier_after": index.tier_of(digest),
                            })
                        })
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error),
                },
                "supersede" => {
                    let superseded = digest();
                    let replacement = pack_index("replacement_pack_index");
                    let tier = tier();
                    let reason = reason();
                    match (superseded, replacement, tier, reason) {
                        (Ok(superseded), Ok(replacement), Ok(tier), Ok(reason)) => index
                            .supersede(superseded, replacement, tier, reason, &policy)
                            .map(|replacement_digest| {
                                json!({
                                    "superseded": superseded,
                                    "replacement": replacement_digest,
                                    "tier": tier,
                                })
                            })
                            .map_err(|error| error.to_string()),
                        (Err(error), _, _, _)
                        | (_, Err(error), _, _)
                        | (_, _, Err(error), _)
                        | (_, _, _, Err(error)) => Err(error),
                    }
                }
                "withdraw" => match (digest(), reason()) {
                    (Ok(digest), Ok(reason)) => index
                        .withdraw(digest, reason)
                        .map(|()| {
                            json!({
                                "digest": digest,
                                "status": index.status(digest),
                            })
                        })
                        .map_err(|error| error.to_string()),
                    (Err(error), _) | (_, Err(error)) => Err(error),
                },
                "resolve" => {
                    let name = object
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| "resolve requires a non-empty name".to_string());
                    name.map(|name| {
                        let resolved = index.resolve(name);
                        json!({
                            "name": name,
                            "found": resolved.is_some(),
                            "digest": resolved,
                            "core_digest": index.content_of(name),
                        })
                    })
                }
                "history" => digest().map(|digest| {
                    let history = index.history(digest);
                    json!({
                        "digest": digest,
                        "found": index.tier_of(digest).is_some(),
                        "event_count": history.len(),
                        "events": history,
                    })
                }),
                "inspect" => digest().map(|digest| {
                    json!({
                        "digest": digest,
                        "found": index.get(digest).is_some(),
                        "tier": index.tier_of(digest),
                        "status": index.status(digest),
                        "core_digest": index.core_digest_of(digest),
                        "artifact": index.get(digest),
                    })
                }),
                "revisions" => {
                    let core_digest = object
                        .get("core_digest")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| "revisions requires a non-empty core_digest".to_string());
                    core_digest.map(|core_digest| {
                        let revisions = index.revisions_of_content(core_digest);
                        json!({
                            "core_digest": core_digest,
                            "revision_count": revisions.len(),
                            "digests": revisions,
                        })
                    })
                }
                "verify_all" => {
                    let broken = index.verify_all();
                    Ok(json!({
                        "clean": broken.is_empty(),
                        "broken_count": broken.len(),
                        "broken": broken.iter().map(|(digest, attestation)| json!({
                            "digest": digest,
                            "attestation": format!("{attestation:?}"),
                        })).collect::<Vec<_>>(),
                    }))
                }
                other => Err(format!(
                    "unknown lifecycle op {other:?}; choose publish, promote, reassess, supersede, withdraw, resolve, history, inspect, revisions, or verify_all"
                )),
            };
            match result {
                Ok(value) => action_rows.push(json!({
                    "index": action_index,
                    "op": operation,
                    "ok": true,
                    "result": value,
                })),
                Err(error) => action_rows.push(refusal(error)),
            }
        }

        let final_broken = index.verify_all();
        let include_index = arguments
            .get("include_index")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let final_registry = if include_index {
            Some(
                serde_json::to_value(&index)
                    .map_err(|error| format!("registry serialization failed: {error}"))?,
            )
        } else {
            None
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/registry-lifecycle/0.1",
            "policy": policy,
            "packs": pack_rows,
            "initial_integrity": initial_integrity,
            "actions": action_rows,
            "final": {
                "artifact_count": index.len(),
                "log_count": index.log().len(),
                "broken_count": final_broken.len(),
                "integrity_clean": final_broken.is_empty(),
                "verification": final_broken.iter().map(|(digest, attestation)| json!({
                    "digest": digest,
                    "attestation": format!("{attestation:?}"),
                })).collect::<Vec<_>>(),
                "log": index.log(),
            },
            "registry": final_registry,
            "guarantees": [
                "attested input packs are re-verified before they can be published or supersede an artifact",
                "artifact bytes remain content-addressed and lifecycle events remain append-only",
                "failed actions are typed refusals and do not abort independent later actions",
                "serialized registry integrity is checked before any lookup or mutation",
                "the returned registry can be supplied as index in a later call to continue the simulation",
            ],
            "limitations": [
                "this is a local deterministic registry projection; it does not provide network transport, signatures, federation, moderation, quarantine, or authentication",
                "a valid attestation proves internal digest consistency, not scientific validity or publisher identity",
                "withdrawal preserves historical bytes and records a reason; it does not delete or hide an artifact",
            ],
        }))
    }
}
