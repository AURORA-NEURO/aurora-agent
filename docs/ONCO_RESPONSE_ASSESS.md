# Oncology response assessment

`onco_response_assess` is a criteria-aware research-world assessment of one imaging timepoint. It
does not diagnose, prognosticate, recommend treatment, or triage care. Its primary safety property
is that a radiologic progression reading is not automatically a reportable progression call.

## Versioned projection

Successful assessments use `bioprism-mcp/onco-response-assess/0.1` and retain:

- `call_kind` and the human-readable `call_label`;
- the `unconfirmed_reading` before confirmation and context rules;
- criterion and treatment projections, including the post-treatment window and pseudoresponse
  possibility;
- measurement-error and threshold-sensitivity state;
- criterion divergence when a criterion-level reading was withheld; and
- surviving hypotheses, non-identifiability, and discriminating evidence requests.

`call_kind: "not_evaluable"` is distinct from `stable`. When post-treatment change remains
possible, `withheld_progression` is true and the response remains not evaluable. The SDK rejects
a forged withheld progression with a reportable call, mismatched call kind/label, divergent
hypothesis counts, or inconsistent sensitivity/divergence projections.

Invalid or unsupported measurements use the same schema with `outcome_kind: "refused"`,
`refusal_kind: "assessment_error"`, and `fail_closed: true`. This keeps malformed inputs separate
from an honest not-evaluable assessment.

```python
from prism_sdk import onco_response_report

report = onco_response_report(response)
print(report.call_kind, report.unconfirmed_reading, report.withheld_progression)
if report.hypothesis_non_identifiable:
    print(report.evidence_requests)
```
