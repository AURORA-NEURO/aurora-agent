---
name: equal-engineering-baselines
description: Build and tune baselines with the same engineering effort as the system under test, rank them on admissibility rather than on getting the right answer, and render refusals as refusals rather than zeros. Use when writing or reviewing any benchmark or comparison table, when a baseline looks suspiciously weak, when a reported advantage depends on the baseline's untuned settings, or when a comparison harness has to represent a strategy that produced no answer at all.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The methods themselves apply to any comparison or benchmarking effort.

# Equal-engineering baselines

A comparison is only evidence if the baselines were engineered as seriously as the system being
sold. The workspace learned this from its own upstream distribution, and encoded the lesson in a
harness (`crates/baseline`) whose design decisions are worth copying anywhere.

## Tune the baseline where it wins, not where it loses

The distribution's own comparison script measured its graph baseline at depth 7 and unbounded
only — the two settings where the walk returns the entire 761-fact world. It never measured
depths 5 or 6, where the identical code returns 11 facts, matching the compiler exactly. The
published comparison therefore showed a 69x advantage that disappears entirely under equal
tuning (`docs/FINDINGS.md`, "The distribution's own baseline script is a strawman").

The method:

1. **Sweep every tuning knob the baseline has** — depth for walks, k for retrievers — and report
   the baseline at its best setting, not at the setting that flatters your system.
2. **Give each family its strongest member.** The workspace's panel runs graph walks at four
   depths, a connected-component upper bound, two lexical budgets, two embedding budgets, and an
   unbounded directed walk (`crates/baseline`, `default_panel()`), alongside full-context as the
   recall ceiling.
3. **Label proxies as proxies.** The panel's "embedding" retriever is a fixed-basis hashed-trigram
   model and every report line says so: "a lexical proxy for embedding retrieval, not a neural
   model." A baseline that impersonates a stronger method inflates your win over the real thing.
4. **If a baseline stays competitive under equal optimization, report that result.** On the
   reference world, three differently-engineered baselines select the identical eleven facts the
   compiler selects — not the same count, the identical set — and the comparison document says the
   cheapest admissible strategy is the graph walk, not the compiler.

## Rank on admissibility, not on the verdict

A strategy can reach the right answer from evidence it was never entitled to skip. The
discriminating-world report (`docs/DISCRIMINATING_COMPARISON.md`) records the trap exactly:

> `lexical-top-11` reached the correct verdict from an **incomplete protected closure** (91%).
> [...] the closure is mandatory before any relevance step, so this is a contract violation that
> guessed right, not a pass.

And `docs/FINDINGS.md` states why the ranking rule exists:

> BM25 still reaches the *correct verdict* at k=11 — but from a 91% protected closure. It dropped
> a protected fact that happened not to participate in any witness. It was right by luck. [...]
> Ranking on verdict would have crowned `lexical-top-11` here, a strategy that violated the
> contract and got away with it.

So the harness ranks on **admissibility**: the reproduced verdict with the same witnesses *and* a
complete protected closure. Define the equivalent for your domain — the inputs a decision was
entitled to see — and verify it before crediting any correct answer. A right answer from an
incomplete basis is a latent failure that this particular world happened not to punish.

## A refusal is not a zero

When a strategy produces no answer, the temptation is to record `facts: 0, verdict: wrong` — a
well-formed row every aggregate will happily consume. The workspace's harness went through this
defect and fixed it structurally: a refused row carries no `status` and no `admissible` key at
all in the JSON — **absence is semantic** — and refusals are surfaced by a dedicated `refused()`
accessor rather than folded into the losers. Two refusals must never compare equal as if two
empty answers agreed; and if the *reference* itself refuses, the whole comparison aborts
(`OracleRefusedReference`) rather than fabricating a baseline for others to beat.

Keep oracle-independent measurements outside the refusal, though: a refused strategy still
selected facts and still paid for them. Cost measurements that were genuinely made stay in the
row; only the judgement fields that the refusal made unknowable are absent.

## Cost is not score

The comparison documents end with the framing sentence, and it belongs at the end of yours:

> Facts exposed is a cost, not a score. It ranks only among verdict-preserving strategies.

Compactness is meaningless for a strategy that got the wrong answer — a walk that returns 98% of
the world *and still misses every decisive witness* is in the worst quadrant, not second place.
First partition by admissibility, then rank the admissible by cost. Never publish a single scalar
that blends the two.

## Checklist before publishing a comparison

- Every baseline swept over its own tuning knobs, best setting reported.
- Proxies labelled as proxies in the same table cell that reports their number.
- Ranking criterion is admissibility (right answer from a complete entitled basis), stated.
- Refused rows rendered as refusals with no fabricated score fields.
- Baseline wins and exact ties reported with the same prominence as your wins.
- The caveat about what the world was constructed to show, carried on the table itself.
