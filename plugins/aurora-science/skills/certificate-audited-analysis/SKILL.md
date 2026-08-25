---
name: certificate-audited-analysis
description: Treat every computed result as unverified until its certificate checks out, review what was omitted rather than only what was included, and digest-chain every published number back to a retained computation. Use when citing any compiled or pipeline output in a document, when reviewing an analysis that publishes numbers, when a result arrives without a receipt for what it left out, or when reproducing someone else's figure or table.
---

> Note: the crate paths, documents, and measured numbers below are illustrations
> from the aurora-agent workspace where these methods were developed and tested.
> The methods themselves apply to any computational analysis whose numbers get published.

# Certificate-audited analysis

A number without a verifiable receipt is a rumor with formatting. The workspace's rule — no
optimized output is published without a certificate — generalizes to any analysis: every result
you cite should come with a self-describing record of its inputs, its omissions, and a digest
that lets someone else check it without trusting you or re-running your engine.

## Never trust a compile without verifying its certificate

The workspace's compiler emits a Context Certificate with every compiled context, and the
verification path is independent of the engine: `ContextCertificate::verify` recomputes the
digest over the canonical body and returns `Valid`, `DigestMismatch { claimed, recomputed }`, or
`Malformed` (`crates/section/src/certificate.rs`). Before citing any certified result:

1. Verify the certificate, and record which of the three states you got. `Malformed` and
   `DigestMismatch` are findings, not inconveniences to route around.
2. Check that the numbers you are about to quote appear *in* the certified body, not in prose
   near it.
3. Check the `limitations` array and carry it forward — the golden reference certificate's
   limitation ("dependency reachability and protected tags; it does not yet implement ...
   formal influence bounds") bounds every claim built on it.

The same discipline applies to reproduction: the workspace's reproduction-check contract
(`docs/EVALUATION_REPRODUCTION_CHECK.md`) certifies each declared output as `matched`,
`diverged`, or `missing`, reports `first_divergence` as the earliest non-matching row rather
than an aggregate match rate, and keeps `missing` separate — so a rerun cannot improve its
apparent match rate by omitting a required artifact.

## Omission accounting is the object of review

Most reviews audit what an analysis included. The certificate's center of gravity is the
opposite: what was **left out, and whether that could have mattered**. The reference profile
records `total_facts`, `exploratory_facts`, a classification string, and anything selected then
cut as inaccessible. And the workspace is honest about that record's own limit — the doc comment
on the type says a count-and-a-string "cannot distinguish 'provably cannot matter' from 'nobody
checked'", which is exactly why an extended profile with an influence-classified omission
manifest exists under a different schema version (changing the hashed bytes changes the schema
name, never silently).

When you review an analysis, ask the omission questions first:

- What did this pipeline drop, and under what stated rule?
- Is "dropped as irrelevant" a proven bound or an unexamined assumption? Those are different
  states and the record must not share one representation between them.
- Were any items selected and then removed by a later stage? Removal after selection is a
  distinct event and must be listed, not netted out of a count.

An absent field carries meaning too: in the compiler's trace, an audit field that is `None`
means *this wire version cannot claim that pass ran* — semantically different from a recorded
pass with a null result. Do not let a renderer or a summary default it.

## Digest-chain every published number

Every number in a published table should be traceable, through digests, to a retained
computation:

- The certificate binds its inputs by hash (`source_hashes`: world, query, decision-section),
  so changing any input — including a decision binding — changes certificate identity.
- The certificate carries its own `certificate_sha256`; the workspace's golden reference value
  (`c0da17ff...` in `fixtures/fiber-v0.1/golden/reference_certificate.json`) is asserted across
  three independent implementations, which is what makes "byte parity" a checkable claim.
- Findings documents pin their numbers to named tests, so a number that stops being true fails
  a build instead of aging quietly: "Every number here is asserted by a test [...] so it fails
  loudly if it stops being true" (`docs/FINDINGS.md`).

The practical discipline for your own documents:

1. Next to every published number, record where it is asserted (test name, certificate digest,
   or retained artifact path).
2. Prefer canonical serialization before hashing, so formatting differences cannot fork
   identities: the workspace's seal digests are computed over canonical JSON, and "a
   semantically identical rubric represented with different whitespace has the same canonical
   content."
3. State what the digest proves and what it does not. The reveal-audit doc draws the line
   precisely: the digest "is an integrity witness, not a provenance witness" — it proves the
   scoring used the sealed rubric, not that the rubric predates a leak or that the timestamp is
   trustworthy. Copy that honesty about your own chain's endpoints.

## Checklist

- Certificate verified (state recorded) before any certified number is cited.
- Omissions reviewed as the primary object: rule, classification, and whether "cannot matter"
  was proven or merely unexamined.
- Absent audit fields read as "cannot claim", never defaulted.
- Every published number pinned to a test, digest, or retained artifact.
- Reproduction reported per-output as matched/diverged/missing with the first divergence named,
  never as a single match percentage.
- The chain's own limits stated: what the digests bind, and what remains a caller assertion.
