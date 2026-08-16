# Oncology Classification Check

`onco_classification_check` applies the bounded integrated molecular criteria table to a
serialized histology and marker panel. It does not infer uncollected assays, expand beyond the
implemented criteria table, or turn a provisional state into a diagnosis. Successful responses
use:

```text
bioprism-mcp/onco-classification-check/0.1
```

## Mutually exclusive resolution states

`resolution_kind` and the tagged `resolution.resolution` field identify one of five states:

| State | Required evidence | Clinical meaning |
| --- | --- | --- |
| `integrated` | One `entity`, `grade`, and satisfied `evidence` list | All implemented criteria for one entity are satisfied |
| `provisional` | One `candidate` and prioritized `obligations` | One candidate survives, but evidence is incomplete |
| `unresolved` | Multiple `candidates` and prioritized `obligations` | Several candidates survive without an integrated call |
| `mixed` | Multiple `candidates` and no obligations | More than one entity is fully satisfied; the criteria table is discordant |
| `not_otherwise_resolved` | `histology` and `excluded` entities | No candidate remains in the implemented scope |

The Python SDK parses these variants as `OncoClassificationResolutionProjection` and rejects
variant-inconsistent fields, such as an entity on an unresolved result or obligations on a mixed
result. `is_integrated` and `entity` are derived summary claims and must reconcile with the tagged
resolution.

## Evidence state is not a boolean

Marker observations use the kernel’s tagged `Observed` representation:

```json
{ "value": "present" }
```

or:

```json
{ "unobserved": "not_collected" }
```

The latter is not a negative marker call. The SDK accepts the complete closed status vocabulary:
`missing`, `not_collected`, `technically_failed`, `below_detection`, `not_applicable`, and
`redacted`. `OncoMarkerObservationProjection` retains whether a call was observed and the exact
call/status separately.

## Obligations and panel accounting

Each `obligations[]` row carries a marker, evidence role, current observation state, and
`discriminates` count. Obligations are emitted in the criteria engine’s priority order and are
retained as typed `OncoClassificationObligationProjection` records. Integrated results instead
carry `evidence[]` rows with satisfied marker calls.

`panel_state_count`, `observed_panel_state_count`, `unobserved_panel_state_count`, and
`obligation_count` are reconciliation fields. The Python parser verifies every count, every panel
row, and the equality between the top-level obligations and the tagged resolution’s obligations.
This prevents a transport adapter from dropping unobserved panel states or fabricating an empty
obligation list while leaving a plausible unresolved label.

## SDK surfaces

Python callers receive typed resolution, obligation, satisfied-evidence, and panel-state records
through `OncoClassificationReport`. The original `resolution`, `obligations`, and `panel_states`
mappings remain available for forward-compatible fields. TypeScript exposes discriminated
`OncoClassificationResolutionResult` variants and typed marker observation, obligation, evidence,
and panel-state records.

The projection is transport-agnostic across direct MCP structured content, HTTP responses, and JSON
text blocks. An unresolved or provisional domain result is successful evidence, not a transport
error; the parser preserves that state without manufacturing an entity.

## Scope and limitations

This is the deliberately bounded worked criteria table shipped by `bioprism-onco`. It does not
infer methylation class, fusion, purity, lower-grade histologic evidence, or an ontology-version
robustness result. It is a research classification projection, not a clinical recommendation.
