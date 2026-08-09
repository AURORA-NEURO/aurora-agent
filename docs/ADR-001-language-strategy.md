# ADR-001 — Language strategy

**Status:** accepted
**Date:** 2026-08-08

## Question

Is Rust the right language for this project, and where should other languages be used?

## Measurement first

`cargo run --release -p bioprism-worldgen --example scaling` on generated worlds:

| distractors | facts | parse_ms | compile_ms | hash_ms | selected |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 1,012 | 10.9 | 4.3 | 0.0 | 11 |
| 10,000 | 10,012 | 107.3 | 39.5 | 0.0 | 11 |
| 100,000 | 100,012 | 1,063.6 | 452.3 | 0.1 | 11 |
| 500,000 | 500,012 | 6,458.5 | 2,630.4 | 0.1 | 11 |
| 1,000,000 | 1,000,012 | 16,551.5 | 5,529.7 | 0.1 | 11 |

Three things follow.

**Ingestion dominates, not compilation.** Parsing is roughly 3× the cost of the compiler pass at
every size. A faster compiler language buys nothing while the process is bounded by reading a JSON
document.

**Both scale with world size while the output does not.** The selected region is 11 facts at every
scale. The compiler is output-sensitive in its *result* and input-sensitive in its *cost* — which
is precisely the behaviour the project's own thesis rejects: *never traverse the whole knowledge
structure by default*. At 1M facts we materialise a million records to deliver eleven.

**Hashing is free.** 0.1 ms, because certificates hash the compiled artifacts, not the world.

So the optimisation that matters is architectural — a content-addressed, indexed store so cost
tracks the compiled region rather than the corpus (blueprint 43.34, 40.07). Rewriting the kernel in
a different language would optimise the 25% that is already fast and leave the 75% untouched.

## Decision

**Rust stays for the kernel**, meaning `ids`, `scope`, `world`, `section`, `fiber`, and the storage
layer. Three reasons, in order of weight:

1. **Determinism is the product.** Certificates hash canonical bytes, and cross-language byte
   parity is genuinely hard — matching CPython required reproducing its `repr` float threshold,
   exponent zero-padding, and JSON object iteration order. Every language in the hashing path is a
   new parity surface. One canonical implementation, with others binding to it, is a correctness
   decision rather than a performance one.
2. The compiler passes are graph traversal over typed arenas, which Rust does well.
3. Single-binary distribution matters for a tool agents shell out to.

**Python becomes the second first-class language**, but only above the kernel:

- **Biological data adapters (§28).** DICOM, BIDS/NIfTI, AnnData/Zarr, VCF. `pydicom`, `nibabel`,
  `anndata`, `zarr` and `pysam` are mature, correct, and encode a decade of format edge cases.
  Reimplementing them in Rust would be slow to write and worse on arrival. Adapters emit
  `fiber-world/0.1` documents; they never touch the hashing path.
- **Evaluation and statistics (§07, §08).** Capability posteriors, information-gain scheduling,
  adaptive suite selection. Research code that changes weekly and leans on `numpy`/`scipy`.
- **The reference oracle**, which already exists and stays as the parity witness.

**TypeScript is deferred** to the graph-native web UI (§42) and the browser-side SDK. It has no
role below that line.

**C FFI when the backend portfolio lands.** Decision diagrams (43.22) should bind CUDD or Sylvan
rather than growing a new BDD library; tensor contraction (43.21) should bind an existing
contraction-order search rather than reinventing one. Both sit behind the `Backend` trait, which
already exists precisely so backends can be foreign.

**Not chosen:** Julia (a third numerical ecosystem and a heavy runtime for a gain Rust+FFI already
covers); C++ (equivalent performance, worse safety guarantees for code that must not corrupt a
certificate); Go (no advantage here, and weaker numerics).

## Outcome

`bioprism-store` was built rather than a rewrite. It indexes a world once per release into on-disk
sorted maps and answers point queries by binary search, serving aggregates from a manifest.

| facts | eager (parse + compile) | index build, once | lazy compile | speedup |
|---:|---:|---:|---:|---:|
| 1,012 | 21.3 ms | 17.1 ms | 11.0 ms | 2× |
| 10,012 | 205.2 ms | 114.9 ms | 11.9 ms | 17× |
| 100,012 | 1,742.9 ms | 1,131.3 ms | 17.5 ms | 100× |
| 500,012 | 10,500.9 ms | 6,536.9 ms | 42.1 ms | 249× |
| 1,000,012 | 26,513.4 ms | 12,299.0 ms | **41.6 ms** | **638×** |

Query cost went from linear in corpus size to roughly logarithmic: a 1000× larger world costs 3.8×
more to compile. The decision stands — the lever was the data layer, and no language change was
involved. A rewrite would have optimised the 25% that was already fast.

Three implementations now agree byte for byte on the reference certificate
(`c0da17ff…7ea4`): the CPython reference, the Rust eager path, and the Rust indexed path.

## Consequences

- `WorldSource` becomes a trait so the compiler can run against an eager in-memory world or a lazy
  indexed store without changing logical semantics — which 43.16 requires anyway ("logical
  semantics are independent of physical backend").
- Any Python component that produces an artifact whose hash is checked must round-trip through the
  Rust canonicaliser, never its own. The `tools/gen_python_ground_truth.py` pattern generalises:
  Python may *generate*, Rust *canonicalises and hashes*.
- The cross-language conformance suite (43.35) is now load-bearing rather than aspirational.
