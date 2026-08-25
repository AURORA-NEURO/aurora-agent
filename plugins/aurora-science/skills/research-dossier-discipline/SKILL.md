---
name: research-dossier-discipline
description: Scope what an autonomous research run may claim and what it must never claim, carry limitations verbatim through every summary layer, and treat negative findings as first-class results. Use when writing up results from an autonomous or agent-driven measurement run, when drafting a research summary, README section, or dossier that describes findings, when a synthetic-world result is at risk of being stated as a real-world claim, or when deciding whether a run's evidence supports a release-level statement.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The discipline itself applies to any autonomous or semi-autonomous research write-up.

# Research-dossier discipline

An autonomous run can produce real measurements and still produce a dishonest dossier, because
the dishonesty lives in the write-up's altitude: measurements made at one level, claims stated
at another. This skill fixes the altitude.

## What a run may claim

An autonomous measurement run over synthetic worlds may claim, at most:

- **Measurements over the worlds it actually ran** — fixtures and seeded generator families —
  identified precisely enough to regenerate (spec, seed, grid), with numbers pinned to retained
  artifacts or tests.
- **Observation- and evaluation-level statements**: strategy X was admissible in N of M cells
  of this family; these two methods selected identical fact sets on every measured world; this
  postcondition failed on these instances.
- **Properties of its own instruments**, stated as such: this benchmark cannot discriminate
  these methods; this tie is overdetermined by this construction property.

Every such claim inherits the scope of its worlds. The workspace's findings say it plainly of
their own headline table: "That is partly a property of the benchmark, not of the methods."

## What a run must never claim

This block is the honesty frame the workspace applies wherever research output is described.
Carry it into the limitations section verbatim wherever your dossier describes research:

> This is autonomous measurement science over SYNTHETIC decision worlds (fixtures and seeded
> generators). It can never claim: biology or medicine, literature or prior-work coverage,
> external-world observation, or release-level claims from fixture evidence. Oracle review is a
> human gate.

Unpacked:

- **No biology, no medicine.** The evaluation contracts enforce this mechanically — the
  reproduction check refuses to convert a matched pipeline into a validity claim ("matching a
  pipeline cannot be promoted into biological validity", `docs/EVALUATION_REPRODUCTION_CHECK.md`),
  and the audit projections each end with the boundary that they do not establish biological,
  causal, or clinical validity. Where research is described at length, include the boundary
  sentence: "Research and developer infrastructure: it does not diagnose an individual,
  recommend treatment, triage care, enroll participants, or claim medical-device functionality."
- **No literature coverage.** A run that searched nothing read nothing. "Related work" and
  "novel" are claims about the world's literature, and a dossier built from local fixtures has
  zero evidence about it. Say "not compared against prior work" rather than implying a survey.
- **No external-world observation.** Synthetic worlds are constructed to have properties;
  finding those properties confirms the generator, not nature.
- **No release-level claims from fixture evidence.** "Works on the shipped fixtures" is a
  regression statement. Readiness, robustness, and generality are claims about worlds not yet
  seen; the workspace's own tie is the cautionary case — a method that matched the compiler on
  every world measured, on a family whose construction overdetermined the match.
- **Oracle review is a human gate.** Where a pipeline's judge is itself under evaluation, or a
  verdict feeds a consequential decision, a human review is part of the method, and the dossier
  records whether it happened — a structural success is never collapsed into approval.

## Limitations travel verbatim

Limitations are load-bearing data, not tone. The workspace propagates them as values: the
certificate carries a `limitations` array inside the hashed body, so it cannot be dropped
without changing the artifact's identity; audit projections return `guarantees` and
`limitations` side by side; the findings document restates its central caveat at every summary
level rather than only in a details section. Follow the mechanics:

- Copy limitation text **verbatim** into every derived layer — abstract, README table, slide.
  Each paraphrase is an opportunity for the scope to quietly widen, and summaries are where
  dossiers lie.
- Keep a limitation next to the claim it bounds, not pooled in a section the claim's reader
  will never visit.
- When a downstream document quotes a number, it inherits the number's caveats. A sweep result
  quoted without "decision-defining knobs were not swept" is a different, stronger, and false
  claim.

## Negative findings are first-class results

The workspace's headline finding is a tie against its own flagship: the directed dependency
walk matches the compiler's selection in every measured cell, and the dossier promotes that to
the summary, bolds it in the tables, and names the experiment that could break it. That is the
posture: a run that spent its budget establishing that your method has no measurable advantage
has produced a result, and the dossier's structure must not demote it. Report negative and
positive findings in the same sections, at the same prominence, with the same digest-chaining
— and end a negative finding with the discriminating experiment it motivates, which is the
honest version of "future work."

## Checklist

- Every claim identified as measurement, instrument property, or out-of-scope — and the
  out-of-scope ones deleted or rewritten as explicit non-claims.
- The synthetic-worlds honesty frame present verbatim in limitations; the research boundary
  sentence present wherever research output is described at length.
- No biology/medicine, literature-coverage, external-validity, or release-level statements
  anywhere fixture evidence is the only support.
- Limitations adjacent to their claims and copied verbatim into every summary layer.
- Human gates (oracle review) recorded as performed or not performed, never implied.
- Negative findings in the main results, with the follow-up experiment they motivate.
