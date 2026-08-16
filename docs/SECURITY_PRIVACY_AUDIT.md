# Security/privacy governance audit

`security_privacy_audit` is the bounded, deterministic governance contract for a declared system.
It complements the existing threat-model and red-team replay routes by checking whether the
system's data assets, permitted flows, identities, threat treatment, independent reviews, and
controls form a coherent evidence chain. It is not a scanner, identity provider, DLP engine,
legal-compliance determination, red-team runner, erasure service, or deployment authorization.

## Wire contract

The input schema is `bioprism-security-privacy/0.1`; the output schema is
`bioprism-security-privacy-audit/0.1`. A manifest contains:

- system identity, version, and accountable owner;
- assets with public/internal/confidential/restricted/regulated classification, owner, purpose,
  retention limit, residency, and deletion process;
- flows that bind an asset to source, destination, purpose, legal basis, allow/deny/conditional
  decision, and digest-bound authorization evidence;
- identities with principal, role, authentication method, MFA, least privilege, and asset scope;
- threats with category, low/medium/high/critical severity, treatment status, control binding,
  evidence digest, and accepted-risk rationale;
- privacy-impact, security-assessment, red-team, or access reviews with independent reviewer,
  scope, status, evidence, expiry, and findings;
- ten controls: access control, encryption at rest/in transit, key rotation, audit logging,
  vulnerability management, backup/restore, incident response, vendor review, and data-subject
  rights; and
- policies that select the required governance layers.

The kernel bounds assets at 4,096, flows/identities/threats at 8,192 each, reviews at 4,096, and
list values at 16,384. The MCP input envelope is capped at 20 MiB. Evidence digests are required
to be 64 hexadecimal characters; the digest binds the canonical declaration, not the truth of an
external system.

## Readiness semantics

`ok` means the route returned a structured result. `valid` and `security_privacy_ready` are
derived from blocking issue rows. They remain false when, for example, a regulated asset has no
retention/deletion record, an allowed flow lacks authorization evidence, sensitive access lacks
MFA, a high threat is untreated, a mitigation lacks evidence, a review is expired, or a required
control is disabled. Every issue preserves code, severity, subject, detail, and remediation.

The output also returns per-asset, per-flow, per-identity, per-threat, per-review, and per-control
rows plus counts, guarantees, limitations, and the canonical manifest digest. A red-team replay
that reports a modelled threat is not silently upgraded into mitigation evidence; those are
separate layers by design.

## What this proves and what it does not

The route proves declaration coherence, asset/reference closure, digest shape, explicit
authorization and treatment records, review independence as declared, and fail-closed issue
classification. It does not prove that a person authenticated, a control is deployed, an
encryption key works, a legal basis is valid, a vendor is trustworthy, a red-team action ran, data
was erased, or a threat is absent. Those claims require external systems and authority.

## SDK surfaces

Python exposes `SecurityPrivacyManifestArgs`, typed nested asset/flow/identity/threat/review and
control records, `security_privacy_audit_report`, and sync/async methods on `Workspace`,
`AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`. TypeScript exposes the matching manifest and
result interfaces and `ApiClient.securityPrivacyAudit(...)`.
