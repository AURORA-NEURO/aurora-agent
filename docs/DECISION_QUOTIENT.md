# Decision-equivalence quotient

`bioprism-epistemic::decision_equivalence_quotient` implements the explicit-contract half of
blueprint 43.10. It removes distinctions between compatible models only when the caller has said
which actions may be taken and supplied the loss of those actions under every model.

The relation is exact and intentionally narrow:

```text
profile(m) = [ loss(a, m) - min permitted_loss(m) ] for a in permitted_actions
m₁ ~ m₂  iff  profile(m₁) == profile(m₂) bit-for-bit
```

Subtracting the model-local minimum makes the profile invariant to an additive baseline that is
constant across the actions for that model. Such a baseline changes absolute expected loss but
cannot change the permitted-action ordering or regret profile. Exact IEEE-754 bit comparison keeps
the relation transitive and replayable; a pairwise “within epsilon” comparison would not necessarily
be transitive and therefore would not define a quotient.

## What the result carries

Every projection contains:

- the canonical permitted action set;
- the original, quotient, and merged model counts;
- a model-to-class map, so class numbers never erase identity;
- each class’s lexical representative and complete member list;
- the loss-difference profile and tied preferred actions for the class;
- an explicit schema and basis name.

The MCP tool is `epistemic_decision_quotient`. It accepts:

```json
{
  "problem": {
    "actions": ["accept", "defer", "reject"],
    "models": ["m-a", "m-b", "m-c"],
    "loss": [0, 7, 0, 4, 11, 5, 8, 15, 8]
  },
  "permitted_actions": ["reject", "accept", "defer"]
}
```

The loss matrix is row-major by action. In this example `m-a` has profile `[0,4,8]` and `m-b`
has `[0,4,8]` after subtracting its shifted baseline, so those models merge. `m-c` has a
different profile and remains separate. The action list is returned in canonical lexical order,
regardless of the order supplied by the caller.

## Refusal boundary

The kernel refuses an empty action set, duplicate action names, unknown action names, malformed
loss shapes, duplicate model/action identifiers, and non-finite losses. The MCP route validates a
deserialized problem before any matrix indexing, so malformed JSON is a structured boundary error,
not a panic or a fabricated empty quotient.

The quotient does not claim:

- causal, biological, clinical, predictive, likelihood, or scientific equivalence;
- equivalence for actions outside the permitted set;
- preservation of absolute expected loss or model probabilities;
- that a merged model is the same mechanism, subject, population, or intervention response.

## Relationship to FIBER wire queries

`fiber-query/0.1` and `fiber-query/0.2` carry neither `permitted_actions` nor `decision_loss`, so
those queries continue to report the decision quotient as deferred. `fiber-query/0.3` is the
versioned executable decision boundary: it makes both fields mandatory, bounds and validates the
matrix, converts utility to loss once, and invokes the same kernel from `bioprism-fiber`.
`fiber-query/0.4` extends that boundary with the observed prior/evidence inputs and executes the
rate-distortion audit described in
[`FIBER_RATE_DISTORTION.md`](FIBER_RATE_DISTORTION.md). The complete raw query is already
included in `source_hashes.query_sha256`, so changing any decision or evidence binding changes
certificate identity.

Python exposes `EpistemicDecisionQuotientArgs` and
`epistemic_decision_quotient_report` over local MCP, sync HTTP, and async HTTP. TypeScript exposes
`epistemicDecisionQuotient` with the same input and projection names.
