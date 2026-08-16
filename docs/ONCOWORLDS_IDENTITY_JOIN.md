# OncoWorlds Identity Join

`oncoworlds_identity_join` decides whether two serialized OncoWorlds artefacts may be treated as
one observation at a declared analysis unit. It is a report-producing boundary: a declined join is
successful domain evidence, not a transport error. Successful responses use:

```text
bioprism-mcp/oncoworlds-identity-join/0.1
```

## Decision record

The typed `report` binds the left and right artefact labels, analysis unit, and tagged verdict:

```json
{
  "left": "preoperative-sample",
  "right": "postoperative-sample",
  "unit": "specimen",
  "verdict": {
    "verdict": "declined",
    "reason": { "refusal": "no_identity_evidence" }
  }
}
```

`verdict_kind`, `joinable`, and `refusal_kind` are reconciled copies. The SDK rejects a response
that claims `joinable=true` while carrying a declined verdict or that supplies a refusal kind not
in the kernel’s typed refusal vocabulary.

## Evidence and epoch bridge accounting

The projection keeps crosswalk evidence explicit:

- `identity_evidence_present` and `identity_link_count` describe whether a caller supplied any
  identity links;
- `bridge_declared` and `epoch_bridge` show whether a disease-epoch bridge was supplied;
- `bridge_warrant_present` confirms that a declared bridge actually carries a non-empty warrant;
  and
- `checked_dimensions` lists the ordered identity, relation, use, lesion, epoch, and specimen
  checks the boundary evaluates.

The bridge is a review warrant, not proof that epochs are biologically interchangeable. The
identity relation and permissible-use license remain inside the caller-supplied evidence and are
not inferred from matching local identifiers.

## Refusal vocabulary

The typed refusal kinds include different participants, truncated identifiers, different lesions,
incompatible epochs, different specimens, missing identity evidence, unlicensed relations,
undeclared permissible use, missing regional provenance, and incomparable coordinates. The first
blocking refusal remains visible so the caller knows which evidence boundary must be repaired.

Python exposes `OncoIdentityJoinDecisionProjection` under `OncoIdentityJoinReport.decision_record`.
TypeScript exposes the corresponding discriminated `OncoIdentityJoinVerdictResult`. Both surfaces
retain the original nested report and preserve declined joins without silently dropping them.

## Scope and limitations

The tool consumes caller-supplied identity evidence; it does not run fingerprint, sex-chromosome,
copy-number, contamination, or regional-coordinate oracles. A join decision is an auditable
research boundary, not a clinical identity assertion.
