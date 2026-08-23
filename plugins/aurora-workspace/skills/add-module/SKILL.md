---
name: add-module
description: (aurora-agent workspace only) Add a new crate to the bioprism workspace implementing a blueprint section, including registering it, choosing dependencies without creating cycles, and the honesty conventions every crate must follow. Use when starting a new module, when asked to cover an uncovered blueprint section, or when setting up work for a subagent to fill in.
---

<!-- Mirrored from .agents/skills/add-module/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->

# Add a module

A module is a crate implementing one blueprint section. The blueprint lives outside the repo; the
path is recorded in `docs/ARCHITECTURE.md`.

## 1. Register before writing

Create the skeleton and register it *first*, so the workspace stays buildable and so a subagent
never has to touch the shared root manifest:

```bash
mkdir -p crates/<name>/src && printf '//! Placeholder.\n' > crates/<name>/src/lib.rs
```

Add `crates/<name>` to `members` and `bioprism-<name> = { path = "crates/<name>" }` to
`[workspace.dependencies]` in the root `Cargo.toml`, then confirm:

```bash
cargo build -p bioprism-<name> --offline
```

**Only ever add dependencies already in `[workspace.dependencies]`**, as `{ workspace = true }`.
Builds are offline against pinned versions; a new external crate will not resolve. Several things
are hand-rolled for exactly this reason — the CSV reader, the arg parser, JSON-RPC, log-gamma,
RFC 3339, the PRNG. Prefer writing the ~100 lines to reaching for a dependency.

## 2. Respect the dependency direction

`ids` depends on nothing. `scope` depends on `ids`. `section` depends on neither `world` nor
`fiber`, deliberately — a consumer must be able to *verify* a compiled context without linking the
engine that produced it. Do not break that to save an import.

If two crates would need each other, the fix is a trait in the lower one implemented by the higher
one, as `WorldSource` does for the compiler and the store.

## 3. Read the spec module, then cite it

Open the blueprint modules the crate claims and cite their ids in the doc comments
("Blueprint 43.13 requires…"). Much of the spec is status `Planned` rather than build-ready —
prose design, not a frozen contract. Where it under-specifies, **say so in a doc comment** instead
of inventing a detail and presenting it as spec. Whole sections are near-identical boilerplate;
§08, §31 and §33 each carry roughly 15 lines of distinguishing content per module. That is worth
reporting, not working around silently.

## 4. The honesty conventions, which are not optional

Every crate's `lib.rs` carries an explicit **what is not implemented** list. A missing capability
that is stated is a limitation; one that is implied to exist is a lie.

Where a rule can be made unrepresentable, make it unrepresentable rather than testing for it. The
workspace has a house pattern for this — private fields with one gated constructor, a type that
does not implement `Clone`, a variant carrying a token only one function can mint. See the table in
`AGENTS.md`.

Distinguish "checked and it cannot matter" from "nobody checked", everywhere the distinction
arises. That is the whole product.

## 5. Green before done

```bash
cargo test -p bioprism-<name> --offline
cargo clippy -p bioprism-<name> --all-targets --offline
```

See the `verify-crate` skill, particularly for the Application Control failure that makes a suite
which never ran look like one that failed.

## 6. Commit it on its own

One concern per commit, with a message that says why:

```
feat(<name>): <what it does and the reason it exists>
```

Not `wip` and not a batch of unrelated crates.

## Delegating to a subagent

If a subagent will fill the crate in, give it: the crate path, the blueprint section, the
dependency list, an explicit instruction to **write files early rather than reading for a long
time first** (agents that read exhaustively before writing have died leaving nothing), a warning
never to run `cargo --workspace` while siblings are mid-edit, and the names of any crates being
written concurrently so it does not depend on a placeholder.
