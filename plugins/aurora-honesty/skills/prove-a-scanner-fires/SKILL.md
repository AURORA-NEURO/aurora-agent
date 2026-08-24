---
name: prove-a-scanner-fires
description: Pair every source-scanning hygiene test with a proof that it detects a planted violation, that its walk actually opened files, and that it is no laxer than the tool it mirrors. Use when writing or reviewing any test that greps a crate's own text — citation audits, hardcoded-constant checks, imputation scans, ambient-input checks — or when a hygiene suite has been passing for a while.
---

<!-- Mirrored from .agents/skills/prove-a-scanner-fires/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->
> Note: the crate paths, file:line anchors, and case studies below are
> illustrations from the aurora-agent workspace where this pattern was
> discovered. The pattern itself applies to any codebase.

# Prove a scanner fires

Some rules cannot be carried by a type. "No file in this crate cites a module it did not implement"
and "no source line names this guarded type" are properties of *text*, and the only way to enforce
them is a test that reads the crate and greps it. This workspace has a dozen such scanners.

`crates/oraclex` articulated the rule they all live under:

> **A scanner that detects nothing is worse than no scanner.** It reports a clean bill of health
> forever, including after the rule it checks stops being true — and the passing test is what stops
> anyone from looking.

A workspace audit found **seven scanners across six crates with no proof they fired** — `bioevalx`,
`interweave`, `megafactory`, `modalities`, `oraclex` and `sweep`. They all have one now. Two of them
were genuinely broken, and neither would have been found by reading the code.

## The four things every scanner needs

### 1. A planted violation, with negative cases

Name it `<thing>_scanner_can_actually_see_one` or `the_<thing>_scanner_sees_a_planted_violation`, and
assert both directions in the same test. `crates/oraclex/tests/hygiene.rs`:

```rust
#[test]
fn the_hardcoded_constant_scanner_can_actually_see_one() {
    assert_eq!(
        numeric_constants("synthetic.rs", "const MIXTURE_FLOOR: f64 = 0.15;").len(),
        1,
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(numeric_constants("synthetic.rs", "pub const ALL: [Family; 12] = [").is_empty());
}
```

The negative case is not decoration. `[Family; 12]` is an array length and says nothing about
biology; a scanner that flags it will be allowlisted into uselessness within a month.
`crates/bioevalx/tests/plane_and_zero.rs` does the same for imputation — it fires on
`cell.score().unwrap_or(0.0)` and on a `fn score_or_zero`, and deliberately does not fire on
`label.unwrap_or("unnamed")` (*a defaulted string is not an imputed score*) or on a comment
describing the rule.

### 2. Planted ids assembled from digits, never written out

`tools/coverage.sh` greps `crates/` whole — test files included. **A citation-audit test that spells
an out-of-scope id as a literal is itself the citation the audit exists to catch.** That is not
hypothetical; `crates/sweep`'s first version listed its two declined ids as string literals in the
array asserting they must never appear.

```rust
let planted = format!("{}.{}", 32, 40);      // crates/oraclex
let planted = format!("{:02}.{:02}", 44, 21); // crates/sweep
```

If the section you are scanning is fully cited elsewhere in the workspace, no id from it can serve as
a violation. `crates/interweave` hit exactly that — every module of §23 is cited somewhere — so its
positive control uses `an_id_no_file_in_the_workspace_contains()` and runs that id through the same
decision procedure the real audit uses.

### 3. An assertion that the walk opened files

An empty walk and a clean crate produce the same verdict, and only one of them is good news. Every
scanner in this workspace now carries a floor:

```rust
assert!(
    files.len() >= SOURCES.len(),
    "the walk found {} files, which is fewer than src/ alone holds; an empty scan and a clean \
     crate look identical from here",
    files.len()
);
```

See `crates/megafactory/tests/hygiene.rs:370`, `crates/oraclex/tests/hygiene.rs:260`,
`crates/bioevalx/tests/citations.rs:129`, `crates/sweep/tests/citations.rs:222` and
`crates/interweave/tests/citations.rs:115`. Where the scanner derives a *set* rather than a count —
"which ids are owned by a sibling" — the floor has to guard against the opposite degeneration too:
`the_ownership_set_is_a_real_reading_of_the_tree_rather_than_a_rubber_stamp` asserts the set has over
a hundred ids, *contains* known sibling-owned ones, and *does not contain everything*.

### 4. Proof that the scanner's input is the whole crate

If the scanner reads a hand-maintained list of sources, that list is a second place the rule can fail,
and it fails invisibly. Assert it against the filesystem:

```rust
#[test]
fn the_baked_in_source_list_names_every_file_under_src() { /* walk src/, compare to SOURCES */ }
```

And scan the **whole crate**, not just `src/`. `coverage.sh` greps a crate whole, so `tests/`,
`examples/` and doc files are all places an id can hide. When `crates/megafactory`'s audit was widened
past `src/` it fired immediately on a literal in a doc comment — a fair demonstration that the
widening was worth doing.

## The two real failures, because neither was visible from the code

**`crates/oraclex`'s four scanners had never read a live module.** They run over a hand-written
`include_str!` list, and `verdict.rs` — 451 lines of public module — was never added to it. All four
silently skipped it and the suite reported a clean crate. This is the non-recursive-walk failure one
level worse: a directory walk that misses a subdirectory is at least fragile in a way a reader can
see, whereas a stale list is invisible from the test. It was found by writing requirement 4 above, not
by review.

**Two scanners were laxer than the tool they mirror.** `sweep` and `megafactory` both treated a
preceding full stop as blocking, on the reasoning that a three-part version string is obviously not a
citation. That reasoning is about what a *human* means, and the coverage number is not computed by a
human: `tools/coverage.sh`'s regex counts `04.04` inside `1.04.04`, so a version-shaped string in
either crate would have passed its own audit and still moved the coverage figure. **`sweep` had a test
asserting the wrong behaviour**, which is the worst case — a proof of the defect.

Three lessons from that one:

- **Mirror the tool, not your reading of the tool.** The fix was verified against real `grep` over a
  fixture rather than against an interpretation of the regex.
- **Check the negatives too.** `v04.04`, `104.04`, `04.049` and `_04.04` are correctly ignored, which
  is why the word boundary had to include underscore — a detail nobody would have derived from the
  intent.
- The corrected assertion is now a named test in three crates:
  `a_dotted_version_string_does_count_because_the_coverage_script_counts_it`.

## What a proof pass is worth

Fixing the lax scanners and the source list **surfaced no previously hidden violation**. That is the
expected outcome and it is not a wasted pass: what changed is that seven clean bills of health are now
evidence rather than assertions. `crates/interweave`'s citation discipline had been stated as a fact
with no mechanism at all, in the crate with the largest citation footprint in the workspace — its
claim survived being tested, and all 32 of its non-implemented ids are genuinely owned elsewhere. It
also corrected its own reported footprint from 42 distinct ids to 41, because it checked before
writing the figure.

Distinguish the two findings a pass like this produces, because they need different responses:

- **An unproven scanner** — pair it, as above.
- **A missing scanner** — a rule stated in prose with nothing enforcing it. `crates/modalities` had no
  citation scanner at all. That is not a failure of the scanner; it is a rule that was never
  mechanised, and it needs one written, plus a hand check in the meantime (its eight out-of-catalogue
  ids were all sibling-owned).

## Checklist

- Every scanner has a `*_can_actually_see_one` / `*_sees_a_planted_violation` partner.
- Each proof asserts a true negative as well as the positive.
- Planted ids are built with `format!` from integers, never as literals.
- The walk asserts it found files; a set-valued scanner asserts its set is neither empty nor
  everything.
- Any baked-in source list is checked against the filesystem.
- The scan covers the whole crate, matching what `tools/coverage.sh` greps.
- Where the scanner mirrors an external tool, its behaviour is verified against that tool on a
  fixture — including the cases where the tool is *less* forgiving than you would be.
