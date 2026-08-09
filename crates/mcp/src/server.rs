//! The MCP server.
//!
//! Implements blueprint 11.11 (MCP server and agent tools) over the FIBER compiler. Four of that
//! module's requirements are structural here rather than advisory:
//!
//! * **Progressive disclosure.** `fiber_compile` returns the L0 contract and a refinement handle,
//!   never the whole section. An agent asks for evidence when it decides it needs evidence.
//! * **Least authority.** Every path is resolved inside a root directory chosen at startup.
//!   Traversal outside it is refused, so an agent cannot turn the server into a file reader.
//! * **Side-effect preview.** Tools that write require an explicit `confirm`, and describe what
//!   they would do without it.
//! * **Audit trail.** Every call is written to stderr as a structured record, keeping stdout a
//!   clean JSON-RPC channel.
//!
//! Omissions travel with every response at every layer. An agent that reads only L0 still learns
//! how much was excluded and whether the sufficiency claim holds.

use crate::rpc::{code, Request, Response};
use bioprism_fiber::{compile, Query};
use bioprism_section::{CertificateProfile, ContextCertificate, Layer, RenderContext};
use bioprism_store::LazyWorld;
use bioprism_world::{World, WorldSource};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "bioprism";

pub struct Server {
    root: PathBuf,
    initialized: bool,
}

impl Server {
    pub fn new(root: PathBuf) -> Self {
        Server {
            root,
            initialized: false,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a client-supplied path inside the root.
    ///
    /// Rejects absolute paths and any `..` component before touching the filesystem, so the check
    /// does not depend on the path existing and cannot be defeated by a symlink race on lookup.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, String> {
        let candidate = Path::new(relative);
        if candidate.is_absolute() {
            return Err(format!("absolute paths are refused: {relative:?}"));
        }
        for component in candidate.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                _ => {
                    return Err(format!(
                        "path escapes the server root and is refused: {relative:?}"
                    ))
                }
            }
        }
        Ok(self.root.join(candidate))
    }

    fn load_source(&self, relative: &str) -> Result<Box<dyn WorldSource>, String> {
        let path = self.resolve(relative)?;
        if path.is_dir() && path.join("manifest.json").exists() {
            LazyWorld::open(&path)
                .map(|lazy| Box::new(lazy) as Box<dyn WorldSource>)
                .map_err(|e| e.to_string())
        } else {
            let raw = self.read_json(&path)?;
            World::from_json(raw)
                .map(|world| Box::new(world) as Box<dyn WorldSource>)
                .map_err(|e| e.to_string())
        }
    }

    fn read_json(&self, path: &Path) -> Result<Value, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
    }

    fn load_query(&self, relative: &str) -> Result<Query, String> {
        let path = self.resolve(relative)?;
        let raw = self.read_json(&path)?;
        Query::from_json(raw).map_err(|e| e.to_string())
    }

    pub fn handle(&mut self, request: &Request) -> Option<Response> {
        if request.is_notification() {
            if request.method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        }

        let id = request.id.clone();
        let response = match request.method.as_str() {
            "initialize" => Response::result(id, self.initialize()),
            "ping" => Response::result(id, json!({})),
            "tools/list" => Response::result(id, json!({ "tools": tool_definitions() })),
            "tools/call" => self.call_tool(request),
            "resources/list" => Response::result(id, json!({ "resources": [] })),
            other => Response::error(
                id,
                code::METHOD_NOT_FOUND,
                format!("unknown method {other:?}"),
                None,
            ),
        };
        Some(response)
    }

    fn initialize(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Compiles a typed decision query against a FIBER world into the \
                smallest decision-sufficient context, with a machine-verifiable certificate of \
                what was omitted. Call fiber_compile first: it returns the decision contract and \
                a refinement handle, not the evidence. Call fiber_refine to descend layers only \
                when the contract is insufficient to act. Every response reports what was omitted \
                and whether the sufficiency claim holds. Research infrastructure, not a medical \
                device."
        })
    }

    fn call_tool(&self, request: &Request) -> Response {
        let id = request.id.clone();
        let Some(name) = request.params.get("name").and_then(Value::as_str) else {
            return Response::error(
                id,
                code::INVALID_PARAMS,
                "tools/call requires a name".into(),
                None,
            );
        };
        let arguments = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        audit(name, &arguments);

        let outcome = match name {
            "fiber_compile" => self.fiber_compile(&arguments),
            "fiber_refine" => self.fiber_refine(&arguments),
            "fiber_explain" => self.fiber_explain(&arguments),
            "fiber_verify" => self.fiber_verify(&arguments),
            "world_index" => self.world_index(&arguments),
            other => Err(format!("unknown tool {other:?}")),
        };

        match outcome {
            Ok(value) => Response::result(id, tool_content(&value, false)),
            Err(message) => Response::result(
                id,
                tool_content(&json!({ "ok": false, "error": message }), true),
            ),
        }
    }

    fn compiled(
        &self,
        arguments: &Value,
    ) -> Result<(bioprism_fiber::CompileOutput, RenderContext), String> {
        let world = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a path relative to the server root)")?;
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or("query is required (a path relative to the server root)")?;

        let source = self.load_source(world)?;
        let query = self.load_query(query)?;
        let out = compile(source.as_ref(), &query).map_err(|e| e.to_string())?;

        let context = RenderContext {
            omitted_facts: out.certificate.omissions.total_facts,
            total_facts: out.certificate.plan.total_fact_count,
            supports_sufficiency_claim: out.certificate.manifest.supports_sufficiency_claim(),
            protected_closure_satisfied: out.protected_closure_satisfied(),
            certificate_sha256: out
                .certificate
                .digest(CertificateProfile::Reference)
                .ok()
                .map(|digest| digest.as_str().to_string()),
        };
        Ok((out, context))
    }

    fn fiber_compile(&self, arguments: &Value) -> Result<Value, String> {
        let (out, context) = self.compiled(arguments)?;
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
        }
        Ok(rendered)
    }

    fn fiber_refine(&self, arguments: &Value) -> Result<Value, String> {
        let text = arguments
            .get("layer")
            .and_then(Value::as_str)
            .ok_or("layer is required, one of l0..l4")?;
        let layer = Layer::parse(text).ok_or(format!("unknown layer {text:?}"))?;
        let (out, context) = self.compiled(arguments)?;
        Ok(out.section.render(layer, &context))
    }

    fn fiber_explain(&self, arguments: &Value) -> Result<Value, String> {
        let (out, _) = self.compiled(arguments)?;
        Ok(json!({
            "ok": true,
            "backend": out.certificate.plan.backend.as_str(),
            "passes": out.trace.passes.iter().map(|pass| json!({
                "name": pass.name, "retained": pass.retained, "note": pass.note
            })).collect::<Vec<_>>(),
            "passes_not_run": out.trace.deferred_passes.iter().map(|(name, reason)| json!({
                "name": name, "reason": reason
            })).collect::<Vec<_>>(),
            "selection": {
                "facts": out.certificate.plan.compiled_fact_count,
                "of_total": out.certificate.plan.total_fact_count,
                "fraction": out.certificate.plan.fact_selection_ratio(),
            },
            "omission_manifest": out.certificate.manifest,
            "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
            "protected_closure_satisfied": out.protected_closure_satisfied(),
            "unmatched_protected_tags": out.trace.unmatched_protected_tags,
        }))
    }

    fn fiber_verify(&self, arguments: &Value) -> Result<Value, String> {
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
            DigestMismatch { claimed, recomputed } => json!({
                "ok": true, "verified": false,
                "detail": format!("digest mismatch: claims {claimed}, recomputes to {recomputed}")
            }),
            Malformed(reason) => json!({
                "ok": true, "verified": false, "detail": format!("malformed: {reason}")
            }),
        })
    }

    /// The one tool with side effects. Without `confirm: true` it previews and writes nothing.
    fn world_index(&self, arguments: &Value) -> Result<Value, String> {
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
}

fn tool_content(value: &Value, is_error: bool) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into()),
        }],
        "isError": is_error,
    })
}

/// Structured audit record on stderr, keeping stdout a clean protocol channel.
fn audit(tool: &str, arguments: &Value) {
    eprintln!(
        "{}",
        json!({ "audit": "tool_call", "tool": tool, "arguments": arguments })
    );
}

pub fn tool_definitions() -> Vec<Value> {
    let world_and_query = json!({
        "type": "object",
        "properties": {
            "world": { "type": "string", "description": "Path to a fiber-world/0.1 document or an indexed store directory, relative to the server root." },
            "query": { "type": "string", "description": "Path to a fiber-query/0.1 document, relative to the server root." }
        },
        "required": ["world", "query"]
    });

    vec![
        json!({
            "name": "fiber_compile",
            "description": "Compile a typed decision query into the smallest decision-sufficient \
                context. Returns the L0 decision contract — goal, verdict, what was omitted, and \
                whether the sufficiency claim holds — plus a handle for descending to evidence. It \
                deliberately does not return the evidence: call fiber_refine when the contract is \
                not enough to act on.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "world": { "type": "string", "description": "Path to a world document or store directory, relative to the server root." },
                    "query": { "type": "string", "description": "Path to a query document, relative to the server root." },
                    "layer": { "type": "string", "enum": ["l0", "l1", "l2", "l3", "l4"], "description": "Starting layer. Defaults to l0." }
                },
                "required": ["world", "query"]
            }
        }),
        json!({
            "name": "fiber_refine",
            "description": "Descend one or more layers of a compiled Decision Section. l1 adds the \
                obligation and evidence inventory without values; l2 adds the evidence the verdict \
                rests on; l3 adds factors, provenance and the refinement frontier; l4 adds raw \
                artifacts. Omissions are reported at every layer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "world": { "type": "string" },
                    "query": { "type": "string" },
                    "layer": { "type": "string", "enum": ["l0", "l1", "l2", "l3", "l4"] }
                },
                "required": ["world", "query", "layer"]
            }
        }),
        json!({
            "name": "fiber_explain",
            "description": "Show the compile plan: which passes ran and what each retained, which \
                passes could not run and why, selection ratios, and omissions grouped by influence \
                class. Read this before trusting a compact context.",
            "inputSchema": world_and_query
        }),
        json!({
            "name": "fiber_verify",
            "description": "Recompute a Context Certificate's digest and report whether it \
                verifies. Use before acting on a certificate produced elsewhere.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "certificate": { "type": "string", "description": "Path to a certificate document, relative to the server root." }
                },
                "required": ["certificate"]
            }
        }),
        json!({
            "name": "world_index",
            "description": "Build a content-addressed index for a world so later compiles cost \
                what the compiled region costs rather than what the corpus costs. Writes to disk: \
                without confirm=true it previews the effect and writes nothing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "world": { "type": "string" },
                    "store": { "type": "string", "description": "Directory to write the index into, relative to the server root." },
                    "confirm": { "type": "boolean", "description": "Must be true to actually write." }
                },
                "required": ["world", "store"]
            }
        }),
    ]
}
