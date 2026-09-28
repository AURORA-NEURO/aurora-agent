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

This is a responsibility map, not the complete crate dependency graph. The workspace has 88 crates;
Cargo manifests are authoritative for dependency edges. Python and TypeScript packages expose typed
facades over selected Rust contracts rather than mirroring every Rust type.

```
                        ┌───────────────────────────────┐
   agent interfaces     │  cli   mcp   api   SDKs       │
                        └──────────────┬────────────────┘
                                       │
   autonomous work      ┌──────────────┴────────────────┐
                        │ brain  autopilot  research    │
                        │ neurosurgery  runtime  factory │
                        └──────────────┬────────────────┘
                                       │
   policy and evidence  ┌──────────────┴────────────────┐
                        │ policy  safety  bundle  ledger│
                        │ registry  conformance  ops     │
                        └──────────────┬────────────────┘
                                       │
   composition          ┌──────────────┴────────────────┐
                        │ weave  fabric  choreography   │
                        └──────────────┬────────────────┘
                                       │
   evaluation           ┌─────────────┴─────────────────┐
                        │ prism baseline mutation eval  │
                        └──────────────┬────────────────┘
                                       │
   compilation          ┌──────────────┴────────────────┐
                        │ fiber  section  domain        │
                        │ project  repair  obligation    │
                        └──────────────┬────────────────┘
                                       │
   world and biology    ┌──────────────┴────────────────┐
                        │ world store worldgen adapter  │
                        │ bioir onco oracle modalities  │
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

When deriving canonical bytes or hashes from typed serializable data, Rust code should use
`to_canonical_bytes_serializable` or `ContentHash::of_serializable` before converting it to
`serde_json::Value`. The typed path rejects NaN and infinities instead of letting an earlier JSON
conversion turn them into `null`; the `Value`-based helpers remain for values already parsed or
constructed as JSON, where the original numeric type is no longer available. The core receipt paths
in `ids`, `brain`, `section`, `fiber`, `registry`, and `store` use the typed path; other legacy
conversion-then-hash call sites remain candidates for migration when those surfaces are next
changed.

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

**`repair` plans and checks, and never edits or executes.** It sits above `project` and `domain`,
turning an issue's compiled evidence region into a plan bound to that region and verifying a
claimed repair three-valued against the plan's own declared criteria — reporting which criteria
held, never that the issue is resolved. See [ISSUE_REPAIR](ISSUE_REPAIR.md).

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

The workspace is local-first, but it does contain network-facing components. `bioprism-api` serves a
bounded local HTTP/REST and JSON-RPC gateway, and selected Python/TypeScript research adapters make
explicit, allow-listed public-source requests. These are not a hosted multi-tenant service: TLS
termination, deployment identity, distributed storage, external workers, and production scheduling
remain outside the workspace. See [HTTP_API.md](HTTP_API.md) and the source-specific adapter
contracts in the backlog.

The API router keeps request dispatch and shared state in
[`router.rs`](../crates/api/src/router.rs), with mission lifecycle routes in
[`router/missions.rs`](../crates/api/src/router/missions.rs), evidence and artifact registry routes in
[`router/evidence.rs`](../crates/api/src/router/evidence.rs), developer workbench and CI evidence
registries in [`router/developer_artifact_routes.rs`](../crates/api/src/router/developer_artifact_routes.rs),
domain workflows and capability routes in [`router/domain_routes.rs`](../crates/api/src/router/domain_routes.rs),
workflow reconciliation in [`router/reconciliation_routes.rs`](../crates/api/src/router/reconciliation_routes.rs),
and operator snapshots and gate reviews in [`router/operations.rs`](../crates/api/src/router/operations.rs).
Event pages, streaming, metrics, delivery receipts, and route-review history live in
[`router/event_routes.rs`](../crates/api/src/router/event_routes.rs). Bounded local checkpoint adapters
live in [`router/persistence.rs`](../crates/api/src/router/persistence.rs). These modules share one
router instance and do not create separate dispatch or persistence authorities.
Router white-box tests stay under `router/tests.rs` and are grouped by transport, mission,
operations, events, registries, and domain workflows so private route behavior remains testable
without keeping every contract in the router implementation file.

The MCP protocol integration target keeps its shared server fixtures in
[`tests/protocol.rs`](../crates/mcp/tests/protocol.rs), with contract tests grouped under
[`tests/protocol/`](../crates/mcp/tests/protocol/). The top-level groups separate transport,
agent workflows, domain evidence, developer operations, evaluation, runtime infrastructure,
context and repository tools, research modeling, governance and safety, and cross-crate contracts.
Glioma workflow contracts are further grouped by research pipeline, experiments, computation,
federation, release, and related operating areas. The groups are child modules of the integration
target, so their tests keep access to the same private fixtures and exercise the same MCP server.

Signed bundles, signed webhook envelopes, and caller-supplied key-registry policy are present; the
workspace does not own production key custody or deployment trust roots. `DockerSandbox` provides a
separate opt-in Docker command boundary for a pinned Linux image. It resolves and pins the selected
local daemon endpoint and refuses remote contexts because bind-mount paths are interpreted by the
daemon host. It is not wired into the SDK plugin dispatcher or trial `ContainerProvider`. Its
caller-managed output directory, image review, credential isolation, and deployment policy remain
explicit responsibilities. See [OCI_SANDBOX.md](OCI_SANDBOX.md).

The backend portfolio for FAQ/InsideOut, worst-case-optimal joins, tensor networks, and decision
diagrams is enumerated in `section::plan::Backend` so plans stay honest about which engine ran, but
only `backward_factor_slice_reference` exists. Heavy biological formats — DICOM, BIDS/NIfTI,
AnnData/Zarr, VCF — belong in a Python layer per ADR-001, where the mature libraries live; the Rust
side owns the adapter *contract*, not the parsers.
