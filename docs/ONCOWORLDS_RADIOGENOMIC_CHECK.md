# OncoWorlds radiogenomic check

`oncoworlds_radiogenomic_check` is a read-only boundary audit for an imaging-to-molecular
claim. It does not extract features, fit a model, compute AUROC, or establish a biological
mechanism. It checks whether the supplied design and transport declaration are strong enough for
the sentence the caller wants to say.

## Versioned projection

Successful and refused domain outcomes use
`bioprism-mcp/oncoworlds-radiogenomic-check/0.1`. The projection keeps these states separate:

- `supported: true` and `outcome_kind: "supported"` retain the serialized supported claim.
- `supported: false` and `outcome_kind: "refused"` retain the fail-closed refusal and its
  `refusal_kind`.
- `claim_target` and `claim_statement` remain visible even when the design is refused, so an
  audit can say which sentence was blocked.

The refusal taxonomy currently includes `leaky_split`, `unstated_assumption`, `undeclared_loss`,
`specimen_scoped_target`, `unstratified_claim`, and `post_hoc_cohort_selection`.

## Design evidence

The `design` projection carries:

- the split unit (`image`, `imaging_series`, `specimen`, `participant`, or `site`);
- whether derived features were fitted on training data only or all data;
- the fixed feature version;
- any prespecified external cohort;
- declared strata; and
- whether both `site` and `scanner` strata are present for a mechanism claim.

An image-, series-, or specimen-level split can put one participant on both sides of evaluation.
Features fitted across all cases leak information even under a participant split. A mechanism
claim additionally needs the declared acquisition strata; an association claim does not get
silently upgraded into a mechanism claim.

## Scope and transport

The underlying OncoWorlds kernel promotes a positive sampled molecular call to a tumour label,
but keeps a negative specimen call specimen-scoped. The response therefore never turns a negative
fragment result into a tumour-level target. The projection also exposes the transport assumption
names and the complete required-assumption list, while the serialized supported claim retains its
loss ledger, target label, and strata.

```python
from prism_sdk import oncoworlds_radiogenomic_check_report

report = oncoworlds_radiogenomic_check_report(response)
if report.supported:
    print(report.supported_claim_record.target)
else:
    print(report.refusal_kind, report.claim_target, report.design.split_unit)
```

The typed SDK projection rejects a forged support/refusal state, unknown refusal or target kinds,
inconsistent design strata, missing supported-claim evidence, and a top-level target or statement
that disagrees with the nested supported claim.
