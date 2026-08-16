# Release-pipeline audit

`release_pipeline_audit` is the artifact-level contract for release delivery. It complements the
review-only CI plan produced by `developer_workbench`: the workbench describes a bounded set of
checks, while this route audits whether a declared release pipeline has enough internal structure
to be considered coherent and release-ready.

The route is deliberately a plan audit, not a CI runner or deployment controller. A valid result
means that the submitted declaration is closed, digest-shaped, and policy-consistent. It does not
mean that a command ran, an artifact exists in a registry, a signature is cryptographically valid,
an approval was issued by the named person, or a production deployment succeeded.

## Wire contract

The manifest schema is `bioprism-release-pipeline/0.1`; the audit schema is
`bioprism-release-pipeline-audit/0.1`. The MCP input is:

```json
{
  "manifest": {
    "schema": "bioprism-release-pipeline/0.1",
    "project": {
      "id": "aurora-agent",
      "version": "0.1.0",
      "repository": "github.com/AURORA-NEURO/aurora-agent"
    },
    "source": {
      "ref_name": "main",
      "commit_digest": "<64 hexadecimal characters>",
      "workflow": "release.yml"
    },
    "environments": [],
    "stages": [],
    "artifacts": [],
    "attestations": [],
    "promotions": [],
    "policies": {
      "require_stage_dag": true,
      "require_provenance": true,
      "require_production_signature": true,
      "require_protected_production": true,
      "require_rollback": true,
      "require_approval": true
    }
  }
}
```

Every repeated surface is bounded before deserialization: 256 environments, 4,096 stages,
8,192 artifacts, 16,384 attestations, 4,096 promotions, and 16,384 entries in a repeated list.
The serialized manifest is limited to 20,000,000 bytes and individual text fields to 4,096 UTF-8
bytes. These are transport and resource bounds, not evidence that an external system accepted the
manifest.

## What is checked

The audit keeps distinct evidence layers so one green-looking field cannot hide a missing layer.

### Source and identity

- schema is exactly `bioprism-release-pipeline/0.1`;
- project id, version, repository, source ref, workflow, and source commit digest are non-empty;
- the source commit digest is exactly 64 hexadecimal characters.

### Environments

Each environment has an id and an explicit class: `development`, `staging`, or `production`.
Production is blocking when the default policy requires protection but `protected` is false, or
when its positive approval floor is absent. Mutable production artifacts produce a warning because
digest-addressed immutable artifacts are the safer promotion boundary.

### Stage graph

Stages have unique ids, declared environments, optional provider-owned commands, and explicit
artifact outputs. Dependency references must resolve, self-dependencies are blocking, and the
stage graph is returned as a deterministic topological order. Cycles are returned in
`cyclic_stages` and make the audit invalid when `require_stage_dag` is enabled. Each stage also
gets a readiness row (`ready_to_schedule`, `blocked`, or `cyclic`) with its unresolved dependency
ids.

### Artifact lineage

Every artifact declares a kind, a 64-character digest, the stage that produced it, input artifact
ids, attestation ids, and an immutability declaration. Producer, input, and attestation references
must close over the manifest. A self-input is rejected. The artifact audit reports digest validity,
producer validity, input validity, attestation validity, provenance presence, and signature
presence independently.

### Attestation binding

Attestations declare one of `test`, `provenance`, `signature`, or `approval`, plus an artifact id,
the artifact digest, issuer, and human-auditable statement. The digest must equal the named
artifact's digest. This proves a declared binding only; it does not verify a signature or authenticate
an issuer. Required promotion attestations must exist, be digest-shaped, and name an artifact in
that promotion.

### Promotions and rollback

An `advance` must move to a higher environment class; a `rollback` must move lower. Both must name
explicit artifacts and resolve all required attestation and approval references. Production targets
must remain protected under policy. Production advances additionally require:

1. a signature-bearing attestation on every promoted artifact;
2. enough approval attestations to meet the target environment's approval floor; and
3. a `rollback_target` that resolves to a declared rollback promotion.

The audit reports missing attestations, missing approvals, production status, and rollback
presence on every promotion. The rollback target itself is checked for kind and existence; the
route does not execute or test the rollback.

## Reading the result

The top-level `ok` reports route execution. It should be true even when the submitted manifest is
invalid, because a well-formed negative audit is useful evidence. `valid` and `release_ready` are
derived from blocking issues. `blocking_issue_count` and `warning_count` are redundant summary
fields for dashboards; callers should retain the detailed `audit.issues` rows.

The `guarantees` array states what the route actually checked. The `limitations` array is part of
the contract and should travel with the result. In particular, neither a valid audit nor a
signature-shaped attestation authorizes an external action.

## SDK surfaces

Python exposes `ReleasePipelineManifestArgs`, typed nested argument classes, and
`ReleasePipelineAuditReport` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
`AsyncApiClient`. The report preserves stage readiness, artifact audits, promotion audits,
blockers, warnings, production promotions, digest, guarantees, and limitations.

TypeScript exposes `ReleasePipelineManifestArgs`, the nested release-pipeline argument/result
interfaces, and `ApiClient.releasePipelineAudit`. Both SDKs perform bounded authoring validation
but leave canonical audit semantics to the Rust authority.

## Non-execution boundary

This route does not read the checkout, run a command, contact GitHub or another CI provider, query
a container/package registry, verify a cryptographic signature, authenticate an approver, sign an
artifact, deploy to an environment, or mutate rollback state. Those capabilities require separate
external adapters and explicit authority. Keeping them out of this route prevents a static
declaration from being confused with observed delivery evidence.
