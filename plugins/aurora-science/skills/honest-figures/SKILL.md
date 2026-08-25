---
name: honest-figures
description: Make every figure and table verifiable and non-misleading - source digests attached, refusals and absences drawn as such rather than as zeros, ties and negative results at equal visual prominence, no truncated-axis emphasis, and captions that state what the figure cannot show. Use when producing any figure, table, dashboard, or summary visualization from measured data, when rendering results that include refused or unmeasured cells, or when reviewing a figure that makes a system look good.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The methods themselves apply to any figure or table built from measured data.

# Honest figures

A figure is a claim with better production values. Everything the workspace enforces about
claims — verifiability, refusal-honesty, negative results as results — applies with more force
to figures, because figures are what readers remember and what gets screenshotted out of
context.

## Every figure carries its source digest

A figure should be reproducible from a retained computation, and should say so on its face:

- Derive figures from digest-bearing artifacts — a certified output's own content hash, a
  deterministic sweep table (same grid + seed gives byte-identical bytes, asserted by test), a
  fixture with a pinned digest. Print the digest and the generating command in the caption or
  the margin.
- Make regeneration a test with the claim in its name —
  `a_sweep_figure_is_byte_stable_for_a_fixed_table` — so a figure that drifts from its data
  fails a build rather than surviving as a stale image.
- The workspace's findings document applies the same rule to tables: every number is asserted
  by a named test, and where a table is transcribed rather than pinned, the document says which
  rows are *reproduced, not pinned* and names the command that reprints the source table. A
  transcription that does not disclose it is a transcription that cannot be audited.

## Refused and absent are drawn as refused and absent, never as zero

The comparison harness's JSON omits the judgement keys entirely on a refused row — absence is
semantic — and the scoring plane keeps `scored`, `unscored` (with a reason), and `inapplicable`
as three distinct states, where an inapplicable cell "is not a zero and does not lower a
fixed-input model's average for an action it was never designed to take"
(`docs/BIOEVAL_PLANE_AUDIT.md`). A renderer is where that discipline usually dies: a chart
library coerces a missing value to zero, and a refusal becomes the shortest bar — visually,
the *cheapest* attempt.

Rules for the rendering layer:

- A refused, unmeasured, or inapplicable cell gets its own visual encoding (hatching, an
  explicit "refused"/"not measured" mark, a gap) — never the zero position on a value axis.
- The sweep summary's "mean facts when admissible" column prints an em dash for a strategy
  admissible in zero cells; a zero there would be a fabricated measurement of a run that never
  qualified.
- Bounded displays must show their truncation: the audit projections attach `total`, returned,
  and `omitted` counts to every bounded list precisely so "an empty returned witness list
  cannot be mistaken for no findings." A top-N figure without its N and its omitted count is a
  claim of completeness it cannot back.

## Ties and negative results get equal visual prominence

In `docs/FINDINGS.md`, the strategy that ties the compiler is bolded in every table exactly as
the compiler is, and the tie is labelled the headline negative result. That is the standard: if
a competitor ties or beats you, it appears with the same weight — same emphasis, same position
in the visual hierarchy — as your wins. The dishonest alternatives are familiar: the tie in a
lighter shade, the losing configuration cropped out, the negative panel exiled to supplementary
material. A reader should be able to reconstruct "who won" from the figure's emphasis alone and
get the same answer the data gives.

## No truncated-axis emphasis

A value axis that starts anywhere but the natural floor manufactures a difference. The
workspace's central contrast — eleven facts against 750 — needs no help; differences that need
a truncated axis to be visible are differences a caption should call small. If a zoomed panel
is genuinely required, pair it with the full-range panel and label both ranges explicitly. The
same rule covers its relatives: log scales that go unmentioned in the caption, and category
orderings chosen so your system lands at the visually privileged position.

## Captions state what the figure cannot show

Both comparison documents end with a scope sentence, and it is the model for captions:

> This world is constructed to expose hub expansion; it demonstrates compiler mechanics, not
> universal superiority.

A caption that only describes what is plotted lets the reader assume the figure generalizes. A
complete caption carries: what the data is (which world family, which seed), what was held
fixed, what the figure was constructed to expose, and the nearest claim it does *not* support.
For the workspace's sweep figure that last clause is precise: the family cannot separate the
compiler from the directed walk, because on every swept world the protected closure is already
decision-sufficient. The caption is where that boundary lives or is lost.

## Checklist

- Source digest and generating command on or beside the figure; regeneration pinned by a
  byte-stability test.
- Refused / unmeasured / inapplicable cells encoded as such; nothing coerced to zero.
- Truncated lists show total and omitted counts.
- Ties and losses rendered at the same visual weight as wins.
- Value axes from the natural floor; any zoom paired with full range and labelled.
- Caption states construction, scope, and the nearest unsupported claim.
