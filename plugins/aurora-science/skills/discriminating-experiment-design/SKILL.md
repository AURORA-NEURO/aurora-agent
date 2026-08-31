---
name: discriminating-experiment-design
description: Recognize when a benchmark cannot discriminate between methods, construct experiment families that could, sweep structural knobs without sweeping the decision itself, and report a tie that survives as the headline result. Use when all methods score the same on your benchmark, when designing a new evaluation world or dataset family, when tempted to vary a knob that changes what the right answer is, or when deciding how prominently to report a negative result.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The methods themselves apply to any experiment or benchmark design.

# Discriminating experiment design

A benchmark on which every serious method gets the same answer has measured the benchmark, not
the methods. The workspace hit this on its own flagship comparison and turned the recovery into a
repeatable method.

## First, notice that your benchmark cannot discriminate

On the shipped reference world, the compiler under test was matched **exactly** — the identical
eleven-fact selection, not merely the same count — by a tuned graph walk, a lexical retriever,
and a directed dependency walk (`docs/FINDINGS.md`). The findings document says what that means,
and yours should too: "That is partly a property of the benchmark, not of the methods." The
reference world sat at one corner of the structural space: distractors on a hub leaf, no relay
chain, tags that name the answer. Any method that exploits *any* of adjacency, lexical overlap,
or dependency structure lands on the same answer there.

Symptoms to check for on your own benchmark: identical selections (not just identical scores),
one structural feature that every method can exploit, and a winner that flips when a single
world-construction choice changes.

## Construct a family that could discriminate

Turn the properties your benchmark accidentally fixed into explicit parameters, holding the
decisive content and the judging oracle constant. The workspace's generator (`crates/worldgen`)
made three properties into knobs:

- **Attachment** — are distractors attached at a hub or near the target?
- **Relay depth** — is decisive evidence behind a chain of intermediate steps?
- **Tag camouflage** — do distractor labels tokenize into the protected vocabulary, or are
  they lexically distinct from it?
- plus **distractor count** as a scale axis.

Each axis is aimed at one method family's known crutch: attachment and relay depth break
undirected adjacency; camouflage breaks character-level similarity (camouflaged tags share most
of their trigrams with the query — exactly the similarity a hashed-trigram basis rewards, which
is why the embedding retriever's closure fell to 36% where BM25 held 91%). Design each knob to
break a *named* assumption, and predict in writing which family it should break before running.

Generation must be a pure function of the spec, seed included, so any cell can be reproduced
byte-identically from its coordinates.

## Sweep discipline: never sweep the decision itself

The sweep runs attachment x relay depth x tag style x distractor count — 36 cells, one seed, the
full strategy panel per cell, deterministic (same grid + seed gives a byte-identical table). But
the sweep deliberately does **not** sweep decision-defining knobs, and the code carries the
caveat; carry it verbatim into anything you build on such a sweep. Knobs like the decisive
skeleton, the event set, the protected set, the decision time, and the policy change **what the
decision is** — sweeping them would average over incomparable questions and launder that into a
single comparability table. Structural knobs vary how hard the same question is to answer;
decision knobs vary the question. Only the first kind belongs in a sweep.

## If the tie survives, the tie is the result

The discriminating family worked on three method families — graph walks lose their entire
sound-and-compact window, lexical retrieval goes right-by-luck, the embedding proxy fails harder
— and did **not** work on the fourth. The directed dependency walk ties the compiler in all 36
cells at the identical fact count. The findings document promotes this to the summary, bolded in
the tables, labelled "the headline negative result":

- **Report the tie at the same prominence as any win.** Not in a footnote; in the abstract-level
  summary, stated as a property of the method pair.
- **Say what the tie is overdetermined by.** On every swept world the protected closure alone
  already carries every decisive witness, so even a depth-zero walk is admissible — the family
  cannot yet separate the methods, and a test pins that overdetermination so it is a recorded
  fact rather than a suspicion.
- **Name the experiment that could still discriminate.** The findings name it exactly: a world
  family whose decisive evidence extends beyond the protected closure. An honest tie report ends
  with the construction that would break the tie, not with a rhetorical recovery.
- **Say what the tie does not cover.** What remained un-exercised by these worlds' verdicts
  (there: the temporal cut, the policy screen, the certificate) is stated as untested, not
  claimed as a silent advantage.

## Checklist

- Identical-output check run before claiming any method advantage on a single benchmark.
- Every accidental structural constant of the old benchmark promoted to an explicit parameter.
- Each new axis aimed at a named assumption of a named method family, with a written prediction.
- Generator pure in the spec and seed; cells reproducible from coordinates.
- Decision-defining knobs excluded from the sweep, with the caveat carried in prose.
- Surviving ties reported as headline results, with their overdetermination and the
  still-missing discriminating experiment stated.
