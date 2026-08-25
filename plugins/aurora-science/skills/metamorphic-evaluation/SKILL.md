---
name: metamorphic-evaluation
description: Grow an evaluation suite by mutation with executable postconditions, count effective diversity in equivalence classes rather than instances, and report yield honestly. Use when generating benchmark variants or augmented test cases, when a suite's instance count is quoted as evidence of coverage, when checking whether a system responds to transformations the way it should, or when a mutation or augmentation pipeline reports only its successes.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The methods themselves apply to any test generation or data augmentation effort.

# Metamorphic evaluation

When ground truth is scarce, transformations with known consequences are the next best oracle:
if renaming every subject should not change the verdict, and it does, something is wrong — in
the system or in the mutation. The discipline is in never letting a transformation into the
suite on its own say-so.

## Postconditions are executable, and they gate admission

In the workspace's mutation engine (`crates/mutation`), a mutation declares *what the oracle
should do* as a result of the transformation, and that declaration is checked by running the
oracle. The crate's own header states the admission rule: "a mutation is admitted only when the
oracle confirms its declared relation." Postconditions are typed, not prose — the verdict is
preserved, or a named witness is removed, or a named witness is added — and the relation module
draws the line that matters:

> A mutation whose postcondition fails is a defect in the mutation, not a new benchmark
> instance, and is rejected. [...] This is the difference between a mutation engine and a
> paraphrase generator.

The rejected instance is retained with what the oracle actually did, because a failing
postcondition is diagnostic evidence about your transformation code.

Declare both relation kinds where they exist. The metamorphic-response audit
(`docs/BIOEVAL_METAMORPHIC_AUDIT.md`) distinguishes invariance ("this change should not move
the response") from directional change ("this change should move it, in this named direction"),
and its finding buckets are deliberately not synonyms: a directional trial that stayed put is
**false sensitivity** (a missed response — not proof of invariance); an invariant trial that
moved is **false invariance** (a shortcut); a move against the declaration is **wrong
direction**. An incomparable observation stays **undetermined** — it is never coerced to
"unchanged" and never counted as a pass or a fail. Keep those four apart in your own reports;
collapsing them into one pass rate destroys exactly the information the method exists to
produce.

## Equivalence classes, not instance counts

An augmentation pipeline can turn one parent into a thousand instances in an afternoon, and a
thousand is not the suite's size. The engine's diversity report (`crates/mutation/src/diversity.rs`)
computes and publishes both numbers plus their quotient:

- `instances` — how many validated worlds exist;
- `equivalence_classes` — distinct (parent, mutation family, oracle signature) classes;
- `inflation_ratio` — instances divided by classes; 1.0 means every instance is independent.

The lib header carries the reason: "a million paraphrases are not a million benchmarks." Two
renamings of the same parent that leave the same oracle signature are one robustness probe run
twice, not two benchmarks. The struct also carries a `caveat` string stating how classes were
counted, because an equivalence relation is itself a modeling choice a reader must be able to
audit. Report all three numbers everywhere the suite's size is quoted; a bare instance count in
an abstract with the class count in an appendix is the inflation it pretends to disclose.

## Yield honesty

The generation report retains every attempt, partitioned: `accepted`, `rejected` (with the
declared relation and what the oracle actually did), and `duplicates` — byte-identical worlds
caught by content digest so a lazy transformation cannot double-count. The yield rate is
accepted over all attempted, and a pipeline that reports only its accepted output is hiding its
denominator.

Yield is diagnostic in both directions:

- A **low** yield says your transformations frequently break their own declared relations —
  the mutation code is buggy, or the declared relations are wrong.
- A **suspiciously perfect** yield says your postconditions may be too weak to reject anything.
  A validator that never fires is worse than none: prove it can reject by feeding it a
  transformation that genuinely violates its declared relation.
- A high **duplicate** count says your mutation space is smaller than its parameter space —
  visible only because duplicates are counted rather than silently dropped.

## Denominator discipline in the report

Two rules from the response audit worth copying verbatim into any suite-level summary:

- Consistency is computed over the **evidential denominator only** — undetermined trials are
  excluded from the rate but remain visible through their counts and identifiers. An
  all-undetermined family "is not a perfect family and it is not a zero-score family."
- **No suite-wide pass percentage.** Families represent different transformations, sample
  sizes, and questions; summing their pass rates "would silently choose weights and imply
  exchangeability that the contract does not declare." If a release gate needs one number,
  define the weighting explicitly and separately — do not let an average imply it.

## Checklist

- Every mutation declares an executable postcondition; admission requires the oracle to confirm it.
- Failed postconditions recorded as mutation defects, kept with the observed behavior.
- Instances, equivalence classes, and inflation ratio reported together, with the
  class-counting caveat.
- Yield rate over all attempts; duplicates deduplicated by content digest and counted.
- False sensitivity, false invariance, wrong direction, and undetermined kept as four distinct
  findings.
- Rates computed over evidential denominators; no implicit suite-wide aggregate.
