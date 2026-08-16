# Sandbox admission audit

`sandbox_admission_audit` is a bounded, deterministic admission contract for untrusted code and
research artifacts. It makes the execution boundary inspectable before a runtime is launched. The
route audits declarations; it never executes the artifact or claims that an external runtime
enforced the declaration.

## Wire contract

The request contains a `bioprism-sandbox/0.1` manifest with:

- content-addressed `artifacts`, each naming its kind, source, producer, trust tier, and parent
  inputs;
- `profiles`, each binding one artifact to a pinned runtime image/environment, non-root identity,
  rootless/read-only/no-escalation settings, network mode, mount set, capability IDs, resource
  ceilings, quarantine, and release-review policy;
- exact `capabilities` with a profile, capability kind, bounded target, allow/deny decision, and
  evidence digest for dangerous capabilities;
- produced `outputs` with profile/artifact references, digest, parent lineage, quarantine state,
  release state, destination, and independent review evidence;
- explicit fail-closed `policies` for digests, lineage, isolation, networking, resources,
  quarantine, reproducible environments, and output review.

The response carries a canonical manifest digest, `sandbox_ready`, counts, and six independent
row families: artifact, profile, capability, boundary, resource, and output audits. Blocking issue
rows retain a stable code, subject, detail, and remediation. A valid audit is therefore useful to
an admission controller without becoming an opaque boolean.

## Admission semantics

Artifact identity is a prerequisite. Missing or malformed digests, unknown parent inputs, absent
producer/source identity, and missing derived-artifact lineage block admission. Untrusted and
internal artifacts are still allowed to proceed when the profile supplies the required hardening;
trust labels never waive the hardening rules.

Profiles require non-root rootless execution, a read-only root, no privilege escalation, pinned
image and environment digests, positive CPU/memory/wall-time/process/output ceilings, and explicit
output quarantine/review. Networking is either denied or a finite allowlist; unrestricted,
wildcard, broad-CIDR, and contradictory modes are blocking. Mounts can only name declared
content-addressed artifacts and private normalized sandbox targets. A read-write mount must have
an exact matching approved filesystem-write capability.

Capabilities are not inferred from the runtime or mount list. Dangerous kinds—including network,
secret, process, device, kernel, write, and publish capabilities—need exact targets and an
evidence digest. A profile may reference only capabilities that belong to it and are explicitly
allowed. Denied or cross-profile capabilities cannot be smuggled in through a profile reference.

Outputs retain lineage and content identity. Released output must first be quarantined and must
carry independent review evidence. The audit does not treat “reviewed” as a signature, does not
dereference a destination, and does not publish output.

## What this proves and does not prove

It proves only that the supplied admission artifact is internally closed under the published
rules. It does not execute code, mount a filesystem, open a socket, inspect the kernel, read or
revoke a credential, scan a container, operate quarantine storage, notify an operator, or test a
runtime's enforcement. Those are external controls and remain visible limitations rather than
invented evidence. See the typed Python and TypeScript SDK facades for callers that need to retain
the six row families without flattening them into a readiness score.
