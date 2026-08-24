---
name: measure-section-boilerplate
description: (aurora-agent workspace only) Measure how much of a blueprint section is repeated template, in a way that another agent can reproduce and that will not disagree with the figure already recorded. Use when reporting how repetitive a section is, when your figure does not match docs/COVERAGE.md, or when deciding how much distinguishing content a batch of modules actually contains.
---

<!-- Mirrored from .agents/skills/measure-section-boilerplate/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->

# Measure section boilerplate

Sections of this blueprint are heavily repetitive. §42 carries about five distinguishing lines per
module; §23 carries about fifty-four. That difference is the whole reason a coverage percentage is
not a progress bar — twenty modules of one section are not twenty modules of another — so every
crate that builds against a section is expected to measure it.

**Agents measuring the same section have disagreed three times.** Every disagreement was run down to
a specific unstated definition, and none was a mistake. The resolutions are in `docs/COVERAGE.md`
under "Three disagreements, three distinct causes". Read them before you measure, because you are
about to make the same three choices.

## State the method before the number

A percentage with no method attached is not reproducible and will start a fourth disagreement. State
all six of these, every time. `crates/megafactory/src/lib.rs` and `crates/sweep/src/lib.rs` are the
worked examples.

1. **Corpus** — which files. Exclude `00_SECTION_INDEX`. Say the count.
2. **Unit** — line, heading-block, or character.
3. **Normalisation** — trailing whitespace trimmed, and anything else you did.
4. **Threshold** — a line is shared when it occurs in at least *k* of *n* modules. Give *k*.
5. **Blank lines** — in or out.
6. **YAML front matter** — in or out.

Then print the numerator and denominator, not just the ratio. `crates/atlasx` could not land on §33's
published 75.4% under any variant it tried — it came out 0.1 point above, about two lines in 1,210 —
so it printed both figures in `lib.rs` to make the gap checkable rather than arguable. That is the
right response to an irreducible discrepancy.

Write percentages to **one decimal place**. Two decimals produce a token `tools/coverage.sh` reads as
a blueprint citation; see the `classify-blueprint-modules` skill.

## Run the sensitivity, not just the measurement

Three knobs. Report what each does, because which one moves is itself the finding.

**Threshold is usually inert, and the reason is structural.** Sharing in most sections is bimodal: a
line is either in exactly one module or in all of them, with almost nothing between.
`crates/worldfactory` found §27 gives 53.6% at *k*=22, *k*=20 and *k*=12 alike — 539 distinct lines in
exactly one module, 28 in all twenty-two, six anywhere between. `crates/megafactory` found §35
identical at *k* = 50%, 80% and 100%. `crates/bioevalx` found §26 and §07 identical at 50, 80 and
100%. Five of `crates/sweep`'s six sections do not move by a tenth of a point between *t*=0.3 and
*t*=0.9.

Threshold bites in exactly one condition: **a corpus split at the file level.** §39 is the case in
`docs/COVERAGE.md` — 13 of 25 modules use the skeleton and the rest are free prose, so the answer is
decided by where the threshold falls relative to 13/25, and the reported figure swings from 30.6% to
6.5%. `crates/sweep` found §10 is the same regime in miniature: four of its twenty-two modules desert
the template the other eighteen follow, and it is the only one of its six sections where threshold
moves anything — six points between *t*=0.7 and *t*=0.9.

**Unit often dominates.** Line versus heading-block costs about ten points in §31 and §32 and about
twenty in `sweep`'s six, because the shared blocks ("Invariants", "Failure modes and mitigations",
"Testing strategy", "Metrics", "Implementation sequence") are byte-identical across a section and
count once each as a block and many times each as lines. Character versus line can go the other way:
§34's character figure is eight points *above* its line figure, because what varies between its
modules is short bullets while what repeats is an eight-step flow, a trust list and a JSON object —
by weight it is 82% the same document twenty-three times.

**Definition matters in proportion to the share of the document the definitional slice covers.** This
is `crates/sweep`'s correction, and it is arithmetic rather than editorial. Front matter moved §43 by
15.1 points and §23 by 4.3, but only 1.8 to 2.5 points across six sections whose modules run about
seventy non-blank lines — seven front-matter lines *cannot* move that fraction further. The rule
predicts §43's swing from its module length rather than from anything about its content, which makes
it testable.

## The three causes, which are the three ways to be wrong

### 1. YAML front matter — §23, worth 4.3 points

Three agents independently measured §23's verbatim duplication over the same 6,001 lines, all
describing the metric the same way, and reported **16.2%, 10.7% and 10.4%**. A fourth found the
cause: seven front-matter lines per file across fifty files, five of them byte-identical, contribute
250 duplicated line occurrences. Counting front matter gives about 16%; stripping it gives about 12%.
Recomputed independently: **967 of 6,001 (16.1%)** as written and **667 of 5,651 (11.8%)** stripped.
Nobody was wrong; the corpus was never agreed.

### 2. A corpus split at the file level — §39, where the threshold decides the answer

Covered above. When you see two figures that are far apart and threshold-sensitive, check whether the
section's modules come in two populations before looking anywhere else.

### 3. Blank lines counted as shared content — §32, 79.3% against 73.4%

`crates/stress` reported 79.3%; `crates/oraclex` could reach at most 73.4% and recomputed the
original exactly: §32 has 2,147 raw lines of which 1,702 are shared by all 23 modules — **79.3%
exactly — and 483 of those 1,702 are blank.**

**Whitespace is identical across every file by construction**, so counting it inflates any section by
its own whitespace density: about 22% of §32's lines. `crates/megafactory` confirmed the same cause
independently on a second section one batch later, reproducing both figures for §35 to the digit:
936 of 1,195 non-blank is 78.3%, and 1,206 of 1,465 with blanks is 82.3%. The distinguishing-line
range of 14 to 16 reproduced either way.

This is the one to guard against generally. Blank lines are shared under any duplication rule and
tell you nothing.

## Report the numbers that survive

A percentage is a band with a method attached. Two numbers are sturdier and belong in every report:

- **Distinguishing lines per module.** It reproduced across every method disagreement above — §35's
  14 to 16 held with blanks in or out, and §23's ~54 median is what actually justifies the claim that
  §23 is the exception. This is the column `docs/COVERAGE.md` tells readers to use.
- **Distinct shared strings, not shared occurrences.** Only **2.1% of §23's distinct line-strings**
  are shared at all, stable at 1.6 to 2.1% across four filters, and those few strings account for the
  whole 16%. A section is not repetitive because it contains a lot of repeated text; it is repetitive
  because a small template is stamped many times, and counting distinct strings sees that directly
  while counting occurrences sees it through the size of the template. §11's shared core is 1,125
  line instances but only **40 distinct texts**; §19's is 66 instances from **2** — a horizontal rule
  and a date.

Also worth reporting when it differs from the line figure: the heading count. Only 11.6% of §23's 802
headings recur verbatim.

## Do not compare rows

`docs/COVERAGE.md`'s boilerplate table was produced by different agents using different methods and
is explicitly **not a single scale**. Only §23 was measured three ways, and its own two headline
numbers differ by 35 points. Read the column as evidence that a section is repetitive or is not, and
distrust small differences between rows. If your figure disagrees with a recorded one, the useful
output is not a corrected percentage — it is the named cause, added to the three above.

## Before you write it down

- Can another agent reproduce your number from your stated method alone?
- Did you run all three sensitivities, and say which moved?
- Are blanks and front matter each explicitly in or out?
- Is the numerator and the denominator printed?
- Is the percentage to one decimal place?
- Did you report distinguishing lines per module?
