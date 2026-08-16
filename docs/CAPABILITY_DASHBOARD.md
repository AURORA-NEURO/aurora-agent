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
`capability_discover` for ranked intent matching and `capability_route_review` before constructing
an executable `agent_mission` allow-list.

## SDK surfaces

- Python: `CapabilityDashboardQueryArgs`, `CapabilityDashboardReport`, and sync/async Workspace
  and HTTP client methods.
- TypeScript: `CapabilityDashboardArgs`, `CapabilityDashboardResult`, and
  `ApiClient.capabilityDashboard(...)`.
- MCP: the `capability_dashboard` tool with the
  `bioprism-devplat-capability-dashboard/0.1` audit schema.
