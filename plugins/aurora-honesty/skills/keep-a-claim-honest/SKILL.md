---
name: keep-a-claim-honest
description: Find and fix the defect this workspace keeps producing — an error or a missing measurement collapsed into a benign default, so a refusal becomes indistinguishable from an answer. Use when writing or reviewing anything that compares two outcomes or feeds a published number, when auditing a crate, and whenever you are about to write unwrap_or, unwrap_or_default or unwrap_or_else on something that is not a collection.
---

<!-- Mirrored from .agents/skills/keep-a-claim-honest/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->
> Note: the crate paths, file:line anchors, and case studies below are
> illustrations from the aurora-agent workspace where this pattern was
> discovered. The pattern itself applies to any codebase.

# Keep a claim honest

AGENTS.md says honest labelling is the product: *"provably cannot matter" and "nobody checked" are
different states and must never share a representation.* Five crates have now broken that rule in the
same shape, under five different names, and the shape is worth recognising on sight.

> **An error is swallowed into a benign value.** A refusal becomes an abstention, a missing rate
> becomes `0.0`, an unscalable stress becomes an offset of zero. Downstream, the benign value is
> indistinguishable from a real measurement — and it usually compares *equal* to something, which is
> how it gets published.

The damage is never the wrong number. It is that the wrong number is well-formed, and every derived
field agrees with it.

## The five, and what each one published

**`prism::fork`** — an oracle refusal became `OracleVerdict::abstain`, which the decision cell then
rejected. A question the oracle *declined to answer* was reported as an architecture that answered it
*wrongly*, and a failed arm was indistinguishable from one that never ran. An empty panel reported
itself regression-free.

**`prism::minimize`** — the identical line, doing more damage. An abstention is a signature like any
other, so a reduction whose oracle refused at both ends compared equal to its target and reported
that it had **preserved** an answer nobody gave. On a candidate the oracle refuses outright, every
removal matches, so the minimizer would eat the world down to nothing while claiming a preserved
verdict.

**`baseline::compare`** — the same swallow in the place it does the most damage: `compare()` generates
`docs/BASELINE_COMPARISON.md`, `docs/DISCRIMINATING_COMPARISON.md` and the README table, and AGENTS.md
says that harness exists to make the central claim falsifiable. Two refusals compared *equal* —
underdetermined against underdetermined, empty witness set against empty witness set — so
`verdict_preserving` said a strategy preserved a verdict nobody obtained, `missing_witnesses` read as
"nothing decisive was dropped", and `cheapest_admissible` would name a winner. Run on a refusing world
(constructible: `crates/mutation/tests/metamorphic.rs` builds one from the shipped fixture by deleting
one subject's split arm), the old code scored **seven of ten strategies verdict-preserving,
closure-complete and admissible**, printed *"Cheapest admissible strategy: graph-5-hop at 11 facts"*,
and rendered the inadmissible rows as *"not sound: missing no witnesses"*. The inversion was total —
FIBER was the only strategy marked inadmissible, because its adapter caught its own compile refusal
and returned an empty selection. **The one world where the harness lied was a world where it lied
against the thesis.** And the shipped oracle cannot abstain at all: `OracleVerdict::new` returns only
valid or invalid, so `underdetermined` in any published comparison could only ever have been a refusal
wearing an abstention's clothes.

**`packs::health`** — `best()` returns `None` when no system has trials, and the result was defaulted
to `0.0`. Every trivial baseline then satisfied the margin, and a *blocking* `Degenerate` finding was
published carrying a fabricated best-pass-rate, for a pack nobody had run.

**`stress::perturb`** — a pooled within-class standard deviation was computed over *resolved* subjects
while validation checked *all* of them. A cohort whose positives all fell below the limit of detection
passed validation, yielded a zero spread, and reported `OffsetConfinedToBatch { offset: 0.0 }` — a
stress that could not be scaled, reported as a stress that was applied and held.

## How to hunt it

Grep for `unwrap_or`, `unwrap_or_default`, `unwrap_or_else`, and for any method named `*_or_zero`.
Then throw most of the hits away: a workspace-wide audit found **105 hits across 42 crates**, of which
80 were `unwrap_or_default` on collections and most of the numeric remainder were sparse-map lookups
where absence genuinely is zero. (Four were `crates/cookbook` quoting AGENTS.md's sentence *"There is
no score_or_zero"* — a rollout has to allowlist the crate that quotes the rule.)

The scan is the cheap filter. The discriminating question is semantic, and it is one question:

> **Does this value feed a comparison, a ranking, or a published number — and if so, what does the
> default mean to a downstream reader who cannot see this line?**

`0.0` in a sparse count vector means "none of these". `0.0` in a best-pass-rate means "the best system
we ran scored zero", which is a claim about a run that did not happen. Same literal, different lie.

`crates/bioevalx/tests/plane_and_zero.rs` encodes exactly this discrimination as a scanner:
`imputations()` flags `cell.score().unwrap_or(0.0)`, `measured.unwrap_or_default()` on an `f64` and a
`fn score_or_zero`, while deliberately passing `label.unwrap_or("unnamed")` — *a defaulted string is
not an imputed score* — and passing prose about the rule. Copy that shape rather than banning a
token.

## Prefer unrepresentable to tested

A test that asserts the collapse cannot happen is good. A type with no field to collapse into is
better, and it is what every one of the five fixes actually did.

`prism::fork::Arm` is `#[serde(tag = "state")]` over three variants, so an arm that produced no verdict
serialises with **no `passed`, `status` or `facts_exposed` key at all** — there is nothing for a
downstream renderer to coerce, default or average. `baseline::compare` does the same: `verdict_preserving`,
`missing_witnesses` and `status` live inside a `Judgement`, so a refused row has no field to set.
`prism::minimize::Minimization::unjudged` deliberately carries **no `#[serde(default)]`**, because an
absent field would deserialize as an empty list, which reads as "every removal was judged" — *a
document that does not say must not be read as a document that said no.*

Watch the zeros you keep, too. `ArmFailure::StrategyDeclined` carries no `facts_exposed`, and the
asymmetry with `OracleRefused` is the point: a refused arm paid for the facts it selected and that
count is a real measurement, whereas a declining arm selected nothing and `0` would report it as the
cheapest attempt in the panel.

## Choosing the remedy: a state or an `Err`

The three `prism`/`baseline` fixes split the same way, for a reason worth internalising.

**Use a state when an `Err` would discard measurements that were actually made.** `matched_fork`
returns arms; an `Err` would answer an arm-level question by throwing away the arms that ran, which is
the entire product of the function. `baseline::compare` is the same at row level — an `Err` on the
seventh of ten rows discards nine measured rows.

**Use an `Err` when the refused thing *is* the target, and is computed before anything else runs.**
`minimize` takes the candidate's verdict as the signature every removal must preserve; if the oracle
refuses the candidate, there is no signature, and `MinimizeError::OracleRefusedCandidate` discards
nothing because nothing has happened yet. `baseline::compare` splits identically: the *reference* is
computed before any strategy runs, so its refusal is a `CompareError`, while a *row* refusal is a
state. Per-removal refusals in `minimize` go to a third place again — a separate `UnjudgedRemoval`
list, kept out of `minimal` so that "facts a reader may cite as load-bearing" and "facts nobody could
rule out" stay two populations rather than one.

**Keep oracle-independent measurements outside the refusal.** This is `baseline`'s deliberate
departure from `fork`, and it is documented as one: `facts_exposed`, `fraction_of_world` and
`protected_recall` live in `Delivered`, outside the judgement. A refused arm in `fork` produced
nothing the cell could judge; a refused *row* still selected facts, still paid for them, and still did
or did not deliver the closure. Suppressing those would be its own dishonesty. Ask, per field,
*whether the refusal is what made this unknowable* — and only hide the ones where it is.

## Two more costumes it wears

**The collapse can be structural rather than a defaulted value.** `metrics::CapabilityBreakdown`
contained no `unwrap_or` at all. Its doc described counting wins and losses across pairs the order
could resolve; the struct had no such fields and `breakdown()` counted no pairs, so a row reading
`best: [a], unmeasured_for: [b]` was byte-identical in shape whether `a` beat three systems or stood
alone. **Removing an unmeasured system is exactly what promotes whoever remains into `best`**, so an
unevaluable comparison was silently strengthening a published claim. The fix mirrors the value case: a
`measured_for` field naming the systems `best` actually won against, plus `lead_is_uncontested()`. Its
sibling `RankInstability` had counted unevaluable perturbations in a denominator they could never
enter the numerator of, twenty lines below a doc comment saying they were not counted; it is now an
`Instability` enum — `Measured { top_changed_in, evaluated }`, `NothingEvaluable { attempted }`,
`NotPerturbable` — **with the fraction derived rather than stored, so no written-down number can
disagree with its own denominator.**

**A swallow can over-claim failure, not only success.** The FIBER strategy adapter honoured an
infallible `select` signature by returning an empty selection wherever its compiler refused, so the
harness published *"fiber is not sound: missing four witnesses"* when the compiler had declined to
produce a context at all. That was live with a `budgets.max_facts: 2` edit. Fixing it meant making
strategy selection fallible, which is where `ArmFailure::StrategyDeclined` came from — the collapse
arrived through the one door `Arm` could not watch, *because the trait promised it could not happen*.
An invariant carried by a signature rather than by a type is an invariant nobody is checking.

## Landing the fix

- Reach a state that was previously unreachable before you claim the fix works. `baseline` added a
  purpose-built strategy for this, on the reasoning that **the shipped panel cannot reach a refusal,
  and a state that exists only in prose is a state nobody has checked**. `minimize`'s
  `UnjudgedRemoval` is unreachable today because the shipped oracle is monotone under key removal —
  it is kept, with a test pinning its serialized shape, precisely so the old swallow cannot reappear
  silently the day a new check fires on a smaller set.
- Check whether any published measurement moved, and say so either way. The `baseline` fix
  regenerated both documents byte-identical, needed no README edit, and left the certificate digest
  unmoved — which is what made it safe to land as a pure correctness fix.
- Check the callers. `routing` had to be changed to fail closed on the two new cases rather than
  record an unjudged observation as a demonstrated failure, which would have moved the router's regret
  on evidence nobody obtained.
- If the same defect exists elsewhere and fixing it needs a public API change outside your brief,
  **report it with a reproduction** rather than silently leaving it. That is how three of the five
  above were found.
