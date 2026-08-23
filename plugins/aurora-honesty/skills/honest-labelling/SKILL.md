---
name: honest-labelling
description: Apply the honest-labelling discipline when designing APIs, reporting results, writing benchmarks, or reviewing code that publishes numbers or claims. Use when a value could mean either "measured as zero" or "never measured", when a success flag could hide an unchecked path, when reporting benchmark or evaluation results, or when a review asks whether a system's claims match what it actually did.
---

# Honest labelling

A system that tells you what it included is a retrieval library. A system that
tells you **what it left out, and whether that could have mattered**, is worth
trusting. These rules came out of the AURORA Agent workspace, where honest
labelling is the product; they apply anywhere claims are produced.

## The non-negotiables

1. **Zero influence is not unknown influence.** "Provably cannot matter" and
   "nobody checked" are different states and must never share a
   representation. A single unknown-influence group voids a sufficiency claim.
2. **Unmeasured is not zero.** A capability with no evidence is `Unmeasured`,
   categorically distinct from measured-and-poor. There is no `score_or_zero`.
3. **A right answer from an incomplete basis is not a pass.** Verify the
   inputs a decision was entitled to see before crediting the decision.
   A strategy must not be credited for guessing correctly from evidence it
   never had.
4. **Instance count is not benchmark count.** Report independent equivalence
   classes; a million paraphrases are a robustness check, not a million
   benchmarks.
5. **Name what is not implemented.** A missing capability that is stated is a
   limitation; one that is implied to exist is a lie. Keep an explicit list
   per module.

## Make the rule unbreakable, not just tested

Where a rule can be made unrepresentable in the type system, make it
unrepresentable. Patterns that shipped:

| Rule | Enforcement |
|---|---|
| A budget cannot be duplicated | the budget type does not implement Clone/Copy |
| An unmeasured capability has no score | private fields + one gated constructor |
| Provenance cannot be forged | fields private with an internal seal; serialize-only |
| Replay cannot fall through to live | the replay host has no live-source field, so no such branch exists |
| A state needs human approval | `approve()` is the only path to the approved type |
| Progression needs confirmation | the variant carries a token only the confirmation gate can mint |

A test that asserts a rule is good. A type that makes the rule unbreakable is
better.

## Reporting rules

- Render refusals, previews, and not-run states verbatim (`dispatch:
  "not_started"`, `readiness_claimed: false`, "passes not run: <reason>").
  Never collapse a structural success into a green checkmark.
- Ship negative results. A measurement that comes out against your thesis and
  lives in the test suite is worth more than ten favourable slides.
- Tests state their claim in the name
  (`a_budget_smaller_than_the_closure_fails_rather_than_truncating`, not
  `test_budget_2`). Smoke tests that assert nothing inflate the count and are
  worse than no test.

## Vocabulary that keeps claims precise

- **Witness** — a concrete checkable object, never a score.
- **Protected closure** — the set of inputs a decision is entitled to see;
  mandatory before any relevance filtering.
- **Certificate** — a receipt stating what was omitted and whether it could
  have changed the outcome, verifiable without the engine that produced it.
