---
name: verify-crate
description: Verify a bioprism crate is genuinely green — tests passing and clippy at zero warnings — and recognise the environment failures that masquerade as test failures. Use before claiming a crate is done, after landing an agent's work, when a test count looks wrong or lower than expected, or when `cargo test` reports a failure with no failing test named.
---

# Verify a crate

Green means both of these, per crate:

```bash
cargo test -p bioprism-<crate> --offline
cargo clippy -p bioprism-<crate> --all-targets --offline
```

Zero failures, zero warnings. `--offline` is not optional — this workspace builds against pinned
versions with no network, and omitting it produces a confusing registry error rather than a build.

## Do not trust a reported test count without running it

Agents and humans both miscount. Run the suite yourself and sum the `test result` lines:

```bash
cargo test -p bioprism-<crate> --offline 2>&1 | grep -E "^test result: ok" | awk '{s+=$4} END {print s}'
```

## The failure that is not a failure

Windows Application Control sometimes blocks a freshly linked test binary:

```
error: test failed, to rerun pass `-p bioprism-<crate> --test <suite>`
Caused by:
  could not execute process ... (never executed)
Caused by:
  An Application Control policy has blocked this file. (os error 4551)
```

**A suite that never ran looks exactly like a suite that failed.** Worse, `cargo test` continues to
the next binary, so a naive sum silently under-reports — this has already produced a wrong count
once (27 reported where the true figure was 75).

Force a relink to get a new binary hash the policy allows:

```bash
touch crates/<crate>/tests/*.rs
cargo test -p bioprism-<crate> --offline
```

If a per-crate count looks lower than expected, check for `never executed` before concluding
anything about the code.

## Verifying the whole workspace

```bash
cargo test --workspace --offline 2>&1 | grep -E "^test result: ok" | awk '{s+=$4} END {print s}'
```

Do **not** run this while agents are concurrently writing crates — a crate mid-edit will not
compile and will fail the whole invocation for reasons that have nothing to do with your change.
Use `-p` per crate in that situation.

## What a real test looks like here

A passing count is not evidence on its own. Before accepting a crate, read a few test names. They
should state the claim being made:

- good: `a_budget_smaller_than_the_closure_fails_rather_than_truncating`
- good: `an_unmeasured_capability_is_never_reported_as_a_low_score`
- bad: `test_budget_2`, `it_works`

A test that exercises a path without asserting an invariant inflates the count and protects
nothing. Prefer deleting it to keeping it.
