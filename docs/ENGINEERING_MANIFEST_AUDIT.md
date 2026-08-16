# Engineering Manifest Audit

The engineering_manifest_audit route validates a bounded, machine-readable engineering
manifest. It is the artifact-level contract for the parts of the build-ready engineering plan
that can be checked without access to a checkout, ticket system, CI runner, GitHub, or a human
approval queue.

The route is intentionally deterministic and in-memory. It accepts a manifest, canonicalizes it
through the Rust identity layer, computes a content digest, and returns an audit projection. The
digest identifies the declared artifact; it does not identify the state of the repository named
by the artifact.

## What the manifest contains

The top-level object has these fields:

- schema: bioprism-engineering-manifest/0.1.
- project: stable id, version, and repository strings.
- baseline: language, runtime, API, storage, observability, deployment, and optional reason
  entries.
- packages: package identity and repository-relative path, language, kind, owner, dependency
  edges, public visibility, and optional test command.
- tickets: implementation identity, title, target package, contract name, status, ticket
  dependencies, acceptance conditions, and an optional blocker.
- adrs: decision identity, title, status, decision text, affected surfaces, and optional
  supersession target.
- ownership: one RACI-style row per surface, with accountable, responsible, consulted,
  informed, and optional independent reviewer parties.
- policies: explicit fail-closed toggles for package acyclicity, ticket contracts, ownership,
  and ADR targets. All four default to true.

The input is bounded at 20 MB. Package, ADR, and ownership lists are bounded at 4,096 entries;
the ticket list is bounded at 10,000 entries. Individual strings are bounded at 4,096 UTF-8
bytes by the Python authoring facade, and all list fields are bounded before transport.

## Audit semantics

### Package topology

Package identifiers and repository-relative paths must be unique. Every dependency must identify
another declared package, and a package cannot depend on itself. The route runs a deterministic
topological ordering and retains cyclic components in cyclic_packages. A cycle is blocking
when require_acyclic_packages is true. If that policy is explicitly disabled, the cycle remains
visible as a warning; disabling the gate does not make the graph acyclic.

The order is a dependency-first order with stable lexical tie-breaking. It is a planning order,
not an instruction to execute builds.

### Ticket readiness

Ticket dependencies are checked separately from package dependencies. Each ticket receives a
readiness row:

- complete: the ticket status is done.
- blocked: the ticket status is blocked.
- waiting: the ticket is planned or in progress but at least one dependency is not done or is
  missing.
- actionable: the ticket is planned or in progress and every named dependency is done and
  declared.

blocking_dependencies lists the exact ticket identifiers that prevent dependency readiness.
Missing dependency identifiers are both visible in the readiness row and reported as blocking
issues. A blocked ticket must carry a non-empty blocker explanation. A complete ticket is not
proof that its acceptance condition passed; it is only the status declared in the artifact.

### ADR history

ADR identifiers must be unique. An ADR with require_adr_targets enabled must name at least one
affected package, contract, or public surface. A supersedes reference must point to another
ADR and cannot point to itself. Supersession cycles are blocking because they cannot describe a
one-way decision history. The audit returns every valid supersession edge as {newer, older,
valid}.

The route does not judge whether an ADR decision is technically correct or whether the affected
code reflects it.

### Ownership and independent review

Ownership surfaces must not have duplicate rows. A required row names one accountable party and
at least one responsible party. If an independent reviewer is declared, that reviewer must not be
the accountable party or one of the responsible parties. This catches a common artifact-level
failure where an apparent review role is identical to the authoring or decision role.

The check proves role separation in the manifest only. It does not prove that the named people or
teams exist, accepted the role, performed review, or are free of conflicts.

## Issue model

Every issue has:

- code: stable machine-readable category;
- severity: blocking or warning;
- subject: the affected identifier or edge;
- detail: the local reason;
- remediation: a concrete repair direction.

Important blocking codes include schema_mismatch, required_field_empty,
duplicate_package_id, duplicate_package_path, missing_package_dependency,
self_dependency, package_cycle, duplicate_ticket_id, ticket_package_missing,
ticket_acceptance_missing, blocked_ticket_without_reason, missing_ticket_dependency,
duplicate_adr_id, invalid_adr_supersession, adr_supersession_cycle,
duplicate_ownership_surface, ownership_row_incomplete, and
reviewer_not_independent.

The server reports valid as true only when no blocking issue exists. Warnings do not disappear
from the response and never get converted into a pass count. The counts object separately reports
packages, public packages, tickets, completed tickets, actionable tickets, ADRs, accepted ADRs,
and ownership rows.

## Digest and transport

EngineeringManifest::digest is the canonical content hash of the decoded manifest, including
the schema, defaults represented by serde, package/ticket/ADR/ownership order, and policies. The
audit includes the digest in both the nested audit and the route envelope. Callers can store the
digest beside a plan, review, or build record and detect whether they are discussing the same
manifest bytes after canonical decoding.

The MCP route returns a structured object with schema, workflow, manifest_digest,
blocking_issue_count, warning_count, audit, guarantees, and limitations. The REST gateway wraps
the same structured content in its normal HTTP/MCP envelope. The Python SDK unwraps both forms; the
TypeScript SDK preserves the raw envelope and types the structured result.

Malformed route arguments are refused before audit execution. A valid response with
audit.valid: false is still a successful audit operation: it is evidence that the artifact was
checked and found incoherent, not a transport failure. Client code should distinguish transport
success, audit validity, and any later human or release decision.

## Guarantees and non-claims

The route guarantees that:

- the supplied manifest is bounded before the in-memory audit;
- package and ticket edges are checked independently;
- cycles, missing references, readiness blockers, ADR supersession, and reviewer-role collisions
  remain visible;
- the digest binds the canonical manifest rather than an external checkout;
- the result preserves warning versus blocking severity.

The route does not:

- read files or compare package paths with a filesystem;
- run tests, compilers, CI, workflows, or deployment;
- query GitHub issues, pull requests, commits, branches, or actions;
- create or update tickets, ADRs, ownership records, or release requests;
- authenticate or verify named people, teams, repositories, or external services;
- prove implementation, review completion, approval, release readiness, security, or biological
  validity.

This boundary is deliberate: the manifest makes engineering intent and internal consistency
machine-checkable while leaving execution and authority to the systems and people that actually
own those responsibilities.

## SDK usage

The Python SDK exposes EngineeringManifestArgs and typed nested argument classes, plus
engineering_manifest_audit_report. The report retains package order, cycle components, issue
objects, ownership surfaces, ticket readiness, digest, and explicit limitations on Workspace,
AsyncWorkspace, ApiClient, and AsyncApiClient.

The TypeScript SDK exposes EngineeringManifestArgs and engineeringManifestAudit, returning the
normal RestToolResponse with EngineeringManifestAuditResult structured content. The raw envelope
remains available for callers that need request IDs or transport metadata.
