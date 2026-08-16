# OncoWorlds model-system transport

`oncoworlds_model_transport` audits whether a result observed in an organoid or patient-derived
xenograft can carry a declared, lossy research claim toward patients. It does not turn a model
effect into clinical evidence and does not compute fidelity scores, response estimates, or
engraftment predictions.

## Versioned projection

Both supported and refused outcomes use
`bioprism-mcp/oncoworlds-model-transport/0.1`. A supported result has
`supported: true` and `outcome_kind: "supported"`; a blocked result has
`supported: false`, `outcome_kind: "refused"`, and a typed `refusal_kind`.

The refusal taxonomy includes:

- `unverified_model_identity`;
- `unmeasured_fidelity`;
- `unmodelled_establishment_selection`;
- `technical_replicates_as_biological`;
- `undeclared_loss`; and
- `unstated_assumption`.

Refusals retain the model-side evidence that was available, so a failed patient transport does
not erase the model effect or make the failure look like an absent experiment.

## Evidence projection

The response separates the boundaries that are often collapsed into one “n” or one fidelity
claim:

- `model_identity` names the model system, source specimen, passage, and source verification;
- `fidelity_axes` records each required axis at the exact passage where it was measured;
- `establishment` retains attempted, established, selection-modelled, and selected state;
- `replicates` distinguishes technical wells from independent biological replicates and carries
  `claimed_n`; and
- `transport_assumption_names` is compared with the complete `required_assumptions` list.

The nested supported claim keeps the original model result, establishment cohort, declared loss
ledger, assumptions, and claimed sample size. This means a downstream caller can tell whether a
claim is supported, which boundary was first refused, and what evidence remains model-scoped.

```python
from prism_sdk import oncoworlds_model_transport_report

report = oncoworlds_model_transport_report(response)
if report.supported:
    print(report.replicates.effective_biological_n)
else:
    print(report.refusal_kind, report.model_identity.verified_against_source)
```

The typed SDK projection rejects unknown schema/refusal/outcome kinds, mismatched support state,
effective sample sizes that disagree with biological replicates, establishment counts that cannot
describe selection, missing versioned evidence, and refusal kinds that disagree with the nested
typed refusal object.
