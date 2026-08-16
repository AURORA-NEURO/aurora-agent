# OncoWorlds methylation projections

The methylation tools preserve the distinction between a classifier result and a clinical or
biological conclusion. `oncoworlds_methylation_classify` applies the classifier's declared
threshold to calibrated scores and keeps QC, tumour-content observation, abstention, and nearest
class evidence explicit. `oncoworlds_methylation_compare` compares two version-pinned results
without deciding that an earlier classifier was simply wrong.

## Classification projection

`oncoworlds_methylation_classify` uses
`bioprism-mcp/oncoworlds-methylation-classify/0.1`. The result carries:

- classifier name, version, reference version, and reporting threshold;
- `threshold_declared` and score cardinality/class coverage;
- the tagged QC and tumour-content observations;
- `outcome_kind`: `classified`, `unclassifiable`, or `refused`;
- caveat count and nearest-class presence; and
- the tagged outcome's class, abstention reason, calibrated nearest score, and caveats.

An unclassifiable result is not converted into the nearest class. A QC failure and a score below
the classifier's threshold remain different reasons, and unobserved tumour content remains a
caveat rather than an invented cutoff. A missing reporting threshold is a typed fail-closed
refusal.

## Version comparison projection

`oncoworlds_methylation_compare` uses
`bioprism-mcp/oncoworlds-methylation-compare/0.1`. It exposes both classifier records,
`classifier_changed`, both outcome kinds, `divergence_kind`, and stable-evidence accounting.
The divergence remains one of:

- `agree`;
- `both_unclassifiable`; or
- `version_conditioned`, with the class under each version when available.

The SDK rejects mismatched divergence kinds, invalid agreement/both-unclassifiable payloads,
and forged stable-evidence counts. It does not manufacture an ontology mapping across classifier
versions.

```python
from prism_sdk import (
    oncoworlds_methylation_classify_report,
    oncoworlds_methylation_compare_report,
)

classification = oncoworlds_methylation_classify_report(response)
print(classification.outcome_kind, classification.classifier.version)

comparison = oncoworlds_methylation_compare_report(comparison_response)
print(comparison.divergence_kind, comparison.classifier_changed)
```
