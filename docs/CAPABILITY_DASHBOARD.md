# Capability dashboard

`capability_dashboard` is the operator-facing projection between the workspace catalogue,
authoritative MCP schemas, and the SDK/CLI surface declarations. It answers a practical question
before an agent builds a mission: which domains are actually callable through this transport, which
ones are only declared, and what surface is missing?

## Query

```json
{
  "domain": "oncology",
  "group_id": "biological",
  "status": "available",
  "max_groups": 128,
  "include_tools": true,
  "include_gaps": true
}
```

Filters are conjunctive. `group_id` is a case-insensitive prefix, `domain` is a case-insensitive
substring over declared labels, and `status` is an exact case-insensitive status. Group output is
bounded to 512 rows, with a default of 128. A bound reached while additional matching groups exist
produces an explicit warning; a filtered query that happens to return exactly its bound does not
pretend that truncation occurred.

## Group readiness

Every row is classified independently:

- `callable`: at least one declared MCP tool exists and every unique tool has a well-formed,
  authoritative `tools/list` input schema;
- `partial`: at least one declared MCP tool exists, but a schema is missing or malformed;
- `declared_only`: the group has no MCP tools, even if its crate, CLI, or Python surface exists.

The row keeps separate counts for crates, MCP memberships, CLI entrypoints, and Python artifacts.
Gaps such as `no_cli_entrypoints`, `no_python_artifact`, `missing_transport_schema`, and
`no_mcp_tools` are labels, not a blended quality score. Duplicate tool membership across groups is
allowed and is not double-counted in unique-tool totals.

## Digests and boundaries

`catalog_digest` identifies the catalogue used for the view. `dashboard_digest` binds that digest,
the normalized query, selected rows, readiness counts, and gap counts. The top-level
`capability_dashboard_ready` means every returned group is callable and the selection was not
bounded; it does not grant permission, prove scientific validity, verify a local CLI installation,
import Python modules, execute a tool, or establish deployment readiness.

The route reads only the in-process catalogue and MCP tool definitions. It does not inspect an
external environment or infer capabilities from package names. Callers should use
`capability_discover` for ranked intent matching, `capability_route` to batch named cross-domain
needs, and `capability_route_review` before constructing an executable `agent_mission` allow-list.

The direct HTTP planning handoff is split into two explicit, non-executing endpoints:

- `POST /v1/capabilities/route` accepts the same bounded route request as the MCP tool and returns
  the raw `capability_route` proposal.
- `POST /v1/capabilities/route/review` accepts that returned route plus caller-selected tools and
  returns the raw `capability_route_review` handoff. A ready review still requires mission
  preflight and never dispatches a tool.
- `POST /v1/capabilities/route/plan` composes that explicit review with authoritative mission
  preflight. It returns the generated mission, plan digest, schema findings, and a structured
  blocked outcome when either review boundary fails; `dispatch` remains `not_started`.
- `POST /v1/capabilities/route/plan/verify` checks a retained plan, reruns mission preflight, and
  optionally replays route review from the original route and selections. It never dispatches and
  distinguishes full replay verification from `verified_without_route_replay`.

Both endpoints record the same tool event and use the same authoritative catalogue as MCP. They
exist for HTTP clients and automation that should not have to unpack an MCP response envelope.

## SDK surfaces

- Python: `CapabilityDashboardQueryArgs`, `CapabilityDashboardReport`, and sync/async Workspace
  and HTTP client methods, plus `capability_route_rest(...)` and
  `capability_route_review_rest(...)`, `capability_route_plan_rest(...)`, and
  `capability_route_plan_verify_rest(...)` for raw planning and replay verification.
- TypeScript: `CapabilityDashboardArgs`, `CapabilityDashboardResult`, and
  `ApiClient.capabilityDashboard(...)`, `capabilityRouteRest(...)`, and
  `capabilityRouteReviewRest(...)`, `capabilityRoutePlanRest(...)`, and
  `capabilityRoutePlanVerifyRest(...)`.
- MCP: the `capability_dashboard` tool with the
  `bioprism-devplat-capability-dashboard/0.1` audit schema.
