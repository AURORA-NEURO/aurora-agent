# Operational-readiness audit

`operational_readiness_audit` is the bounded, deterministic contract for a service's declared
operational posture. It joins service objectives, indicator observations, dependency failure
handling, reviewed runbooks, incident closure, and baseline controls while keeping each evidence
layer inspectable. The route is intentionally an artifact audit: it does not query telemetry,
page an on-call schedule, inspect a dependency, create or update an incident, execute a runbook,
test a restore, or authorize a deployment.

## Wire contract

The input schema is `bioprism-operational-readiness/0.1`; the output schema is
`bioprism-operational-readiness-audit/0.1`. The manifest contains:

- a service identity, version, accountable owner, and `critical`/`important`/`advisory`
  criticality;
- operational contracts for availability, latency, durability, recovery, security, privacy, or
  capacity, each with an objective and target;
- indicators naming their contract, metric, measurement source, status, optional measurement, and
  a SHA-256 evidence digest when observed;
- dependencies with owners, explicit failure modes, criticality, and a declared fallback;
- runbooks with triggers, owners, non-empty steps, review status, and incident classes;
- incidents with severity, lifecycle state, runbook binding, owner, timeline, and postmortem;
- seven controls: on-call, alerting, tracing, audit logging, backup, restore testing, and access
  review; and
- policies that decide which evidence layers are required.

The Rust kernel enforces bounded collections (4,096 contracts, 8,192 indicators, 8,192
dependencies, 4,096 runbooks, 4,096 incidents, and 16,384 list values) and rejects no malformed
digest silently. The MCP envelope is capped at 20 MiB before JSON deserialization.

## Readiness semantics

The result keeps three states separate:

- `ok` means the tool call produced a structured response;
- `valid` means the manifest has no blocking contract, evidence, fallback, runbook, incident, or
  control issues; and
- `operationally_ready` means the derived readiness posture is true after those blocking issues
  are evaluated.

Every issue has a stable code, severity, subject, explanation, and remediation. The audit also
returns per-indicator, per-dependency, per-runbook, per-incident, and per-control rows plus
counts, guarantees, limitations, and the canonical manifest digest. A closed incident without a
timeline or postmortem is not treated as complete learning evidence. A critical dependency
without a fallback remains a blocker. An observed indicator without a digest remains unproven.

## What this proves and what it does not

The route proves internal coherence of the supplied declaration, reference closure, digest shape,
required-layer coverage, and fail-closed issue classification. It does not prove that a metric was
actually collected, that an alert fires, that a person is reachable, that a dependency fallback
works, that a runbook was followed, that a restore succeeded, that an incident was fully learned,
or that a production system meets its target. Those require external telemetry, identity,
incident-management, infrastructure, and controlled-execution systems.

## SDK surfaces

Python exposes `OperationalReadinessManifestArgs`, typed nested argument records,
`operational_readiness_audit_report`, and sync/async methods on `Workspace`, `AsyncWorkspace`,
`ApiClient`, and `AsyncApiClient`. TypeScript exposes the corresponding manifest/result interfaces
and `ApiClient.operationalReadinessAudit(...)`. All facades preserve the raw MCP/HTTP envelope
and the separate observation, fallback, incident, and control rows.
