# Architecture

How the pieces fit, and why the boundaries fall where they do.

## The one idea

Context assembly is a **compiler pass**, not a retrieval heuristic. A typed decision query compiles
into the smallest decision-sufficient evidence region, delivered as a Decision Section and
accompanied by a Context Certificate that states what was omitted and whether the omission could
have changed the decision.

Everything else in the workspace is downstream of that: storage exists to make compilation
output-sensitive, evaluation exists to test whether the compiled context was good, mutation exists
to generate more decisions to test against, and the agent surface exists so the compiled context
reaches something that can act on it.

## Layers

```
                        ┌───────────────────────────────┐
   agent surface        │  cli          mcp             │
                        └──────────────┬────────────────┘
                                       │
   evaluation           ┌──────────────┴────────────────┐
                        │  prism   baseline   mutation  │
                        │  registry                     │
                        └──────────────┬────────────────┘
                                       │
   composition          ┌──────────────┴────────────────┐
                        │  weave                        │
                        └──────────────┬────────────────┘
                                       │
   compilation          ┌──────────────┴────────────────┐
                        │  fiber    section    domain   │
                        │  project                      │
                        └──────────────┬────────────────┘
                                       │
   world and storage    ┌──────────────┴────────────────┐
                        │  world  store  worldgen       │
                        │  adapter  bioir  onco  oracle │
                        └──────────────┬────────────────┘
                                       │
   foundation           ┌──────────────┴────────────────┐
                        │  ids          scope           │
                        └───────────────────────────────┘
```

## Why these boundaries

**`ids` depends on nothing.** Certificates hash canonical bytes, and cross-language byte parity is
hard — matching CPython required reproducing its `repr` float threshold, exponent zero-padding and
JSON object iteration order. One canonical implementation at the root of the graph means one place
where that can go wrong. See [ADR-001](ADR-001-language-strategy.md).

**`section` depends on neither `world` nor `fiber`.** A consumer — an MCP client, a CI gate, an
auditor — must be able to read and *verify* a compiled context without linking the engine that
produced it. If verification required the compiler, "independently verifiable" would be a slogan.

**`fiber` is generic over `WorldSource`.** Blueprint 43.16 requires logical semantics to be
independent of the physical backend. That is only real if it is checked, so the same query compiled
against an in-memory world and against an indexed store must produce byte-identical certificates —
asserted in `crates/store/tests/store_parity.rs`.

**`domain` is the oracle as data.** Domain packs: declarative rule oracles and scope vocabularies
that carry the FIBER pipeline to non-biological decision questions. It depends on `fiber`,
`section`, `scope` and `ids`, and plugs into `compile_with_oracle`; the default `compile()` and
its parity bytes are untouched, so a pack changes certificate bytes only through the verdict it
returns. See [GENERALIZATION](GENERALIZATION.md).

**`weave`'s kernel is small on purpose.** It is a trusted computing base. Per 23.49 it enforces
identity, protocol legality, authority, budgets and causal ordering, and explicitly does *not*
decide what is true. Contradictory claims both stay in the ledger.

**`baseline` exists to make the central claim falsifiable.** Without competent comparators,
"FIBER compiles a smaller context" is unfalsifiable. With them it came out a draw on the reference
world — see [FINDINGS](FINDINGS.md).

## The flywheel

```
worldgen  →  an audited parent world with controlled structure
   ↓
mutation  →  a validated family; every metamorphic relation checked by the oracle,
             reported by independent equivalence classes rather than instance count
   ↓
prism     →  Decision Cells frozen from the full-context verdict; architectures forked
             from the identical state, so a difference is attributable to context policy
   ↓
registry  →  packs earn a trust tier from evidence, and gate CI
   ↓
mcp       →  agents consume the compiled context progressively, L0 first
```

## Invariants that cross crate boundaries

These are the properties that would be easy to lose in a refactor, so each is pinned by a test.

| Invariant | Where it is enforced | Where it is tested |
|---|---|---|
| Canonical bytes match CPython exactly | `ids::canonical` | `ids/tests/python_parity.rs` |
| Eager and indexed backends agree byte for byte | `world::WorldSource` | `store/tests/store_parity.rs` |
| Protected closure is computed *before* any relevance step | `fiber::compile` pass order | `fiber/tests/reference_parity.rs` |
| Zero influence is distinguishable from unknown influence | `section::omission` | `section/tests/…` |
| A budget smaller than the closure fails rather than truncating | `fiber::compile` | `fiber/tests/reference_parity.rs` |
| Omissions are reported at every disclosure layer | `section::layers` | `mcp/tests/protocol.rs` |
| A nondeterministic judgement cannot overturn a deterministic one | `oracle` | `oracle/tests/…` |
| Authority can only attenuate; revocation is transitive | `weave::authority` | `weave/tests/kernel_conformance.rs` |
| Budgets are affine — duplication is a compile error | `weave::budget` | `weave/tests/kernel_conformance.rs` |
| A mutation cannot validate its own postcondition | `mutation::{apply, lineage}` split | `mutation/tests/metamorphic.rs` |
| Instance count is never reported as benchmark count | `mutation::diversity` | `mutation/tests/metamorphic.rs` |
| An adapter must declare what it could not preserve | `adapter` | `adapter/tests/…` |
| A pack cannot be promoted past its evidence | `registry` | `registry/tests/…` |

## What is not here

No network layer, no multi-tenancy, no signing keys, no hosted execution — local-first only. The
backend portfolio of 43.19–43.24 (FAQ/InsideOut, worst-case-optimal joins, tensor networks,
decision diagrams) is enumerated in `section::plan::Backend` so plans stay honest about which
engine ran, but only `backward_factor_slice_reference` exists. Heavy biological formats — DICOM,
BIDS/NIfTI, AnnData/Zarr, VCF — belong in a Python layer per ADR-001, where the mature libraries
live; the Rust side owns the adapter *contract*, not the parsers.
