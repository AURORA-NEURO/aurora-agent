# Security, safety, and red-team program audit

`security_program_audit` is a bounded, deterministic governance contract for the program around
red-team work. The existing `security_redteam_simulate` route models typed section-13 replay; this
route audits whether the surrounding program has an authorized perimeter, independent review,
evidence custody, finding closure, incident response, disclosure sequencing, and regression
controls. It audits declarations and never runs an adversarial action.

## Wire contract

The request contains a `bioprism-security-program/0.1` manifest with:

- `scopes` that bind a target, owner, authorization digest, finite methods, forbidden actions,
  environment, and data-handling posture;
- `campaigns` that bind a scope to an operator, separate reviewer, methodology, hypothesis, stop
  conditions, status, timestamps, and immutable evidence;
- `findings` that bind observations to campaigns, severity, evidence, reproduction, affected
  targets, remediation links, incident links, and publication safety;
- `remediations` that retain owner, action, deadline, completion verification, and bounded waiver
  approval;
- `incidents` that retain severity, containment/closure witnesses, notification obligation, and an
  increasing append-only event timeline;
- `disclosures` that retain audience, approval, advisory identity, publication time, and stage
  order; and
- explicit `controls` and `policies` for the program's non-waivable evidence layers.

The response carries a canonical manifest digest, `security_program_ready`, counts, and seven row
families: scope, campaign, finding, remediation, incident, disclosure, and control audits. Each
blocking row has a stable code, subject, detail, and remediation. This preserves the distinction
between a clean declaration and proof that a live program is operating.

## Fail-closed semantics

Scope authorization is content-addressed and finite. Wildcards and traversal-like values are
rejected from methods, guardrails, environments, and references. A completed campaign requires a
separate reviewer, bounded methodology and stop conditions, completion time, and evidence.

High and critical findings require evidence, a reproduction witness, remediation, and an incident
link. A closed finding requires a regression witness. Complete remediations require verification;
waivers require a rationale and approval digest. Incidents require a valid finding, ordered
timeline, containment evidence when contained, and closure evidence when closed.

Disclosure is sequential. Public disclosure requires an advisory predecessor, independent
approval, an advisory digest, a publication time, and a finding explicitly reviewed as safe to
publish. These checks are rows, not a single caller-provided `ready` flag.

## What this proves and does not prove

It proves only that the supplied program artifact is internally closed under the published
declaration rules. It does not run scanners, fuzzers, probes, sandboxes, or containment; inspect a
production boundary; contact a vendor; send a notification; publish an advisory; verify a
signature; or attest that a control is live. Scope authorization, evidence, timestamps, approvals,
and control states remain caller-supplied. See the typed Python and TypeScript SDK facades for
callers that need the seven row families without flattening them into a score.
